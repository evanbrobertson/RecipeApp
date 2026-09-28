//! Where the QML engine's network stack gets the session cookie and its photo cache.
//!
//! QML's `Image` elements load through the QML engine's own `QNetworkAccessManager`, which
//! has no Crumb session cookie, so every `/img/...` request would be a 401. `native.cpp`
//! installs a `QQmlNetworkAccessManagerFactory` whose managers add the current session to
//! each request for the configured server's exact origin (scheme, host and port, default
//! ports normalized: [`origin_of`]), and to nothing else. [`sync`] sets that session
//! whenever someone signs in or out; [`origin_allows`] is the same rule, tested here.

use url::{Host, Url};

/// A URL's `(scheme, host, port)` origin, with the default port filled in. None when the
/// URL isn't http(s) (so `qrc:` and relative image URLs never match).
fn origin_parts(input: &str) -> Option<(String, String, u16)> {
    let url = Url::parse(input.trim()).ok()?;
    let scheme = url.scheme().to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let default_port = if scheme == "https" { 443 } else { 80 };
    let host = match url.host()? {
        Host::Domain(name) => name.to_string(),
        Host::Ipv4(ip) => ip.to_string(),
        Host::Ipv6(ip) => format!("[{ip}]"),
    };
    Some((scheme, host, url.port().unwrap_or(default_port)))
}

/// The scheme+host+port of a server URL, e.g. `https://crumb.example.com` or
/// `http://127.0.0.1:3000`. The default port is dropped so the string is stable. A server
/// URL's sub-path is ignored: photos share the server's host and port.
pub fn origin_of(server_url: &str) -> Option<String> {
    let (scheme, host, port) = origin_parts(server_url)?;
    let default_port = if scheme == "https" { 443 } else { 80 };
    Some(if port == default_port {
        format!("{scheme}://{host}")
    } else {
        format!("{scheme}://{host}:{port}")
    })
}

/// Whether a request URL belongs to the configured server origin: the session cookie is
/// sent only to the exact scheme, host and port it was configured for, never to another
/// host and never over http when the server is https. `native.cpp` applies the same rule.
#[cfg(test)]
pub fn origin_allows(origin: &str, request_url: &str) -> bool {
    match (origin_parts(origin), origin_parts(request_url)) {
        (Some(configured), Some(request)) => configured == request,
        _ => false,
    }
}

/// The `Cookie` header for a stored session: `name=value` as kept, or a bare value as the
/// app-password session cookie.
pub fn cookie_header(stored: &str) -> String {
    if stored.contains('=') {
        stored.to_string()
    } else {
        format!("{}={stored}", crumb_client::SESSION_COOKIE)
    }
}

/// Points the photo loader at `server` with `session` (the stored cookie), or signs it out.
pub fn sync(server: &str, session: Option<&str>) {
    use cxx_qt_lib::QString;
    match (origin_of(server), session) {
        (Some(origin), Some(cookie)) if !cookie.is_empty() => crate::native::set_photo_session(
            &QString::from(&origin),
            &QString::from(&cookie_header(cookie)),
        ),
        _ => crate::native::set_photo_session(&QString::default(), &QString::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins_drop_default_ports_and_sub_paths() {
        assert_eq!(
            origin_of("https://crumb.example.com/").as_deref(),
            Some("https://crumb.example.com")
        );
        assert_eq!(
            origin_of("https://crumb.example.com:443/").as_deref(),
            Some("https://crumb.example.com")
        );
        assert_eq!(
            origin_of("http://127.0.0.1:3000/").as_deref(),
            Some("http://127.0.0.1:3000")
        );
        assert_eq!(
            origin_of("https://crumb.example.com/crumb/?x=1").as_deref(),
            Some("https://crumb.example.com")
        );
        assert_eq!(origin_of("qrc:/img/fixture.png"), None);
        assert_eq!(origin_of("not a url"), None);
    }

    #[test]
    fn the_cookie_only_matches_the_configured_origin() {
        let origin = "https://crumb.example.com";
        assert!(origin_allows(
            origin,
            "https://crumb.example.com/img/7/320?v=abc"
        ));
        // The same host and scheme but the explicit default port.
        assert!(origin_allows(
            origin,
            "https://crumb.example.com:443/img/7/320"
        ));
        // Another host, another scheme, or another port must not get the cookie.
        assert!(!origin_allows(origin, "https://evil.example.com/img/7/320"));
        assert!(!origin_allows(origin, "http://crumb.example.com/img/7/320"));
        assert!(!origin_allows(
            origin,
            "https://crumb.example.com:8443/img/7/320"
        ));
        assert!(!origin_allows(origin, "qrc:/img/fixture.png"));
    }

    #[test]
    fn cookies_keep_their_name() {
        assert_eq!(cookie_header("abc"), "crumb_session=abc");
        assert_eq!(
            cookie_header("crumb.session_token=x.y"),
            "crumb.session_token=x.y"
        );
    }

    #[test]
    fn cleartext_servers_do_not_leak_over_https() {
        let origin = "http://192.168.1.5:3000";
        assert!(origin_allows(origin, "http://192.168.1.5:3000/img/1/160"));
        assert!(!origin_allows(origin, "https://192.168.1.5:3000/img/1/160"));
        assert!(!origin_allows(origin, "http://192.168.1.5/img/1/160"));
    }
}
