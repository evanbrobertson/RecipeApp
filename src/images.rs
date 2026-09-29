//! Sized recipe photos at `/img/{recipe_id}/{width}?v={key}`.
//!
//! A recipe's `image` is a third-party URL or an embedded `data:` URI. This fetches it,
//! scales it down to one of a few fixed widths, re-encodes it as lossy WebP and keeps the
//! result in `img-cache/` next to the database. `key` is a hash of the image string (the
//! same one `web/src/lib/img.ts` computes), so a URL with the current key can be cached
//! forever and a changed image gets a new URL.
//!
//! A failure is remembered for a while, so a broken photo isn't refetched on every card
//! render. A link the site refuses for good (see [`LoadError::dead`]) is a quick 404 and is
//! flagged for the cook to fix on the Suggestions page ([`crate::checks::flag_dead_photo`]).
//! Any other linked photo that can't be had here (a bot shield, a slow site, a format the
//! resizer can't read) is most likely fine in a browser, so the request is redirected to
//! the original: the photo shows without a failed request in the console.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use axum::extract::{Path as UrlPath, Query};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Router, routing};
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader, Limits};
use rusqlite::OptionalExtension;

use crate::AppState;
use crumb_core::photo::{MAX_SOURCE_BYTES, TOO_LARGE, decode_data_uri};
pub use crumb_core::photo::{embed, is_embedded_photo};

/// Widths the server makes. Must match `IMG_WIDTHS` in `web/src/lib/img.ts`.
pub const WIDTHS: [u32; 5] = [160, 320, 480, 768, 1200];

const FETCH_TIMEOUT: Duration = Duration::from_secs(15);
/// Decoded size guards against decompression bombs.
const MAX_SIDE: u32 = 12_000;
const MAX_PIXELS: u64 = 50_000_000;
const MAX_DECODE_ALLOC: u64 = 320 * 1024 * 1024;
const WEBP_QUALITY: f32 = 78.0;
/// Resized files kept on disk before the oldest are deleted.
pub const CACHE_CAP_BYTES: u64 = 300 * 1024 * 1024;
const FAILURE_TTL: Duration = Duration::from_secs(10 * 60);
/// `private`: photos sit behind the login, so shared caches and CDNs must not keep them.
const IMMUTABLE: &str = "private, max-age=31536000, immutable";

/// FNV-1a 32-bit over the UTF-8 bytes, as 8 lowercase hex digits (`imageKey` in img.ts).
pub fn image_key(image: &str) -> String {
    let mut h: u32 = 0x811c_9dc5;
    for b in image.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    format!("{h:08x}")
}

/// The nearest width the server makes (ties go to the larger one).
pub fn snap_width(width: u32) -> u32 {
    WIDTHS
        .iter()
        .copied()
        .min_by_key(|w| (w.abs_diff(width), u32::MAX - w))
        .unwrap_or(WIDTHS[0])
}

/// The sized URL for a recipe photo, for the image string currently stored.
pub fn url(recipe_id: i64, width: u32, image: &str) -> String {
    format!("/img/{recipe_id}/{width}?v={}", image_key(image))
}

/// `Link` header value preloading the recipe page's hero photo.
pub fn hero_preload(recipe_id: i64, image: &str) -> String {
    let (m, l) = (url(recipe_id, 768, image), url(recipe_id, 1200, image));
    format!(
        "<{m}>; rel=preload; as=image; fetchpriority=high; \
         imagesrcset=\"{m} 768w, {l} 1200w\"; imagesizes=\"(min-width: 1024px) 640px, 100vw\""
    )
}

/// Disk cache, in-flight coalescing and the failure memory.
pub struct Images {
    dir: Option<PathBuf>,
    cap: u64,
    /// Bytes on disk, counted on the first write and kept up to date after.
    disk_bytes: Mutex<Option<u64>>,
    inflight: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    failures: Mutex<HashMap<String, (Instant, Failure)>>,
    /// Fetch + decode + resize jobs at once. A decoded photo can take a few hundred MB, so a
    /// cold cache behind a page of cards must not decode them all together.
    work: tokio::sync::Semaphore,
}

