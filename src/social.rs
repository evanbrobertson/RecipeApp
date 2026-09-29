//! Sign in with Google and Apple for Crumb's own accounts (`AUTH_MODE=accounts`). The
//! hosted edition gets the same from Better Auth (`auth/src/auth.ts`), with the same env vars.
//!
//! OpenID Connect's authorization code flow, by hand: `POST /api/auth/social/{provider}/start`
//! keeps what the sign-in is for (sign in, link to the signed-in account, or join an invite's
//! household) under a random `state`, bound to this browser by the `crumb_social` cookie, and
//! answers with the provider's URL. The provider sends the person back to
//! `/api/auth/social/{provider}/callback` (Apple posts a form there, which is turned into a
//! GET so the SameSite=Lax cookie comes along), and the code is swapped for an ID token.
//!
//! The ID token comes straight from the provider's token endpoint over TLS, so its signature
//! isn't checked again (OpenID Connect Core 3.1.3.7); its issuer, audience, expiry and nonce
//! are. Nobody takes over an account by its email: a new Google or Apple sign-in whose email
//! already has an account is refused, and linking is done from the signed-in account.

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::AppState;
use crate::account_api::{accounts, json_body, new_session};
use crate::accounts::SocialLogin;
use crate::auth::{self, SignedIn, random_token, sha256_hex};
use crate::error::{AppError, AppResult};
use crate::model::now_secs;

/// The cookie that ties a sign-in's `state` to the browser that started it.
const COOKIE: &str = "crumb_social";
const COOKIE_PATH: &str = "/api/auth/social";

/// How the client proves itself at the token endpoint.
#[derive(Debug, Clone)]
pub enum Secret {
    /// Google's client secret.
    Fixed(String),
    /// Apple's: a short-lived ES256 JWT signed with the key from the developer account.
    Apple {
        team_id: String,
        key_id: String,
        /// PKCS#8 PEM (the `.p8` file's contents).
        private_key: String,
    },
}

/// A provider people can sign in with.
#[derive(Debug, Clone)]
pub struct Provider {
    /// `google` or `apple`.
    pub id: &'static str,
    pub client_id: String,
    pub secret: Secret,
    pub auth_url: String,
    pub token_url: String,
    /// What the ID token's `iss` may be.
    pub issuers: Vec<String>,
}

impl Provider {
    pub fn google(client_id: &str, client_secret: &str) -> Self {
        Self {
            id: "google",
            client_id: client_id.into(),
            secret: Secret::Fixed(client_secret.into()),
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_url: "https://oauth2.googleapis.com/token".into(),
            issuers: vec![
                "https://accounts.google.com".into(),
                "accounts.google.com".into(),
            ],
        }
    }

    pub fn apple(client_id: &str, team_id: &str, key_id: &str, private_key: &str) -> Self {
        Self {
            id: "apple",
            client_id: client_id.into(),
            secret: Secret::Apple {
                team_id: team_id.into(),
                key_id: key_id.into(),
                // Env vars often carry the PEM's newlines as `\n`
                private_key: private_key.replace("\\n", "\n"),
            },
            auth_url: "https://appleid.apple.com/auth/authorize".into(),
            token_url: "https://appleid.apple.com/auth/token".into(),
            issuers: vec!["https://appleid.apple.com".into()],
        }
    }

    /// Google with `GOOGLE_CLIENT_ID` and `GOOGLE_CLIENT_SECRET`; Apple with
    /// `APPLE_CLIENT_ID` (the Services ID), `APPLE_TEAM_ID`, `APPLE_KEY_ID` and
    /// `APPLE_PRIVATE_KEY`.
    pub fn from_env(env: fn(&[&str]) -> Option<String>) -> Vec<Self> {
        let mut out = Vec::new();
        if let (Some(id), Some(secret)) =
            (env(&["GOOGLE_CLIENT_ID"]), env(&["GOOGLE_CLIENT_SECRET"]))
        {
            out.push(Self::google(&id, &secret));
        }
        let apple = (
            env(&["APPLE_CLIENT_ID"]),
            env(&["APPLE_TEAM_ID"]),
            env(&["APPLE_KEY_ID"]),
            env(&["APPLE_PRIVATE_KEY"]),
        );
        if let (Some(id), Some(team), Some(key), Some(pem)) = apple {
            let provider = Self::apple(&id, &team, &key, &pem);
            match provider.client_secret() {
                Ok(_) => out.push(provider),
                Err(err) => tracing::warn!("Sign in with Apple is off: {}", err.message),
            }
        }
        out
    }

