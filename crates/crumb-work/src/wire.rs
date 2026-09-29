//! What the server asks a `crumb-relay` that works for it, beside `POST /fetch`
//! (`crumb_fetch::wire`). A relay says what it can do in `GET /health`'s `can`
//! ([`CAN_RENDER`], [`CAN_VIDEO`]): it has Chromium, or `yt-dlp` and `ffmpeg`.
//!
//! Every route takes the same bearer token and answers a non-200 status with
//! `crumb_fetch::wire::ErrorReply`: 400 a refused request or link, 401 the token, 429 busy (try
//! another relay, or the server's own tools), 422 the work was done but gave nothing (Chromium
//! was blocked too, the video couldn't be had), 501 this relay can't do it.
//!
//! | Route | Request | Reply |
//! | --- | --- | --- |
//! | `POST /render` | [`RenderRequest`] | [`RenderReply`]: the page after its scripts ran |
//! | `POST /video/meta` | [`VideoRequest`] | [`VideoMetaReply`]: the video's details |
//! | `POST /video/watch` | [`WatchRequest`] | [`WatchReply`]: what's said and stills |
//!
//! The recipe is always worked out on the server (Wee Chef's keys never leave it); a relay
//! only does what needs a browser, a download or a CPU.

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::video::{VideoMeta, Watched};

/// `GET /health`'s `can` for a relay with Chromium.
pub const CAN_RENDER: &str = "render";
/// `GET /health`'s `can` for a relay with `yt-dlp` and `ffmpeg`.
pub const CAN_VIDEO: &str = "video";
/// Every relay can fetch.
pub const CAN_FETCH: &str = "fetch";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderRequest {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderReply {
    pub html: String,
    pub relay: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoRequest {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoMetaReply {
    pub meta: VideoMeta,
    pub relay: String,
}

/// The video to watch: its details as `/video/meta` (or the server) read them, so its
/// length and subtitles needn't be asked again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatchRequest {
    pub meta: VideoMeta,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchReply {
    pub transcript: Option<String>,
    /// The stills, JPEG, base64.
    pub frames: Vec<String>,
    pub relay: String,
}

/// The most stills a reply may carry, and the largest one (a 720-pixel JPEG is ~100 KB).
pub const MAX_FRAMES: usize = crate::video::MAX_FRAMES;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

impl WatchReply {
    pub fn new(watched: Watched, relay: &str) -> Self {
        let b64 = base64::engine::general_purpose::STANDARD;
        Self {
            transcript: watched.transcript,
            frames: watched.frames.iter().map(|f| b64.encode(f)).collect(),
            relay: relay.to_string(),
        }
    }

    /// Back to what was watched: frames that aren't base64 JPEG, or too many or too big, are
    /// dropped (the relay is trusted with the token, not with the server's memory).
    pub fn watched(self) -> Watched {
        let b64 = base64::engine::general_purpose::STANDARD;
        let frames = self
            .frames
            .iter()
            .take(MAX_FRAMES)
            .filter(|f| f.len() <= MAX_FRAME_BYTES * 4 / 3 + 4)
            .filter_map(|f| b64.decode(f).ok())
            .filter(|f| f.starts_with(&[0xFF, 0xD8, 0xFF]))
            .collect();
        let transcript = self.transcript.map(|mut t| {
            if t.len() > crate::video::MAX_TRANSCRIPT_CHARS {
                t.truncate(t.floor_char_boundary(crate::video::MAX_TRANSCRIPT_CHARS));
            }
            t
        });
        Watched { transcript, frames }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_watch_reply_keeps_only_jpeg_frames_in_bounds() {
        let jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3];
        let reply = WatchReply::new(
            Watched {
                transcript: Some("x".repeat(crate::video::MAX_TRANSCRIPT_CHARS + 10)),
                frames: vec![jpeg.clone(), b"<svg>".to_vec()],
            },
            "pi1",
        );
        let mut tampered = reply.clone();
        tampered.frames.push("not base64!".into());
        let back = tampered.watched();
        assert_eq!(back.frames, vec![jpeg]);
        assert_eq!(
            back.transcript.unwrap().len(),
            crate::video::MAX_TRANSCRIPT_CHARS
        );
        let json = serde_json::to_string(&reply).unwrap();
        assert_eq!(serde_json::from_str::<WatchReply>(&json).unwrap(), reply);
    }
}