const PARALLEL_RESIZES: usize = 3;

fn locked<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Images {
    pub fn new(dir: Option<PathBuf>, cap: u64) -> Self {
        Self {
            dir,
            cap,
            disk_bytes: Mutex::new(None),
            inflight: Mutex::default(),
            failures: Mutex::default(),
            work: tokio::sync::Semaphore::new(PARALLEL_RESIZES),
        }
    }

    fn failed_recently(&self, source: &str) -> Option<Failure> {
        locked(&self.failures)
            .get(source)
            .filter(|(at, _)| at.elapsed() < FAILURE_TTL)
            .map(|(_, failure)| *failure)
    }

    fn record_failure(&self, source: String, failure: Failure) {
        let mut f = locked(&self.failures);
        f.retain(|_, (at, _)| at.elapsed() < FAILURE_TTL);
        f.insert(source, (Instant::now(), failure));
    }

    /// Deletes a household's sized photos (see `source` in the handler for their names:
    /// the home household's start with the recipe id, another's with `h{id}-`). Blocking.
    pub fn forget_household(&self, household: crate::households::HouseholdId) {
        let Some(dir) = &self.dir else { return };
        let prefix = format!("h{household}-");
        let theirs = |name: &str| {
            if household == crate::households::HOME {
                name.starts_with(|c: char| c.is_ascii_digit())
            } else {
                name.starts_with(&prefix)
            }
        };
        for (_, _, path) in cache_files(dir) {
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(theirs)
            {
                let _ = std::fs::remove_file(path);
            }
        }
        // Counted again on the next write
        *locked(&self.disk_bytes) = None;
    }

    fn cached_path(&self, name: &str) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(name))
    }

    /// Waits for any other request making `name`, then holds the slot until dropped.
    async fn claim(&self, name: &str) -> Slot<'_> {
        let lock = locked(&self.inflight)
            .entry(name.to_string())
            .or_default()
            .clone();
        let guard = lock.clone().lock_owned().await;
        Slot {
            images: self,
            name: name.to_string(),
            lock,
            guard: Some(guard),
        }
    }

    /// Writes a resized file and trims the cache back under its cap. Blocking.
    fn store(&self, name: &str, bytes: &[u8]) {
        let Some(dir) = &self.dir else { return };
        let tmp = dir.join(format!("{name}.tmp"));
        let written = std::fs::create_dir_all(dir)
            .and_then(|_| std::fs::write(&tmp, bytes))
            .and_then(|_| std::fs::rename(&tmp, dir.join(name)));
        if let Err(err) = written {
            tracing::warn!("[img] couldn't cache {name}: {err}");
            let _ = std::fs::remove_file(&tmp);
            return;
        }
        let mut total = locked(&self.disk_bytes);
        let mut now = match *total {
            Some(t) => t + bytes.len() as u64,
            None => cache_files(dir).iter().map(|f| f.1).sum(),
        };
        if now > self.cap {
            now = prune(dir, self.cap / 10 * 8);
        }
        *total = Some(now);
    }
}

struct Slot<'a> {
    images: &'a Images,
    name: String,
    lock: Arc<tokio::sync::Mutex<()>>,
    guard: Option<tokio::sync::OwnedMutexGuard<()>>,
}

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        self.guard.take();
        let mut map = locked(&self.images.inflight);
        // The map's copy and ours: nobody else is waiting
        if Arc::strong_count(&self.lock) == 2 {
            map.remove(&self.name);
        }
    }
}

/// (modified, size, path) of every file in the cache directory.
fn cache_files(dir: &Path) -> Vec<(std::time::SystemTime, u64, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let meta = e.metadata().ok().filter(|m| m.is_file())?;
            Some((meta.modified().ok()?, meta.len(), e.path()))
        })
        .collect()
}

/// Deletes the oldest files until at most `target` bytes remain; returns what's left.
fn prune(dir: &Path, target: u64) -> u64 {
    let mut files = cache_files(dir);
    files.sort_by_key(|f| f.0);
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    for (_, size, path) in files {
        if total <= target {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= size;
        }
    }
    total
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/img/{id}/{width}", routing::get(serve))
}

