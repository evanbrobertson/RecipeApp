//! Recipes from cooking videos (TikTok, Instagram Reels, YouTube videos and Shorts). The
//! caption comes first: when it already holds the recipe, nothing is downloaded. Otherwise the
//! video is fetched with `yt-dlp`, what the cook says is taken from its English subtitles when
//! it has them (YouTube's automatic captions) or else transcribed on this machine
//! (`whisper.cpp`), a handful of frames are taken for on-screen text (`ffmpeg`), and Wee Chef
//! reads all three together, with the frames going to its vision model.
//!
//! Every tool is optional: without `yt-dlp` the caption is read from the page itself, and
//! without Wee Chef only a caption that is a whole recipe can be saved.
//!
//! YouTube often won't serve a server at all ("Sign in to confirm you're not a bot"). The
//! Crumb browser extension then reads the video's page in the cook's own browser and sends
//! its details and the words of its captions with the link ([`FromBrowser`]); nothing of the
//! cook's YouTube sign-in leaves the browser.
//!
//! Imports wait their turn in [`crate::video_jobs`], which runs [`import`] on its workers.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use regex::Regex;
use scraper::{Html, Selector};
use serde::Deserialize;
use serde_json::Value;
use tokio::process::Command;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::model::{Recipe, RecipeFields};
use crate::photos::Photo;

/// Longest video that is downloaded; a longer one is only read from its caption.
const MAX_SECONDS: f64 = 20.0 * 60.0;
/// Speech after this is not transcribed (a recipe is said well before then).
const MAX_AUDIO_SECONDS: u32 = 15 * 60;
/// Largest download yt-dlp is allowed.
const MAX_DOWNLOAD: &str = "250M";
/// Frames sent to Wee Chef, at most.
const MAX_FRAMES: usize = 12;
/// Frames taken from the video before near-duplicates are dropped.
const SAMPLED_FRAMES: u32 = 48;
/// Mean difference (0-255) of two 32×32 grey thumbnails below which a frame repeats the last.
const SAME_FRAME: f64 = 7.0;
/// Width frames are scaled to: on-screen text stays legible, the upload stays small.
const FRAME_WIDTH: u32 = 720;

const METADATA_TIMEOUT: Duration = Duration::from_secs(45);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(180);
const SUBTITLES_TIMEOUT: Duration = Duration::from_secs(45);
/// Longest transcript Wee Chef is sent (a 15-minute video says about 15 000 characters).
const MAX_TRANSCRIPT_CHARS: usize = 30_000;
const FFMPEG_TIMEOUT: Duration = Duration::from_secs(120);
const WHISPER_TIMEOUT: Duration = Duration::from_secs(420);

const PASTE_HINT: &str = "Try copying the recipe text and pasting it instead.";
/// For a YouTube video the server had to read on its own: YouTube often turns servers away.
const YOUTUBE_HINT: &str = "YouTube often won't let Crumb in on its own: open the video with the Crumb browser extension and choose Read in Crumb, or paste the recipe text instead.";

fn is_youtube(url: &str) -> bool {
    let host = crate::telemetry::host_of(url);
    host == "youtu.be" || host == "youtube.com" || host.ends_with(".youtube.com")
}

/// The programs a video import can use. Found at start (see [`VideoTools::from_env`]).
#[derive(Debug, Clone, Default)]
pub struct VideoTools {
    pub yt_dlp: Option<PathBuf>,
    pub ffmpeg: Option<PathBuf>,
    /// `whisper-cli` and its ggml model file.
    pub whisper: Option<(PathBuf, PathBuf)>,
}

impl VideoTools {
    /// `YT_DLP_PATH`, `FFMPEG_PATH`, `WHISPER_PATH` and `WHISPER_MODEL`, else the programs
    /// on `PATH` and the model where the Docker image keeps it. `VIDEO_IMPORT=off` turns
    /// downloading off (captions are still read from the page).
    pub fn from_env() -> Self {
        if std::env::var("VIDEO_IMPORT").is_ok_and(|v| v.eq_ignore_ascii_case("off")) {
            return Self::default();
        }
        let find = |var: &str, name: &str| {
            std::env::var_os(var)
                .map(PathBuf::from)
                .filter(|p| p.is_file())
                .or_else(|| on_path(name))
        };
        let model = std::env::var_os("WHISPER_MODEL")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/opt/video/models/ggml-base.en.bin"));
        let whisper = find("WHISPER_PATH", "whisper-cli").filter(|_| model.is_file());
        Self {
            yt_dlp: find("YT_DLP_PATH", "yt-dlp"),
            ffmpeg: find("FFMPEG_PATH", "ffmpeg"),
            whisper: whisper.map(|cli| (cli, model)),
        }
    }

    /// For the start-up log line.
    pub fn describe(&self) -> String {
        let yes = |b: bool| if b { "yes" } else { "no" };
        format!(
            "video import: yt-dlp {}, ffmpeg {}, whisper {}",
            yes(self.yt_dlp.is_some()),
            yes(self.ffmpeg.is_some()),
            yes(self.whisper.is_some())
        )
    }
}

fn on_path(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}

/// Whether a link is a cooking video we know how to read: TikTok, an Instagram reel, or a
/// YouTube video or Short.
pub fn is_video_url(url: &str) -> bool {
    let Ok(u) = url::Url::parse(url) else {
        return false;
    };
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    let host = host
        .strip_prefix("www.")
        .or_else(|| host.strip_prefix("m."))
        .unwrap_or(&host);
    let path = u.path();
    match host {
        "tiktok.com" | "vm.tiktok.com" | "vt.tiktok.com" => path.len() > 1,
        "instagram.com" => ["/reel/", "/reels/", "/tv/"]
            .iter()
            .any(|p| path.starts_with(p)),
        "youtube.com" => {
            ["/shorts/", "/live/"]
                .iter()
                .any(|p| path.len() > p.len() && path.starts_with(p))
                || (path == "/watch" && u.query_pairs().any(|(k, v)| k == "v" && !v.is_empty()))
        }
        "youtu.be" => path.len() > 1,
        _ => false,
    }
}

