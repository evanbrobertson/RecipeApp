//! Where a recipe came from: the links a page or connector may show, and the cooking
//! videos an import watches. A recipe saved from a share is kept under that link, so it
//! must never be published again.

use std::sync::LazyLock;

use regex::Regex;

use crate::model::{Recipe, is_valid_url};

static SHARE_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^/s/[A-Za-z0-9_-]{16,}(?:/\d+)?/?$").unwrap());

/// Whether a URL is a Crumb share link (this box's or any other's): `/s/{token}`, or a
/// recipe's page in a shared cookbook, `/s/{token}/{id}`. A recipe saved from a share is kept
/// under that link, which must never be published again: whoever sees it can open the share
/// (and, from a book recipe's link, the whole book).
pub fn is_share_link(url: &str) -> bool {
    url::Url::parse(url.trim()).is_ok_and(|u| SHARE_PATH.is_match(u.path()))
}

/// A link a share may publish as the recipe's source: http(s), and never a share link.
pub fn publishable_source(url: &str) -> bool {
    is_valid_url(url) && !is_share_link(url)
}

/// Where the recipe came from, as a link the page may show: its original source when it was
/// saved from a share, else its own link. Only http(s) (older rows may hold anything), and
/// never a share link: a recipe saved from a share without a source has none to show.
pub fn source_url(r: &Recipe) -> Option<&str> {
    [&r.original_url, &r.url]
        .into_iter()
        .filter_map(|v| v.as_deref().map(str::trim).filter(|s| !s.is_empty()))
        .find(|u| publishable_source(u))
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
