//! `crumb-relay`: a small service that fetches a recipe page for a Crumb server from a different
//! network, with the same browser fingerprints the server uses. Some sites' bot protection
//! refuses a datacenter address whatever the fingerprint; a Raspberry Pi on a home connection,
//! reached over Tailscale, isn't refused.
//!
//! The server asks only after its own fetches were blocked. The wire format is in
//! [`crumb_fetch::wire`]; running one is in docs/RELAY.md.
//!
//! What it will not do: fetch anything but a public web address (see [`crumb_fetch::guard`]:
//! the relay sits next to a router and a tailnet), answer without the token, or fetch more than
//! its [`limits`] allow. What it keeps: nothing. It logs the site's host name and how it went,
//! never a full address, query string or page.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use crumb_fetch::wire::{ErrorReply, FetchReply, FetchRequest, Health};
use crumb_fetch::{Fetched, Forbidden, Profile};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use url::Url;

pub mod config;
pub mod limits;

pub use config::Config;
use limits::Limits;

/// Requests are a URL and a profile name.
const MAX_REQUEST_BYTES: usize = 16 * 1024;

/// How pages are fetched: the real one is [`crumb_fetch::fetch_public`]; tests bring their own.
pub type Fetcher = Arc<
    dyn Fn(Profile, String) -> Pin<Box<dyn Future<Output = Result<Fetched, Forbidden>> + Send>>
        + Send
        + Sync,
>;

pub struct Relay {
    name: String,
    /// SHA-256 of the token, so comparing is constant-time whatever the guess's length.
    token_digest: [u8; 32],
    limits: Limits,
    fetcher: Fetcher,
}

impl Relay {
    pub fn new(config: &Config) -> Arc<Self> {
        Self::with_fetcher(
            config,
            Arc::new(|profile, url| {
                Box::pin(async move { crumb_fetch::fetch_public(profile, &url).await })
            }),
        )
    }

    pub fn with_fetcher(config: &Config, fetcher: Fetcher) -> Arc<Self> {
        Arc::new(Self {
            name: config.name.clone(),
            token_digest: Sha256::digest(config.token.as_bytes()).into(),
            limits: Limits::new(config.host_interval, config.concurrency, config.per_minute),
            fetcher,
        })
    }

    fn authorised(&self, headers: &HeaderMap) -> bool {
        let given = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| {
                let (scheme, token) = v.split_once(' ')?;
                scheme
                    .eq_ignore_ascii_case("bearer")
                    .then_some(token.trim())
            })
            .unwrap_or("");
        let digest: [u8; 32] = Sha256::digest(given.as_bytes()).into();
        // The comparison runs whether or not a token was sent
        bool::from(digest.ct_eq(&self.token_digest)) && !given.is_empty()
    }
}

pub fn router(relay: Arc<Relay>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/fetch", post(fetch))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_REQUEST_BYTES))
        .with_state(relay)
}

fn error(status: StatusCode, message: impl Into<String>) -> Response {
    (
        status,
        Json(ErrorReply {
            error: message.into(),
        }),
    )
        .into_response()
}