/// Why a photo couldn't be made, as remembered for [`FAILURE_TTL`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Failure {
    /// Nothing to show: a dead link or an embedded photo that can't be read.
    Gone,
    /// A linked photo this server couldn't fetch or resize, most likely fine in a browser.
    Elsewhere,
}

/// The answer for a photo that couldn't be made: the original link, for a browser to load
/// itself, or a 404. Only signed-in requests are sent on: a share page's photo is public, and
/// its creator chooses the link, so it must not redirect visitors to wherever that points.
fn unavailable(failure: Failure, image: &str, caching: Caching) -> Response {
    let original = url::Url::parse(image)
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"))
        .and_then(|u| HeaderValue::from_str(u.as_str()).ok());
    match (failure, original) {
        (Failure::Elsewhere, Some(location)) if caching == Caching::Private => {
            let mut res = StatusCode::TEMPORARY_REDIRECT.into_response();
            let h = res.headers_mut();
            h.insert(header::LOCATION, location);
            // As long as the failure is remembered; a later try may resize it after all
            h.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, max-age=600"),
            );
            res
        }
        _ => not_found(),
    }
}

fn not_found() -> Response {
    let mut res = StatusCode::NOT_FOUND.into_response();
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    res
}

/// What to make from a recipe photo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// WebP at one of [`WIDTHS`].
    Webp(u32),
    /// A 1200×630 JPEG crop for link previews (Open Graph).
    Preview,
}

impl Variant {
    fn file_suffix(self) -> String {
        match self {
            Variant::Webp(w) => format!("{w}.webp"),
            Variant::Preview => "og.jpg".into(),
        }
    }
}

/// Who may keep a copy of a sized photo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caching {
    /// Behind the login: the browser only, for good once the URL has the current key.
    Private,
    /// A share page's photo: link-preview bots and shared caches may keep it for a day.
    Public,
}

/// Width and height of the link-preview crop.
pub const PREVIEW_SIZE: (u32, u32) = (1200, 630);
const PREVIEW_JPEG_QUALITY: u8 = 82;

fn image_response(
    bytes: Vec<u8>,
    key: &str,
    variant: Variant,
    current: bool,
    caching: Caching,
) -> Response {
    let mut res = bytes.into_response();
    let h = res.headers_mut();
    let content_type = match variant {
        Variant::Webp(_) => "image/webp",
        Variant::Preview => "image/jpeg",
    };
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    let cache = match (current, caching) {
        (false, _) => "no-cache",
        (true, Caching::Private) => IMMUTABLE,
        (true, Caching::Public) => "public, max-age=86400",
    };
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    let tag = match variant {
        Variant::Webp(w) => format!("\"{key}-{w}\""),
        Variant::Preview => format!("\"{key}-og\""),
    };
    if let Ok(tag) = HeaderValue::from_str(&tag) {
        h.insert(header::ETAG, tag);
    }
    res
}

fn digits<T: std::str::FromStr>(s: &str) -> Option<T> {
    (!s.is_empty() && s.len() <= 12 && s.bytes().all(|b| b.is_ascii_digit()))
        .then(|| s.parse().ok())
        .flatten()
}

/// A width from a URL segment, snapped to the nearest one the server makes.
pub fn parse_width(raw: &str) -> Option<u32> {
    digits::<u32>(raw).map(snap_width)
}

async fn serve(
    crate::Scoped(state): crate::Scoped,
    UrlPath((id, width)): UrlPath<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let (Some(id), Some(width)) = (digits::<i64>(&id), parse_width(&width)) else {
        return not_found();
    };
    serve_photo(
        &state,
        id,
        Variant::Webp(width),
        q.get("v").map(String::as_str),
        Caching::Private,
    )
    .await
}

