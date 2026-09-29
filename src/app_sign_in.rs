//! Google and Apple sign-in for the native apps, in either account mode. The provider only
//! sends people back to this server's web address, so the app signs in through the browser
//! and is handed the session afterwards (see `crumb_core::app_link`):
//!
//! 1. `POST /api/auth/app/start` `{provider, challenge}` → `{url}`: the app keeps a PKCE
//!    verifier and opens `url`, the `/app/sign-in` page, in the browser.
//! 2. The page signs in as the web does, sending the browser on to
//!    `GET /api/auth/app/done?id=…`, which gives the browser's session a one-time code and
//!    sends it to `app.crumb://signed-in?code=…`.
//! 3. `POST /api/auth/app/redeem` `{code, verifier}` answers with the app's own session
//!    cookie. Accounts: a new session, named for the app. Hosted: the browser's Better Auth
//!    session, since only the auth service makes those.
//!
//! The code only reaches the app on the device that signed in, and is no good without the
//! verifier, so a link someone is tricked into opening gives the trickster nothing. Both
//! steps are held in memory, briefly: a restart only drops sign-ins in progress.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::{Json, Router, routing};
use crumb_core::app_link::{pkce_challenge, signed_in_link};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::account_api::{json_body, new_session};
use crate::auth::{self, random_token};
use crate::error::{AppError, AppResult};

/// How long the browser has to sign in.
const START_TTL: Duration = Duration::from_secs(10 * 60);
/// How long the app has to redeem its code.
const CODE_TTL: Duration = Duration::from_secs(2 * 60);
/// Sign-ins in progress at once, so nobody fills memory with starts.
const MAX_WAITING: usize = 500;

/// What the app is handed.
#[derive(Clone)]
enum Grant {
    /// Accounts: who and which household, for a new session.
    Accounts { user: i64, household: i64 },
    /// Hosted: the browser's Better Auth cookie, `name=value`.
    Hosted(String),
}

struct Started {
    challenge: String,
    at: Instant,
}

struct Done {
    challenge: String,
    grant: Grant,
    at: Instant,
}

