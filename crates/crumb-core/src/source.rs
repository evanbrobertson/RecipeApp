//! Where a recipe came from: the links a page or connector may show. A recipe saved
//! from a share is kept under that link, so it must never be published again.

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