/// What the cook's browser read from a video's page (the Crumb extension), sent with the
/// link to `POST /api/recipes/import` as `video`. It's whatever the cook sends, so it's
/// treated like pasted text: trimmed to size, and only a YouTube image address is fetched.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FromBrowser {
    pub title: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub thumbnail: Option<String>,
    /// Seconds.
    pub duration: Option<f64>,
    /// The words of the video's captions.
    pub transcript: Option<String>,
}

/// Longest description kept from the browser (YouTube's own limit is 5 000).
const MAX_DESCRIPTION_CHARS: usize = 10_000;
/// How long what the browser read waits for its import to start.
const KEEP_FROM_BROWSER: Duration = Duration::from_secs(15 * 60);
const MAX_KEPT_FROM_BROWSER: usize = 64;

fn clip(value: Option<String>, max: usize) -> Option<String> {
    let value = value?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(match value.char_indices().nth(max) {
        Some((cut, _)) => value[..cut].to_string(),
        None => value.to_string(),
    })
}

/// A YouTube image address (`i.ytimg.com`, `yt3.ggpht.com`), else nothing: the server
/// fetches it for the recipe's photo, so it can't be just any address.
fn youtube_image(url: Option<String>) -> Option<String> {
    let parsed = url::Url::parse(url.as_deref()?).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    let ours = ["ytimg.com", "ggpht.com"]
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")));
    (parsed.scheme() == "https" && ours).then(|| parsed.to_string())
}

impl FromBrowser {
    /// Trimmed to size, with a thumbnail only when it's YouTube's.
    pub fn cleaned(self) -> Self {
        let transcript =
            clip(self.transcript, MAX_TRANSCRIPT_CHARS).and_then(|t| clean_transcript(&t));
        Self {
            title: clip(self.title, 300),
            description: clip(self.description, MAX_DESCRIPTION_CHARS),
            author: clip(self.author, 200),
            thumbnail: youtube_image(self.thumbnail),
            duration: self.duration.filter(|d| d.is_finite() && *d > 0.0),
            transcript,
        }
    }

    fn meta(&self, url: &str) -> VideoMeta {
        VideoMeta {
            url: url.to_string(),
            title: self.title.clone(),
            caption: self.description.clone().unwrap_or_default(),
            author: self.author.clone(),
            thumbnail: self.thumbnail.clone(),
            duration: self.duration,
            subtitles: false,
        }
    }
}

type BrowserKey = (crate::households::HouseholdId, String);

static FROM_BROWSER: LazyLock<Mutex<HashMap<BrowserKey, (Instant, FromBrowser)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Keeps what the browser read for `url` until its import job runs ([`import`] takes it).
pub fn offer(state: &AppState, url: &str, read: FromBrowser) {
    let mut map = FROM_BROWSER.lock().unwrap_or_else(|e| e.into_inner());
    map.retain(|_, (at, _)| at.elapsed() < KEEP_FROM_BROWSER);
    while map.len() >= MAX_KEPT_FROM_BROWSER {
        let Some(oldest) = map
            .iter()
            .min_by_key(|(_, (at, _))| *at)
            .map(|(k, _)| k.clone())
        else {
            break;
        };
        map.remove(&oldest);
    }
    map.insert(
        (state.household, url.to_string()),
        (Instant::now(), read.cleaned()),
    );
}

fn take_from_browser(state: &AppState, url: &str) -> Option<FromBrowser> {
    let mut map = FROM_BROWSER.lock().unwrap_or_else(|e| e.into_inner());
    map.retain(|_, (at, _)| at.elapsed() < KEEP_FROM_BROWSER);
    map.remove(&(state.household, url.to_string()))
        .map(|(_, read)| read)
}

/// What a video's page says about it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VideoMeta {
    /// The video's own address (a share link resolved), for the recipe and deduplication.
    pub url: String,
    pub title: Option<String>,
    pub caption: String,
    pub author: Option<String>,
    pub thumbnail: Option<String>,
    pub duration: Option<f64>,
    /// Whether the video has English subtitles or automatic captions to read instead of
    /// transcribing it.
    pub subtitles: bool,
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// Reads `yt-dlp --dump-single-json`.
pub fn meta_from_ytdlp(json: &Value, asked: &str) -> VideoMeta {
    let caption = text(json, "description").unwrap_or_default();
    // TikTok's "title" is the start of the caption; YouTube's is a real title
    // yt-dlp's stand-in title for a video without one ("TikTok video #7412…") is dropped too
    let title = text(json, "title")
        .filter(|t| !caption.starts_with(t.trim_end_matches('…')) && !t.contains(" video #"));
    let author = text(json, "uploader")
        .or_else(|| text(json, "channel"))
        .or_else(|| text(json, "uploader_id"));
    VideoMeta {
        url: text(json, "webpage_url").unwrap_or_else(|| asked.to_string()),
        title,
        caption,
        author,
        thumbnail: text(json, "thumbnail"),
        duration: json.get("duration").and_then(Value::as_f64),
        subtitles: ["subtitles", "automatic_captions"].iter().any(|key| {
            json.get(key)
                .and_then(Value::as_object)
                .is_some_and(|langs| langs.keys().any(|l| is_english(l)))
        }),
    }
}

