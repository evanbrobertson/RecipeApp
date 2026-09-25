//! Minimal OAuth 2.1 authorization server (dynamic client registration + PKCE)
//! so the app can be added to Claude as a custom connector.

use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::{Json, Router, routing};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

use crate::AppState;
use crate::auth::{check_password, is_logged_in, login_cookie, random_token, sha256_hex};
use crate::error::AppError;
use crate::model::now_secs;

const MINUTE: i64 = 60;
const DAY: i64 = 24 * 60 * MINUTE;

#[derive(Clone, Copy)]
enum Kind {
    Code,
    Access,
    Refresh,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Code => "code",
            Kind::Access => "access",
            Kind::Refresh => "refresh",
        }
    }

    fn lifetime(self) -> i64 {
        match self {
            Kind::Code => 5 * MINUTE,
            Kind::Access => 30 * DAY,
            Kind::Refresh => 365 * DAY,
        }
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            routing::get(auth_server_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource",
            routing::get(protected_resource_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            routing::get(protected_resource_metadata),
        )
        .route(
            "/oauth/authorize",
            routing::get(authorize_get).post(authorize_post),
        )
        .route(
            "/oauth/register",
            routing::post(register).options(preflight),
        )
        .route("/oauth/token", routing::post(token).options(preflight))
        .route("/oauth/revoke", routing::post(revoke).options(preflight))
}

fn cors(mut res: Response) -> Response {
    res.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    res
}

async fn preflight() -> Response {
    let mut res = cors(StatusCode::NO_CONTENT.into_response());
    let h = res.headers_mut();
    h.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("POST, OPTIONS"),
    );
    h.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("content-type, authorization"),
    );
    res
}

pub fn auth_server_metadata_json(origin: &str) -> Value {
    json!({
        "issuer": origin,
        "authorization_endpoint": format!("{origin}/oauth/authorize"),
        "token_endpoint": format!("{origin}/oauth/token"),
        "registration_endpoint": format!("{origin}/oauth/register"),
        "revocation_endpoint": format!("{origin}/oauth/revoke"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
        "scopes_supported": ["recipes"],
    })
}

async fn auth_server_metadata(State(state): State<AppState>, headers: HeaderMap) -> Response {
    cors(
        Json(auth_server_metadata_json(
            &state.config.public_origin(&headers),
        ))
        .into_response(),
    )
}

async fn protected_resource_metadata(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let origin = state.config.public_origin(&headers);
    cors(
        Json(json!({
            "resource": format!("{origin}/mcp"),
            "authorization_servers": [origin],
            "scopes_supported": ["recipes"],
            "bearer_methods_supported": ["header"],
            "resource_name": "Crumb",
        }))
        .into_response(),
    )
}

pub fn is_allowed_redirect_uri(uri: &str) -> bool {
    let Ok(url) = url::Url::parse(uri) else {
        return false;
    };
    if url.fragment().is_some() {
        return false;
    }
    match url.scheme() {
        "https" => true,
        "http" => matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")),
        _ => false,
    }
}

/// h3's readBody: form-encoded or JSON depending on the content type.
fn read_body(headers: &HeaderMap, body: &Bytes) -> Option<Map<String, Value>> {
    let ctype = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let form = || -> Option<Map<String, Value>> {
        let pairs: Vec<(String, String)> = serde_urlencoded::from_bytes(body).ok()?;
        Some(
            pairs
                .into_iter()
                .map(|(k, v)| (k, Value::String(v)))
                .collect(),
        )
    };
    if ctype.contains("application/x-www-form-urlencoded") {
        return form();
    }
    match serde_json::from_slice::<Value>(body) {
        Ok(Value::Object(m)) => Some(m),
        Ok(_) => None,
        Err(_) if !ctype.contains("json") => form(),
        Err(_) => None,
    }
}

fn get_str<'a>(m: &'a Map<String, Value>, k: &str) -> Option<&'a str> {
    m.get(k).and_then(Value::as_str)
}

