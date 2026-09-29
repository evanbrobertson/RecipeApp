//! Fetching a page the way a browser does, shared by the server and `crumb-relay` so both send
//! the same fingerprints.
//!
//! `wreq` sends a real browser's TLS and HTTP/2 fingerprint and headers. Many recipe sites
//! (behind Cloudflare, Akamai, PerimeterX and the like) refuse a plain Rust client on its
//! fingerprint alone, whatever its User-Agent says. [`Profile`] picks Firefox or Safari.
//!
//! Two ways to fetch:
//! - [`fetch`] follows redirects and connects anywhere. The server uses it for the links its
//!   own cooks paste.
//! - [`fetch_public`] (see [`guard`]) is for a service that fetches on behalf of someone else
//!   (`crumb-relay`): it refuses anything that isn't a public web address, on every redirect.

use http_body_util::BodyExt;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::Duration;

pub mod guard;
pub mod wire;

pub use guard::{Forbidden, check_url, fetch_public, is_public_ip};

/// One request's time, from connecting to the last byte of the body.
pub const PAGE_TIMEOUT: Duration = Duration::from_secs(15);
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Largest recipe page read; real ones are well under 2 MB.
pub const MAX_PAGE_BYTES: usize = 10 * 1024 * 1024;

/// A browser to look like. On the wire to `crumb-relay` it is `"firefox"` or `"safari"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    Firefox,
    Safari,
}

impl Profile {
    /// The name in log lines.
    pub fn label(self) -> &'static str {
        match self {
            Profile::Firefox => "wreq-firefox",
            Profile::Safari => "wreq-safari",
        }
    }

    fn emulation(self) -> wreq_util::Profile {
        match self {
            Profile::Firefox => wreq_util::Emulation::Firefox151,
            Profile::Safari => wreq_util::Emulation::Safari26_4,
        }
    }
}

/// What one fetch attempt got back.
#[derive(Debug)]
pub enum Fetched {
    /// A response. The body is only read for a 2xx.
    Page { status: u16, html: String },
    /// No response (DNS, connection, TLS, timeout); the reason is for the log.
    Unreachable(String),
}

/// The client builder both kinds of fetch start from: the profile, cookies (challenge
/// cookies set along a redirect chain) and the timeouts.
fn builder(profile: Profile) -> wreq::ClientBuilder {
    wreq::Client::builder()
        .emulation(profile.emulation())
        .cookie_store(true)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(PAGE_TIMEOUT)
        .pool_idle_timeout(Duration::from_secs(60))
        .pool_max_idle_per_host(2)
}

fn built(profile: Profile, builder: wreq::ClientBuilder) -> Option<wreq::Client> {
    builder
        .build()
        .inspect_err(|e| {
            tracing::error!("[fetch] couldn't build the {} client: {e}", profile.label())
        })
        .ok()
}

/// The shared browser-profile client for `profile`. Built on first use; `None` if it can't
/// be built (logged once).
pub fn client(profile: Profile) -> Option<&'static wreq::Client> {
    static FIREFOX: OnceLock<Option<wreq::Client>> = OnceLock::new();
    static SAFARI: OnceLock<Option<wreq::Client>> = OnceLock::new();
    let cell = match profile {
        Profile::Firefox => &FIREFOX,
        Profile::Safari => &SAFARI,
    };
    cell.get_or_init(|| {
        built(
            profile,
            // wreq doesn't follow redirects unless told to
            builder(profile).redirect(wreq::redirect::Policy::limited(10)),
        )
    })
    .as_ref()
}

/// The client behind [`fetch_public`]: no redirects (they're followed by hand, checking each
/// hop), no proxy (a proxy would resolve the name where nothing can check it) and a resolver
/// that only ever returns public addresses.
pub(crate) fn public_client(profile: Profile) -> Option<&'static wreq::Client> {
    static FIREFOX: OnceLock<Option<wreq::Client>> = OnceLock::new();
    static SAFARI: OnceLock<Option<wreq::Client>> = OnceLock::new();
    let cell = match profile {
        Profile::Firefox => &FIREFOX,
        Profile::Safari => &SAFARI,
    };
    cell.get_or_init(|| {
        built(
            profile,
            builder(profile)
                .redirect(wreq::redirect::Policy::none())
                .no_proxy()
                .dns_resolver(guard::PublicResolver),
        )
    })
    .as_ref()
}