/// `en`, `en-US`, `en-orig` and the like (not `en-fr`-style translations *from* English).
fn is_english(lang: &str) -> bool {
    lang == "en"
        || lang
            .strip_prefix("en-")
            .is_some_and(|rest| rest == "orig" || rest.chars().all(|c| c.is_ascii_uppercase()))
}

static REHYDRATION: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(r#"script#__UNIVERSAL_DATA_FOR_REHYDRATION__"#).expect("selector")
});

fn meta_tag(doc: &Html, property: &str) -> Option<String> {
    let sel = Selector::parse(&format!(
        r#"meta[property="{property}"], meta[name="{property}"]"#
    ))
    .ok()?;
    doc.select(&sel)
        .find_map(|m| m.value().attr("content"))
        .map(|s| crate::scraper::decode_text(s.trim()))
        .filter(|s| !s.is_empty())
}

/// The caption from a video's page, when `yt-dlp` isn't there or failed: TikTok's page data
/// (the whole caption and the handle), else the Open Graph tags (which may be cut short).
pub fn meta_from_html(html: &str, url: &str) -> Option<VideoMeta> {
    let doc = Html::parse_document(html);
    let mut meta = VideoMeta {
        url: meta_tag(&doc, "og:url")
            .filter(|u| is_video_url(u))
            .unwrap_or_else(|| url.to_string()),
        thumbnail: meta_tag(&doc, "og:image"),
        ..Default::default()
    };
    if let Some(data) = doc
        .select(&REHYDRATION)
        .next()
        .and_then(|s| serde_json::from_str::<Value>(&s.inner_html()).ok())
    {
        let item = data
            .pointer("/__DEFAULT_SCOPE__/webapp.video-detail/itemInfo/itemStruct")
            .cloned()
            .unwrap_or(Value::Null);
        meta.caption = text(&item, "desc").unwrap_or_default();
        meta.author = item
            .pointer("/author/nickname")
            .or_else(|| item.pointer("/author/uniqueId"))
            .and_then(Value::as_str)
            .map(String::from);
        meta.duration = item.pointer("/video/duration").and_then(Value::as_f64);
        if let Some(cover) = item.pointer("/video/cover").and_then(Value::as_str) {
            meta.thumbnail = Some(cover.to_string());
        }
    }
    if meta.caption.is_empty() {
        meta.caption = meta_tag(&doc, "og:description")
            .or_else(|| meta_tag(&doc, "description"))
            .unwrap_or_default();
        meta.title = meta_tag(&doc, "og:title");
    }
    (!meta.caption.is_empty() || meta.title.is_some()).then_some(meta)
}

static QUANTITY_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?im)^\s*(?:[-•*▪️·✅🔸🔹]\s*)?(?:\d+[\d/.,]*|[½⅓⅔¼¾⅛]|a |an |one |two |three |half )\s*(?:[½⅓⅔¼¾⅛])?\s*(?:cups?|c\b|tbsp|tablespoons?|tsp|teaspoons?|g\b|grams?|kg|ml|l\b|litres?|liters?|oz|ounces?|lbs?|pounds?|cloves?|pinch|handful|cans?|sticks?|slices?|pieces?|large|medium|small|whole|[a-z])",
    )
    .unwrap()
});

/// Whether a caption is already the whole recipe: several measured ingredients and
/// at least two steps, so there's no need to download and watch the video.
pub fn caption_has_recipe(caption: &str) -> bool {
    let measured = QUANTITY_LINE.find_iter(caption).count();
    if measured < 3 {
        return false;
    }
    let parsed = crate::text_parser::parse_recipe_text(caption).recipe;
    let count = |s: &[crate::model::Section]| s.iter().map(|s| s.items.len()).sum::<usize>();
    count(&parsed.ingredients) >= 3 && count(&parsed.instructions) >= 2
}