async fn register(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let invalid = || {
        cors(
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "invalid_redirect_uri",
                    "error_description": "redirect_uris must be https (or http://localhost) URLs",
                })),
            )
                .into_response(),
        )
    };
    let Some(m) = read_body(&headers, &body) else {
        return invalid();
    };
    let client_name = match m.get("client_name") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.chars().count() <= 200 => Some(s.clone()),
        _ => return invalid(),
    };
    let Some(Value::Array(uris)) = m.get("redirect_uris") else {
        return invalid();
    };
    let uris: Option<Vec<String>> = uris.iter().map(|u| u.as_str().map(String::from)).collect();
    let Some(uris) = uris.filter(|u| (1..=10).contains(&u.len())) else {
        return invalid();
    };
    if !uris.iter().all(|u| is_allowed_redirect_uri(u)) {
        return invalid();
    }

    let id = random_token(16);
    let saved = state.db.lock().execute(
        "INSERT INTO oauth_clients (id, name, redirect_uris, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![
            id,
            client_name,
            serde_json::to_string(&uris).unwrap_or_default(),
            now_secs()
        ],
    );
    if let Err(err) = saved {
        return AppError::internal(err).into_response();
    }
    let mut out = json!({
        "client_id": id,
        "client_id_issued_at": now_secs(),
        "redirect_uris": uris,
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none",
    });
    if let Some(name) = client_name {
        out["client_name"] = json!(name);
    }
    cors((StatusCode::CREATED, Json(out)).into_response())
}

struct Client {
    id: String,
    name: Option<String>,
    redirect_uris: Vec<String>,
}

fn get_client(state: &AppState, id: &str) -> Option<Client> {
    state
        .db
        .lock()
        .query_row(
            "SELECT id, name, redirect_uris FROM oauth_clients WHERE id = ?1",
            [id],
            |r| {
                let uris: String = r.get(2)?;
                Ok(Client {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    redirect_uris: serde_json::from_str(&uris).unwrap_or_default(),
                })
            },
        )
        .optional()
        .ok()
        .flatten()
}

fn issue(
    state: &AppState,
    kind: Kind,
    client_id: &str,
    challenge: Option<&str>,
    redirect: Option<&str>,
) -> Result<String, AppError> {
    let token = random_token(32);
    let now = now_secs();
    state.db.lock().execute(
        "INSERT INTO oauth_tokens (hash, kind, client_id, code_challenge, redirect_uri, expires_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![sha256_hex(&token), kind.as_str(), client_id, challenge, redirect, now + kind.lifetime(), now],
    )?;
    Ok(token)
}

struct TokenRow {
    hash: String,
    client_id: String,
    code_challenge: Option<String>,
    redirect_uri: Option<String>,
}

/// Looks up an unexpired token of the given kind and deletes it (single use).
fn consume(state: &AppState, kind: Kind, token: &str) -> Option<TokenRow> {
    let conn = state.db.lock();
    let row = conn
        .query_row(
            "SELECT hash, client_id, code_challenge, redirect_uri FROM oauth_tokens
             WHERE hash = ?1 AND kind = ?2 AND expires_at > ?3",
            params![sha256_hex(token), kind.as_str(), now_secs()],
            |r| {
                Ok(TokenRow {
                    hash: r.get(0)?,
                    client_id: r.get(1)?,
                    code_challenge: r.get(2)?,
                    redirect_uri: r.get(3)?,
                })
            },
        )
        .optional()
        .ok()
        .flatten()?;
    let _ = conn.execute("DELETE FROM oauth_tokens WHERE hash = ?1", [&row.hash]);
    Some(row)
}

/// Validates the bearer token on an MCP request. Always true when auth is disabled.
pub fn has_valid_access_token(state: &AppState, headers: &HeaderMap) -> bool {
    if !state.config.auth_enabled() {
        return true;
    }
    let header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let Some((scheme, token)) = header.split_once(char::is_whitespace) else {
        return false;
    };
    let token = token.trim();
    if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
        return false;
    }
    state
        .db
        .lock()
        .query_row(
            "SELECT 1 FROM oauth_tokens WHERE hash = ?1 AND kind = 'access' AND expires_at > ?2",
            params![sha256_hex(token), now_secs()],
            |_| Ok(()),
        )
        .optional()
        .ok()
        .flatten()
        .is_some()
}

fn oauth_error(status: StatusCode, error: &str, description: &str) -> Response {
    (
        status,
        Json(json!({"error": error, "error_description": description})),
    )
        .into_response()
}

