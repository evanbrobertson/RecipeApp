//! Relays: `crumb-relay` running on other networks (a Raspberry Pi at home, a droplet), asked
//! to fetch a page the server's own IP was refused. Some sites' bot protection turns away a
//! datacenter address whatever the browser fingerprint; a home connection isn't turned away.
//!
//! The scraper only gets here when Firefox and Safari were both blocked (see
//! `scraper::scrape_with`), so relays see a small share of imports. `SCRAPE_RELAYS` lists
//! them; the operator reaches them over a private network (Tailscale), which is why this
//! client does no address filtering: the relay's own address is in the private range on
//! purpose. The relay does the filtering on what it is asked to fetch (`crumb_fetch::guard`).
//!
//! Each call tries the relays in turn, starting from a different one each time, with the
//! Firefox profile then Safari's. One that can't be reached is left alone for five minutes.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crumb_fetch::wire::{ErrorReply, FetchReply, FetchRequest};
use crumb_fetch::{Fetched, Profile};

use crate::config::Config;

/// How long a relay that couldn't be reached or answered badly is left alone.
const DOWN_FOR: Duration = Duration::from_secs(5 * 60);
/// The whole relay step, across all relays.
const BUDGET: Duration = Duration::from_secs(30);
/// One relay call. The relay gives a site up to 30 seconds itself, so this is only the step's
/// budget in effect; a call that runs out of it is not the relay's fault.
const CALL_TIMEOUT: Duration = Duration::from_secs(30);

/// Why one call to a relay didn't give a page.
#[derive(Debug, PartialEq, Eq)]
enum CallError {
    /// The relay refused the link itself (400): no other relay will take it.
    Refused,
    /// Not now, but nothing wrong with the relay: rate limited or busy (429), or the call ran
    /// out of time (a slow site).
    Skip,
    /// Unreachable, wrong token or broken: leave it alone for a while.
    Down(String),
}

pub struct Relays {
    /// Base URLs without a trailing slash.
    urls: Vec<String>,
    token: String,
    http: reqwest::Client,
    /// Where the next call starts, so the relays take turns.
    next: AtomicUsize,
    /// When each relay may be tried again.
    down: Mutex<HashMap<String, Instant>>,
    down_for: Duration,
}

impl Relays {
    /// The relays named in `config`. None (and so no step) unless there is a token too.
    pub fn from_config(config: &Config) -> Self {
        let urls = match &config.scrape_relay_token {
            Some(_) => config.scrape_relays.clone(),
            None => {
                if !config.scrape_relays.is_empty() {
                    tracing::warn!(
                        "[relay] SCRAPE_RELAYS is set but SCRAPE_RELAY_TOKEN is not; relays are off"
                    );
                }
                Vec::new()
            }
        };
        Self::new(
            urls,
            config.scrape_relay_token.clone().unwrap_or_default(),
            config.scrape_relay_proxy.as_deref(),
        )
    }