async fn health(State(relay): State<Arc<Relay>>) -> Json<Health> {
    Json(Health {
        name: relay.name.clone(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}

async fn fetch(State(relay): State<Arc<Relay>>, headers: HeaderMap, body: Bytes) -> Response {
    if !relay.authorised(&headers) {
        return error(StatusCode::UNAUTHORIZED, "Missing or wrong token.");
    }
    let Ok(request) = serde_json::from_slice::<FetchRequest>(&body) else {
        return error(
            StatusCode::BAD_REQUEST,
            r#"Expected {"url": "...", "profile": "firefox" | "safari"}."#,
        );
    };
    let url = match Url::parse(&request.url) {
        Ok(url) => url,
        Err(_) => return error(StatusCode::BAD_REQUEST, "That isn't a valid link."),
    };
    // What needs no lookup is refused before it costs a rate-limit slot
    if let Err(Forbidden(why)) = crumb_fetch::check_url(&url) {
        return error(StatusCode::BAD_REQUEST, why);
    }
    let host = url
        .host_str()
        .unwrap_or_default()
        .trim_end_matches('.')
        .to_ascii_lowercase();

    let _running = match relay.limits.admit(&host, Instant::now()) {
        Ok(permit) => permit,
        Err(denied) => {
            tracing::info!("{host}: turned away ({denied:?})");
            let mut res = error(StatusCode::TOO_MANY_REQUESTS, denied.message());
            if let Ok(secs) = HeaderValue::from_str(&denied.retry_after().as_secs().to_string()) {
                res.headers_mut().insert(header::RETRY_AFTER, secs);
            }
            return res;
        }
    };

    let started = Instant::now();
    let fetched = (relay.fetcher)(request.profile, request.url).await;
    let ms = started.elapsed().as_millis();
    match fetched {
        Ok(Fetched::Page { status, html }) => {
            tracing::info!("{host}: {} {status} in {ms} ms", request.profile.label());
            Json(FetchReply {
                status,
                body: html,
                relay: relay.name.clone(),
            })
            .into_response()
        }
        Ok(Fetched::Unreachable(why)) => {
            tracing::info!("{host}: {} unreachable in {ms} ms", request.profile.label());
            error(
                StatusCode::BAD_GATEWAY,
                format!("Couldn't reach the site: {why}"),
            )
        }
        Err(Forbidden(why)) => {
            tracing::info!("{host}: refused ({why})");
            error(StatusCode::BAD_REQUEST, why)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use std::sync::Mutex;
    use std::time::Duration;
    use tokio::sync::Notify;
    use tower::ServiceExt;

    /// The fetches a test relay was asked for.
    type Calls = Arc<Mutex<Vec<(Profile, String)>>>;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    fn config() -> Config {
        Config::from_lookup(|key| match key {
            "RELAY_TOKEN" => Some(TOKEN.into()),
            "RELAY_NAME" => Some("pi-test".into()),
            _ => None,
        })
        .unwrap()
    }

    /// A relay whose fetches are recorded and answered by `answer`, never over the network.
    fn relay_with(
        config: &Config,
        answer: impl Fn(Profile, &str) -> Result<Fetched, Forbidden> + Send + Sync + 'static,
    ) -> (Router, Calls) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let seen = calls.clone();
        let relay = Relay::with_fetcher(
            config,
            Arc::new(move |profile, url| {
                seen.lock().unwrap().push((profile, url.clone()));
                let result = answer(profile, &url);
                Box::pin(async move { result })
            }),
        );
        (router(relay), calls)
    }

    fn page(status: u16, html: &str) -> Result<Fetched, Forbidden> {
        Ok(Fetched::Page {
            status,
            html: html.into(),
        })
    }

    async fn post_fetch(
        app: &Router,
        token: Option<&str>,
        body: &str,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut req = Request::builder()
            .method("POST")
            .uri("/fetch")
            .header("content-type", "application/json");
        if let Some(token) = token {
            req = req.header("authorization", format!("Bearer {token}"));
        }
        let res = app
            .clone()
            .oneshot(req.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let (status, headers) = (res.status(), res.headers().clone());
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            headers,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    fn request(url: &str) -> String {
        json!({"url": url, "profile": "firefox"}).to_string()
    }

    #[tokio::test]
    async fn health_needs_no_token_and_says_little() {
        let (app, _) = relay_with(&config(), |_, _| page(200, ""));
        let res = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let health: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(health["name"], "pi-test");
        assert_eq!(health["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(health.as_object().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn fetch_returns_the_page() {
        let (app, calls) = relay_with(&config(), |_, _| page(200, "<p>recipe</p>"));
        let body = json!({"url": "https://food.test/r?x=1", "profile": "safari"}).to_string();
        let (status, _, json) = post_fetch(&app, Some(TOKEN), &body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json,
            json!({"status": 200, "body": "<p>recipe</p>", "relay": "pi-test"})
        );
        assert_eq!(
            *calls.lock().unwrap(),
            [(Profile::Safari, "https://food.test/r?x=1".to_string())]
        );
    }

    #[tokio::test]
    async fn a_blocked_site_is_a_200_with_its_status_and_no_body() {
        let (app, _) = relay_with(&config(), |_, _| page(403, ""));
        let (status, _, json) =
            post_fetch(&app, Some(TOKEN), &request("https://food.test/r")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["status"], 403);
        assert_eq!(json["body"], "");
    }

    #[tokio::test]
    async fn rejects_a_missing_or_wrong_token() {
        let (app, calls) = relay_with(&config(), |_, _| page(200, "x"));
        let body = request("https://food.test/r");
        for token in [
            None,
            Some(""),
            Some("wrong"),
            Some(&TOKEN[..TOKEN.len() - 1]),
            Some(&format!("{TOKEN}x")),
            Some(&TOKEN.to_uppercase()),
        ] {
            let (status, _, json) = post_fetch(&app, token, &body).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{token:?}");
            assert!(json["error"].is_string());
        }
        // Other schemes and the bare token don't count either
        for value in [
            TOKEN.to_string(),
            format!("Basic {TOKEN}"),
            "Bearer".to_string(),
        ] {
            let res = app
                .clone()
                .oneshot(
                    Request::post("/fetch")
                        .header("authorization", value)
                        .body(Body::from(body.clone()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
        }
        assert!(calls.lock().unwrap().is_empty());
        // The right token, in any case of the scheme, works
        let res = app
            .clone()
            .oneshot(
                Request::post("/fetch")
                    .header("authorization", format!("bearer {TOKEN}"))
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn the_token_is_checked_before_the_request_is_read() {
        let (app, _) = relay_with(&config(), |_, _| page(200, "x"));
        let (status, _, _) = post_fetch(&app, None, "not json").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, _, _) = post_fetch(&app, Some(TOKEN), "not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn refuses_links_into_the_network_without_fetching() {
        let (app, calls) = relay_with(&config(), |_, _| page(200, "secret"));
        for url in [
            "http://127.0.0.1/",
            "http://127.0.0.1:8787/health",
            "http://169.254.169.254/latest/meta-data/",
            "http://100.100.100.100/",
            "http://100.64.0.7/",
            "http://[::1]/",
            "http://[fd7a:115c:a1e0::1]/",
            "http://192.168.1.1/",
            "http://10.0.0.1/",
            "http://localhost/",
            "http://0x7f.1/",
            "ftp://food.test/",
            "file:///etc/passwd",
            "http://food.test:22/",
            "http://user:pw@food.test/",
            "not a url",
            "",
        ] {
            let (status, _, json) = post_fetch(&app, Some(TOKEN), &request(url)).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{url}");
            assert!(json["error"].is_string(), "{url}");
        }
        assert!(calls.lock().unwrap().is_empty(), "nothing was fetched");
    }

    #[tokio::test]
    async fn bad_requests_are_400() {
        let (app, _) = relay_with(&config(), |_, _| page(200, "x"));
        for body in [
            "",
            "{}",
            r#"{"url": "https://food.test/"}"#,
            r#"{"url": "https://food.test/", "profile": "chrome"}"#,
            r#"{"url": 5, "profile": "firefox"}"#,
            "[]",
        ] {
            let (status, _, _) = post_fetch(&app, Some(TOKEN), body).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        }
        let (status, _, _) =
            post_fetch(&app, Some(TOKEN), &"x".repeat(MAX_REQUEST_BYTES + 1)).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn the_fetchers_refusals_and_failures_map_to_400_and_502() {
        let (app, _) = relay_with(&config(), |_, url| {
            if url.contains("rebinds") {
                Err(Forbidden("That host isn't public.".into()))
            } else {
                Ok(Fetched::Unreachable("connection refused".into()))
            }
        });
        let (status, _, _) = post_fetch(&app, Some(TOKEN), &request("https://rebinds.test/")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _, json) = post_fetch(&app, Some(TOKEN), &request("https://down.test/")).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert!(
            json["error"]
                .as_str()
                .unwrap()
                .contains("connection refused")
        );
    }

    #[tokio::test]
    async fn the_same_site_twice_in_a_row_is_429() {
        let (app, calls) = relay_with(&config(), |_, _| page(200, "x"));
        let (status, _, _) = post_fetch(&app, Some(TOKEN), &request("https://food.test/a")).await;
        assert_eq!(status, StatusCode::OK);
        let (status, headers, json) =
            post_fetch(&app, Some(TOKEN), &request("https://FOOD.test/b")).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert!(json["error"].is_string());
        let wait: u64 = headers["retry-after"].to_str().unwrap().parse().unwrap();
        assert!((1..=5).contains(&wait));
        // Another site is fine
        let (status, _, _) = post_fetch(&app, Some(TOKEN), &request("https://other.test/a")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(calls.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn the_per_minute_cap_is_429() {
        let mut config = config();
        config.per_minute = 2;
        config.host_interval = Duration::ZERO;
        let (app, _) = relay_with(&config, |_, _| page(200, "x"));
        for site in ["a", "b"] {
            let (status, _, _) = post_fetch(
                &app,
                Some(TOKEN),
                &request(&format!("https://{site}.test/")),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
        }
        let (status, headers, _) = post_fetch(&app, Some(TOKEN), &request("https://c.test/")).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert!(headers.contains_key("retry-after"));
    }

    #[tokio::test]
    async fn too_many_at_once_is_429() {
        let mut config = config();
        config.concurrency = 1;
        let gate = Arc::new(Notify::new());
        let entered = Arc::new(Notify::new());
        let (g, e) = (gate.clone(), entered.clone());
        let relay = Relay::with_fetcher(
            &config,
            Arc::new(move |_, _| {
                let (g, e) = (g.clone(), e.clone());
                Box::pin(async move {
                    e.notify_one();
                    g.notified().await;
                    page(200, "x")
                })
            }),
        );
        let app = router(relay);

        let slow = tokio::spawn({
            let app = app.clone();
            async move {
                post_fetch(&app, Some(TOKEN), &request("https://slow.test/"))
                    .await
                    .0
            }
        });
        entered.notified().await;
        let (status, _, _) = post_fetch(&app, Some(TOKEN), &request("https://other.test/")).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

        gate.notify_one();
        assert_eq!(slow.await.unwrap(), StatusCode::OK);
    }
}
