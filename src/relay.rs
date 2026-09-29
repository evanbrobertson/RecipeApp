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
//!
//! A relay with Chromium or the video tools also works for the server (`crumb_work::wire`): it
//! says so in `GET /health` (asked every [`CAN_FRESH_FOR`]), and [`Relays::render`],
//! [`Relays::video_meta`] and [`Relays::watch`] hand it that work before the server does it
//! itself. Wee Chef's calls, and their keys, stay on the server.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crumb_fetch::wire::{ErrorReply, FetchReply, FetchRequest, Health};
use crumb_fetch::{Fetched, Profile};
use crumb_work::video::{VideoMeta, Watched};
use crumb_work::wire::{
    CAN_RENDER, CAN_VIDEO, RenderReply, RenderRequest, VideoMetaReply, VideoRequest, WatchReply,
    WatchRequest,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::config::Config;

/// How long a relay that couldn't be reached or answered badly is left alone.
const DOWN_FOR: Duration = Duration::from_secs(5 * 60);
/// The whole relay step, across all relays.
const BUDGET: Duration = Duration::from_secs(30);
/// One relay call. The relay gives a site up to 30 seconds itself, so this is only the step's
/// budget in effect; a call that runs out of it is not the relay's fault.
const CALL_TIMEOUT: Duration = Duration::from_secs(30);
/// How long what a relay said it can do is believed.
const CAN_FRESH_FOR: Duration = Duration::from_secs(10 * 60);
/// A page in a relay's Chromium (the relay gives it 45 seconds, plus a wait for its turn).
const RENDER_TIMEOUT: Duration = Duration::from_secs(75);
const VIDEO_META_TIMEOUT: Duration = Duration::from_secs(60);
/// Downloading, transcribing and taking stills on a small machine.
const WATCH_TIMEOUT: Duration = Duration::from_secs(12 * 60);

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
    /// What each relay said it can do (`GET /health`), and when.
    can: tokio::sync::Mutex<HashMap<String, (Instant, Vec<String>)>>,
}

/// Why no relay did a piece of work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotDone {
    /// No relay that can do it was free (or none is set up): the server does it itself.
    NoWorker,
    /// A relay did it and it came to nothing (blocked there too, a video it couldn't get),
    /// so the server doing it from its own address is unlikely to fare better.
    Nothing,
}

/// Why a relay didn't do a piece of work.
#[derive(Debug, PartialEq, Eq)]
enum WorkError {
    /// It did it, and it came to nothing (blocked there too, a video it couldn't get): the
    /// server needn't ask another.
    Nothing(String),
    /// Busy, can't, or out of time: another relay, or the server itself.
    Skip,
    /// Unreachable or broken: leave it alone for a while.
    Down(String),
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
            can: tokio::sync::Mutex::new(HashMap::new()),
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

    /// The relays that can do `what` (`render`, `video`), in turn order, skipping any that's
    /// down. Asks each one's `/health` when what it said is stale.
    async fn workers(&self, what: &str) -> Vec<String> {
        let count = self.urls.len();
        if count == 0 {
            return Vec::new();
        }
        let start = self.next.fetch_add(1, Ordering::Relaxed);
        let mut out = Vec::new();
        for i in 0..count {
            let relay = &self.urls[(start + i) % count];
            if self.is_down(relay) {
                continue;
            }
            if self.can_do(relay, what).await {
                out.push(relay.clone());
            }
        }
        out
    }

    /// Whether any relay says it can do `what` (asking the stale ones).
    pub async fn can(&self, what: &str) -> bool {
        !self.workers(what).await.is_empty()
    }

    async fn can_do(&self, relay: &str, what: &str) -> bool {
        let mut can = self.can.lock().await;
        if let Some((at, said)) = can.get(relay)
            && at.elapsed() < CAN_FRESH_FOR
        {
            return said.iter().any(|c| c == what);
        }
        let asked = self
            .http
            .get(format!("{relay}/health"))
            .timeout(Duration::from_secs(5))
            .send()
            .await;
        let said = match asked {
            Ok(res) if res.status().is_success() => res
                .json::<Health>()
                .await
                .map(|h| h.can)
                .unwrap_or_default(),
            Ok(res) => {
                tracing::warn!("[relay] {relay}: /health answered {}", res.status());
                Vec::new()
            }
            Err(e) => {
                tracing::warn!(
                    "[relay] {relay}: unreachable ({}); skipping it for a while",
                    e.without_url()
                );
                drop(can);
                self.mark_down(relay);
                return false;
            }
        };
        let yes = said.iter().any(|c| c == what);
        can.insert(relay.to_string(), (Instant::now(), said));
        yes
    }

    /// Hands work to the relays that can do `what`, in turn, until one does it.
    async fn work<Q: Serialize, R: DeserializeOwned>(
        &self,
        what: &str,
        path: &str,
        request: &Q,
        timeout: Duration,
        host: &str,
    ) -> Result<R, NotDone> {
        for relay in self.workers(what).await {
            let call = self.work_call::<Q, R>(&relay, path, request, timeout);
            match tokio::time::timeout(timeout, call)
                .await
                .unwrap_or(Err(WorkError::Skip))
            {
                Ok(reply) => return Ok(reply),
                Err(WorkError::Nothing(why)) => {
                    tracing::info!("[relay] {host}: {relay}{path} gave nothing ({why})");
                    return Err(NotDone::Nothing);
                }
                Err(WorkError::Skip) => continue,
                Err(WorkError::Down(why)) => {
                    tracing::warn!("[relay] {relay}: {why}; skipping it for a while");
                    self.mark_down(&relay);
                }
            }
        }
        Err(NotDone::NoWorker)
    }

