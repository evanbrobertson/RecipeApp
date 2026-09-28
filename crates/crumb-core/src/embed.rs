//! A recipe's video: which site it's on and how a page plays it in place. The recipe keeps
//! one link (`video`); everything here is worked out from it, so every client embeds the
//! same way and an unknown site still gets a plain link.

use serde::{Deserialize, Serialize};

use crate::model::is_valid_url;

/// How to play a recipe's video on its page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoEmbed {
    /// "youtube", "vimeo", "tiktok", "instagram", "dailymotion", "jwplayer" or "file" (a
    /// video file the page plays itself, in a `<video>` element).
    pub provider: String,
    /// The site's name for people: "YouTube", "Vimeo", ... ("Video" for a file).
    pub label: String,
    /// What the player loads: an iframe's address, or the file for "file".
    pub embed_url: String,
    /// The video on its own site, for "Watch on YouTube" and the like.
    pub watch_url: String,
    /// A still to show before it plays, when the site has a well-known one.
    pub thumbnail: Option<String>,
    /// Portrait (Shorts, TikTok, Reels): the player is 9:16 rather than 16:9.
    pub vertical: bool,
}

fn host(u: &url::Url) -> String {
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    ["www.", "m.", "player.", "music."]
        .iter()
        .find_map(|p| host.strip_prefix(p))
        .map(String::from)
        .unwrap_or(host)
}

/// A path's segments, empty ones dropped: "/embed/abc/" → ["embed", "abc"].
fn segments(u: &url::Url) -> Vec<&str> {
    u.path_segments()
        .map(|s| s.filter(|p| !p.is_empty()).collect())
        .unwrap_or_default()
}