/// A recipe's photo made into `variant`, from the disk cache when it's there. `v` is the
/// key the URL carries: only a URL with the current key may be cached for long.
pub async fn serve_photo(
    state: &AppState,
    id: i64,
    variant: Variant,
    v: Option<&str>,
    caching: Caching,
) -> Response {
    let row: Option<(Option<String>, Option<String>)> = state
        .db
        .lock()
        .query_row("SELECT image, url FROM recipes WHERE id = ?1", [id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()
        .unwrap_or(None);
    let Some((Some(image), page_url)) =
        row.filter(|r| r.0.as_deref().is_some_and(|i| !i.is_empty()))
    else {
        return not_found();
    };
    let key = image_key(&image);
    let current = v.is_some_and(|v| v == key);
    let images = &state.images;
    // The home household's names are as they always were; another's carry its id, so
    // two boxes' recipe 12 never share a cached photo
    let source = if state.household == crate::households::HOME {
        format!("{id}-{key}")
    } else {
        format!("h{}-{id}-{key}", state.household)
    };
    let name = format!("{source}-{}", variant.file_suffix());

    if let Some(failure) = images.failed_recently(&source) {
        return unavailable(failure, &image, caching);
    }
    let read_cached = || async {
        match images.cached_path(&name) {
            Some(p) => tokio::fs::read(p).await.ok(),
            None => None,
        }
    };
    if let Some(bytes) = read_cached().await {
        return image_response(bytes, &key, variant, current, caching);
    }

    let _slot = images.claim(&name).await;
    // Someone else may have made it (or failed) while this request waited
    if let Some(bytes) = read_cached().await {
        return image_response(bytes, &key, variant, current, caching);
    }
    if let Some(failure) = images.failed_recently(&source) {
        return unavailable(failure, &image, caching);
    }

    let Ok(_permit) = images.work.acquire().await else {
        return not_found();
    };
    let linked = !image.starts_with("data:");
    let elsewhere = if linked {
        Failure::Elsewhere
    } else {
        Failure::Gone
    };
    let original = match load_source(state, &image, page_url.as_deref()).await {
        Ok(b) => b,
        Err(err) => {
            tracing::info!("[img] recipe {id}: {err}");
            let failure = if err.dead { Failure::Gone } else { elsewhere };
            images.record_failure(source, failure);
            // A link the site refuses for good goes on the Suggestions page (see checks.rs)
            if linked
                && err.dead
                && let Err(e) = crate::checks::flag_dead_photo(&state.db.lock(), id, &image)
            {
                tracing::warn!("[img] recipe {id}: couldn't flag its photo: {e:?}");
            }
            return unavailable(failure, &image, caching);
        }
    };
    if linked && let Err(e) = crate::checks::photo_works(&state.db.lock(), id, &image) {
        tracing::warn!("[img] recipe {id}: couldn't clear its photo flag: {e:?}");
    }
    let worker = state.images.clone();
    let cache_name = name.clone();
    let made = tokio::task::spawn_blocking(move || {
        let out = match variant {
            Variant::Webp(width) => resize_to_webp(&original, width)?,
            Variant::Preview => preview_jpeg(&original)?,
        };
        worker.store(&cache_name, &out);
        Ok::<_, String>(out)
    })
    .await
    .unwrap_or_else(|e| Err(format!("resize task failed: {e}")));
    match made {
        Ok(bytes) => image_response(bytes, &key, variant, current, caching),
        Err(err) => {
            tracing::info!("[img] recipe {id}: {err}");
            images.record_failure(source, elsewhere);
            unavailable(elsewhere, &image, caching)
        }
    }
}

/// The `reqwest` client of the image fallback for links the cook supplied: it connects only to
/// public addresses, whether the link names one or a redirect does (the wreq clients do the
/// same, see `crumb_fetch::guard`).
fn guarded_http() -> &'static reqwest::Client {
    static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .dns_resolver(Arc::new(crate::scraper::PublicResolver))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 10 {
                    attempt.error("too many redirects")
                } else if crumb_fetch::check_target(attempt.url()).is_err() {
                    attempt.error("redirected to a private address")
                } else {
                    attempt.follow()
                }
            }))
            .build()
            .expect("HTTP client")
    });
    &CLIENT
}