/// Saves the recipe in a cooking video (see the module docs for how it's read).
pub async fn import(state: &AppState, url: &str) -> AppResult<(Recipe, bool)> {
    let started = Instant::now();
    let tools = &state.config.video;
    // The cook's browser already read the page: the site isn't asked again (it may refuse)
    let from_browser = take_from_browser(state, url);
    let hint = if from_browser.is_none() && is_youtube(url) {
        YOUTUBE_HINT
    } else {
        PASTE_HINT
    };
    let meta = match &from_browser {
        Some(read) => read.meta(url),
        None => metadata(state, url)
            .await
            .ok_or_else(|| AppError::new(422, format!("Couldn't open that video. {hint}")))?,
    };
    let said_in_browser = from_browser.and_then(|read| read.transcript);
    // A different share link to a video that's already saved
    if meta.url != url {
        let conn = state.db.lock();
        if let Some(existing) = crate::recipes::find_by_url(&conn, &meta.url)? {
            return Ok((crate::recipes::require_recipe(&conn, existing)?, false));
        }
    }

    let ai = crate::llm::available(state);
    let caption = caption_text(&meta);
    // 1. The caption, when it's the whole recipe
    let read_caption = caption_has_recipe(&caption);
    let mut from_caption = None;
    if read_caption {
        from_caption = if ai {
            crate::llm::extract_recipe(state, &caption).await
        } else {
            Some(crate::text_parser::parse_recipe_text(&caption).recipe)
        };
    }
    let mut how = "caption";
    let mut fields = from_caption.clone().filter(has_both);
    // 2. The captions the browser read, when there are any: no download needed
    if fields.is_none()
        && ai
        && let Some(said) = &said_in_browser
    {
        let text = video_prompt(&meta, Some(said), false);
        fields = crate::llm::extract_recipe_from_video(state, &[], &text).await;
        how = "captions";
    }
    // 3 and 4. What's said and what's shown
    let mut frames = Vec::new();
    if fields.is_none() && ai && tools.yt_dlp.is_some() && tools.ffmpeg.is_some() {
        if meta.duration.is_some_and(|d| d > MAX_SECONDS) {
            tracing::info!("[video] too long to watch ({:?}s)", meta.duration);
        } else if let Some(watched) = watch(state, &meta).await {
            let text = video_prompt(&meta, watched.transcript.as_deref(), true);
            fields = crate::llm::extract_recipe_from_video(state, &watched.frames, &text).await;
            frames = watched.frames;
            how = "video";
        }
    }
    // Couldn't watch it: better a part of the recipe from the caption than none
    if fields.is_none() {
        if !read_caption && ai && !caption.trim().is_empty() {
            from_caption = crate::llm::extract_recipe(state, &caption).await;
        }
        fields = from_caption;
        how = "caption";
    }
    tracing::info!(
        "[video] {}: read from the {how} in {} ms",
        crate::telemetry::host_of(url),
        started.elapsed().as_millis()
    );

    let Some(mut fields) =
        fields.filter(|f| !f.ingredients.is_empty() || !f.instructions.is_empty())
    else {
        let why = if ai {
            "Couldn't find a recipe in that video."
        } else {
            "That video's caption doesn't have the recipe, and Wee Chef isn't set up to watch it."
        };
        return Err(AppError::new(422, format!("{why} {hint}")));
    };
    fields.url = Some(meta.url.clone());
    // The recipe's own video plays on its page
    if fields.video.is_none() {
        fields.video = crumb_core::embed::video_link(&meta.url);
    }
    if fields.author.as_deref().is_none_or(|a| a.trim().is_empty()) {
        fields.author = meta.author.clone();
    }
    if fields.title.trim().is_empty() || fields.title == "Untitled recipe" {
        fields.title = meta
            .title
            .clone()
            .unwrap_or_else(|| "Recipe from a video".into());
    }
    fields.image = photo(state, &meta, &frames).await;
    crate::recipes::create_checked(state, fields, "video")
}

fn has_both(f: &RecipeFields) -> bool {
    !f.ingredients.is_empty() && !f.instructions.is_empty()
}

/// The caption as Wee Chef (or the text parser) reads it, with the title when there is one.
fn caption_text(meta: &VideoMeta) -> String {
    match &meta.title {
        Some(title) if !meta.caption.is_empty() => format!("{title}\n\n{}", meta.caption),
        Some(title) => title.clone(),
        None => meta.caption.clone(),
    }
}

/// What Wee Chef is told alongside the frames.
pub fn video_prompt(meta: &VideoMeta, transcript: Option<&str>, frames: bool) -> String {
    let mut out = String::from("A cooking video");
    if let Some(author) = &meta.author {
        out.push_str(&format!(" by {author}"));
    }
    out.push_str(".\n\n");
    if let Some(title) = &meta.title {
        out.push_str(&format!("Title: {title}\n\n"));
    }
    let caption = meta.caption.trim();
    out.push_str("Caption:\n");
    out.push_str(if caption.is_empty() {
        "(none)"
    } else {
        caption
    });
    out.push_str("\n\nWhat the cook says (automatic transcript, which may mishear words):\n");
    out.push_str(transcript.unwrap_or("(no speech)"));
    if frames {
        out.push_str(
            "\n\nThe frames above are stills from the video, in order. Read any on-screen text in them (ingredient lists, amounts, temperatures, times).",
        );
    }
    out
}

/// The video's details: `yt-dlp` when it's there, else the page itself.
async fn metadata(state: &AppState, url: &str) -> Option<VideoMeta> {
    if let Some(yt_dlp) = &state.config.video.yt_dlp {
        let out = run(
            Command::new(yt_dlp).args([
                "--dump-single-json",
                "--skip-download",
                "--no-playlist",
                "--no-warnings",
                "--",
                url,
            ]),
            METADATA_TIMEOUT,
        )
        .await;
        match out.and_then(|b| serde_json::from_slice::<Value>(&b).map_err(|e| e.to_string())) {
            Ok(json) => return Some(meta_from_ytdlp(&json, url)),
            Err(err) => tracing::warn!(
                "[video] {}: yt-dlp couldn't read it: {err}",
                crate::telemetry::host_of(url)
            ),
        }
    }
    use crate::scraper::{Fetched, Method, fetch_wreq};
    if let Fetched::Page { html, .. } = fetch_wreq(Method::Firefox, url).await
        && let Some(meta) = meta_from_html(&html, url)
    {
        return Some(meta);
    }
    if state.browser.available()
        && let Ok(html) = state.browser.fetch(url).await
    {
        return meta_from_html(&html, url);
    }
    None
}

/// What watching a video gave.
struct Watched {
    transcript: Option<String>,
    frames: Vec<Photo>,
}

