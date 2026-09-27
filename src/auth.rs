//! App password, session cookie and the middleware that protects every page and API route.

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use rand::RngCore;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::AppState;
use crate::config::Config;
use crate::error::AppError;

pub const COOKIE: &str = "crumb_session";
const MAX_AGE_SECS: i64 = 60 * 60 * 24 * 90;

pub fn sha256_hex(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn random_token(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

pub fn check_password(config: &Config, candidate: &str) -> bool {
    let Some(password) = &config.app_password else {
        return true;
    };
    let expected = Sha256::digest(password.as_bytes());
    let actual = Sha256::digest(candidate.as_bytes());
    expected.ct_eq(&actual).into()
}

/// Session key derived from the password, so changing it signs everyone out.
fn signature(password: &str, issued: i64) -> Vec<u8> {
    let key = Sha256::digest(format!("crumb-session:{password}").as_bytes());
    let mut mac = Hmac::<Sha256>::new_from_slice(&key).expect("any key length");
    mac.update(format!("ok:{issued}").as_bytes());
    mac.finalize().into_bytes().to_vec()
}

pub fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_string())
}

pub fn is_logged_in(config: &Config, headers: &HeaderMap) -> bool {
    let Some(password) = &config.app_password else {
        return true;
    };
    let Some(value) = cookie_value(headers, COOKIE) else {
        return false;
    };
    let Some((issued, sig)) = value.split_once('.') else {
        return false;
    };
    let Ok(issued) = issued.parse::<i64>() else {
        return false;
    };
    let Ok(sig) = URL_SAFE_NO_PAD.decode(sig) else {
        return false;
    };
    let fresh = (0..MAX_AGE_SECS).contains(&(crate::model::now_secs() - issued));
    fresh && bool::from(signature(password, issued).ct_eq(&sig))
}

/// Whether the client reached us over HTTPS (Railway terminates TLS at its proxy).
pub fn is_https(headers: &HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .is_some_and(|p| p.trim().eq_ignore_ascii_case("https"))
}

/// `Set-Cookie` for a fresh session. SameSite=Lax also stops cross-site form posts.
pub fn login_cookie(config: &Config, headers: &HeaderMap) -> Option<HeaderValue> {
    let password = config.app_password.as_ref()?;
    let issued = crate::model::now_secs();
    let sig = URL_SAFE_NO_PAD.encode(signature(password, issued));
    let secure = if is_https(headers) { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{COOKIE}={issued}.{sig}; Path=/; Max-Age={MAX_AGE_SECS}; HttpOnly; SameSite=Lax{secure}"
    ))
    .ok()
}

/// A plain 302, which is what the Nuxt server sent for every redirect.
pub fn found(location: &str) -> Response {
    let mut res = axum::http::StatusCode::FOUND.into_response();
    if let Ok(v) = HeaderValue::from_str(location) {
        res.headers_mut().insert(header::LOCATION, v);
    }
    res
}

pub fn logout_cookie(headers: &HeaderMap) -> HeaderValue {
    let secure = if is_https(headers) { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{COOKIE}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax{secure}"
    ))
    .expect("static cookie")
}

/// `/s/` is share links (src/share.rs): a token gives one recipe, read-only.
const PUBLIC_PREFIXES: [&str; 6] = [
    "/_astro/",
    "/fonts/",
    "/oauth/",
    "/.well-known/",
    "/mcp",
    "/s/",
];
const PUBLIC_PATHS: [&str; 19] = [
    "/login",
    "/api/auth/login",
    // Accounts: first-run setup and sign-up (both refuse when they don't apply)
    "/setup",
    "/signup",
    "/api/auth/status",
    "/api/auth/setup",
    "/api/auth/signup",
    // Accounts: an invite link's page, and joining with it (signed in or not)
    "/invite",
    "/api/auth/invite/preview",
    "/api/auth/invite/accept",
    // Hosted: where a password reset email's link lands
    "/reset-password",
    "/api/health",
    "/robots.txt",
    "/manifest.webmanifest",
    "/favicon.svg",
    "/favicon.ico",
    "/icon.svg",
    "/apple-touch-icon.png",
    "/speculation-rules.json",
];

pub fn is_public(path: &str) -> bool {
    let trimmed = if path.len() > 1 {
        path.trim_end_matches('/')
    } else {
        path
    };
    // No dot segments into a public prefix ("/s/../api")
    let climbs = path.split('/').any(|seg| seg == "..");
    PUBLIC_PATHS.contains(&trimmed)
        || (!climbs && PUBLIC_PREFIXES.iter().any(|p| path.starts_with(p)))
        || (trimmed.starts_with("/icon-")
            && trimmed.ends_with(".png")
            && !trimmed[1..].contains('/'))
}

/// Who a request is signed in as, with accounts: set by [`require_login`] beside the
/// [`crate::Scoped`] state for their household.
#[derive(Clone)]
pub struct SignedIn(pub crate::accounts::Session);

impl axum::extract::FromRequestParts<AppState> for SignedIn {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _: &AppState,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<SignedIn>()
            .cloned()
            .ok_or_else(|| AppError::new(401, "Not signed in"))
    }
}