fn token_response(state: &AppState, client_id: &str) -> Result<Value, AppError> {
    // Opportunistically purge expired rows
    state.db.lock().execute(
        "DELETE FROM oauth_tokens WHERE expires_at < ?1",
        [now_secs()],
    )?;
    Ok(json!({
        "access_token": issue(state, Kind::Access, client_id, None, None)?,
        "token_type": "Bearer",
        "expires_in": Kind::Access.lifetime(),
        "refresh_token": issue(state, Kind::Refresh, client_id, None, None)?,
        "scope": "recipes",
    }))
}

async fn token(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let m = read_body(&headers, &body).unwrap_or_default();
    let bad = StatusCode::BAD_REQUEST;
    let result = match get_str(&m, "grant_type") {
        Some("authorization_code") => {
            let (Some(code), Some(verifier)) = (get_str(&m, "code"), get_str(&m, "code_verifier"))
            else {
                return no_store(cors(oauth_error(
                    bad,
                    "invalid_request",
                    "Missing code or code_verifier",
                )));
            };
            let Some(row) = consume(&state, Kind::Code, code) else {
                return no_store(cors(oauth_error(
                    bad,
                    "invalid_grant",
                    "Authorization code is invalid or expired",
                )));
            };
            if get_str(&m, "client_id").is_some_and(|c| !c.is_empty() && c != row.client_id) {
                return no_store(cors(oauth_error(
                    bad,
                    "invalid_grant",
                    "Code was issued to another client",
                )));
            }
            if get_str(&m, "redirect_uri")
                .is_some_and(|r| !r.is_empty() && Some(r) != row.redirect_uri.as_deref())
            {
                return no_store(cors(oauth_error(
                    bad,
                    "invalid_grant",
                    "redirect_uri does not match",
                )));
            }
            let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
            if Some(challenge.as_str()) != row.code_challenge.as_deref() {
                return no_store(cors(oauth_error(
                    bad,
                    "invalid_grant",
                    "PKCE verification failed",
                )));
            }
            token_response(&state, &row.client_id)
        }
        Some("refresh_token") => {
            let Some(refresh) = get_str(&m, "refresh_token") else {
                return no_store(cors(oauth_error(
                    bad,
                    "invalid_request",
                    "Missing refresh_token",
                )));
            };
            let Some(row) = consume(&state, Kind::Refresh, refresh) else {
                return no_store(cors(oauth_error(
                    bad,
                    "invalid_grant",
                    "Refresh token is invalid or expired",
                )));
            };
            token_response(&state, &row.client_id)
        }
        _ => {
            return no_store(cors(
                (bad, Json(json!({"error": "unsupported_grant_type"}))).into_response(),
            ));
        }
    };
    match result {
        Ok(v) => no_store(cors(Json(v).into_response())),
        Err(err) => cors(err.into_response()),
    }
}