/// Downloads the video, transcribes it and takes its frames. None when it can't be had.
async fn watch(state: &AppState, meta: &VideoMeta) -> Option<Watched> {
    let tools = &state.config.video;
    let (yt_dlp, ffmpeg) = (tools.yt_dlp.as_ref()?, tools.ffmpeg.as_ref()?);
    // The heavy part holds one of the workers' permits, which headless Chromium shares
    let _turn = state.video_jobs.heavy.acquire().await.ok()?;
    let threads = Threads::for_config(&state.config);
    let dir = tempfile::tempdir().ok()?;
    let host = crate::telemetry::host_of(&meta.url);

    let started = Instant::now();
    // Subtitles, when there are any, say it better than a transcript and cost nothing to run
    let subtitles = if meta.subtitles {
        read_subtitles(yt_dlp, &meta.url, dir.path()).await
    } else {
        None
    };
    let template = dir.path().join("video.%(ext)s");
    if let Err(err) = run(
        Command::new(yt_dlp)
            .args([
                "--quiet",
                "--no-playlist",
                "--no-warnings",
                "--no-part",
                "--max-filesize",
                MAX_DOWNLOAD,
                "-f",
                "b[height<=720][ext=mp4]/b[height<=720]/b",
                "-o",
            ])
            .arg(&template)
            .args(["--", &meta.url]),
        DOWNLOAD_TIMEOUT,
    )
    .await
    {
        tracing::warn!("[video] {host}: download failed: {err}");
        return None;
    }
    let video = downloaded(dir.path())?;
    let downloaded_ms = started.elapsed().as_millis();

    let subtitles_read = subtitles.is_some();
    let transcript = match (subtitles, &tools.whisper) {
        (Some(said), _) => Some(said),
        (None, Some((cli, model))) => {
            transcribe(ffmpeg, cli, model, &video, dir.path(), threads).await
        }
        (None, None) => None,
    };
    let transcribed_ms = started.elapsed().as_millis();
    let frames = frames(ffmpeg, &video, dir.path(), meta.duration, threads).await;
    tracing::info!(
        "[video] {host}: downloaded in {downloaded_ms} ms, {} words {} by {transcribed_ms} ms, {} frames by {} ms",
        transcript
            .as_deref()
            .map_or(0, |t| t.split_whitespace().count()),
        if subtitles_read {
            "from subtitles"
        } else {
            "heard"
        },
        frames.len(),
        started.elapsed().as_millis()
    );
    Some(Watched { transcript, frames })
}

/// The video's English subtitles (its own, else YouTube's automatic captions) as one
/// paragraph, without downloading the video. None when there are none or yt-dlp fails.
async fn read_subtitles(yt_dlp: &Path, url: &str, dir: &Path) -> Option<String> {
    let template = dir.join("subs.%(ext)s");
    run(
        Command::new(yt_dlp)
            .args([
                "--quiet",
                "--no-playlist",
                "--no-warnings",
                "--skip-download",
                "--write-subs",
                "--write-auto-subs",
                "--sub-langs",
                "en,en-orig,en-[A-Z]*",
                "--sub-format",
                "vtt",
                "-o",
            ])
            .arg(&template)
            .args(["--", url]),
        SUBTITLES_TIMEOUT,
    )
    .await
    .inspect_err(|err| tracing::info!("[video] no subtitles: {err}"))
    .ok()?;
    // subs.en.vtt, subs.en-orig.vtt, ...: the video's own language first
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "vtt")
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("subs."))
        })
        .collect();
    files.sort_by_key(|p| {
        let name = p.to_string_lossy();
        (!name.contains("-orig."), name.len())
    });
    let text = std::fs::read_to_string(files.first()?).ok()?;
    let mut said = clean_transcript(&vtt_text(&text))?;
    if said.len() > MAX_TRANSCRIPT_CHARS {
        let cut = said.floor_char_boundary(MAX_TRANSCRIPT_CHARS);
        said.truncate(cut);
    }
    Some(said)
}

static VTT_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());

/// The words of a WebVTT file. YouTube's automatic captions roll: each cue repeats the line
/// before it, so a line the same as the last one kept is dropped.
pub fn vtt_text(vtt: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut in_note = false;
    for raw in vtt.lines() {
        let line = raw.trim();
        if line.is_empty() {
            in_note = false;
            continue;
        }
        if in_note
            || line.starts_with("WEBVTT")
            || line.starts_with("Kind:")
            || line.starts_with("Language:")
            || line.contains("-->")
            || line.chars().all(|c| c.is_ascii_digit())
        {
            continue;
        }
        if line.starts_with("NOTE") || line.starts_with("STYLE") || line.starts_with("REGION") {
            in_note = true;
            continue;
        }
        let words = crate::scraper::decode_text(&VTT_TAG.replace_all(line, ""));
        let words = words.split_whitespace().collect::<Vec<_>>().join(" ");
        if !words.is_empty() && lines.last() != Some(&words) {
            lines.push(words);
        }
    }
    lines.join(" ")
}

/// The CPU a job may use: the cores split between the workers, so jobs running together
/// share the machine rather than oversubscribe it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Threads {
    pub ffmpeg: usize,
    pub whisper: usize,
}

impl Threads {
    pub fn for_config(config: &crate::config::Config) -> Self {
        let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
        Self::split(cores, config.video_workers, config.whisper_threads)
    }

    /// `cores` shared by `workers`; `whisper` (`WHISPER_THREADS`) overrides whisper's share.
    pub fn split(cores: usize, workers: usize, whisper: Option<usize>) -> Self {
        let share = (cores / workers.max(1)).max(1);
        Self {
            ffmpeg: share,
            whisper: whisper.unwrap_or(share.min(8)).max(1),
        }
    }
}

/// The file yt-dlp wrote (its extension depends on the format it picked).
fn downloaded(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| {
            p.file_stem().is_some_and(|s| s == "video")
                && p.extension().is_some_and(|e| e != "json" && e != "part")
        })
}