/// The live accounts session for a request's cookie (None without accounts): ours with
/// `AUTH_MODE=accounts`, Better Auth's (see [`crate::hosted`]) with `hosted`.
pub async fn session(state: &AppState, headers: &HeaderMap) -> Option<crate::accounts::Session> {
    let accounts = state.accounts.as_ref()?;
    let found = match &state.hosted {
        Some(hosted) => hosted.session(accounts, headers).await,
        None => {
            let token = cookie_value(headers, COOKIE)?;
            accounts.session(&token)
        }
    };
    match found {
        Ok(found) => found,
        Err(err) => {
            tracing::warn!("[auth] couldn't read a session: {}", err.message);
            None
        }
    }
}

/// Whether the request is signed in, however this install signs people in.
pub async fn signed_in(state: &AppState, headers: &HeaderMap) -> bool {
    if state.accounts.is_some() {
        session(state, headers).await.is_some()
    } else {
        is_logged_in(&state.config, headers)
    }
}

/// `Set-Cookie` for an accounts session's token.
pub fn session_cookie(token: &str, headers: &HeaderMap) -> Option<HeaderValue> {
    let secure = if is_https(headers) { "; Secure" } else { "" };
    let max_age = crate::accounts::SESSION_SECS;
    HeaderValue::from_str(&format!(
        "{COOKIE}={token}; Path=/; Max-Age={max_age}; HttpOnly; SameSite=Lax{secure}"
    ))
    .ok()
}

/// Protects every page and API route behind the app password, or with accounts, a
/// session (and scopes the request to that session's household). The MCP endpoint does
/// its own bearer-token check; OAuth endpoints must stay public.
pub async fn require_login(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path();
    if let Some(accounts) = &state.accounts {
        // Hosted: public pages and Better Auth's routes never look the session up, since
        // that makes a kitchen for someone in no household (who may be about to accept an
        // invite instead). Handlers that want it ask for themselves.
        let skip = state.hosted.is_some() && (is_public(path) || path.starts_with("/api/auth/"));
        if !skip && let Some(signed) = session(&state, req.headers()).await {
            let scoped = match state.for_household(signed.household_id) {
                Ok(scoped) => scoped,
                Err(err) => return err.into_response(),
            };
            req.extensions_mut().insert(crate::Scoped(scoped));
            req.extensions_mut().insert(SignedIn(signed));
            return next.run(req).await;
        }
        // Hosted: Better Auth's own routes (signing in, up, resets) check for themselves
        if is_public(path) || (state.hosted.is_some() && path.starts_with("/api/auth/")) {
            return next.run(req).await;
        }
        if path.starts_with("/api/") {
            return AppError::new(401, "Not signed in").into_response();
        }
        let page = if state.hosted.is_none() && accounts.needs_setup().unwrap_or(false) {
            "/setup"
        } else {
            "/login"
        };
        let target = req
            .uri()
            .path_and_query()
            .map(|p| p.as_str())
            .unwrap_or("/");
        let next_param = utf8_percent_encode(target, NON_ALPHANUMERIC);
        return found(&format!("{page}?next={next_param}"));
    }
    if state.config.app_password.is_none()
        || is_public(path)
        || is_logged_in(&state.config, req.headers())
    {
        return next.run(req).await;
    }
    if path.starts_with("/api/") {
        return AppError::new(401, "Not signed in").into_response();
    }
    let target = req
        .uri()
        .path_and_query()
        .map(|p| p.as_str())
        .unwrap_or("/");
    let next_param = utf8_percent_encode(target, NON_ALPHANUMERIC);
    found(&format!("/login?next={next_param}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(pw: Option<&str>) -> Config {
        Config {
            app_password: pw.map(String::from),
            ..Config::default()
        }
    }

    #[test]
    fn sessions_round_trip_and_rotate_with_the_password() {
        let cfg = config(Some("hunter2"));
        let set = login_cookie(&cfg, &HeaderMap::new()).unwrap();
        let pair = set.to_str().unwrap().split(';').next().unwrap().to_string();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("other=1; {pair}")).unwrap(),
        );
        assert!(is_logged_in(&cfg, &headers));
        assert!(!is_logged_in(&config(Some("changed")), &headers));
        assert!(!is_logged_in(&cfg, &HeaderMap::new()));
        assert!(is_logged_in(&config(None), &HeaderMap::new()));
    }

    #[test]
    fn passwords_and_public_paths() {
        let cfg = config(Some("pw"));
        assert!(check_password(&cfg, "pw"));
        assert!(!check_password(&cfg, "nope"));
        assert!(is_public("/login"));
        assert!(is_public("/_astro/app.123.js"));
        assert!(is_public("/icon-192.png"));
        assert!(!is_public("/icon-192.png/../api"));
        assert!(!is_public("/recipes"));
        assert!(!is_public("/api/recipes"));
        assert!(is_public("/s/abcdefghijklmnopqrstuv"));
        assert!(is_public("/s/abcdefghijklmnopqrstuv/img/768"));
        assert!(!is_public("/s"));
        assert!(!is_public("/sx"));
        assert!(!is_public("/sxyz/abc"));
        assert!(!is_public("/s/../api/recipes"));
    }
}