    /// The secret for one token request.
    fn client_secret(&self) -> AppResult<String> {
        match &self.secret {
            Secret::Fixed(s) => Ok(s.clone()),
            Secret::Apple {
                team_id,
                key_id,
                private_key,
            } => apple_client_secret(&self.client_id, team_id, key_id, private_key),
        }
    }
}

/// Apple's client secret: a JWT signed with ES256, good for five minutes.
fn apple_client_secret(
    client_id: &str,
    team_id: &str,
    key_id: &str,
    pem: &str,
) -> AppResult<String> {
    use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair};
    let body: String = pem
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("-----"))
        .collect();
    let der = STANDARD
        .decode(body)
        .map_err(|_| AppError::internal("APPLE_PRIVATE_KEY isn't a PEM key"))?;
    let rng = ring::rand::SystemRandom::new();
    let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &der, &rng)
        .map_err(|_| AppError::internal("APPLE_PRIVATE_KEY isn't a P-256 key"))?;
    let now = now_secs();
    let header = URL_SAFE_NO_PAD.encode(json!({"alg": "ES256", "kid": key_id}).to_string());
    let claims = URL_SAFE_NO_PAD.encode(
        json!({
            "iss": team_id,
            "iat": now,
            "exp": now + 300,
            "aud": "https://appleid.apple.com",
            "sub": client_id,
        })
        .to_string(),
    );
    let signing_input = format!("{header}.{claims}");
    let sig = key
        .sign(&rng, signing_input.as_bytes())
        .map_err(|_| AppError::internal("couldn't sign Apple's client secret"))?;
    Ok(format!(
        "{signing_input}.{}",
        URL_SAFE_NO_PAD.encode(sig.as_ref())
    ))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/auth/social/{provider}/start", routing::post(start))
        .route(
            "/api/auth/social/{provider}/callback",
            routing::get(callback).post(form_post),
        )
        .route("/api/auth/identities", routing::get(methods))
        .route("/api/auth/identities/{provider}", routing::delete(unlink))
}

fn provider<'a>(state: &'a AppState, id: &str) -> AppResult<&'a crate::social::Provider> {
    state
        .config
        .social
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| AppError::not_found("That sign-in isn't set up here"))
}

/// The provider ids people can sign in with here, for `/api/auth/status`.
pub fn provider_ids(state: &AppState) -> Vec<&'static str> {
    state.config.social.iter().map(|p| p.id).collect()
}

/// Only same-origin paths. Browsers read `/\host` and `/<TAB>/host` as `//host`, so anything
/// with a control character or a backslash is refused, and what is left is resolved against a
/// stand-in origin and must still be on it.
fn same_origin(next: Option<&str>) -> Option<String> {
    let next = next?;
    if !next.starts_with('/')
        || next.starts_with("//")
        || next.chars().any(|c| c.is_control() || c == '\\')
    {
        return None;
    }
    let base = url::Url::parse("https://crumb.invalid").ok()?;
    let resolved = base.join(next).ok()?;
    if resolved.origin() != base.origin() {
        return None;
    }
    let mut out = resolved.path().to_string();
    if let Some(q) = resolved.query() {
        out.push('?');
        out.push_str(q);
    }
    if let Some(f) = resolved.fragment() {
        out.push('#');
        out.push_str(f);
    }
    Some(out)
}

fn redirect_uri(state: &AppState, headers: &HeaderMap, provider: &str) -> String {
    format!(
        "{}/api/auth/social/{provider}/callback",
        state.config.public_origin(headers)
    )
}