/// What the cook says, from local whisper.cpp. None for a silent video or a failure.
async fn transcribe(
    ffmpeg: &Path,
    whisper: &Path,
    model: &Path,
    video: &Path,
    dir: &Path,
    threads: Threads,
) -> Option<String> {
    let wav = dir.join("audio.wav");
    run(
        Command::new(ffmpeg)
            .args(["-nostdin", "-v", "error", "-threads"])
            .arg(threads.ffmpeg.to_string())
            .arg("-i")
            .arg(video)
            .args(["-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le", "-t"])
            .arg(MAX_AUDIO_SECONDS.to_string())
            .arg(&wav),
        FFMPEG_TIMEOUT,
    )
    .await
    .inspect_err(|err| tracing::info!("[video] no audio: {err}"))
    .ok()?;
    let english_only = model
        .file_name()
        .is_some_and(|n| n.to_string_lossy().contains(".en."));
    let out = run(
        Command::new(whisper)
            .arg("-m")
            .arg(model)
            .arg("-f")
            .arg(&wav)
            .args(["-nt", "-np", "-t", &threads.whisper.to_string(), "-l"])
            .arg(if english_only { "en" } else { "auto" }),
        WHISPER_TIMEOUT,
    )
    .await
    .inspect_err(|err| tracing::warn!("[video] whisper failed: {err}"))
    .ok()?;
    clean_transcript(&String::from_utf8_lossy(&out))
}

static NOISE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[[^\]]*\]|\([^)]*\)|♪+|\*[^*]*\*").unwrap());

/// Whisper's output without its sound labels ("[BLANK_AUDIO]", "(upbeat music)", "♪"),
/// as one paragraph. None when nothing was said.
pub fn clean_transcript(raw: &str) -> Option<String> {
    let text = NOISE.replace_all(raw, " ");
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (text.chars().filter(|c| c.is_alphabetic()).count() >= 12).then_some(text)
}

