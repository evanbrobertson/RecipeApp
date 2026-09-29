//! Links into the native apps: signing in with Google or Apple through the browser, and
//! invite links.
//!
//! An app can't finish a Google or Apple sign-in itself, since the provider sends the person
//! back to the server's web address. So the app opens the server's `/app/sign-in` page in the
//! browser, the page signs in as the web does, and the server sends the browser on to
//! `app.crumb://signed-in?code=…`. The code is only good with the verifier the app kept
//! (PKCE, RFC 7636, as RFC 8252 asks of native apps), so another app that catches the link
//! can't use it.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use sha2::{Digest, Sha256};

/// The apps' URL scheme (reverse domain, as RFC 8252 asks).
pub const APP_SCHEME: &str = "app.crumb";

/// A new sign-in's secret: the app keeps `verifier` and sends the server `challenge`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSignIn {
    pub verifier: String,
    pub challenge: String,
}

impl AppSignIn {
    pub fn new() -> Self {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let verifier = URL_SAFE_NO_PAD.encode(bytes);
        let challenge = pkce_challenge(&verifier);
        Self {
            verifier,
            challenge,
        }
    }
}

impl Default for AppSignIn {
    fn default() -> Self {
        Self::new()
    }
}

/// PKCE's S256 challenge for a verifier.
pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Where the server sends the browser when an app's sign-in is done.
pub fn signed_in_link(code: &str) -> String {
    format!("{APP_SCHEME}://signed-in?code={code}")
}

/// The code from an `app.crumb://signed-in?code=…` link.
pub fn signed_in_code(link: &str) -> Option<String> {
    let url = url::Url::parse(link.trim()).ok()?;
    if url.scheme() != APP_SCHEME || url.host_str() != Some("signed-in") {
        return None;
    }
    url.query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .filter(|c| is_token(c))
}

/// An invite: the server it's for and its token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InviteLink {
    /// The server's base URL, ending in "/".
    pub server: String,
    pub token: String,
}

/// An `app.crumb://invite?server=…&token=…` link to hand an invite to the app.
pub fn invite_app_link(server: &str, token: &str) -> String {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("server", server)
        .append_pair("token", token)
        .finish();
    format!("{APP_SCHEME}://invite?{query}")
}

/// An invite someone pasted or opened: the web's `https://crumb.example.com/invite#token`,
/// or the app link the invite page offers. Plain HTTP only for a server on this machine or
/// the local network.
pub fn invite_link(text: &str) -> Option<InviteLink> {
    let url = url::Url::parse(text.trim()).ok()?;
    let (server, token) = if url.scheme() == APP_SCHEME {
        if url.host_str() != Some("invite") {
            return None;
        }
        let get = |key: &str| {
            url.query_pairs()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.into_owned())
        };
        let server = url::Url::parse(&get("server")?).ok()?;
        (server, get("token")?)
    } else {
        let path = url.path().trim_end_matches('/');
        let base = path.strip_suffix("/invite")?;
        let mut server = url.clone();
        server.set_path(base);
        (server, url.fragment()?.to_string())
    };
    if !matches!(server.scheme(), "http" | "https") || server.host_str().is_none() {
        return None;
    }
    let server = crate::client::server_url(server.as_str())?;
    if !crate::client::allows_cleartext(&server) || !is_token(&token) {
        return None;
    }
    Some(InviteLink { server, token })
}

/// Tokens and codes are URL-safe and short: anything else isn't one of Crumb's.
fn is_token(s: &str) -> bool {
    (1..=256).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_is_rfc7636s_example() {
        // RFC 7636, appendix B
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let made = AppSignIn::new();
        assert_eq!(made.verifier.len(), 43);
        assert_eq!(made.challenge, pkce_challenge(&made.verifier));
        assert_ne!(made.verifier, AppSignIn::new().verifier);
    }

    #[test]
    fn signed_in_links_round_trip() {
        let link = signed_in_link("abc_DEF-123");
        assert_eq!(link, "app.crumb://signed-in?code=abc_DEF-123");
        assert_eq!(signed_in_code(&link).as_deref(), Some("abc_DEF-123"));
        assert_eq!(signed_in_code("app.crumb://invite?code=abc"), None);
        assert_eq!(signed_in_code("https://signed-in/?code=abc"), None);
        assert_eq!(signed_in_code("app.crumb://signed-in?code=a%20b"), None);
        assert_eq!(signed_in_code("app.crumb://signed-in"), None);
    }

    #[test]
    fn invite_links_from_the_web_and_the_app() {
        let web = invite_link("  https://crumb.example.com/invite#tok_123  ").unwrap();
        assert_eq!(web.server, "https://crumb.example.com/");
        assert_eq!(web.token, "tok_123");
        let sub = invite_link("https://example.com/crumb/invite/#t").unwrap();
        assert_eq!(sub.server, "https://example.com/crumb/");

        let app = invite_app_link("https://crumb.example.com/", "tok_123");
        assert_eq!(invite_link(&app), Some(web));

        let local = invite_link("http://192.168.1.5:3000/invite#t").unwrap();
        assert_eq!(local.server, "http://192.168.1.5:3000/");
    }

    #[test]
    fn not_invite_links() {
        for text in [
            "",
            "hello",
            "https://crumb.example.com/invite",
            "https://crumb.example.com/invite#",
            "https://crumb.example.com/recipes/1#tok",
            "https://crumb.example.com/invite#a/b",
            "http://crumb.example.com/invite#tok",
            "ftp://crumb.example.com/invite#tok",
            "app.crumb://signed-in?server=https://x.com&token=t",
            "app.crumb://invite?server=javascript:alert(1)&token=t",
            "app.crumb://invite?server=https%3A%2F%2Fx.com",
        ] {
            assert_eq!(invite_link(text), None, "{text}");
        }
    }
}