/// `{intent, next, invite}` → `{url}`: where to send the browser. `intent` is `login` (the
/// default), `link` (signed in: add this sign-in to the account) or `invite` (sign in or
/// up, and join the household `invite` is for).
async fn start(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> AppResult<Response> {
    let accounts = accounts(&state)?;
    let provider = provider(&state, &id)?;
    let body = if body.is_empty() {
        json!({})
    } else {
        json_body(&body)?
    };
    let get = |k: &str| body.get(k).and_then(Value::as_str);
    let intent = get("intent").unwrap_or("login");
    let (user, invite) = match intent {
        "login" => (None, None),
        "link" => {
            let signed = auth::session(&state, &headers)
                .await
                .ok_or_else(|| AppError::new(401, "Not signed in"))?;
            (Some(signed.user_id), None)
        }
        "invite" => {
            let token = get("invite").unwrap_or("");
            if accounts.preview_invite(token)?.is_none() {
                return Err(AppError::new(
                    410,
                    "This invite has expired or was already used. Ask for a new one.",
                ));
            }
            (None, Some(token.to_string()))
        }
        _ => return Err(AppError::bad_request("intent: Unknown intent")),
    };
    let verifier = random_token(32);
    let nonce = random_token(16);
    let state_token = accounts.start_social(&SocialLogin {
        provider: provider.id.into(),
        verifier: verifier.clone(),
        nonce: nonce.clone(),
        intent: intent.into(),
        user,
        invite,
        next: same_origin(get("next")),
    })?;
    let mut url = url::Url::parse(&provider.auth_url).map_err(AppError::internal)?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("client_id", &provider.client_id)
            .append_pair("redirect_uri", &redirect_uri(&state, &headers, provider.id))
            .append_pair("response_type", "code")
            .append_pair("state", &state_token)
            .append_pair("nonce", &nonce);
        if provider.id == "apple" {
            // Asking Apple for the name and email needs the answer as a form post
            q.append_pair("scope", "name email")
                .append_pair("response_mode", "form_post");
        } else {
            let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
            q.append_pair("scope", "openid email profile")
                .append_pair("code_challenge", &challenge)
                .append_pair("code_challenge_method", "S256")
                .append_pair("prompt", "select_account");
        }
    }
    let mut res = Json(json!({"url": url.as_str()})).into_response();
    let secure = if auth::is_https(&headers) {
        "; Secure"
    } else {
        ""
    };
    if let Ok(cookie) = HeaderValue::from_str(&format!(
        "{COOKIE}={state_token}; Path={COOKIE_PATH}; Max-Age=600; HttpOnly; SameSite=Lax{secure}"
    )) {
        res.headers_mut().append(header::SET_COOKIE, cookie);
    }
    Ok(res)
}

#[derive(Deserialize, Default)]
struct Back {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    /// Apple, the first time only: `{"name": {"firstName", "lastName"}, "email"}`.
    user: Option<String>,
}

/// Apple's form post: the same answer, as a GET, so the Lax cookie is sent with it.
async fn form_post(Path(id): Path<String>, body: Bytes) -> Response {
    let form: Vec<(String, String)> = serde_urlencoded::from_bytes(&body).unwrap_or_default();
    let keep: Vec<(String, String)> = form
        .into_iter()
        .filter(|(k, _)| matches!(k.as_str(), "code" | "state" | "error" | "user"))
        .collect();
    let query = serde_urlencoded::to_string(keep).unwrap_or_default();
    let mut res = axum::http::StatusCode::SEE_OTHER.into_response();
    if let Ok(v) = HeaderValue::from_str(&format!("/api/auth/social/{id}/callback?{query}")) {
        res.headers_mut().insert(header::LOCATION, v);
    }
    res
}

/// Who the provider says signed in.
#[derive(Debug, Clone, PartialEq)]
struct Person {
    subject: String,
    email: Option<String>,
    email_verified: bool,
    name: Option<String>,
}

/// Where a finished (or failed) sign-in goes, clearing the state cookie on the way.
fn finish(headers: &HeaderMap, location: &str, session: Option<HeaderValue>) -> Response {
    let mut res = auth::found(location);
    let secure = if auth::is_https(headers) {
        "; Secure"
    } else {
        ""
    };
    if let Ok(v) = HeaderValue::from_str(&format!(
        "{COOKIE}=; Path={COOKIE_PATH}; Max-Age=0; HttpOnly; SameSite=Lax{secure}"
    )) {
        res.headers_mut().append(header::SET_COOKIE, v);
    }
    if let Some(cookie) = session {
        res.headers_mut().append(header::SET_COOKIE, cookie);
    }
    res
}

/// Where a failed sign-in shows its error: a short code the page turns into words (never
/// text from the URL, so a link can't put words in Crumb's mouth).
fn failed(headers: &HeaderMap, login: Option<&SocialLogin>, code: &str) -> Response {
    let location = match login {
        Some(l) if l.intent == "link" => format!("/more/account?error={code}"),
        Some(l) if l.intent == "invite" => {
            format!("/invite?error={code}#{}", l.invite.as_deref().unwrap_or(""))
        }
        _ => format!("/login?error={code}"),
    };
    finish(headers, &location, None)
}