/// The original image bytes, from a `data:` URI or over HTTP.
async fn load_source(
    state: &AppState,
    image: &str,
    referer: Option<&str>,
) -> Result<Vec<u8>, LoadError> {
    if image.starts_with("data:") {
        return decode_data_uri(image).map_err(LoadError::dead);
    }
    let parsed =
        url::Url::parse(image).map_err(|e| LoadError::dead(format!("bad image URL: {e}")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(LoadError::dead(format!(
            "unsupported image URL scheme {}",
            parsed.scheme()
        )));
    }
    // A photo link is the recipe site's to name, so it can name anything: never the server's
    // own network
    let allow_private = state.config.scrape_allow_private;
    if !allow_private && crumb_fetch::check_resolved(&parsed).await.is_err() {
        return Err(LoadError::dead("the image link points somewhere private"));
    }
    // What a browser on the recipe's page would send; some CDNs refuse hotlinks without it
    let referer = referer.filter(|r| r.starts_with("http"));
    // A browser fingerprint first (some CDNs answer a plain client with an HTML block page).
    // Plain reqwest only if wreq got no answer at all; an answer (a 404, a page) stands.
    if let Some(client) = crate::scraper::wreq_client(crate::scraper::Method::Firefox) {
        match load_with_wreq(client, parsed.as_str(), referer).await {
            Ok(body) => return Ok(body),
            Err(Wreq::Answered(err)) => return Err(err),
            Err(Wreq::NoAnswer(err)) => tracing::debug!(
                "[img] {}: wreq failed ({err}), trying reqwest",
                parsed.host_str().unwrap_or("?")
            ),
        }
    }
    let http = if allow_private {
        &state.http
    } else {
        guarded_http()
    };
    load_with_reqwest(http, parsed, referer).await
}

/// Image types first, as a browser's `<img>` request asks (no AVIF: it can't be decoded here).
const IMAGE_ACCEPT: &str = "image/webp,image/png,image/jpeg,image/gif,image/*;q=0.8";

/// Why a photo couldn't be had.
#[derive(Debug)]
pub struct LoadError {
    message: String,
    /// The link itself is bad: the site answered 4xx or with a page instead of a photo, or
    /// the URL can't be fetched at all. Asking again won't help. A timeout, a 5xx or a photo
    /// too large to resize may be fine later, or in a browser.
    pub dead: bool,
}

impl LoadError {
    fn dead(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            dead: true,
        }
    }

    fn passing(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            dead: false,
        }
    }
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Headers a bot shield sets on its challenge page (SiteGround, Cloudflare, AWS WAF). A
/// browser passes the challenge and gets the photo, so the link isn't dead.
const CHALLENGE_HEADERS: [&str; 3] = ["sg-captcha", "cf-mitigated", "x-amzn-waf-action"];

/// Refuses a response that isn't a usable image before reading its body. `challenged`: the
/// answer carries one of [`CHALLENGE_HEADERS`].
fn check_response(
    status: u16,
    content_type: Option<&str>,
    length: Option<u64>,
    challenged: bool,
) -> Result<(), LoadError> {
    if challenged {
        return Err(LoadError::passing(format!("bot challenge ({status})")));
    }
    if !(200..300).contains(&status) {
        let message = format!("fetch returned {status}");
        // 429 is "slow down", not "gone"
        return Err(if (400..500).contains(&status) && status != 429 {
            LoadError::dead(message)
        } else {
            LoadError::passing(message)
        });
    }
    let kind = content_type.unwrap_or("").to_ascii_lowercase();
    if kind.starts_with("text/") || kind.contains("json") {
        let message = format!("not an image ({status} {kind})");
        // Only a plain 200 page means the link leads somewhere else; a 202 or the like is
        // a holding page (often an unlabelled bot challenge)
        return Err(if status == 200 {
            LoadError::dead(message)
        } else {
            LoadError::passing(message)
        });
    }
    if length.is_some_and(|n| n > MAX_SOURCE_BYTES as u64) {
        return Err(LoadError::passing(TOO_LARGE));
    }
    Ok(())
}

/// How a wreq image fetch failed.
enum Wreq {
    /// The server answered, but not with a usable image.
    Answered(LoadError),
    /// No response, or the body didn't arrive; worth trying another client.
    NoAnswer(String),
}

