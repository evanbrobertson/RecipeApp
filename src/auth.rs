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

const PUBLIC_PREFIXES: [&str; 5] = ["/_astro/", "/fonts/", "/oauth/", "/.well-known/", "/mcp"];
const PUBLIC_PATHS: [&str; 10] = [
    "/login",
    "/api/auth/login",
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
    PUBLIC_PATHS.contains(&trimmed)
        || PUBLIC_PREFIXES.iter().any(|p| path.starts_with(p))
        || (trimmed.starts_with("/icon-")
            && trimmed.ends_with(".png")
            && !trimmed[1..].contains('/'))
}

/// Protects every page and API route behind the app password. The MCP endpoint does
/// its own bearer-token check; OAuth endpoints must stay public.
pub async fn require_login(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path();
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
    }
}
