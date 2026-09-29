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
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use regex::Regex;
use scraper::{Html, Selector};
use serde::Deserialize;
use serde_json::Value;

use crumb_work::wire::CAN_VIDEO;

pub use crumb_work::video::{
    MAX_FRAMES, MAX_SECONDS, MAX_TRANSCRIPT_CHARS, Threads, VideoMeta, VideoTools,
    clean_transcript, meta_from_ytdlp, pick_frames, text, vtt_text,
};

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::model::{Recipe, RecipeFields};
use crate::photos::Photo;

const PASTE_HINT: &str = "Try copying the recipe text and pasting it instead.";
/// For a YouTube video the server had to read on its own: YouTube often turns servers away.
const YOUTUBE_HINT: &str = "YouTube often won't let Crumb in on its own: open the video with the Crumb browser extension and choose Read in Crumb, or paste the recipe text instead.";

fn is_youtube(url: &str) -> bool {
    let host = crate::telemetry::host_of(url);
    host == "youtu.be" || host == "youtube.com" || host.ends_with(".youtube.com")
}

pub use crumb_core::source::is_video_url;

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
/// What one household may have waiting, so one box can't push everyone else's out.
const MAX_KEPT_FROM_BROWSER_PER_HOUSEHOLD: usize = 8;

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
    let key = (state.household, url.to_string());
    while !map.contains_key(&key)
        && map.keys().filter(|(h, _)| *h == state.household).count()
            >= MAX_KEPT_FROM_BROWSER_PER_HOUSEHOLD
    {
        let Some(oldest) = map
            .iter()
            .filter(|((h, _), _)| *h == state.household)
            .min_by_key(|(_, (at, _))| *at)
            .map(|(k, _)| k.clone())
        else {
            break;
        };
        map.remove(&oldest);
    }
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
    map.insert(key, (Instant::now(), read.cleaned()));
}

fn take_from_browser(state: &AppState, url: &str) -> Option<FromBrowser> {
    let mut map = FROM_BROWSER.lock().unwrap_or_else(|e| e.into_inner());
    map.retain(|_, (at, _)| at.elapsed() < KEEP_FROM_BROWSER);
    map.remove(&(state.household, url.to_string()))
        .map(|(_, read)| read)
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
    // The cook's browser already read the page: the site isn't asked again (it may refuse)
    let from_browser = take_from_browser(state, url);
    if from_browser.is_none() {
        // Not a private address, whatever yt-dlp or the page fallback would be sent to
        let parsed =
            url::Url::parse(url).map_err(|_| AppError::bad_request("Please enter a valid URL"))?;
        crate::scraper::check_public(state, &parsed).await?;
    }
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
    if let Some(read) = &from_browser {
        tracing::info!(
            "[video] {}: read in the browser: {} characters of description, {} words of captions",
            crate::telemetry::host_of(url),
            read.description.as_deref().map_or(0, |d| d.chars().count()),
            read.transcript
                .as_deref()
                .map_or(0, |t| t.split_whitespace().count())
        );
    }
    let browser_read = from_browser.is_some();
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
    // The extension only reads a video for sites that turn the server away, so a download
    // would be refused too
    if fields.is_none() && ai && !browser_read && can_watch(state).await {
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

/// The video's details: a relay's `yt-dlp` when one works for the server (from a home
/// connection, which video sites turn away less), else this server's, else the page itself.
async fn metadata(state: &AppState, url: &str) -> Option<VideoMeta> {
    let relayed = state.relays.video_meta(url).await;
    let tried = match relayed {
        Ok(meta) => return Some(meta),
        Err(not) => not == crate::relay::NotDone::Nothing,
    };
    // A relay's yt-dlp couldn't read it: this one, from a datacenter, won't either
    if let Some(yt_dlp) = state.config.video.yt_dlp.as_ref().filter(|_| !tried) {
        match crumb_work::video::read_meta(yt_dlp, url).await {
            Ok(meta) => return Some(meta),
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
    if let Some(html) = crate::scraper::render(state, url).await {
        return meta_from_html(&html, url);
    }
    None
}

/// What watching a video gave, with the stills ready for Wee Chef.
struct Watched {
    transcript: Option<String>,
    frames: Vec<Photo>,
}

/// Whether this server, or a relay working for it, can download and watch a video.
async fn can_watch(state: &AppState) -> bool {
    let tools = &state.config.video;
    (tools.yt_dlp.is_some() && tools.ffmpeg.is_some()) || state.relays.can(CAN_VIDEO).await
}

/// Downloads the video, hears it and takes its frames: on a relay that works for the server
/// when there is one (the machine at home does the heavy part), else here. None when it
/// can't be had.
async fn watch(state: &AppState, meta: &VideoMeta) -> Option<Watched> {
    let watched = match state.relays.watch(meta).await {
        Ok(watched) => watched,
        // A relay couldn't get it from a home connection; the server won't from here
        Err(crate::relay::NotDone::Nothing) => return None,
        Err(crate::relay::NotDone::NoWorker) => {
            let tools = &state.config.video;
            if tools.yt_dlp.is_none() || tools.ffmpeg.is_none() {
                return None;
            }
            // The heavy part holds one of the workers' permits, which headless Chromium shares
            let _turn = state.video_jobs.heavy.acquire().await.ok()?;
            let threads =
                Threads::for_workers(state.config.video_workers, state.config.whisper_threads);
            crumb_work::video::watch(tools, meta, threads).await?
        }
    };
    Some(Watched {
        transcript: watched.transcript,
        frames: watched
            .frames
            .into_iter()
            .map(|bytes| Photo {
                media_type: "image/jpeg",
                bytes,
            })
            .collect(),
    })
}

/// The recipe's photo, kept in the recipe (video sites' thumbnail links expire): the
/// video's cover, else a still from the end, where the dish is usually shown.
async fn photo(state: &AppState, meta: &VideoMeta, frames: &[Photo]) -> Option<String> {
    if let Some(thumbnail) = &meta.thumbnail
        && let Some(kept) = crate::images::fetch_to_embed(state, thumbnail).await
    {
        return Some(kept);
    }
    frames.last().and_then(|f| crate::images::embed(&f.bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