async fn load_with_wreq(
    client: &wreq::Client,
    url: &str,
    referer: Option<&str>,
) -> Result<Vec<u8>, Wreq> {
    // The profile sets the other headers; these two are what an image request differs by
    let mut req = client
        .get(url)
        .timeout(FETCH_TIMEOUT)
        .header(header::ACCEPT, IMAGE_ACCEPT);
    if let Some(r) = referer {
        req = req.header(header::REFERER, r);
    }
    let res = req
        .send()
        .await
        .map_err(|e| Wreq::NoAnswer(format!("fetch failed: {e}")))?;
    let content_type = res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    let challenged = CHALLENGE_HEADERS
        .iter()
        .any(|h| res.headers().contains_key(*h));
    check_response(
        res.status().as_u16(),
        content_type,
        res.content_length(),
        challenged,
    )
    .map_err(Wreq::Answered)?;
    crate::scraper::read_capped(res, MAX_SOURCE_BYTES)
        .await
        .map_err(|e| match e {
            crate::scraper::ReadError::TooLarge => Wreq::Answered(LoadError::passing(TOO_LARGE)),
            crate::scraper::ReadError::Failed(e) => Wreq::NoAnswer(format!("read failed: {e}")),
        })
}

async fn load_with_reqwest(
    http: &reqwest::Client,
    url: url::Url,
    referer: Option<&str>,
) -> Result<Vec<u8>, LoadError> {
    let mut req = http
        .get(url)
        .timeout(FETCH_TIMEOUT)
        .header(header::USER_AGENT, crate::scraper::USER_AGENT)
        .header(header::ACCEPT, IMAGE_ACCEPT);
    if let Some(r) = referer {
        req = req.header(header::REFERER, r);
    }
    let mut res = req
        .send()
        .await
        .map_err(|e| LoadError::passing(format!("fetch failed: {e}")))?;
    let content_type = res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    let challenged = CHALLENGE_HEADERS
        .iter()
        .any(|h| res.headers().contains_key(*h));
    check_response(
        res.status().as_u16(),
        content_type,
        res.content_length(),
        challenged,
    )?;
    let mut body = Vec::new();
    while let Some(chunk) = res
        .chunk()
        .await
        .map_err(|e| LoadError::passing(format!("read failed: {e}")))?
    {
        if body.len() + chunk.len() > MAX_SOURCE_BYTES {
            return Err(LoadError::passing(TOO_LARGE));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Whether a new recipe's photo link is dead, fetched as the resizer would. Only a link the
/// site itself refuses counts (see [`LoadError::dead`]); a slow or failing site gets the
/// benefit of the doubt. An embedded photo is never dead here: it was read when kept.
pub async fn photo_is_dead(state: &AppState, image: &str, referer: Option<&str>) -> bool {
    if image.starts_with("data:") {
        return false;
    }
    match load_source(state, image, referer).await {
        Ok(_) => false,
        Err(err) => {
            tracing::info!(
                "[img] {}: new recipe's photo {}: {err}",
                crate::telemetry::host_of(image),
                if err.dead { "is dead" } else { "didn't load" }
            );
            err.dead
        }
    }
}

/// Fetches a photo (as the resizer would) to keep in the recipe itself: a `data:` URI, or
/// None when it can't be had or isn't a JPEG, PNG or WebP.
pub async fn fetch_to_embed(state: &AppState, url: &str) -> Option<String> {
    match load_source(state, url, None).await {
        Ok(bytes) => embed(&bytes),
        Err(err) => {
            tracing::info!(
                "[img] {}: couldn't fetch a photo to keep: {err}",
                crate::telemetry::host_of(url)
            );
            None
        }
    }
}

/// Decodes with the size guards and applies EXIF orientation.
fn decode(bytes: &[u8]) -> Result<DynamicImage, String> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| format!("can't decode: {e}"))?;
    let (w, h) = decoder.dimensions();
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(format!("image too large ({w}x{h})"));
    }
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| format!("can't decode: {e}"))?;
    img.apply_orientation(orientation);
    if img.width() == 0 || img.height() == 0 {
        return Err("empty image".into());
    }
    Ok(img)
}