    fn new(urls: Vec<String>, token: String, proxy: Option<&str>) -> Self {
        // Relays are reached directly (they are on a private network of their own), so the
        // environment's proxy settings are ignored. The one exception is an explicit
        // SCRAPE_RELAY_PROXY, for a server that reaches Tailscale through a SOCKS5 proxy.
        let mut builder = reqwest::Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none());
        if let Some(proxy) = proxy {
            match reqwest::Proxy::all(proxy) {
                Ok(proxy) => builder = builder.proxy(proxy),
                Err(e) => tracing::warn!("[relay] SCRAPE_RELAY_PROXY isn't usable: {e}"),
            }
        }
        Self {
            urls,
            token,
            http: builder.build().unwrap_or_default(),
            next: AtomicUsize::new(0),
            down: Mutex::new(HashMap::new()),
            down_for: DOWN_FOR,
        }
    }

    /// Whether any relay is set up.
    pub fn is_empty(&self) -> bool {
        self.urls.is_empty()
    }

    /// Asks the relays for `url` until one gives a page `accept` takes, within the step's time
    /// budget. Returns that page. `None` when none did (or none is set up).
    pub async fn fetch(&self, url: &str, accept: impl Fn(&Fetched) -> bool) -> Option<Fetched> {
        let count = self.urls.len();
        if count == 0 {
            return None;
        }
        let host = crate::telemetry::host_of(url);
        let deadline = Instant::now() + BUDGET;
        let start = self.next.fetch_add(1, Ordering::Relaxed);
        for i in 0..count {
            let relay = &self.urls[(start + i) % count];
            if self.is_down(relay) {
                continue;
            }
            for profile in [Profile::Firefox, Profile::Safari] {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    tracing::info!("[relay] {host}: out of time");
                    return None;
                }
                let call = self.call(relay, profile, url);
                let outcome = tokio::time::timeout(left.min(CALL_TIMEOUT), call)
                    .await
                    .unwrap_or(Err(CallError::Skip));
                match outcome {
                    Ok((name, fetched)) => {
                        if accept(&fetched) {
                            tracing::info!("[relay] {host}: {name} worked ({})", profile.label());
                            return Some(fetched);
                        }
                        // Blocked there too, or no recipe: the other profile may differ
                    }
                    Err(CallError::Refused) => {
                        tracing::info!("[relay] {host}: refused the link");
                        return None;
                    }
                    Err(CallError::Skip) => break,
                    Err(CallError::Down(why)) => {
                        tracing::warn!("[relay] {relay}: {why}; skipping it for a while");
                        self.mark_down(relay);
                        break;
                    }
                }
            }
        }
        tracing::info!("[relay] {host}: no relay got the recipe");
        None
    }

    fn is_down(&self, relay: &str) -> bool {
        let mut down = self.down.lock().unwrap();
        match down.get(relay) {
            Some(until) if *until > Instant::now() => true,
            Some(_) => {
                down.remove(relay);
                false
            }
            None => false,
        }
    }

    fn mark_down(&self, relay: &str) {
        self.down
            .lock()
            .unwrap()
            .insert(relay.to_string(), Instant::now() + self.down_for);
    }

    /// One `POST /fetch`. The relay's name and what the site answered.
    async fn call(
        &self,
        relay: &str,
        profile: Profile,
        url: &str,
    ) -> Result<(String, Fetched), CallError> {
        let res = self
            .http
            .post(format!("{relay}/fetch"))
            .bearer_auth(&self.token)
            .json(&FetchRequest {
                url: url.to_string(),
                profile,
            })
            .send()
            .await
            .map_err(|e| CallError::Down(e.without_url().to_string()))?;
        match res.status().as_u16() {
            200 => {}
            400 => return Err(CallError::Refused),
            429 => return Err(CallError::Skip),
            // The relay is fine; the site didn't answer it
            502 => {
                return Ok((
                    relay.to_string(),
                    Fetched::Unreachable("the relay couldn't reach the site".into()),
                ));
            }
            401 => {
                return Err(CallError::Down(
                    "it didn't accept SCRAPE_RELAY_TOKEN".into(),
                ));
            }
            code => {
                let why = res
                    .json::<ErrorReply>()
                    .await
                    .map(|e| e.error)
                    .unwrap_or_default();
                return Err(CallError::Down(format!("answered {code} {why}")));
            }
        }
        let reply: FetchReply = res
            .json()
            .await
            .map_err(|e| CallError::Down(format!("unreadable reply: {}", e.without_url())))?;
        Ok((
            reply.relay,
            Fetched::Page {
                status: reply.status,
                html: reply.body,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use axum::{Json, Router};
    use std::sync::Arc;

    /// What a stub relay saw, and how it answers.
    struct Stub {
        name: &'static str,
        /// (status, body) per profile name; a missing profile answers 403.
        pages: Vec<(&'static str, u16, &'static str)>,
        /// Answers with this HTTP status instead, when set.
        fail_with: Option<StatusCode>,
        seen: Mutex<Vec<(String, String, String)>>,
    }

    async fn handle(
        State(stub): State<Arc<Stub>>,
        headers: HeaderMap,
        Json(req): Json<FetchRequest>,
    ) -> Result<Json<FetchReply>, (StatusCode, Json<ErrorReply>)> {
        let auth = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let profile = format!("{:?}", req.profile).to_lowercase();
        stub.seen
            .lock()
            .unwrap()
            .push((auth, profile.clone(), req.url));
        if let Some(code) = stub.fail_with {
            return Err((code, Json(ErrorReply { error: "no".into() })));
        }
        let (status, body) = stub
            .pages
            .iter()
            .find(|(p, _, _)| *p == profile)
            .map(|(_, s, b)| (*s, *b))
            .unwrap_or((403, ""));
        Ok(Json(FetchReply {
            status,
            body: body.into(),
            relay: stub.name.into(),
        }))
    }

    async fn serve(stub: Stub) -> (String, Arc<Stub>) {
        let stub = Arc::new(stub);
        let app = Router::new()
            .route("/fetch", post(handle))
            .with_state(stub.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (url, stub)
    }

    fn stub(name: &'static str, pages: Vec<(&'static str, u16, &'static str)>) -> Stub {
        Stub {
            name,
            pages,
            fail_with: None,
            seen: Mutex::new(Vec::new()),
        }
    }

    fn html_of(fetched: &Fetched) -> &str {
        match fetched {
            Fetched::Page { html, .. } => html,
            Fetched::Unreachable(_) => "",
        }
    }

    fn accepts_ok(fetched: &Fetched) -> bool {
        matches!(fetched, Fetched::Page { status: 200, html } if !html.is_empty())
    }

    fn relays(urls: &[&str]) -> Relays {
        Relays::new(
            urls.iter().map(|u| u.to_string()).collect(),
            "sekret".into(),
            None,
        )
    }

    #[tokio::test]
    async fn first_relay_that_gives_a_page_wins_and_gets_the_token() {
        let (url, seen) = serve(stub("pi1", vec![("firefox", 200, "<p>recipe</p>")])).await;
        let relays = relays(&[&url]);
        let got = relays
            .fetch("https://food.test/r?x=1", accepts_ok)
            .await
            .unwrap();
        assert_eq!(html_of(&got), "<p>recipe</p>");
        let seen = seen.seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, "Bearer sekret");
        assert_eq!(seen[0].1, "firefox");
        assert_eq!(seen[0].2, "https://food.test/r?x=1");
    }

    #[tokio::test]
    async fn safari_is_tried_when_firefox_gets_nothing() {
        let (url, seen) = serve(stub("pi1", vec![("safari", 200, "<p>recipe</p>")])).await;
        let relays = relays(&[&url]);
        let got = relays
            .fetch("https://food.test/r", accepts_ok)
            .await
            .unwrap();
        assert_eq!(html_of(&got), "<p>recipe</p>");
        let profiles: Vec<String> = seen
            .seen
            .lock()
            .unwrap()
            .iter()
            .map(|s| s.1.clone())
            .collect();
        assert_eq!(profiles, ["firefox", "safari"]);
    }

    #[tokio::test]
    async fn next_relay_when_one_is_blocked_too() {
        let (blocked, blocked_seen) = serve(stub("pi1", vec![])).await;
        let (works, _) = serve(stub("pi2", vec![("firefox", 200, "<p>ok</p>")])).await;
        let relays = relays(&[&blocked, &works]);
        let got = relays
            .fetch("https://food.test/r", accepts_ok)
            .await
            .unwrap();
        assert_eq!(html_of(&got), "<p>ok</p>");
        assert_eq!(
            blocked_seen.seen.lock().unwrap().len(),
            2,
            "both profiles tried there"
        );
    }

    #[tokio::test]
    async fn none_when_no_relay_gives_a_page() {
        let (url, _) = serve(stub("pi1", vec![])).await;
        assert!(
            relays(&[&url])
                .fetch("https://food.test/r", accepts_ok)
                .await
                .is_none()
        );
        assert!(
            relays(&[])
                .fetch("https://food.test/r", accepts_ok)
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn calls_take_turns_starting_from_different_relays() {
        let (a, a_seen) = serve(stub("a", vec![("firefox", 200, "<p>a</p>")])).await;
        let (b, b_seen) = serve(stub("b", vec![("firefox", 200, "<p>b</p>")])).await;
        let relays = relays(&[&a, &b]);
        for _ in 0..4 {
            relays
                .fetch("https://food.test/r", accepts_ok)
                .await
                .unwrap();
        }
        assert_eq!(a_seen.seen.lock().unwrap().len(), 2);
        assert_eq!(b_seen.seen.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn an_unreachable_relay_is_skipped_for_a_while() {
        // Nothing listens here
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let dead = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let (works, seen) = serve(stub("pi2", vec![("firefox", 200, "<p>ok</p>")])).await;
        let relays = relays(&[&dead, &works]);

        // Whichever it starts with, it ends up with the working one and remembers the dead one
        relays
            .fetch("https://food.test/r", accepts_ok)
            .await
            .unwrap();
        assert!(relays.is_down(&dead));
        assert!(!relays.is_down(&works));

        // Later calls don't spend time on it
        relays
            .fetch("https://food.test/r", accepts_ok)
            .await
            .unwrap();
        relays
            .fetch("https://food.test/r", accepts_ok)
            .await
            .unwrap();
        assert_eq!(seen.seen.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn a_relay_is_tried_again_after_its_time_is_up() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let dead = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let mut relays = relays(&[&dead]);
        relays.down_for = Duration::from_millis(20);
        assert!(
            relays
                .fetch("https://food.test/r", accepts_ok)
                .await
                .is_none()
        );
        assert!(relays.is_down(&dead));
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(!relays.is_down(&dead));
    }

    #[tokio::test]
    async fn a_wrong_token_or_server_error_takes_a_relay_out_but_busy_or_bad_gateway_does_not() {
        for (code, marked) in [
            (StatusCode::UNAUTHORIZED, true),
            (StatusCode::INTERNAL_SERVER_ERROR, true),
            // The site was unreachable from there, or the relay is busy: not the relay's fault
            (StatusCode::BAD_GATEWAY, false),
            (StatusCode::TOO_MANY_REQUESTS, false),
        ] {
            let mut s = stub("pi1", vec![]);
            s.fail_with = Some(code);
            let (url, _) = serve(s).await;
            let relays = relays(&[&url]);
            assert!(
                relays
                    .fetch("https://food.test/r", accepts_ok)
                    .await
                    .is_none()
            );
            assert_eq!(relays.is_down(&url), marked, "{code}");
        }
    }

    #[tokio::test]
    async fn a_refused_link_stops_the_step() {
        let mut refusing = stub("pi1", vec![]);
        refusing.fail_with = Some(StatusCode::BAD_REQUEST);
        let (refusing, _) = serve(refusing).await;
        let (works, works_seen) = serve(stub("pi2", vec![("firefox", 200, "<p>ok</p>")])).await;
        // Whichever is first, the refusal ends it; the working one is only reached when it
        // comes first
        let relays = relays(&[&refusing, &works]);
        let first = relays.fetch("https://food.test/r", accepts_ok).await;
        let second = relays.fetch("https://food.test/r", accepts_ok).await;
        assert_eq!(
            [first.is_some(), second.is_some()]
                .iter()
                .filter(|x| **x)
                .count(),
            1
        );
        assert_eq!(works_seen.seen.lock().unwrap().len(), 1);
        assert!(!relays.is_down(&refusing));
    }

    #[test]
    fn needs_a_token() {
        let config = Config {
            scrape_relays: vec!["http://pi:8787".into()],
            ..Config::default()
        };
        assert!(Relays::from_config(&config).is_empty());
        let config = Config {
            scrape_relay_token: Some("t".into()),
            ..config
        };
        assert!(!Relays::from_config(&config).is_empty());
    }
}