fn no_store(mut res: Response) -> Response {
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

async fn revoke(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let m = read_body(&headers, &body).unwrap_or_default();
    if let Some(token) = get_str(&m, "token") {
        let _ = state.db.lock().execute(
            "DELETE FROM oauth_tokens WHERE hash = ?1",
            [sha256_hex(token)],
        );
    }
    cors(Json(json!({})).into_response())
}

// ─── Consent screen ─────────────────────────────────────────────────────────

pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

fn page(title: &str, body: &str) -> String {
    format!(
        r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<meta name="theme-color" content="#ee5a3a">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<title>{} · Crumb</title>
<style>
  :root {{ color-scheme: light dark; --bg:#fbf7f1; --card:#fffdf8; --fg:#2a211a; --muted:#7a6a5b; --line:#e6dac8; --accent:#ee5a3a; --accent-fg:#fff; --r:22px }}
  @media (prefers-color-scheme: dark) {{ :root {{ --bg:#15110d; --card:#221c16; --fg:#f6efe6; --muted:#b3a393; --line:#342b22; }} }}
  * {{ box-sizing:border-box }}
  body {{ margin:0; min-height:100vh; display:grid; place-items:center; padding:16px; background:var(--bg); color:var(--fg);
    font:16px/1.5 ui-rounded, system-ui, -apple-system, "Segoe UI", sans-serif }}
  main {{ width:100%; max-width:420px; background:var(--card); border:1px solid var(--line); border-radius:var(--r); padding:28px }}
  .brand {{ display:flex; align-items:center; gap:10px; font-weight:700; margin-bottom:18px }}
  .brand span {{ display:grid; place-items:center; width:36px; height:36px; border-radius:12px; background:var(--accent); color:var(--accent-fg); font-size:20px }}
  h1 {{ font-size:1.3rem; margin:0 0 8px; line-height:1.25 }} p {{ color:var(--muted); margin:0 0 20px }}
  label {{ display:block; font-size:.875rem; font-weight:600; margin-bottom:6px }}
  input[type=password] {{ width:100%; padding:12px 16px; border-radius:var(--r); border:1px solid var(--line); background:var(--bg); color:inherit; font:inherit; margin-bottom:16px }}
  input[type=password]:focus {{ outline:2px solid var(--accent); outline-offset:1px }}
  .row {{ display:flex; gap:10px }}
  button {{ flex:1; padding:12px 16px; border-radius:var(--r); border:1px solid var(--line); font:inherit; font-weight:600; cursor:pointer; background:transparent; color:inherit }}
  button.primary {{ background:var(--accent); border-color:var(--accent); color:var(--accent-fg) }}
  .error {{ color:#d9432a; font-size:.875rem; margin:-8px 0 12px }} code {{ font-size:.85em }}
</style></head><body><main><div class="brand"><span aria-hidden="true">✦</span>Crumb</div>{}</main></body></html>"##,
        escape_html(title),
        body
    )
}

#[derive(Default)]
struct AuthorizeParams {
    client_id: String,
    redirect_uri: String,
    state: String,
    code_challenge: String,
    code_challenge_method: String,
    response_type: String,
}

impl AuthorizeParams {
    fn read(get: impl Fn(&str) -> Option<String>) -> Self {
        let v = |k: &str| get(k).unwrap_or_default();
        let or = |k: &str, d: &str| {
            get(k)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| d.to_string())
        };
        Self {
            client_id: v("client_id"),
            redirect_uri: v("redirect_uri"),
            state: v("state"),
            code_challenge: v("code_challenge"),
            code_challenge_method: or("code_challenge_method", "S256"),
            response_type: or("response_type", "code"),
        }
    }

    fn pairs(&self) -> [(&'static str, &str); 6] {
        [
            ("client_id", &self.client_id),
            ("redirect_uri", &self.redirect_uri),
            ("state", &self.state),
            ("code_challenge", &self.code_challenge),
            ("code_challenge_method", &self.code_challenge_method),
            ("response_type", &self.response_type),
        ]
    }
}

fn fail(message: &str) -> Response {
    let mut res = (
        StatusCode::BAD_REQUEST,
        Html(page(
            "Can't connect",
            &format!("<h1>Can't connect</h1><p>{}</p>", escape_html(message)),
        )),
    )
        .into_response();
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

fn redirect_back(redirect_uri: &str, params: &[(&str, &str)]) -> Response {
    let Ok(mut url) = url::Url::parse(redirect_uri) else {
        return fail("Invalid redirect URI.");
    };
    let keys: Vec<&str> = params
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, _)| *k)
        .collect();
    let kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| !keys.contains(&k.as_ref()))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    {
        let mut q = url.query_pairs_mut();
        q.clear();
        for (k, v) in &kept {
            q.append_pair(k, v);
        }
        for (k, v) in params.iter().filter(|(_, v)| !v.is_empty()) {
            q.append_pair(k, v);
        }
    }
    if url.query() == Some("") {
        url.set_query(None);
    }
    crate::auth::found(url.as_str())
}

async fn authorize_get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let params = AuthorizeParams::read(|k| q.get(k).cloned());
    authorize(&state, &headers, Method::GET, params, HashMap::new()).await
}

async fn authorize_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let form: HashMap<String, String> = serde_urlencoded::from_bytes(&body).unwrap_or_default();
    let params = AuthorizeParams::read(|k| form.get(k).cloned());
    authorize(&state, &headers, Method::POST, params, form).await
}

async fn authorize(
    state: &AppState,
    headers: &HeaderMap,
    method: Method,
    params: AuthorizeParams,
    form: HashMap<String, String>,
) -> Response {
    let client = if params.client_id.is_empty() {
        None
    } else {
        get_client(state, &params.client_id)
    };
    let Some(client) = client else {
        return fail("Unknown client. Remove the connector in Claude and add it again.");
    };
    if !client.redirect_uris.contains(&params.redirect_uri) {
        return fail("The redirect URI doesn't match this client's registration.");
    }
    if params.response_type != "code" {
        return redirect_back(
            &params.redirect_uri,
            &[
                ("error", "unsupported_response_type"),
                ("state", &params.state),
            ],
        );
    }
    if params.code_challenge.is_empty() || params.code_challenge_method != "S256" {
        return redirect_back(
            &params.redirect_uri,
            &[
                ("error", "invalid_request"),
                ("error_description", "PKCE with S256 is required"),
                ("state", &params.state),
            ],
        );
    }

    let logged_in = is_logged_in(&state.config, headers);
    let mut error = "";
    if method == Method::POST {
        if form.get("action").map(String::as_str) == Some("deny") {
            return redirect_back(
                &params.redirect_uri,
                &[("error", "access_denied"), ("state", &params.state)],
            );
        }
        let password = form.get("password").map(String::as_str).unwrap_or("");
        if logged_in || check_password(&state.config, password) {
            let code = match issue(
                state,
                Kind::Code,
                &client.id,
                Some(&params.code_challenge),
                Some(&params.redirect_uri),
            ) {
                Ok(code) => code,
                Err(err) => return err.into_response(),
            };
            let mut res = redirect_back(
                &params.redirect_uri,
                &[("code", &code), ("state", &params.state)],
            );
            if !logged_in && let Some(cookie) = login_cookie(&state.config, headers) {
                res.headers_mut().append(header::SET_COOKIE, cookie);
            }
            return res;
        }
        tokio::time::sleep(std::time::Duration::from_millis(750)).await;
        error = "Incorrect password.";
    }

    let client_name = client
        .name
        .as_deref()
        .filter(|n| !n.is_empty())
        .unwrap_or("An application");
    let host = url::Url::parse(&params.redirect_uri)
        .ok()
        .and_then(|u| {
            u.host_str().map(|h| match u.port() {
                Some(p) => format!("{h}:{p}"),
                None => h.to_string(),
            })
        })
        .unwrap_or_default();
    let hidden: String = params
        .pairs()
        .iter()
        .map(|(k, v)| {
            format!(
                r#"<input type="hidden" name="{k}" value="{}">"#,
                escape_html(v)
            )
        })
        .collect();
    let password_field = if logged_in {
        String::new()
    } else {
        r#"<label for="password">App password</label>
             <input id="password" name="password" type="password" autocomplete="current-password" required autofocus>"#
            .to_string()
    };
    let error_html = if error.is_empty() {
        String::new()
    } else {
        format!(r#"<div class="error">{}</div>"#, escape_html(error))
    };
    let name = escape_html(client_name);
    let body = format!(
        r#"<h1>Connect {name}?</h1>
    <p><strong>{name}</strong> (<code>{}</code>) wants to read, add and edit recipes in Crumb.</p>
    <form method="post" action="/oauth/authorize">
      {hidden}
      {password_field}
      {error_html}
      <div class="row">
        <button type="submit" name="action" value="deny" formnovalidate>Cancel</button>
        <button type="submit" name="action" value="allow" class="primary">Allow</button>
      </div>
    </form>"#,
        escape_html(&host)
    );
    let mut res = Html(page("Connect", &body)).into_response();
    let h = res.headers_mut();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirect_uri_rules() {
        assert!(is_allowed_redirect_uri(
            "https://claude.ai/api/mcp/auth_callback"
        ));
        assert!(is_allowed_redirect_uri("http://localhost:6274/cb"));
        assert!(!is_allowed_redirect_uri("http://evil.test/cb"));
        assert!(!is_allowed_redirect_uri("https://a.test/cb#frag"));
        assert!(!is_allowed_redirect_uri("javascript:alert(1)"));
    }

    #[test]
    fn redirect_back_sets_params() {
        let res = redirect_back(
            "https://a.test/cb?state=old&x=1",
            &[("code", "abc"), ("state", "s1")],
        );
        let loc = res.headers()[header::LOCATION].to_str().unwrap();
        assert_eq!(loc, "https://a.test/cb?x=1&code=abc&state=s1");
    }
}