/// Decodes, applies EXIF orientation, scales to `width` (never up) and encodes lossy WebP.
pub fn resize_to_webp(bytes: &[u8], width: u32) -> Result<Vec<u8>, String> {
    let mut img = decode(bytes)?;
    let (w, h) = (img.width(), img.height());
    if width < w {
        let height = ((f64::from(h) * f64::from(width) / f64::from(w)).round() as u32).max(1);
        img = img.resize_exact(width, height, FilterType::CatmullRom);
    }
    let (w, h) = (img.width(), img.height());
    let encoded = if img.color().has_alpha() {
        let px = img.to_rgba8();
        webp::Encoder::from_rgba(&px, w, h).encode_simple(false, WEBP_QUALITY)
    } else {
        let px = img.to_rgb8();
        webp::Encoder::from_rgb(&px, w, h).encode_simple(false, WEBP_QUALITY)
    };
    encoded
        .map(|m| m.to_vec())
        .map_err(|e| format!("can't encode WebP: {e:?}"))
}

/// The link-preview card: the photo scaled to cover 1200×630 and cropped to its centre,
/// as a JPEG (what every link-preview bot can read).
pub fn preview_jpeg(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let (w, h) = PREVIEW_SIZE;
    let img = decode(bytes)?.resize_to_fill(w, h, FilterType::CatmullRom);
    let mut out = Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, PREVIEW_JPEG_QUALITY)
        .encode_image(&img.to_rgb8())
        .map_err(|e| format!("can't encode JPEG: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    pub fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 90])
        });
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn dims(webp: &[u8]) -> (u32, u32) {
        let img = image::load_from_memory_with_format(webp, image::ImageFormat::WebP).unwrap();
        (img.width(), img.height())
    }

    #[test]
    fn keys_match_the_frontend() {
        // Vectors from imageKey in web/src/lib/img.ts
        assert_eq!(image_key(""), "811c9dc5");
        assert_eq!(image_key("a"), "e40c292c");
        assert_eq!(image_key("foobar"), "bf9cf968");
        assert_eq!(image_key("https://img.test/crème-brûlée.jpg"), "eaee3b9d");
    }

    #[test]
    fn widths_snap_to_the_nearest() {
        assert_eq!(snap_width(0), 160);
        assert_eq!(snap_width(160), 160);
        assert_eq!(snap_width(240), 320);
        assert_eq!(snap_width(239), 160);
        assert_eq!(snap_width(500), 480);
        assert_eq!(snap_width(768), 768);
        assert_eq!(snap_width(1000), 1200);
        assert_eq!(snap_width(u32::MAX), 1200);
    }

    #[test]
    fn hero_preload_lists_both_sizes() {
        let key = image_key("https://x.test/a.jpg");
        assert_eq!(
            hero_preload(7, "https://x.test/a.jpg"),
            format!(
                "</img/7/768?v={key}>; rel=preload; as=image; fetchpriority=high; \
                 imagesrcset=\"/img/7/768?v={key} 768w, /img/7/1200?v={key} 1200w\"; \
                 imagesizes=\"(min-width: 1024px) 640px, 100vw\""
            )
        );
    }

    #[test]
    fn resizes_down_and_never_up() {
        let src = png(2000, 1000);
        assert_eq!(dims(&resize_to_webp(&src, 768).unwrap()), (768, 384));
        let small = png(300, 200);
        assert_eq!(dims(&resize_to_webp(&small, 1200).unwrap()), (300, 200));
        assert!(resize_to_webp(b"<html>nope</html>", 320).is_err());
    }

    #[test]
    fn refuses_decompression_bombs() {
        // A valid header for a 20000x20000 PNG is rejected before any pixels are allocated
        let img = image::GrayImage::new(1, 1);
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        let mut bytes = out.into_inner();
        // IHDR width/height live at bytes 16..24
        bytes[16..20].copy_from_slice(&20_000u32.to_be_bytes());
        bytes[20..24].copy_from_slice(&20_000u32.to_be_bytes());
        let mut crc = flate2::Crc::new();
        crc.update(&bytes[12..29]);
        bytes[29..33].copy_from_slice(&crc.sum().to_be_bytes());
        let err = resize_to_webp(&bytes, 320).unwrap_err();
        assert!(err.contains("limit") || err.contains("too large"), "{err}");
    }

    #[test]
    fn reads_base64_data_uris() {
        let bytes = png(4, 4);
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        assert_eq!(
            decode_data_uri(&format!("data:image/png;base64,{b64}")).unwrap(),
            bytes
        );
        assert!(decode_data_uri("data:text/plain;base64,aGk=").is_err());
        assert!(decode_data_uri("data:image/svg+xml,<svg/>").is_err());
    }

    #[test]
    fn only_jpeg_png_and_webp_photos_are_kept_in_a_recipe() {
        let png = png(4, 4);
        let uri = embed(&png).unwrap();
        assert!(uri.starts_with("data:image/png;base64,"));
        assert!(is_embedded_photo(&uri));
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
        // Labelled as one of the three, and really one of them (the label may be off)
        assert!(is_embedded_photo(&format!("data:image/jpeg;base64,{b64}")));
        assert!(!is_embedded_photo(&format!("data:image/gif;base64,{b64}")));
        assert!(!is_embedded_photo(&format!("data:text/html;base64,{b64}")));
        assert!(!is_embedded_photo("data:image/png;base64,AA"));
        assert!(!is_embedded_photo("data:image/svg+xml;base64,PHN2Zy8+"));
        assert!(!is_embedded_photo("https://example.com/a.png"));
        assert!(!is_embedded_photo("javascript:alert(1)"));
        assert_eq!(embed(b"<svg/>"), None);
        let webp = resize_to_webp(&png, 160).unwrap();
        assert!(embed(&webp).unwrap().starts_with("data:image/webp;base64,"));
    }

    #[test]
    fn only_a_refusal_makes_a_photo_link_dead() {
        let dead = |status, kind, challenged| {
            check_response(status, Some(kind), None, challenged)
                .err()
                .map(|e| e.dead)
        };
        assert_eq!(dead(200, "image/jpeg", false), None);
        assert_eq!(dead(404, "text/html", false), Some(true));
        assert_eq!(dead(200, "text/html", false), Some(true));
        // SiteGround's shield: 202, an HTML page and `sg-captcha: challenge`
        assert_eq!(dead(202, "text/html", true), Some(false));
        assert_eq!(dead(202, "text/html", false), Some(false));
        // Cloudflare's: 403 with `cf-mitigated: challenge`
        assert_eq!(dead(403, "text/html", true), Some(false));
        assert_eq!(dead(429, "text/html", false), Some(false));
        assert_eq!(dead(503, "text/html", false), Some(false));
    }

    #[test]
    fn a_photo_a_browser_may_get_is_sent_to_the_original() {
        let photo = "https://example.com/wp-content/uploads/crepes.jpg";
        let res = unavailable(Failure::Elsewhere, photo, Caching::Private);
        assert_eq!(res.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(res.headers()[header::LOCATION], photo);
        assert_eq!(
            unavailable(Failure::Gone, photo, Caching::Private).status(),
            StatusCode::NOT_FOUND
        );
        let embedded = "data:image/png;base64,AAAA";
        assert_eq!(
            unavailable(Failure::Elsewhere, embedded, Caching::Private).status(),
            StatusCode::NOT_FOUND
        );
        // A public share page's photo never redirects
        assert_eq!(
            unavailable(Failure::Elsewhere, photo, Caching::Public).status(),
            StatusCode::NOT_FOUND
        );
        let ftp = "ftp://example.com/a.jpg";
        assert_eq!(
            unavailable(Failure::Elsewhere, ftp, Caching::Private).status(),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn prunes_the_oldest_files() {
        let dir = tempfile::tempdir().unwrap();
        let images = Images::new(Some(dir.path().to_path_buf()), 250);
        for i in 0..5 {
            images.store(&format!("{i}.webp"), &[0u8; 100]);
            // Distinct mtimes so "oldest" is well defined
            let f = std::fs::File::options()
                .append(true)
                .open(dir.path().join(format!("{i}.webp")))
                .unwrap();
            f.set_modified(std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1000 + i))
                .unwrap();
        }
        let mut left: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        // Over 250 bytes after the third write: trimmed to 200 (80%), then again after the fifth
        assert_eq!(left, vec!["3.webp", "4.webp"]);
    }
}