    async fn work_call<Q: Serialize, R: DeserializeOwned>(
        &self,
        relay: &str,
        path: &str,
        request: &Q,
        timeout: Duration,
    ) -> Result<R, WorkError> {
        let res = self
            .http
            .post(format!("{relay}{path}"))
            .bearer_auth(&self.token)
            .timeout(timeout)
            .json(request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    WorkError::Skip
                } else {
                    WorkError::Down(e.without_url().to_string())
                }
            })?;
        let status = res.status().as_u16();
        if status == 200 {
            return res
                .json::<R>()
                .await
                .map_err(|e| WorkError::Down(format!("unreadable reply: {}", e.without_url())));
        }
        let why = res
            .json::<ErrorReply>()
            .await
            .map(|e| e.error)
            .unwrap_or_default();
        Err(match status {
            400 | 422 => WorkError::Nothing(why),
            // Busy, or it can't after all (what it said is refreshed on the next ask)
            429 | 501 => {
                if status == 501 {
                    self.can.lock().await.remove(relay);
                }
                WorkError::Skip
            }
            401 => WorkError::Down("it didn't accept SCRAPE_RELAY_TOKEN".into()),
            code => WorkError::Down(format!("answered {code} {why}")),
        })
    }

    /// The page at `url` as a relay's Chromium rendered it.
    pub async fn render(&self, url: &str) -> Result<String, NotDone> {
        let host = crate::telemetry::host_of(url);
        let started = Instant::now();
        let request = RenderRequest {
            url: url.to_string(),
        };
        let reply: RenderReply = self
            .work(CAN_RENDER, "/render", &request, RENDER_TIMEOUT, &host)
            .await?;
        tracing::info!(
            "[relay] {host}: {} rendered it in {} ms",
            reply.relay,
            started.elapsed().as_millis()
        );
        Ok(reply.html)
    }

    /// A video's details as a relay's `yt-dlp` read them.
    pub async fn video_meta(&self, url: &str) -> Result<VideoMeta, NotDone> {
        let host = crate::telemetry::host_of(url);
        let request = VideoRequest {
            url: url.to_string(),
        };
        let reply: VideoMetaReply = self
            .work(
                CAN_VIDEO,
                "/video/meta",
                &request,
                VIDEO_META_TIMEOUT,
                &host,
            )
            .await?;
        tracing::info!("[relay] {host}: {} read the video's details", reply.relay);
        Ok(reply.meta)
    }

    /// A video watched on a relay: what's said and its stills.
    pub async fn watch(&self, meta: &VideoMeta) -> Result<Watched, NotDone> {
        let host = crate::telemetry::host_of(&meta.url);
        let started = Instant::now();
        let request = WatchRequest { meta: meta.clone() };
        let reply: WatchReply = self
            .work(CAN_VIDEO, "/video/watch", &request, WATCH_TIMEOUT, &host)
            .await?;
        tracing::info!(
            "[relay] {host}: {} watched it in {} ms",
            reply.relay,
            started.elapsed().as_millis()
        );
        Ok(reply.watched())
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
                link: None,
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
        matches!(fetched, Fetched::Page { status: 200, html, .. } if !html.is_empty())
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

    /// A relay that works for the server: says what it can do, renders, and reads videos.
    async fn serve_worker(can: Vec<&'static str>, render_status: StatusCode) -> String {
        let can: Vec<String> = can.into_iter().map(String::from).collect();
        let app = Router::new()
            .route(
                "/health",
                axum::routing::get(move || {
                    let can = can.clone();
                    async move {
                        Json(Health {
                            name: "pi1".into(),
                            version: "0.2.0".into(),
                            can,
                        })
                    }
                }),
            )
            .route(
                "/render",
                post(move |Json(req): Json<RenderRequest>| async move {
                    if render_status != StatusCode::OK {
                        return Err((
                            render_status,
                            Json(ErrorReply {
                                error: "blocked".into(),
                            }),
                        ));
                    }
                    Ok(Json(RenderReply {
                        html: format!("<p>{}</p>", req.url),
                        relay: "pi1".into(),
                    }))
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        url
    }

    #[tokio::test]
    async fn chromium_work_goes_to_a_relay_that_has_it() {
        let fetch_only = serve_worker(vec![], StatusCode::OK).await;
        let worker = serve_worker(vec!["render"], StatusCode::OK).await;
        let both = relays(&[&fetch_only, &worker]);
        assert!(both.can(CAN_RENDER).await);
        assert!(!both.can(CAN_VIDEO).await);
        for _ in 0..2 {
            let html = both.render("https://food.test/r").await.unwrap();
            assert_eq!(html, "<p>https://food.test/r</p>");
        }
        // Blocked in the relay's Chromium: the server is told not to bother itself
        let blocked = relays_blocked().await;
        assert_eq!(
            blocked.render("https://food.test/r").await,
            Err(NotDone::Nothing)
        );
        // No relay with Chromium: the server does it
        let none = relays(&[&fetch_only]);
        assert_eq!(
            none.render("https://food.test/r").await,
            Err(NotDone::NoWorker)
        );
    }

    async fn relays_blocked() -> Relays {
        let url = serve_worker(vec!["render"], StatusCode::UNPROCESSABLE_ENTITY).await;
        relays(&[&url])
    }
}