/// Stills from across the video, near-duplicates dropped, at most [`MAX_FRAMES`].
async fn frames(
    ffmpeg: &Path,
    video: &Path,
    dir: &Path,
    duration: Option<f64>,
    threads: Threads,
) -> Vec<Photo> {
    let frames_dir = dir.join("frames");
    if std::fs::create_dir(&frames_dir).is_err() {
        return Vec::new();
    }
    // One still every couple of seconds, spread over longer videos
    let every = duration.map_or(2.0, |d| (d / f64::from(SAMPLED_FRAMES)).max(1.5));
    let filter = format!("fps=1/{every:.2},scale='min({FRAME_WIDTH},iw)':-2");
    let result = run(
        Command::new(ffmpeg)
            .args(["-nostdin", "-v", "error", "-threads"])
            .arg(threads.ffmpeg.to_string())
            .arg("-i")
            .arg(video)
            .args(["-threads"])
            .arg(threads.ffmpeg.to_string())
            .args(["-vf", &filter, "-frames:v"])
            .arg(SAMPLED_FRAMES.to_string())
            .args(["-q:v", "4"])
            .arg(frames_dir.join("f%03d.jpg")),
        FFMPEG_TIMEOUT,
    )
    .await;
    if let Err(err) = result {
        tracing::warn!("[video] couldn't take frames: {err}");
        return Vec::new();
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&frames_dir)
        .map(|d| d.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    paths.sort();
    let stills: Vec<Vec<u8>> = paths.iter().filter_map(|p| std::fs::read(p).ok()).collect();
    tokio::task::spawn_blocking(move || pick_frames(stills, MAX_FRAMES))
        .await
        .unwrap_or_default()
}

/// A 32×32 grey thumbnail, for telling frames apart.
fn thumb(jpeg: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(jpeg).ok()?;
    Some(
        img.resize_exact(32, 32, image::imageops::FilterType::Triangle)
            .to_luma8()
            .into_raw(),
    )
}

fn difference(a: &[u8], b: &[u8]) -> f64 {
    let total: u64 = a
        .iter()
        .zip(b)
        .map(|(x, y)| u64::from(x.abs_diff(*y)))
        .sum();
    total as f64 / a.len().max(1) as f64
}

/// Drops frames that look like the one before, then keeps at most `max`, evenly spread.
pub fn pick_frames(stills: Vec<Vec<u8>>, max: usize) -> Vec<Photo> {
    let mut kept: Vec<Vec<u8>> = Vec::new();
    let mut last: Option<Vec<u8>> = None;
    for jpeg in stills {
        let Some(t) = thumb(&jpeg) else { continue };
        if last
            .as_ref()
            .is_some_and(|l| difference(l, &t) < SAME_FRAME)
        {
            continue;
        }
        last = Some(t);
        kept.push(jpeg);
    }
    let n = kept.len();
    let chosen: Vec<Vec<u8>> = if n <= max {
        kept
    } else {
        let picks: Vec<usize> = (0..max).map(|i| i * (n - 1) / (max - 1).max(1)).collect();
        kept.into_iter()
            .enumerate()
            .filter(|(i, _)| picks.contains(i))
            .map(|(_, f)| f)
            .collect()
    };
    chosen
        .into_iter()
        .map(|bytes| Photo {
            media_type: "image/jpeg",
            bytes,
        })
        .collect()
}

/// The recipe's photo, kept in the recipe (video sites' thumbnail links expire): the
/// video's cover, else a still from the end, where the dish is usually shown.
async fn photo(state: &AppState, meta: &VideoMeta, frames: &[Photo]) -> Option<String> {
    if let Some(thumbnail) = &meta.thumbnail
        && let Some(kept) = crate::images::fetch_to_embed(&state.http, thumbnail).await
    {
        return Some(kept);
    }
    frames.last().and_then(|f| crate::images::embed(&f.bytes))
}

/// Runs a program to completion within `timeout`; its stdout, or why it failed.
async fn run(command: &mut Command, timeout: Duration) -> Result<Vec<u8>, String> {
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("couldn't start: {e}"))?;
    let out = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| "timed out".to_string())?
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(out.stdout);
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    let last = stderr.lines().rev().find(|l| !l.trim().is_empty());
    Err(format!(
        "{}: {}",
        out.status,
        last.unwrap_or("").chars().take(300).collect::<String>()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn knows_video_links() {
        for yes in [
            "https://vt.tiktok.com/ZSb2DYyNb/",
            "https://vm.tiktok.com/ZMabc123/",
            "https://www.tiktok.com/@chef/video/7412345678901234567?_r=1",
            "https://m.tiktok.com/v/7412345678901234567.html",
            "https://www.instagram.com/reel/C9abcDEF/",
            "https://instagram.com/reels/C9abcDEF/",
            "https://www.youtube.com/shorts/dQw4w9WgXcQ",
            "https://www.youtube.com/watch?v=Xy_djhH3WE4&t=122s",
            "https://m.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ?si=abc",
            "https://www.youtube.com/live/dQw4w9WgXcQ",
        ] {
            assert!(is_video_url(yes), "{yes}");
        }
        for no in [
            "https://www.tiktok.com/",
            "https://www.instagram.com/p/C9abcDEF/",
            "https://www.youtube.com/watch",
            "https://www.youtube.com/@chef",
            "https://www.youtube.com/shorts/",
            "https://youtu.be/",
            "https://www.allrecipes.com/recipe/1/tiktok-pasta/",
            "https://nottiktok.com/@a/video/1",
            "not a url",
        ] {
            assert!(!is_video_url(no), "{no}");
        }
    }

    #[test]
    fn reads_ytdlp_json() {
        let tiktok = json!({
            "title": "Feta pasta that broke the internet…",
            "description": "Feta pasta that broke the internet 🍝 #pasta",
            "uploader": "chefjo",
            "thumbnail": "https://p16.tiktokcdn.com/cover.jpg",
            "duration": 58,
            "webpage_url": "https://www.tiktok.com/@chefjo/video/7412345678901234567"
        });
        let m = meta_from_ytdlp(&tiktok, "https://vt.tiktok.com/x/");
        assert_eq!(
            m.url,
            "https://www.tiktok.com/@chefjo/video/7412345678901234567"
        );
        assert_eq!(m.title, None, "TikTok's title is only the caption again");
        assert_eq!(m.author.as_deref(), Some("chefjo"));
        assert_eq!(m.duration, Some(58.0));

        let bare = json!({"title": "TikTok video #7412345678901234567", "description": ""});
        assert_eq!(
            meta_from_ytdlp(&bare, "https://vt.tiktok.com/x/").title,
            None
        );

        let short = json!({"title": "5 minute noodles", "description": "Recipe below", "channel": "Noodle Co"});
        let m = meta_from_ytdlp(&short, "https://www.youtube.com/shorts/abc");
        assert_eq!(m.url, "https://www.youtube.com/shorts/abc");
        assert_eq!(m.title.as_deref(), Some("5 minute noodles"));
        assert_eq!(m.author.as_deref(), Some("Noodle Co"));
        assert!(!m.subtitles);

        let captioned =
            json!({"title": "Pad thai", "automatic_captions": {"de": [], "en-orig": []}});
        assert!(meta_from_ytdlp(&captioned, "https://youtu.be/x").subtitles);
        let foreign = json!({"subtitles": {"fr": []}, "automatic_captions": {"en-fr": []}});
        assert!(!meta_from_ytdlp(&foreign, "https://youtu.be/x").subtitles);
    }

    #[test]
    fn reads_rolling_captions() {
        let vtt = "WEBVTT\nKind: captions\nLanguage: en\n\n\
            00:00:00.160 --> 00:00:02.869 align:start position:0%\n\
            \n\
            so<00:00:00.480><c> today</c><00:00:00.960><c> we're</c><c> making</c>\n\n\
            00:00:02.869 --> 00:00:02.879 align:start position:0%\n\
            so today we're making\n \n\n\
            00:00:02.879 --> 00:00:05.000 align:start position:0%\n\
            so today we're making\n\
            pad<00:00:03.100><c> thai</c> &amp; rice\n\n\
            2\n00:00:05.000 --> 00:00:06.000\n[Music]\n";
        assert_eq!(
            vtt_text(vtt),
            "so today we're making pad thai & rice [Music]"
        );
    }

    #[test]
    fn reads_tiktok_page_data() {
        let data = json!({"__DEFAULT_SCOPE__": {"webapp.video-detail": {"itemInfo": {"itemStruct": {
            "desc": "Crispy rice salad\nIngredients:\n2 cups rice",
            "author": {"uniqueId": "chefjo", "nickname": "Chef Jo"},
            "video": {"duration": 61, "cover": "https://p16.tiktokcdn.com/c.jpg"}
        }}}}});
        let html = format!(
            r#"<html><head><meta property="og:description" content="cut short…"></head><body>
            <script id="__UNIVERSAL_DATA_FOR_REHYDRATION__" type="application/json">{data}</script></body></html>"#
        );
        let m = meta_from_html(&html, "https://www.tiktok.com/@chefjo/video/1").unwrap();
        assert!(m.caption.starts_with("Crispy rice salad\nIngredients:"));
        assert_eq!(m.author.as_deref(), Some("Chef Jo"));
        assert_eq!(m.duration, Some(61.0));
        assert_eq!(
            m.thumbnail.as_deref(),
            Some("https://p16.tiktokcdn.com/c.jpg")
        );
    }

    #[test]
    fn falls_back_to_open_graph() {
        let html = r#"<html><head>
            <meta property="og:title" content="Chef Jo on TikTok">
            <meta property="og:description" content="Best garlic bread &amp; butter">
            <meta property="og:image" content="https://x.example/c.jpg"></head></html>"#;
        let m = meta_from_html(html, "https://www.tiktok.com/@chefjo/video/1").unwrap();
        assert_eq!(m.caption, "Best garlic bread & butter");
        assert_eq!(m.title.as_deref(), Some("Chef Jo on TikTok"));
        assert!(meta_from_html("<html></html>", "https://www.tiktok.com/@a/video/1").is_none());
    }

    #[test]
    fn knows_a_caption_that_is_the_recipe() {
        let full = "Garlic butter noodles 🍜\n\nIngredients:\n- 200g spaghetti\n- 3 tbsp butter\n- 4 cloves garlic, minced\n- 1/2 cup parmesan\n\nMethod:\n1. Boil the spaghetti until al dente.\n2. Melt the butter and fry the garlic for 1 minute.\n3. Toss everything together with the parmesan.\n\n#pasta #easyrecipe";
        assert!(caption_has_recipe(full));
        assert!(!caption_has_recipe(
            "The BEST garlic noodles you'll ever make 🤤 full recipe on my blog! #pasta #fyp"
        ));
        // Only the ingredients: the steps are in the video
        assert!(!caption_has_recipe(
            "Ingredients:\n- 200g spaghetti\n- 3 tbsp butter\n- 4 cloves garlic\n#pasta"
        ));
    }

    #[test]
    fn cleans_whisper_output() {
        let raw = " [BLANK_AUDIO]\n (upbeat music)\n So today we're making garlic noodles.\n ♪ ♪\n First, two tablespoons of butter.\n";
        assert_eq!(
            clean_transcript(raw).as_deref(),
            Some("So today we're making garlic noodles. First, two tablespoons of butter.")
        );
        assert_eq!(clean_transcript("[MUSIC PLAYING]\n(sizzling)\n"), None);
    }

    fn jpeg(shade: u8, stripe: bool) -> Vec<u8> {
        let img = image::RgbImage::from_fn(64, 64, |x, _| {
            let v = if stripe && x < 32 { 255 - shade } else { shade };
            image::Rgb([v, v, v])
        });
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90)
            .encode_image(&img)
            .unwrap();
        out
    }

    #[test]
    fn drops_repeated_frames_and_spreads_the_rest() {
        let stills = vec![
            jpeg(20, false),
            jpeg(21, false),
            jpeg(20, true),
            jpeg(20, true),
            jpeg(200, false),
        ];
        assert_eq!(pick_frames(stills, 12).len(), 3);

        let many: Vec<Vec<u8>> = (0..30).map(|i| jpeg((i * 8) as u8, i % 2 == 0)).collect();
        let picked = pick_frames(many, 12);
        assert_eq!(picked.len(), 12);
        assert!(picked.iter().all(|p| p.media_type == "image/jpeg"));
        assert!(pick_frames(vec![b"not a jpeg".to_vec()], 12).is_empty());
    }

    #[test]
    fn splits_the_cores_between_workers() {
        assert_eq!(
            Threads::split(8, 1, None),
            Threads {
                ffmpeg: 8,
                whisper: 8
            }
        );
        assert_eq!(
            Threads::split(8, 2, None),
            Threads {
                ffmpeg: 4,
                whisper: 4
            }
        );
        assert_eq!(
            Threads::split(2, 4, None),
            Threads {
                ffmpeg: 1,
                whisper: 1
            }
        );
        assert_eq!(
            Threads::split(32, 1, None).whisper,
            8,
            "whisper gains little past 8"
        );
        assert_eq!(Threads::split(8, 2, Some(3)).whisper, 3);
    }

    #[test]
    fn keeps_what_the_browser_read_in_bounds() {
        let read = FromBrowser {
            title: Some("  Pad thai  ".into()),
            description: Some("x".repeat(MAX_DESCRIPTION_CHARS + 50)),
            author: Some(String::new()),
            thumbnail: Some("https://i.ytimg.com/vi/abc/maxresdefault.jpg".into()),
            duration: Some(f64::NAN),
            transcript: Some("so today we're making [Music] pad thai with rice noodles".into()),
        }
        .cleaned();
        assert_eq!(read.title.as_deref(), Some("Pad thai"));
        assert_eq!(
            read.description.unwrap().chars().count(),
            MAX_DESCRIPTION_CHARS
        );
        assert_eq!(read.author, None);
        assert_eq!(read.duration, None);
        assert_eq!(
            read.transcript.as_deref(),
            Some("so today we're making pad thai with rice noodles")
        );
        assert!(read.thumbnail.is_some());
        for not_youtube in [
            "http://i.ytimg.com/vi/abc/0.jpg",
            "https://ytimg.com.evil.test/0.jpg",
            "http://127.0.0.1:3000/api/recipes",
        ] {
            assert_eq!(
                youtube_image(Some(not_youtube.into())),
                None,
                "{not_youtube}"
            );
        }
        assert!(youtube_image(Some("https://yt3.ggpht.com/a/b=s88".into())).is_some());
    }

    #[test]
    fn knows_youtube() {
        assert!(is_youtube(
            "https://www.youtube.com/watch?v=Xy_djhH3WE4&t=122s"
        ));
        assert!(is_youtube("https://youtu.be/Xy_djhH3WE4"));
        assert!(!is_youtube("https://www.tiktok.com/@a/video/1"));
    }

    #[test]
    fn tells_wee_chef_what_it_has() {
        let meta = VideoMeta {
            author: Some("Chef Jo".into()),
            caption: "Noodles!".into(),
            ..Default::default()
        };
        let p = video_prompt(&meta, None, true);
        assert!(p.starts_with("A cooking video by Chef Jo."));
        assert!(p.contains("Caption:\nNoodles!"));
        assert!(p.contains("(no speech)"));
        assert!(p.contains("The frames above"));
        let p = video_prompt(&meta, Some("hello there"), false);
        assert!(!p.contains("frames"), "no stills were sent");
    }
}
