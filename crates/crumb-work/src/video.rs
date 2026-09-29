//! The heavy half of reading a cooking video, on the server or on a `crumb-relay` working for
//! it: the video's details from `yt-dlp`, and watching it: its subtitles, else a local
//! `whisper.cpp` transcript, and a handful of stills (`ffmpeg`). What the recipe is (Wee
//! Chef, whose keys stay on the server) is the server's `video.rs`.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;

/// Longest video that is downloaded; a longer one is only read from its caption.
pub const MAX_SECONDS: f64 = 20.0 * 60.0;
/// Speech after this is not transcribed (a recipe is said well before then).
const MAX_AUDIO_SECONDS: u32 = 15 * 60;
/// Largest download yt-dlp is allowed.
const MAX_DOWNLOAD: &str = "250M";
/// Frames sent to Wee Chef, at most.
pub const MAX_FRAMES: usize = 12;
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
pub const MAX_TRANSCRIPT_CHARS: usize = 30_000;
const FFMPEG_TIMEOUT: Duration = Duration::from_secs(120);
const WHISPER_TIMEOUT: Duration = Duration::from_secs(420);

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

/// What a video's page says about it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
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

/// A JSON string field, trimmed; None when missing or blank.
pub fn text(v: &Value, key: &str) -> Option<String> {
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

/// The video's details from `yt-dlp --dump-single-json` (no download).
pub async fn read_meta(yt_dlp: &Path, url: &str) -> Result<VideoMeta, String> {
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
    .await?;
    let json = serde_json::from_slice::<Value>(&out).map_err(|e| e.to_string())?;
    Ok(meta_from_ytdlp(&json, url))
}

/// What watching a video gave: what's said, and stills (JPEG) in order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Watched {
    pub transcript: Option<String>,
    pub frames: Vec<Vec<u8>>,
}

/// Downloads the video at `meta.url`, reads its subtitles or transcribes it, and takes its
/// frames. None when it can't be had. The caller holds whatever limits how many run at once.
pub async fn watch(tools: &VideoTools, meta: &VideoMeta, threads: Threads) -> Option<Watched> {
    let (yt_dlp, ffmpeg) = (tools.yt_dlp.as_ref()?, tools.ffmpeg.as_ref()?);
    if meta.duration.is_some_and(|d| d > MAX_SECONDS) {
        tracing::info!("[video] too long to watch ({:?}s)", meta.duration);
        return None;
    }
    let dir = tempfile::tempdir().ok()?;
    let host = host_of(&meta.url);

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

/// A link's host without `www.`, for the log (never the path).
fn host_of(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.host_str()
                .map(|h| h.trim_start_matches("www.").to_string())
        })
        .unwrap_or_default()
}

/// `&amp;`, `&lt;`, `&#39;` and the like, as in subtitles.
fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    static ENTITY: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"&(#[0-9]+|#[xX][0-9a-fA-F]+|[a-zA-Z]+);").unwrap());
    ENTITY
        .replace_all(text, |c: &regex::Captures| {
            let name = &c[1];
            let decoded = if let Some(hex) = name.strip_prefix("#x").or(name.strip_prefix("#X")) {
                u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
            } else if let Some(dec) = name.strip_prefix('#') {
                dec.parse().ok().and_then(char::from_u32)
            } else {
                match name {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "nbsp" => Some(' '),
                    _ => None,
                }
            };
            decoded.map_or_else(|| c[0].to_string(), String::from)
        })
        .into_owned()
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
        let words = decode_entities(&VTT_TAG.replace_all(line, ""));
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
    /// This machine's cores shared by `workers` jobs at once.
    pub fn for_workers(workers: usize, whisper: Option<usize>) -> Self {
        let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
        Self::split(cores, workers, whisper)
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
) -> Vec<Vec<u8>> {
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
pub fn pick_frames(stills: Vec<Vec<u8>>, max: usize) -> Vec<Vec<u8>> {
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
