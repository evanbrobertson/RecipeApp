//! Request checks that don't belong to one route: refusing state changes that another
//! site started, validating `/mcp`'s `Origin`, and telling browsers to stay on HTTPS.

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::AppState;
use crate::error::AppError;

/// Protocol endpoints that other servers and apps call, with no page of ours behind them:
/// OAuth token exchange and registration, the connector, and sign-in providers' callbacks
/// (Apple posts its form cross-site). They carry their own credentials.
const OPEN_TO_OTHER_ORIGINS: [&str; 5] = [
    "/oauth/token",
    "/oauth/register",
    "/oauth/revoke",
    "/mcp",
    "/api/auth/social/",
];

/// `host[:port]` of an origin or URL, lower-cased, without the scheme's default port.
fn host_of(origin: &str) -> Option<String> {
    let url = url::Url::parse(origin).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    Some(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host,
    })
}

/// The `Host` header as `host[:port]`, lower-cased.
fn request_host(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(header::HOST)?.to_str().ok()?;
    host_of(&format!("http://{raw}"))
}

/// Whether an `Origin` header is this app's own: the configured public address, or (without
/// one) the host the request was sent to.
fn is_own_origin(state: &AppState, headers: &HeaderMap, origin: &str) -> bool {
    let Some(host) = host_of(origin) else {
        return false;
    };
    if let Some(public) = host_of(&state.config.public_origin(headers))
        && public == host
    {
        return true;
    }
    !state.config.origin_is_fixed() && request_host(headers).is_some_and(|h| h == host)
}

/// State-changing requests must come from this site. Browsers say where a request came from in
/// `Sec-Fetch-Site` (and `Origin`), and SameSite=Lax cookies don't cover other pages on the same
/// registrable domain, so `cross-site` and `same-site` are refused. Requests with neither header
/// (the native apps, scripts) aren't a browser acting for a page, and are let through.
pub async fn same_origin_only(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS) {
        return next.run(req).await;
    }
    let path = req.uri().path();
    if OPEN_TO_OTHER_ORIGINS.iter().any(|p| path.starts_with(p)) {
        return next.run(req).await;
    }
    let headers = req.headers();
    let fetch_site = headers
        .get("sec-fetch-site")
        .and_then(|v| v.to_str().ok())
        .map(str::to_ascii_lowercase);
    let allowed = match fetch_site.as_deref() {
        Some("same-origin" | "none") => true,
        Some(_) => false,
        None => match headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
            Some(origin) => is_own_origin(&state, headers, origin),
            None => true,
        },
    };
    if !allowed {
        return AppError::new(403, "This request came from another site").into_response();
    }
    next.run(req).await
}

/// Origins that may call `/mcp` from a browser besides this app's own: Claude's, and local
/// tools during development.
fn is_mcp_client_origin(origin: &str) -> bool {
    let Some(host) = url::Url::parse(origin)
        .ok()
        .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
    else {
        return false;
    };
    ["claude.ai", "claude.com"]
        .iter()
        .any(|k| host == *k || host.ends_with(&format!(".{k}")))
        || matches!(host.as_str(), "localhost" | "127.0.0.1" | "[::1]")
}

/// `/mcp` checks a browser's `Origin`, as the MCP transport spec asks, so a web page can't
/// drive the connector by rebinding its own name to this server. Requests without an `Origin`
/// (Claude's servers, scripts) are not browsers and pass.
pub async fn mcp_origin(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    if path == "/mcp" || path.starts_with("/mcp/") {
        let headers = req.headers();
        if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok())
            && !is_own_origin(&state, headers, origin)
            && !is_mcp_client_origin(origin)
        {
            return AppError::new(403, "Origin not allowed").into_response();
        }
    }
    next.run(req).await
}

/// Six months of HTTPS-only, on responses that reached us over HTTPS (behind a proxy we
/// trust, or at an `https://` `SITE_URL`).
pub async fn hsts(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let secure = crate::auth::is_https(req.headers()) && state.config.trust_proxy_headers
        || state
            .config
            .site_url
            .as_deref()
            .is_some_and(|u| u.starts_with("https://"))
            && crate::auth::is_https(req.headers());
    let mut res = next.run(req).await;
    if secure {
        res.headers_mut().insert(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=15552000"),
        );
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_ignore_case_and_default_ports() {
        assert_eq!(
            host_of("https://Crumb.Example").as_deref(),
            Some("crumb.example")
        );
        assert_eq!(
            host_of("https://crumb.example:443").as_deref(),
            Some("crumb.example")
        );
        assert_eq!(
            host_of("http://localhost:3000").as_deref(),
            Some("localhost:3000")
        );
        assert_eq!(host_of("null"), None);
    }

    #[test]
    fn mcp_browser_origins() {
        assert!(is_mcp_client_origin("https://claude.ai"));
        assert!(is_mcp_client_origin("https://www.claude.com"));
        assert!(is_mcp_client_origin("http://localhost:6274"));
        assert!(!is_mcp_client_origin("https://claude.ai.evil.example"));
        assert!(!is_mcp_client_origin("https://evilclaude.ai"));
        assert!(!is_mcp_client_origin("null"));
    }
}