async fn callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(back): Query<Back>,
) -> Response {
    let Ok(accounts) = accounts(&state) else {
        return AppError::not_found("Not found").into_response();
    };
    let Ok(provider) = provider(&state, &id) else {
        return AppError::not_found("That sign-in isn't set up here").into_response();
    };
    // The state must be the one this browser started with
    let given = back.state.as_deref().unwrap_or("");
    let cookie = auth::cookie_value(&headers, COOKIE).unwrap_or_default();
    let ours = !given.is_empty()
        && bool::from(subtle::ConstantTimeEq::ct_eq(
            sha256_hex(given).as_bytes(),
            sha256_hex(&cookie).as_bytes(),
        ));
    let login = if ours {
        accounts.take_social(given).ok().flatten()
    } else {
        None
    };
    let Some(login) = login.filter(|l| l.provider == provider.id) else {
        return failed(&headers, None, "expired");
    };
    if back.error.is_some() {
        return failed(&headers, Some(&login), "cancelled");
    }
    let Some(code) = back.code.as_deref().filter(|c| !c.is_empty()) else {
        return failed(&headers, Some(&login), "cancelled");
    };
    let person = match exchange(&state, &headers, provider, &login, code).await {
        Ok(mut person) => {
            if person.name.is_none() {
                person.name = back.user.as_deref().and_then(apple_name);
            }
            person
        }
        Err(err) => {
            tracing::warn!("[social] {} sign-in failed: {}", provider.id, err.message);
            return failed(&headers, Some(&login), "failed");
        }
    };
    match sign_in(&state, &headers, &login, provider.id, &person) {
        Ok((location, session)) => finish(&headers, &location, session),
        Err(code) => failed(&headers, Some(&login), code),
    }
}

/// What the sign-in was for, done: where to go next, and a new session's cookie.
fn sign_in(
    state: &AppState,
    headers: &HeaderMap,
    login: &SocialLogin,
    provider: &str,
    person: &Person,
) -> Result<(String, Option<HeaderValue>), &'static str> {
    let accounts = accounts(state).map_err(|_| "failed")?;
    let internal = |err: AppError| {
        tracing::warn!("[social] {}", err.message);
        "failed"
    };
    if login.intent == "link" {
        let user = login.user.ok_or("failed")?;
        return match accounts.link_identity(
            user,
            provider,
            &person.subject,
            person.email.as_deref(),
        ) {
            Ok(()) => Ok((
                login
                    .next
                    .clone()
                    .unwrap_or_else(|| format!("/more/account?linked={provider}")),
                None,
            )),
            Err(err) if err.status == 409 => Err("already_linked"),
            Err(err) => Err(internal(err)),
        };
    }
    let known = accounts
        .identity_user(provider, &person.subject)
        .map_err(internal)?;
    let (user, household) = match known {
        Some(user) => {
            let household = match &login.invite {
                Some(token) => match accounts.accept_invite(token, user) {
                    Ok(h) => h,
                    Err(err) if err.status == 410 => return Err("invite_gone"),
                    Err(err) => return Err(internal(err)),
                },
                None => accounts
                    .default_household(user)
                    .map_err(internal)?
                    .ok_or("failed")?,
            };
            (user, household)
        }
        None => {
            let email = person.email.as_deref().ok_or("no_email")?;
            if !person.email_verified {
                return Err("unverified");
            }
            if accounts.needs_setup().map_err(internal)? {
                return Err("not_set_up");
            }
            if accounts.email_taken(email).map_err(internal)? {
                return Err("email_taken");
            }
            let name = person.name.as_deref().unwrap_or("");
            match accounts.sign_up_social(
                provider,
                &person.subject,
                email,
                name,
                login.invite.as_deref(),
                state.config.open_signup,
            ) {
                Ok(made) => made,
                Err(err) => {
                    return Err(match err.status.as_u16() {
                        403 => "signup_closed",
                        409 => "email_taken",
                        410 => "invite_gone",
                        _ => internal(err),
                    });
                }
            }
        }
    };
    let session = new_session(accounts, user, household, headers).map_err(internal)?;
    let next = match &login.invite {
        Some(_) => "/".to_string(),
        None => login.next.clone().unwrap_or_else(|| "/".into()),
    };
    Ok((next, session))
}

/// Apple's `user` form field: the name, sent only the first time someone signs in.
fn apple_name(user: &str) -> Option<String> {
    let v: Value = serde_json::from_str(user).ok()?;
    let name = &v["name"];
    let full = [name["firstName"].as_str(), name["lastName"].as_str()]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    (!full.is_empty()).then_some(full)
}