/// The apps' sign-ins in progress.
#[derive(Default)]
pub struct AppSignIns {
    started: Mutex<HashMap<String, Started>>,
    done: Mutex<HashMap<String, Done>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl AppSignIns {
    fn start(&self, challenge: String) -> AppResult<String> {
        let mut started = lock(&self.started);
        started.retain(|_, s| s.at.elapsed() < START_TTL);
        if started.len() >= MAX_WAITING {
            return Err(AppError::new(
                429,
                "Too many sign-ins at once. Try again soon.",
            ));
        }
        let id = random_token(24);
        started.insert(
            id.clone(),
            Started {
                challenge,
                at: Instant::now(),
            },
        );
        Ok(id)
    }

    /// The browser signed in: swaps the start for a code.
    fn finish(&self, id: &str, grant: Grant) -> Option<String> {
        let started = lock(&self.started)
            .remove(id)
            .filter(|s| s.at.elapsed() < START_TTL)?;
        let code = random_token(32);
        let mut done = lock(&self.done);
        done.retain(|_, d| d.at.elapsed() < CODE_TTL);
        done.insert(
            code.clone(),
            Done {
                challenge: started.challenge,
                grant,
                at: Instant::now(),
            },
        );
        Some(code)
    }

    /// One try per code: a wrong verifier uses it up.
    fn redeem(&self, code: &str, verifier: &str) -> Option<Grant> {
        let done = lock(&self.done)
            .remove(code)
            .filter(|d| d.at.elapsed() < CODE_TTL)?;
        let ok: bool = subtle::ConstantTimeEq::ct_eq(
            pkce_challenge(verifier).as_bytes(),
            done.challenge.as_bytes(),
        )
        .into();
        ok.then_some(done.grant)
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/auth/app/start", routing::post(start))
        .route("/api/auth/app/done", routing::get(done))
        .route("/api/auth/app/redeem", routing::post(redeem))
}

/// PKCE verifiers and S256 challenges: 43 to 128 URL-safe characters.
fn pkce_shaped(s: &str) -> bool {
    (43..=128).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~'))
}

async fn offered(state: &AppState, provider: &str) -> bool {
    match &state.hosted {
        Some(hosted) => hosted.providers().await.iter().any(|p| p == provider),
        None => crate::social::provider_ids(state).contains(&provider),
    }
}

async fn start(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Json<Value>> {
    if state.accounts.is_none() {
        return Err(AppError::not_found("Not found"));
    }
    let body = json_body(&body)?;
    let get = |k: &str| body.get(k).and_then(Value::as_str).unwrap_or("");
    let provider = get("provider");
    if !offered(&state, provider).await {
        return Err(AppError::not_found("That sign-in isn't set up here"));
    }
    let challenge = get("challenge");
    if !pkce_shaped(challenge) {
        return Err(AppError::bad_request("challenge: Not a PKCE challenge"));
    }
    let id = state.app_sign_ins.start(challenge.to_string())?;
    let url = format!(
        "{}/app/sign-in?provider={provider}&id={id}",
        state.config.public_origin(&headers)
    );
    Ok(Json(json!({ "url": url })))
}

#[derive(Deserialize)]
struct DoneQuery {
    id: Option<String>,
}

/// The browser's Better Auth cookie, as `name=value`.
fn hosted_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .map(str::trim)
        .find(|pair| {
            pair.split_once('=')
                .is_some_and(|(k, v)| crumb_core::client::is_hosted_cookie(k) && !v.is_empty())
        })
        .map(str::to_string)
}

async fn done(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<DoneQuery>,
) -> Response {
    let Some(session) = auth::session(&state, &headers).await else {
        return page(
            StatusCode::UNAUTHORIZED,
            "That sign-in didn't finish",
            "Go back to the Crumb app and try again.",
            None,
        );
    };
    let grant = if state.hosted.is_some() {
        match hosted_cookie(&headers) {
            Some(cookie) => Grant::Hosted(cookie),
            None => return expired(),
        }
    } else {
        Grant::Accounts {
            user: session.user_id,
            household: session.household_id,
        }
    };
    let id = q.id.unwrap_or_default();
    match state.app_sign_ins.finish(&id, grant) {
        Some(code) => page(
            StatusCode::OK,
            "You're signed in",
            "Back to the Crumb app you go.",
            Some(&signed_in_link(&code)),
        ),
        None => expired(),
    }
}

fn expired() -> Response {
    page(
        StatusCode::GONE,
        "That sign-in link has expired",
        "Go back to the Crumb app and try signing in again.",
        None,
    )
}

/// A plain page for the end of the browser's part: with `link`, it goes straight on to the
/// app, and has a button in case the browser asks first.
fn page(status: StatusCode, title: &str, text: &str, link: Option<&str>) -> Response {
    let (script, button) = match link {
        Some(link) => (
            format!("<script>location.replace({})</script>", json!(link)),
            format!(r#"<p><a class="btn" href="{link}">Open the Crumb app</a></p>"#),
        ),
        None => (String::new(), String::new()),
    };
    let html = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="referrer" content="no-referrer"><title>{title}</title>
<style>
:root{{color-scheme:light dark;--bg:#fbf7ef;--ink:#23302a;--muted:#5d6b63;--tile:#2f6b4f;--on:#fff}}
@media (prefers-color-scheme:dark){{:root{{--bg:#151c18;--ink:#eef2ee;--muted:#a9b5ad;--tile:#3f8a66}}}}
body{{margin:0;background:var(--bg);color:var(--ink);font:16px/1.5 system-ui,sans-serif}}
main{{max-width:24rem;margin:0 auto;padding:4rem 1.25rem}}
h1{{font-size:1.25rem;margin:0 0 .5rem}}p{{color:var(--muted);margin:0 0 1.5rem}}
.btn{{display:inline-block;background:var(--tile);color:var(--on);text-decoration:none;
padding:.75rem 1.25rem;border-radius:12px;font-weight:700}}
</style></head><body><main><h1>{title}</h1><p>{text}</p>{button}</main>{script}</body></html>"#
    );
    let mut res = (status, Html(html)).into_response();
    let h = res.headers_mut();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    res
}

async fn redeem(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Response> {
    if state.accounts.is_none() {
        return Err(AppError::not_found("Not found"));
    }
    let body = json_body(&body)?;
    let get = |k: &str| body.get(k).and_then(Value::as_str).unwrap_or("");
    let (code, verifier) = (get("code"), get("verifier"));
    let gone = || AppError::new(410, "That sign-in has expired. Try again.");
    if code.is_empty() || !pkce_shaped(verifier) {
        return Err(gone());
    }
    let cookie = match state.app_sign_ins.redeem(code, verifier).ok_or_else(gone)? {
        Grant::Accounts { user, household } => {
            let accounts = crate::account_api::accounts(&state)?;
            new_session(accounts, user, household, &headers)?
        }
        Grant::Hosted(pair) => {
            let secure = if auth::is_https(&headers) {
                "; Secure"
            } else {
                ""
            };
            HeaderValue::from_str(&format!("{pair}; Path=/; HttpOnly; SameSite=Lax{secure}")).ok()
        }
    };
    let mut res = Json(json!({ "ok": true })).into_response();
    if let Some(cookie) = cookie {
        res.headers_mut().append(header::SET_COOKIE, cookie);
    }
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_needs_its_verifier_and_works_once() {
        let ins = AppSignIns::default();
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let id = ins.start(pkce_challenge(verifier)).unwrap();
        let grant = Grant::Accounts {
            user: 1,
            household: 2,
        };
        assert!(ins.finish("nope", grant.clone()).is_none());
        let code = ins.finish(&id, grant.clone()).unwrap();
        assert!(ins.finish(&id, grant).is_none(), "a start finishes once");

        let other = "x".repeat(43);
        assert!(ins.redeem(&code, &other).is_none());
        assert!(
            ins.redeem(&code, verifier).is_none(),
            "a wrong try uses it up"
        );

        let id = ins.start(pkce_challenge(verifier)).unwrap();
        let code = ins.finish(&id, Grant::Hosted("a=b".into())).unwrap();
        assert!(matches!(ins.redeem(&code, verifier), Some(Grant::Hosted(c)) if c == "a=b"));
        assert!(ins.redeem(&code, verifier).is_none());
    }

    #[test]
    fn challenges_are_pkce_shaped() {
        assert!(pkce_shaped(&pkce_challenge("anything")));
        assert!(!pkce_shaped("short"));
        assert!(!pkce_shaped(&format!("{}\"", "a".repeat(43))));
        assert!(!pkce_shaped(&"a".repeat(129)));
    }

    #[test]
    fn picks_the_better_auth_cookie() {
        let mut h = HeaderMap::new();
        h.insert(
            header::COOKIE,
            HeaderValue::from_static("tz=UTC; __Secure-crumb.session_token=abc.sig; x=1"),
        );
        assert_eq!(
            hosted_cookie(&h).as_deref(),
            Some("__Secure-crumb.session_token=abc.sig")
        );
        h.insert(
            header::COOKIE,
            HeaderValue::from_static("crumb.session_token="),
        );
        assert_eq!(hosted_cookie(&h), None);
    }
}