fn is_id(id: &str, max: usize) -> bool {
    !id.is_empty()
        && id.len() <= max
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn youtube(id: &str, vertical: bool, start: Option<u32>) -> VideoEmbed {
    let start_param = start.map(|s| format!("&start={s}")).unwrap_or_default();
    VideoEmbed {
        provider: "youtube".into(),
        label: "YouTube".into(),
        // The privacy-enhanced player: no cookies until it plays
        embed_url: format!("https://www.youtube-nocookie.com/embed/{id}?rel=0{start_param}"),
        watch_url: if vertical {
            format!("https://www.youtube.com/shorts/{id}")
        } else {
            format!("https://www.youtube.com/watch?v={id}")
        },
        thumbnail: Some(format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg")),
        vertical,
    }
}

/// "90", "1m30s" or "1h2m3s" (YouTube's `t=`/`start=`) as seconds.
fn start_seconds(t: &str) -> Option<u32> {
    let t = t.trim().trim_end_matches('s');
    if let Ok(n) = t.parse::<u32>() {
        return (n > 0).then_some(n);
    }
    let mut total = 0u32;
    let mut num = String::new();
    for c in t.chars() {
        if c.is_ascii_digit() {
            num.push(c);
            continue;
        }
        let n: u32 = num.parse().ok()?;
        num.clear();
        total += match c {
            'h' => n * 3600,
            'm' => n * 60,
            _ => return None,
        };
    }
    if !num.is_empty() {
        total += num.parse::<u32>().ok()?;
    }
    (total > 0).then_some(total)
}

fn youtube_from(u: &url::Url, host: &str) -> Option<VideoEmbed> {
    let seg = segments(u);
    let query = |k: &str| {
        u.query_pairs()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.into_owned())
    };
    let start = query("t")
        .or_else(|| query("start"))
        .and_then(|t| start_seconds(&t));
    let (id, vertical) = match host {
        "youtu.be" => (seg.first().copied()?.to_string(), false),
        "youtube.com" | "youtube-nocookie.com" => match seg.as_slice() {
            ["watch"] => (query("v")?, false),
            ["shorts", id, ..] => (id.to_string(), true),
            ["embed" | "v" | "live", id, ..] => (id.to_string(), false),
            _ => return None,
        },
        _ => return None,
    };
    is_id(&id, 16).then(|| youtube(&id, vertical, start))
}

fn vimeo_from(u: &url::Url, host: &str) -> Option<VideoEmbed> {
    let seg = segments(u);
    // vimeo.com/123, vimeo.com/channels/staffpicks/123, player.vimeo.com/video/123;
    // an unlisted video's hash follows its id (vimeo.com/123/abcdef) or comes as `h=`
    let pos = seg
        .iter()
        .position(|s| s.chars().all(|c| c.is_ascii_digit()))?;
    let id = seg[pos];
    let hash = u
        .query_pairs()
        .find(|(k, _)| k == "h")
        .map(|(_, v)| v.into_owned())
        .or_else(|| {
            (host == "vimeo.com")
                .then(|| seg.get(pos + 1).map(|s| s.to_string()))
                .flatten()
        })
        .filter(|h| is_id(h, 32));
    let (embed_url, watch_url) = match &hash {
        Some(h) => (
            format!("https://player.vimeo.com/video/{id}?h={h}&dnt=1"),
            format!("https://vimeo.com/{id}/{h}"),
        ),
        None => (
            format!("https://player.vimeo.com/video/{id}?dnt=1"),
            format!("https://vimeo.com/{id}"),
        ),
    };
    Some(VideoEmbed {
        provider: "vimeo".into(),
        label: "Vimeo".into(),
        embed_url,
        watch_url,
        thumbnail: None,
        vertical: false,
    })
}

fn tiktok_from(u: &url::Url) -> Option<VideoEmbed> {
    // tiktok.com/@cook/video/123, tiktok.com/embed/v2/123
    let seg = segments(u);
    let pos = seg.iter().position(|s| *s == "video" || *s == "v2")?;
    let id = seg.get(pos + 1)?;
    if !id.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let watch_url = match seg.first() {
        Some(user) if user.starts_with('@') => format!("https://www.tiktok.com/{user}/video/{id}"),
        _ => format!("https://www.tiktok.com/embed/v2/{id}"),
    };
    Some(VideoEmbed {
        provider: "tiktok".into(),
        label: "TikTok".into(),
        embed_url: format!("https://www.tiktok.com/embed/v2/{id}"),
        watch_url,
        thumbnail: None,
        vertical: true,
    })
}

fn instagram_from(u: &url::Url) -> Option<VideoEmbed> {
    let seg = segments(u);
    let (kind, id) = match seg.as_slice() {
        [kind @ ("reel" | "reels" | "p" | "tv"), id, ..] => (*kind, *id),
        _ => return None,
    };
    if !is_id(id, 40) {
        return None;
    }
    let kind = if kind == "reels" { "reel" } else { kind };
    Some(VideoEmbed {
        provider: "instagram".into(),
        label: "Instagram".into(),
        embed_url: format!("https://www.instagram.com/{kind}/{id}/embed/"),
        watch_url: format!("https://www.instagram.com/{kind}/{id}/"),
        thumbnail: None,
        vertical: true,
    })
}

fn dailymotion_from(u: &url::Url, host: &str) -> Option<VideoEmbed> {
    let seg = segments(u);
    let id = match (host, seg.as_slice()) {
        ("dai.ly", [id, ..]) => *id,
        (_, ["video", id, ..]) | (_, ["embed", "video", id, ..]) => *id,
        _ => return None,
    };
    // "x8abc12_beef-and-broccoli" → "x8abc12"
    let id = id.split('_').next()?;
    is_id(id, 16).then(|| VideoEmbed {
        provider: "dailymotion".into(),
        label: "Dailymotion".into(),
        embed_url: format!("https://www.dailymotion.com/embed/video/{id}"),
        watch_url: format!("https://www.dailymotion.com/video/{id}"),
        thumbnail: None,
        vertical: false,
    })
}

fn jwplayer_from(u: &url::Url) -> Option<VideoEmbed> {
    // content.jwplatform.com/players/{media}-{player}.html (or .js, the same player as a script)
    let seg = segments(u);
    let ["players", file] = seg.as_slice() else {
        return None;
    };
    let stem = file
        .strip_suffix(".html")
        .or_else(|| file.strip_suffix(".js"))?;
    if !is_id(stem, 40) {
        return None;
    }
    let url = format!("https://cdn.jwplayer.com/players/{stem}.html");
    Some(VideoEmbed {
        provider: "jwplayer".into(),
        label: "Video".into(),
        embed_url: url.clone(),
        watch_url: url,
        thumbnail: None,
        vertical: false,
    })
}

const FILE_TYPES: [&str; 4] = [".mp4", ".webm", ".m4v", ".mov"];

/// How a page plays the video at `url`, or `None` when it isn't a video Crumb knows how
/// to play in place (the page links to it instead).
pub fn video_embed(url: &str) -> Option<VideoEmbed> {
    let url = url.trim();
    if !is_valid_url(url) {
        return None;
    }
    let u = url::Url::parse(url).ok()?;
    let host = host(&u);
    match host.as_str() {
        "youtube.com" | "youtu.be" | "youtube-nocookie.com" => youtube_from(&u, &host),
        "vimeo.com" => vimeo_from(&u, &host),
        "tiktok.com" => tiktok_from(&u),
        "instagram.com" => instagram_from(&u),
        "dailymotion.com" | "dai.ly" => dailymotion_from(&u, &host),
        "content.jwplatform.com" | "cdn.jwplayer.com" => jwplayer_from(&u),
        _ if u.scheme() == "https"
            && FILE_TYPES
                .iter()
                .any(|t| u.path().to_ascii_lowercase().ends_with(t)) =>
        {
            Some(VideoEmbed {
                provider: "file".into(),
                label: "Video".into(),
                embed_url: url.to_string(),
                watch_url: url.to_string(),
                thumbnail: None,
                vertical: false,
            })
        }
        _ => None,
    }
}

/// The link to keep for a video found on a page: its own page on its site when Crumb can
/// play it (so an embed address and a watch link for the same video are one video), else
/// `None`, as a page's other videos (ad players, scripts) are no use to the cook.
pub fn video_link(url: &str) -> Option<String> {
    video_embed(url).map(|e| e.watch_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn youtube_in_all_its_shapes() {
        for url in [
            "https://youtu.be/8eITNSfct3Q?si=YEDH1yr00LEEj6dN",
            "https://www.youtube.com/watch?v=8eITNSfct3Q&feature=share",
            "https://m.youtube.com/watch?v=8eITNSfct3Q",
            "https://www.youtube.com/embed/8eITNSfct3Q?feature=oembed",
            "https://www.youtube-nocookie.com/embed/8eITNSfct3Q",
        ] {
            let e = video_embed(url).unwrap_or_else(|| panic!("{url}"));
            assert_eq!(e.provider, "youtube");
            assert_eq!(e.watch_url, "https://www.youtube.com/watch?v=8eITNSfct3Q");
            assert_eq!(
                e.embed_url,
                "https://www.youtube-nocookie.com/embed/8eITNSfct3Q?rel=0"
            );
            assert!(!e.vertical);
        }
        let short = video_embed("https://youtube.com/shorts/abcDEF12345?feature=share").unwrap();
        assert!(short.vertical);
        assert_eq!(
            short.watch_url,
            "https://www.youtube.com/shorts/abcDEF12345"
        );
        let at = video_embed("https://youtu.be/8eITNSfct3Q?t=1m30s").unwrap();
        assert!(at.embed_url.ends_with("&start=90"));
        assert!(video_embed("https://www.youtube.com/@natashaskitchen").is_none());
        assert!(video_embed("https://www.youtube.com/watch?v=<script>").is_none());
    }

    #[test]
    fn other_sites() {
        let v = video_embed("https://vimeo.com/76979871").unwrap();
        assert_eq!(v.embed_url, "https://player.vimeo.com/video/76979871?dnt=1");
        let v = video_embed("https://player.vimeo.com/video/76979871?h=8272103f6e").unwrap();
        assert_eq!(v.watch_url, "https://vimeo.com/76979871/8272103f6e");
        let t =
            video_embed("https://www.tiktok.com/@cook/video/7291234567890123456?lang=en").unwrap();
        assert_eq!(
            t.embed_url,
            "https://www.tiktok.com/embed/v2/7291234567890123456"
        );
        assert!(t.vertical);
        let i = video_embed("https://www.instagram.com/reels/C1a2b3c4d5/").unwrap();
        assert_eq!(
            i.embed_url,
            "https://www.instagram.com/reel/C1a2b3c4d5/embed/"
        );
        let d = video_embed("https://www.dailymotion.com/video/x8abc12_beef").unwrap();
        assert_eq!(
            d.embed_url,
            "https://www.dailymotion.com/embed/video/x8abc12"
        );
        let j = video_embed("https://content.jwplatform.com/players/AbCd1234-XyZ98765.js").unwrap();
        assert_eq!(
            j.embed_url,
            "https://cdn.jwplayer.com/players/AbCd1234-XyZ98765.html"
        );
        let f = video_embed("https://cdn.test/v/beef.MP4?x=1").unwrap();
        assert_eq!(f.provider, "file");
        assert!(video_embed("http://cdn.test/v/beef.mp4").is_none());
        assert!(video_embed("https://video.mediavine.com/videos/abc.js").is_none());
        assert!(video_embed("javascript:alert(1)").is_none());
    }

    #[test]
    fn keeps_the_watch_link() {
        assert_eq!(
            video_link("https://www.youtube.com/embed/8eITNSfct3Q?feature=oembed").as_deref(),
            Some("https://www.youtube.com/watch?v=8eITNSfct3Q")
        );
        assert_eq!(video_link("https://example.com/about"), None);
    }
}