/// Why a body read stopped.
#[derive(Debug, PartialEq, Eq)]
pub enum ReadError {
    TooLarge,
    Failed(String),
}

/// Reads a wreq response body, giving up once it passes `cap` bytes.
pub async fn read_capped(mut res: wreq::Response, cap: usize) -> Result<Vec<u8>, ReadError> {
    if res.content_length().is_some_and(|n| n > cap as u64) {
        return Err(ReadError::TooLarge);
    }
    let mut body = Vec::new();
    while let Some(frame) = res.frame().await {
        let frame = frame.map_err(|e| ReadError::Failed(e.to_string()))?;
        if let Ok(chunk) = frame.into_data() {
            if body.len() + chunk.len() > cap {
                return Err(ReadError::TooLarge);
            }
            body.extend_from_slice(&chunk);
        }
    }
    Ok(body)
}

/// A final response as [`Fetched`]: the body is read only for a 2xx.
pub(crate) async fn page_of(res: wreq::Response) -> Fetched {
    let status = res.status().as_u16();
    if !res.status().is_success() {
        return Fetched::Page {
            status,
            html: String::new(),
        };
    }
    match read_capped(res, MAX_PAGE_BYTES).await {
        Ok(body) => Fetched::Page {
            status,
            html: String::from_utf8_lossy(&body).into_owned(),
        },
        Err(ReadError::TooLarge) => Fetched::Unreachable("page too large".into()),
        Err(ReadError::Failed(e)) => Fetched::Unreachable(e),
    }
}

/// Fetches a page with one of the browser profiles. The profile sets every header.
pub async fn fetch(profile: Profile, url: &str) -> Fetched {
    let Some(client) = client(profile) else {
        return Fetched::Unreachable("client unavailable".into());
    };
    match client.get(url).send().await {
        Ok(res) => page_of(res).await,
        Err(e) => Fetched::Unreachable(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Serves `responses` in order, one per connection, and returns the address.
    async fn serve(responses: Vec<String>) -> std::net::SocketAddr {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            for response in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buf = [0u8; 4096];
                let _ = socket.read(&mut buf).await;
                socket.write_all(response.as_bytes()).await.unwrap();
                let _ = socket.shutdown().await;
            }
        });
        addr
    }

    fn http(status: &str, headers: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
            body.len()
        )
    }

    #[tokio::test]
    async fn fetch_reads_a_2xx_body_and_only_the_status_otherwise() {
        let addr = serve(vec![http("200 OK", "", "<p>hi</p>")]).await;
        let Fetched::Page { status, html } =
            fetch(Profile::Firefox, &format!("http://{addr}/")).await
        else {
            panic!("expected a page");
        };
        assert_eq!((status, html.as_str()), (200, "<p>hi</p>"));

        let addr = serve(vec![http("403 Forbidden", "", "blocked")]).await;
        let Fetched::Page { status, html } =
            fetch(Profile::Safari, &format!("http://{addr}/")).await
        else {
            panic!("expected a page");
        };
        assert_eq!((status, html.as_str()), (403, ""));
    }

    #[tokio::test]
    async fn fetch_follows_redirects() {
        let landing = serve(vec![http("200 OK", "", "landed")]).await;
        let addr = serve(vec![http(
            "302 Found",
            &format!("Location: http://{landing}/final\r\n"),
            "",
        )])
        .await;
        let Fetched::Page { html, .. } = fetch(Profile::Firefox, &format!("http://{addr}/")).await
        else {
            panic!("expected a page");
        };
        assert_eq!(html, "landed");
    }

    #[tokio::test]
    async fn unreachable_when_nothing_answers() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        assert!(matches!(
            fetch(Profile::Firefox, &format!("http://{addr}/")).await,
            Fetched::Unreachable(_)
        ));
    }

    #[test]
    fn profile_names_on_the_wire() {
        assert_eq!(
            serde_json::to_string(&Profile::Safari).unwrap(),
            "\"safari\""
        );
        assert_eq!(
            serde_json::from_str::<Profile>("\"firefox\"").unwrap(),
            Profile::Firefox
        );
        assert!(serde_json::from_str::<Profile>("\"chrome\"").is_err());
    }
}
