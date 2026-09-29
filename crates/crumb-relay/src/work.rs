//! A relay that works for the server: it loads a page in its own headless Chromium
//! (`POST /render`) and reads cooking videos (`POST /video/meta`, `POST /video/watch`), so that
//! heavy work runs on a machine at home instead of the server, from a home connection that
//! sites and YouTube don't turn away. The wire format is `crumb_work::wire`.
//!
//! What it can do is found at start (Chromium, `yt-dlp` and `ffmpeg`, optionally
//! `whisper-cli` and its model) and said in `GET /health`. [`HEAVY`] jobs run at once
//! (`RELAY_HEAVY_WORKERS`, default 1); more are answered 429 and the server does the work
//! itself. Chromium's every request goes through `crumb_fetch::proxy`, which refuses private
//! addresses, as on the server; a video link must be one of the video sites Crumb reads.

use std::sync::Arc;
use std::time::Instant;

use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use crumb_work::browser::Browser;
use crumb_work::video::{Threads, VideoTools};
use crumb_work::wire::{
    CAN_RENDER, CAN_VIDEO, RenderReply, RenderRequest, VideoMetaReply, VideoRequest, WatchReply,
    WatchRequest,
};
use tokio::sync::Semaphore;
use url::Url;

use crate::{Relay, error};

/// Longest a request to these routes may be: a video's details carry its caption.
pub const MAX_WORK_REQUEST_BYTES: usize = 64 * 1024;

/// The tools found, and how many heavy jobs may run at once.
pub struct Work {
    pub browser: Option<Browser>,
    pub video: VideoTools,
    pub heavy: Arc<Semaphore>,
    pub workers: usize,
}

impl Work {
    /// What this machine has (`RELAY_WORK=off`: nothing, a plain fetching relay).
    pub fn from_env(workers: usize, enabled: bool) -> Self {
        let heavy = Arc::new(Semaphore::new(workers));
        if !enabled {
            return Self {
                browser: None,
                video: VideoTools::default(),
                heavy,
                workers,
            };
        }
        let mut browser = Browser::from_env();
        browser.share_budget(heavy.clone());
        Self {
            browser: browser.available().then_some(browser),
            video: VideoTools::from_env(),
            heavy,
            workers,
        }
    }

    pub fn none() -> Self {
        Self::from_env(1, false)
    }

    fn can_video(&self) -> bool {
        self.video.yt_dlp.is_some() && self.video.ffmpeg.is_some()
    }

    /// `GET /health`'s `can`.
    pub fn can(&self) -> Vec<String> {
        let mut can = Vec::new();
        if self.browser.is_some() {
            can.push(CAN_RENDER.to_string());
        }
        if self.can_video() {
            can.push(CAN_VIDEO.to_string());
        }
        can
    }
}

fn unable(what: &str) -> Response {
    error(
        StatusCode::NOT_IMPLEMENTED,
        format!("This relay can't {what}."),
    )
}

fn busy() -> Response {
    error(StatusCode::TOO_MANY_REQUESTS, "The relay is busy.")
}

/// The link, refused unless it's a public web address.
fn public_link(raw: &str) -> Result<(Url, String), Box<Response>> {
    let url = Url::parse(raw)
        .map_err(|_| Box::new(error(StatusCode::BAD_REQUEST, "That isn't a valid link.")))?;
    if let Err(crumb_fetch::Forbidden(why)) = crumb_fetch::check_url(&url) {
        return Err(Box::new(error(StatusCode::BAD_REQUEST, why)));
    }
    let host = url
        .host_str()
        .unwrap_or_default()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    Ok((url, host))
}

/// A video link Crumb reads (TikTok, Instagram, YouTube), else refused: yt-dlp would fetch
/// anything it's given.
fn video_link(raw: &str) -> Result<String, Box<Response>> {
    let (_, host) = public_link(raw)?;
    if !crumb_core::source::is_video_url(raw) {
        return Err(Box::new(error(
            StatusCode::BAD_REQUEST,
            "Only TikTok, Instagram and YouTube links are read as videos.",
        )));
    }
    Ok(host)
}

