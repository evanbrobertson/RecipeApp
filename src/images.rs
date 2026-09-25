//! Sized recipe photos at `/img/{recipe_id}/{width}?v={key}`.
//!
//! A recipe's `image` is a third-party URL or an embedded `data:` URI. This fetches it,
//! scales it down to one of a few fixed widths, re-encodes it as lossy WebP and keeps the
//! result in `img-cache/` next to the database. `key` is a hash of the image string (the
//! same one `web/src/lib/img.ts` computes), so a URL with the current key can be cached
//! forever and a changed image gets a new URL.
//!
//! Any failure is a quick 404 (the page falls back to the original URL) and is
//! remembered for a while, so a broken photo isn't refetched on every card render.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use axum::extract::{Path as UrlPath, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Router, routing};
use base64::Engine;
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader, Limits};
use rusqlite::OptionalExtension;

use crate::AppState;

/// Widths the server makes. Must match `IMG_WIDTHS` in `web/src/lib/img.ts`.
pub const WIDTHS: [u32; 5] = [160, 320, 480, 768, 1200];

const FETCH_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_SOURCE_BYTES: usize = 15 * 1024 * 1024;
/// Decoded size guards against decompression bombs.
const MAX_SIDE: u32 = 12_000;
const MAX_PIXELS: u64 = 50_000_000;
const MAX_DECODE_ALLOC: u64 = 320 * 1024 * 1024;
const WEBP_QUALITY: f32 = 78.0;
/// Resized files kept on disk before the oldest are deleted.
pub const CACHE_CAP_BYTES: u64 = 300 * 1024 * 1024;
const FAILURE_TTL: Duration = Duration::from_secs(10 * 60);
const IMMUTABLE: &str = "public, max-age=31536000, immutable";

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
    failures: Mutex<HashMap<String, Instant>>,
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

    fn failed_recently(&self, source: &str) -> bool {
        locked(&self.failures)
            .get(source)
            .is_some_and(|at| at.elapsed() < FAILURE_TTL)
    }

    fn record_failure(&self, source: String) {
        let mut f = locked(&self.failures);
        f.retain(|_, at| at.elapsed() < FAILURE_TTL);
        f.insert(source, Instant::now());
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

fn not_found() -> Response {
    let mut res = StatusCode::NOT_FOUND.into_response();
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    res
}

fn webp_response(bytes: Vec<u8>, key: &str, width: u32, current: bool) -> Response {
    let mut res = bytes.into_response();
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/webp"));
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(if current { IMMUTABLE } else { "no-cache" }),
    );
    if let Ok(tag) = HeaderValue::from_str(&format!("\"{key}-{width}\"")) {
        h.insert(header::ETAG, tag);
    }
    res
}

fn digits<T: std::str::FromStr>(s: &str) -> Option<T> {
    (!s.is_empty() && s.len() <= 12 && s.bytes().all(|b| b.is_ascii_digit()))
        .then(|| s.parse().ok())
        .flatten()
}

async fn serve(
    State(state): State<AppState>,
    UrlPath((id, width)): UrlPath<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let (Some(id), Some(width)) = (digits::<i64>(&id), digits::<u32>(&width)) else {
        return not_found();
    };
    let width = snap_width(width);
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
    let current = q.get("v").is_some_and(|v| *v == key);
    let images = &state.images;
    let source = format!("{id}-{key}");
    let name = format!("{source}-{width}.webp");

    if images.failed_recently(&source) {
        return not_found();
    }
    let read_cached = || async {
        match images.cached_path(&name) {
            Some(p) => tokio::fs::read(p).await.ok(),
            None => None,
        }
    };
    if let Some(bytes) = read_cached().await {
        return webp_response(bytes, &key, width, current);
    }

    let _slot = images.claim(&name).await;
    // Someone else may have made it (or failed) while this request waited
    if let Some(bytes) = read_cached().await {
        return webp_response(bytes, &key, width, current);
    }
    if images.failed_recently(&source) {
        return not_found();
    }

    let Ok(_permit) = images.work.acquire().await else {
        return not_found();
    };
    let original = match load_source(&state.http, &image, page_url.as_deref()).await {
        Ok(b) => b,
        Err(err) => {
            tracing::info!("[img] recipe {id}: {err}");
            images.record_failure(source);
            return not_found();
        }
    };
    let worker = state.images.clone();
    let cache_name = name.clone();
    let made = tokio::task::spawn_blocking(move || {
        let out = resize_to_webp(&original, width)?;
        worker.store(&cache_name, &out);
        Ok::<_, String>(out)
    })
    .await
    .unwrap_or_else(|e| Err(format!("resize task failed: {e}")));
    match made {
        Ok(bytes) => webp_response(bytes, &key, width, current),
        Err(err) => {
            tracing::info!("[img] recipe {id}: {err}");
            images.record_failure(source);
            not_found()
        }
    }
}

/// The original image bytes, from a `data:` URI or over HTTP.
async fn load_source(
    http: &reqwest::Client,
    image: &str,
    referer: Option<&str>,
) -> Result<Vec<u8>, String> {
    if image.starts_with("data:") {
        return decode_data_uri(image);
    }
    let parsed = url::Url::parse(image).map_err(|e| format!("bad image URL: {e}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(format!("unsupported image URL scheme {}", parsed.scheme()));
    }
    let mut req = http
        .get(parsed)
        .timeout(FETCH_TIMEOUT)
        .header(header::USER_AGENT, crate::scraper::USER_AGENT)
        .header(
            header::ACCEPT,
            "image/webp,image/png,image/jpeg,image/gif,image/*;q=0.8",
        );
    // What a browser on the recipe's page would send; some CDNs refuse hotlinks without it
    if let Some(r) = referer.filter(|r| r.starts_with("http")) {
        req = req.header(header::REFERER, r);
    }
    let mut res = req.send().await.map_err(|e| format!("fetch failed: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("fetch returned {}", res.status()));
    }
    let kind = res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if kind.starts_with("text/") || kind.contains("json") {
        return Err(format!("not an image ({kind})"));
    }
    if res
        .content_length()
        .is_some_and(|n| n > MAX_SOURCE_BYTES as u64)
    {
        return Err("image too large".into());
    }
    let mut body = Vec::new();
    while let Some(chunk) = res.chunk().await.map_err(|e| format!("read failed: {e}"))? {
        if body.len() + chunk.len() > MAX_SOURCE_BYTES {
            return Err("image too large".into());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Bytes of a `data:image/...;base64,` URI.
fn decode_data_uri(uri: &str) -> Result<Vec<u8>, String> {
    let rest = uri.strip_prefix("data:").ok_or("not a data URI")?;
    let (meta, data) = rest.split_once(',').ok_or("malformed data URI")?;
    let meta = meta.to_ascii_lowercase();
    if !meta.starts_with("image/") || !meta.split(';').any(|p| p == "base64") {
        return Err("data URI isn't a base64 image".into());
    }
    if data.len() > MAX_SOURCE_BYTES / 3 * 4 + 4096 {
        return Err("image too large".into());
    }
    let data: String = data.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let data = percent_encoding::percent_decode_str(&data).decode_utf8_lossy();
    let engine = base64::engine::general_purpose::STANDARD;
    engine
        .decode(data.as_bytes())
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(data.as_bytes()))
        .map_err(|e| format!("bad base64 in data URI: {e}"))
}

/// Decodes, applies EXIF orientation, scales to `width` (never up) and encodes lossy WebP.
pub fn resize_to_webp(bytes: &[u8], width: u32) -> Result<Vec<u8>, String> {
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

    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return Err("empty image".into());
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