/// Swaps the code for an ID token, and reads who it's for.
async fn exchange(
    state: &AppState,
    headers: &HeaderMap,
    provider: &Provider,
    login: &SocialLogin,
    code: &str,
) -> AppResult<Person> {
    let secret = provider.client_secret()?;
    let redirect = redirect_uri(state, headers, provider.id);
    let mut form = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("client_id", provider.client_id.as_str()),
        ("client_secret", secret.as_str()),
        ("redirect_uri", redirect.as_str()),
    ];
    if provider.id != "apple" {
        form.push(("code_verifier", login.verifier.as_str()));
    }
    let reply: Value = state
        .http
        .post(&provider.token_url)
        .header(header::ACCEPT, "application/json")
        .form(&form)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| AppError::internal(format!("token endpoint: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::internal(format!("token endpoint answer: {e}")))?;
    let id_token = reply["id_token"].as_str().ok_or_else(|| {
        AppError::internal(format!(
            "no ID token ({})",
            reply["error"].as_str().unwrap_or("no error given")
        ))
    })?;
    read_id_token(provider, id_token, &login.nonce, now_secs())
}

/// The person an ID token is for, once its issuer, audience, expiry and nonce check out.
fn read_id_token(provider: &Provider, token: &str, nonce: &str, now: i64) -> AppResult<Person> {
    let bad = |why: &str| AppError::internal(format!("ID token: {why}"));
    let payload = token.split('.').nth(1).ok_or_else(|| bad("not a JWT"))?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .map_err(|_| bad("not base64"))?;
    let claims: Value = serde_json::from_slice(&bytes).map_err(|_| bad("not JSON"))?;
    let iss = claims["iss"].as_str().unwrap_or("");
    if !provider.issuers.iter().any(|i| i == iss) {
        return Err(bad("wrong issuer"));
    }
    let aud_ok = match &claims["aud"] {
        Value::String(a) => *a == provider.client_id,
        Value::Array(list) => list.iter().any(|a| a.as_str() == Some(&provider.client_id)),
        _ => false,
    };
    if !aud_ok {
        return Err(bad("wrong audience"));
    }
    if claims["exp"].as_i64().is_none_or(|exp| exp + 60 < now) {
        return Err(bad("expired"));
    }
    // Exactly as sent; Apple may give back its SHA-256 instead
    let given = claims["nonce"].as_str().unwrap_or("");
    let hashed: String = Sha256::digest(nonce.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if given.is_empty() || (given != nonce && given != hashed) {
        return Err(bad("wrong nonce"));
    }
    let subject = claims["sub"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| bad("no subject"))?;
    let email_verified = match &claims["email_verified"] {
        Value::Bool(b) => *b,
        Value::String(s) => s == "true",
        _ => false,
    };
    Ok(Person {
        subject: subject.into(),
        email: claims["email"].as_str().map(String::from),
        email_verified,
        name: claims["name"]
            .as_str()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(String::from),
    })
}

/// How the signed-in person can sign in: `{password, linked: [{provider, email, createdAt}],
/// available: [provider ids]}`.
async fn methods(
    State(state): State<AppState>,
    SignedIn(signed): SignedIn,
) -> AppResult<Json<Value>> {
    let (password, linked) = accounts(&state)?.sign_in_methods(signed.user_id)?;
    Ok(Json(json!({
        "password": password,
        "linked": linked,
        "available": provider_ids(&state),
    })))
}

async fn unlink(
    State(state): State<AppState>,
    SignedIn(signed): SignedIn,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    if !accounts(&state)?.unlink_identity(signed.user_id, &id)? {
        return Err(AppError::not_found("That sign-in isn't linked"));
    }
    Ok(Json(json!({"ok": true})))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jwt(claims: Value) -> String {
        format!(
            "e30.{}.sig",
            URL_SAFE_NO_PAD.encode(claims.to_string().as_bytes())
        )
    }

    #[test]
    fn id_tokens_are_checked() {
        let google = Provider::google("client-1", "secret");
        let good = json!({
            "iss": "https://accounts.google.com", "aud": "client-1", "exp": 1_000,
            "nonce": "n1", "sub": "123", "email": "ann@example.com",
            "email_verified": true, "name": " Ann Cook ",
        });
        let person = read_id_token(&google, &jwt(good.clone()), "n1", 900).unwrap();
        assert_eq!(
            person,
            Person {
                subject: "123".into(),
                email: Some("ann@example.com".into()),
                email_verified: true,
                name: Some("Ann Cook".into()),
            }
        );
        for (key, value) in [
            ("iss", json!("https://evil.test")),
            ("aud", json!("client-2")),
            ("exp", json!(100)),
            ("nonce", json!("n2")),
            ("sub", json!("")),
        ] {
            let mut bad = good.clone();
            bad[key] = value;
            assert!(
                read_id_token(&google, &jwt(bad), "n1", 900).is_err(),
                "{key}"
            );
        }
        let mut listed = good.clone();
        listed["aud"] = json!(["other", "client-1"]);
        assert!(read_id_token(&google, &jwt(listed), "n1", 900).is_ok());

        // Apple: a string email_verified, and the nonce may come back hashed
        let apple = Provider {
            secret: Secret::Fixed(String::new()),
            ..Provider::apple("com.example.web", "T", "K", "")
        };
        let hashed: String = Sha256::digest(b"n1")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let token = jwt(json!({
            "iss": "https://appleid.apple.com", "aud": "com.example.web", "exp": 1_000,
            "nonce": hashed, "sub": "000.abc", "email": "x@privaterelay.appleid.com",
            "email_verified": "true",
        }));
        let person = read_id_token(&apple, &token, "n1", 900).unwrap();
        assert!(person.email_verified);
        assert_eq!(person.name, None);
    }

    #[test]
    fn apple_names_and_safe_redirects() {
        assert_eq!(
            apple_name(r#"{"name":{"firstName":"Ann","lastName":"Cook"},"email":"a@b.c"}"#),
            Some("Ann Cook".into())
        );
        assert_eq!(apple_name(r#"{"email":"a@b.c"}"#), None);
        assert_eq!(same_origin(Some("/recipes/1")), Some("/recipes/1".into()));
        assert_eq!(same_origin(Some("//evil.test")), None);
        assert_eq!(same_origin(Some("https://evil.test")), None);
        assert_eq!(same_origin(Some("/\\evil.test")), None);
        assert_eq!(
            same_origin(Some("/%5Cevil.com")),
            Some("/%5Cevil.com".into())
        );
        for evil in [
            "/\tevil.com",
            "/\t/evil.com",
            "/\n/evil.com",
            "/\r/evil.com",
            "\\evil.com",
            "//evil.com",
            "https://evil.com",
            "///evil.com",
            "/\\/evil.com",
            "",
        ] {
            assert_eq!(same_origin(Some(evil)), None, "{evil:?}");
        }
        assert_eq!(same_origin(None), None);
        assert_eq!(
            same_origin(Some("/recipes/1?a=b#c")),
            Some("/recipes/1?a=b#c".into())
        );
    }

    #[test]
    fn apple_client_secret_is_an_es256_jwt() {
        use ring::signature::{ECDSA_P256_SHA256_FIXED, EcdsaKeyPair, KeyPair};
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 =
            EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING_ALG, &rng).unwrap();
        let pem = format!(
            "-----BEGIN PRIVATE KEY-----\\n{}\\n-----END PRIVATE KEY-----",
            STANDARD.encode(pkcs8.as_ref())
        );
        let apple = Provider::apple("com.example.web", "TEAM", "KEY1", &pem);
        let secret = apple.client_secret().unwrap();
        let parts: Vec<&str> = secret.split('.').collect();
        assert_eq!(parts.len(), 3);
        let header: Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0]).unwrap()).unwrap();
        assert_eq!(header, json!({"alg": "ES256", "kid": "KEY1"}));
        let claims: Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
        assert_eq!(claims["iss"], "TEAM");
        assert_eq!(claims["sub"], "com.example.web");
        assert_eq!(claims["aud"], "https://appleid.apple.com");
        let key =
            EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING_ALG, pkcs8.as_ref(), &rng)
                .unwrap();
        let public = ring::signature::UnparsedPublicKey::new(
            &ECDSA_P256_SHA256_FIXED,
            key.public_key().as_ref(),
        );
        let sig = URL_SAFE_NO_PAD.decode(parts[2]).unwrap();
        public
            .verify(format!("{}.{}", parts[0], parts[1]).as_bytes(), &sig)
            .unwrap();
    }

    use ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING as ECDSA_P256_SHA256_FIXED_SIGNING_ALG;
}