pub async fn render(State(relay): State<Arc<Relay>>, headers: HeaderMap, body: Bytes) -> Response {
    if !relay.authorised(&headers) {
        return error(StatusCode::UNAUTHORIZED, "Missing or wrong token.");
    }
    let Some(browser) = &relay.work.browser else {
        return unable("load pages in Chromium");
    };
    let Ok(request) = serde_json::from_slice::<RenderRequest>(&body) else {
        return error(StatusCode::BAD_REQUEST, r#"Expected {"url": "..."}."#);
    };
    let host = match public_link(&request.url) {
        Ok((_, host)) => host,
        Err(res) => return *res,
    };
    // Counts against the same limits as a fetch: it's a visit to the site all the same
    let _running = match relay.limits.admit(&host, Instant::now()) {
        Ok(permit) => permit,
        Err(denied) => {
            tracing::info!("{host}: render turned away ({denied:?})");
            return error(StatusCode::TOO_MANY_REQUESTS, denied.message());
        }
    };
    if relay.work.heavy.available_permits() == 0 {
        return busy();
    }
    let started = Instant::now();
    let result = browser.fetch(&request.url).await;
    let ms = started.elapsed().as_millis();
    match result {
        Ok(html) => {
            tracing::info!("{host}: rendered in {ms} ms");
            Json(RenderReply {
                html,
                relay: relay.name.clone(),
            })
            .into_response()
        }
        Err(why) => {
            tracing::info!("{host}: render failed in {ms} ms ({why})");
            error(StatusCode::UNPROCESSABLE_ENTITY, why)
        }
    }
}

pub async fn video_meta(
    State(relay): State<Arc<Relay>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !relay.authorised(&headers) {
        return error(StatusCode::UNAUTHORIZED, "Missing or wrong token.");
    }
    let (Some(yt_dlp), true) = (&relay.work.video.yt_dlp, relay.work.can_video()) else {
        return unable("read videos");
    };
    let Ok(request) = serde_json::from_slice::<VideoRequest>(&body) else {
        return error(StatusCode::BAD_REQUEST, r#"Expected {"url": "..."}."#);
    };
    let host = match video_link(&request.url) {
        Ok(host) => host,
        Err(res) => return *res,
    };
    let _running = match relay.limits.admit(&host, Instant::now()) {
        Ok(permit) => permit,
        Err(denied) => return error(StatusCode::TOO_MANY_REQUESTS, denied.message()),
    };
    let started = Instant::now();
    match crumb_work::video::read_meta(yt_dlp, &request.url).await {
        Ok(meta) => {
            tracing::info!(
                "{host}: video details in {} ms",
                started.elapsed().as_millis()
            );
            Json(VideoMetaReply {
                meta,
                relay: relay.name.clone(),
            })
            .into_response()
        }
        Err(why) => {
            tracing::info!("{host}: yt-dlp couldn't read it ({why})");
            error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Couldn't read that video.",
            )
        }
    }
}

pub async fn watch(State(relay): State<Arc<Relay>>, headers: HeaderMap, body: Bytes) -> Response {
    if !relay.authorised(&headers) {
        return error(StatusCode::UNAUTHORIZED, "Missing or wrong token.");
    }
    if !relay.work.can_video() {
        return unable("read videos");
    }
    let Ok(request) = serde_json::from_slice::<WatchRequest>(&body) else {
        return error(
            StatusCode::BAD_REQUEST,
            r#"Expected {"meta": {"url": "...", ...}}."#,
        );
    };
    let host = match video_link(&request.meta.url) {
        Ok(host) => host,
        Err(res) => return *res,
    };
    // Watching isn't rate limited per site like a page (it's one video), only by the workers
    let Ok(_turn) = relay.work.heavy.clone().try_acquire_owned() else {
        return busy();
    };
    let threads = Threads::for_workers(relay.work.workers, None);
    let started = Instant::now();
    match crumb_work::video::watch(&relay.work.video, &request.meta, threads).await {
        Some(watched) => {
            tracing::info!(
                "{host}: watched in {} ms ({} frames)",
                started.elapsed().as_millis(),
                watched.frames.len()
            );
            Json(WatchReply::new(watched, &relay.name)).into_response()
        }
        None => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Couldn't download or watch that video.",
        ),
    }
}
