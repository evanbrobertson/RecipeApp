//! What every native client works out the same way before or after a request: the
//! server address someone typed, photo URLs, the session cookie, error wording, whether a
//! paste is a link or a recipe, times and cook counts as the web shows them, and the steps
//! cook mode pages through. Ported from `web/src/lib` so the apps can't drift from it.

use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;

use crate::duration::iso_duration_minutes;
use crate::model::Recipe;

/// Name of the session cookie the server sets (`src/auth.rs`).
pub const SESSION_COOKIE: &str = "crumb_session";

/// Widths the server's photo resizer makes. Must match `WIDTHS` in `src/images.rs` and
/// `IMG_WIDTHS` in `web/src/lib/img.ts`.
pub const IMAGE_WIDTHS: [u32; 5] = [160, 320, 480, 768, 1200];

/// FNV-1a 32-bit over the image string's UTF-8 bytes, as 8 lowercase hex digits
/// (`imageKey` in `web/src/lib/img.ts`), so a new image gets a new photo URL.
pub fn image_key(image: &str) -> String {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in image.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    format!("{hash:08x}")
}

/// The smallest server width at least `px` wide, or the largest.
pub fn snap_width(px: u32) -> u32 {
    IMAGE_WIDTHS
        .iter()
        .copied()
        .find(|w| *w >= px)
        .unwrap_or(IMAGE_WIDTHS[IMAGE_WIDTHS.len() - 1])
}

/// `img/{id}/{width}?v={key}`, relative to the server's base URL, for a photo shown about
/// `px` pixels wide. None when the recipe has no image.
pub fn photo_path(recipe_id: i64, image: Option<&str>, px: u32) -> Option<String> {
    let image = image.map(str::trim).filter(|s| !s.is_empty())?;
    Some(format!(
        "img/{recipe_id}/{}?v={}",
        snap_width(px),
        image_key(image)
    ))
}

/// What someone typed as their server ("crumb.example.com", "http://192.168.1.5:3000/")
/// as a base URL ending in "/", or None if it isn't a web address. Defaults to HTTPS;
/// the query and fragment are dropped, a sub-path is kept.
pub fn server_url(input: &str) -> Option<String> {
    let trimmed = input.trim().trim_end_matches('/');
    if trimmed.is_empty() || trimmed.chars().any(char::is_whitespace) {
        return None;
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let mut url = url::Url::parse(&with_scheme).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = url.host_str()?.to_string();
    let ip = matches!(url.host(), Some(url::Host::Ipv4(_) | url::Host::Ipv6(_)));
    if !ip && !host.contains('.') && host != "localhost" {
        return None;
    }
    let path = format!("{}/", url.path().trim_end_matches('/'));
    url.set_path(&path);
    url.set_query(None);
    url.set_fragment(None);
    Some(url.to_string())
}

/// Whether plain HTTP is acceptable for this server: only the device itself or a private
/// network address (a Crumb on the kitchen Wi-Fi). Anything else must be HTTPS.
pub fn allows_cleartext(base_url: &str) -> bool {
    let Ok(url) = url::Url::parse(base_url) else {
        return false;
    };
    if url.scheme() == "https" {
        return true;
    }
    match url.host() {
        Some(url::Host::Domain(d)) => d == "localhost" || d.ends_with(".local"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

/// The value of a `crumb_session=…` `Set-Cookie` header, unless it clears the cookie.
pub fn session_cookie(set_cookie: &str) -> Option<String> {
    let pair = set_cookie.split(';').next()?.trim();
    let value = pair.strip_prefix(SESSION_COOKIE)?.strip_prefix('=')?;
    (!value.is_empty()).then(|| value.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody {
    message: Option<String>,
    status_message: Option<String>,
}

/// What to show for a failed request: the server's own `{message}` (without the
/// "field: " prefix validation errors carry), or a kitchen-words default for `status`.
pub fn error_message(status: u16, body: &str) -> String {
    let parsed = serde_json::from_str::<ErrorBody>(body).ok();
    let raw = parsed.and_then(|b| {
        b.message
            .filter(|m| !m.trim().is_empty())
            .or(b.status_message)
    });
    let cleaned = raw
        .map(|m| match m.split_once(": ") {
            Some((_, rest)) => rest.to_string(),
            None => m,
        })
        .filter(|m| !m.trim().is_empty());
    cleaned.unwrap_or_else(|| {
        match status {
            401 => "Please sign in again.",
            404 => "That isn't in your recipe box any more.",
            500..=599 => "Your Crumb server had a problem. Try again in a moment.",
            _ => return format!("Something went wrong ({status})."),
        }
        .to_string()
    })
}

/// What someone pasted or shared into Add.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportInput {
    /// A link to a recipe page, for `{"url": …}`.
    Link(String),
    /// The recipe itself, for `{"text": …}`.
    Text(String),
}

static URL_IN_TEXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)https?://[^\s<>"]+"#).unwrap());

/// Shares from browsers are often "Page title https://…"; that's still a link.
const SHARED_LINK_MAX: usize = 500;

/// A link when the text is short and holds exactly one URL, else the recipe text itself.
/// None for blank input.
pub fn classify_import(raw: &str) -> Option<ImportInput> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }
    let urls: Vec<&str> = URL_IN_TEXT
        .find_iter(text)
        .map(|m| m.as_str().trim_end_matches(['.', ',', ')', ']']))
        .collect();
    Some(
        if urls.len() == 1 && text.chars().count() <= SHARED_LINK_MAX {
            ImportInput::Link(urls[0].to_string())
        } else {
            ImportInput::Text(text.to_string())
        },
    )
}

/// Offline search: every word of `query` in the title, category or cuisine, ignoring case.
pub fn matches_search(
    query: &str,
    title: &str,
    category: Option<&str>,
    cuisine: Option<&str>,
) -> bool {
    let haystack = [Some(title), category, cuisine]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|word| haystack.contains(word))
}

/// A recipe time for display. ISO 8601 ("PT1H30M") is shown the way the server writes
/// times ("1h 30m"); anything else ("about 40 minutes") as written. None when blank or zero.
pub fn display_duration(raw: Option<&str>) -> Option<String> {
    let text = raw.map(str::trim).filter(|s| !s.is_empty())?;
    let Some(exact) = iso_duration_minutes(text) else {
        return Some(text.to_string());
    };
    if exact <= 0.0 {
        return None;
    }
    let minutes = (exact.round() as i64).max(1);
    let (h, m) = (minutes / 60, minutes % 60);
    Some(match (h, m) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    })
}

/// A kitchen timer's remaining time: "4:05", or "1:02:03" past the hour
/// (`formatClock` in `web/src/lib/timers.svelte.ts`).
pub fn format_clock(seconds: f64) -> String {
    let s = seconds.round().max(0.0) as u64;
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}

/// "Cooked 3 times · last 2 weeks ago" (`cookedLine` in `web/src/lib/history.ts`).
/// `last_cooked_ms` and `now_ms` are Unix milliseconds. None when never cooked.
pub fn cooked_line(count: u32, last_cooked_ms: Option<i64>, now_ms: i64) -> Option<String> {
    if count == 0 {
        return None;
    }
    let times = if count == 1 {
        "Cooked once".to_string()
    } else {
        format!("Cooked {count} times")
    };
    let Some(last) = last_cooked_ms else {
        return Some(times);
    };
    let days = (now_ms - last).div_euclid(86_400_000);
    let round = |n: f64| n.round() as i64;
    let ago = match days {
        ..1 => "today".to_string(),
        1 => "yesterday".to_string(),
        2..14 => format!("{days} days ago"),
        14..60 => format!("{} weeks ago", round(days as f64 / 7.0)),
        60..365 => format!("{} months ago", round(days as f64 / 30.0)),
        _ => format!("{} years ago", round(days as f64 / 365.0)),
    };
    Some(format!("{times} · last {ago}"))
}

/// One step of cook mode, with the section it belongs to ("For the sauce").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookStep {
    pub section: Option<String>,
    pub text: String,
}

/// Every non-blank instruction in order, each with its (non-blank) section name.
pub fn cook_steps(recipe: &Recipe) -> Vec<CookStep> {
    recipe
        .instructions
        .iter()
        .flat_map(|section| {
            let name = section
                .name
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(str::to_string);
            section
                .items
                .iter()
                .filter(|item| !item.trim().is_empty())
                .map(move |item| CookStep {
                    section: name.clone(),
                    text: item.clone(),
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_keys_match_the_web() {
        // Values from imageKey() in web/src/lib/img.ts
        assert_eq!(image_key(""), "811c9dc5");
        assert_eq!(image_key("a"), "e40c292c");
        assert_eq!(image_key("https://example.com/pie.jpg").len(), 8);
        assert_eq!(snap_width(1), 160);
        assert_eq!(snap_width(321), 480);
        assert_eq!(snap_width(5000), 1200);
        assert_eq!(
            photo_path(7, Some("a"), 300).as_deref(),
            Some("img/7/320?v=e40c292c")
        );
        assert_eq!(photo_path(7, Some("  "), 300), None);
        assert_eq!(photo_path(7, None, 300), None);
    }

    #[test]
    fn server_addresses_are_normalized() {
        assert_eq!(
            server_url("crumb.example.com").as_deref(),
            Some("https://crumb.example.com/")
        );
        assert_eq!(
            server_url(" http://192.168.1.5:3000/ ").as_deref(),
            Some("http://192.168.1.5:3000/")
        );
        assert_eq!(
            server_url("https://example.com/crumb/?x=1#top").as_deref(),
            Some("https://example.com/crumb/")
        );
        assert_eq!(
            server_url("localhost:3000").as_deref(),
            Some("https://localhost:3000/")
        );
        assert_eq!(server_url("crumb"), None);
        assert_eq!(server_url("not a url"), None);
        assert_eq!(server_url(""), None);
        assert_eq!(server_url("ftp://example.com"), None);
    }

    #[test]
    fn cleartext_only_on_the_local_network() {
        assert!(allows_cleartext("https://crumb.example.com/"));
        assert!(allows_cleartext("http://localhost:3000/"));
        assert!(allows_cleartext("http://192.168.1.5:3000/"));
        assert!(allows_cleartext("http://10.0.0.2/"));
        assert!(allows_cleartext("http://kitchen.local/"));
        assert!(!allows_cleartext("http://crumb.example.com/"));
        assert!(!allows_cleartext("http://8.8.8.8/"));
        assert!(!allows_cleartext("nonsense"));
    }

    #[test]
    fn session_cookies_are_read_from_set_cookie() {
        assert_eq!(
            session_cookie("crumb_session=abc.def; Path=/; HttpOnly").as_deref(),
            Some("abc.def")
        );
        assert_eq!(session_cookie("crumb_session=; Max-Age=0"), None);
        assert_eq!(session_cookie("other=1"), None);
        assert_eq!(session_cookie("crumb_sessionx=1"), None);
    }

    #[test]
    fn errors_read_like_the_server_meant() {
        let body = r#"{"statusCode":400,"statusMessage":"Bad Request","message":"password: Password is required"}"#;
        assert_eq!(error_message(400, body), "Password is required");
        assert_eq!(
            error_message(400, r#"{"statusMessage":"Bad Request"}"#),
            "Bad Request"
        );
        assert_eq!(error_message(401, ""), "Please sign in again.");
        assert_eq!(
            error_message(404, "<html>"),
            "That isn't in your recipe box any more."
        );
        assert_eq!(
            error_message(502, ""),
            "Your Crumb server had a problem. Try again in a moment."
        );
        assert_eq!(error_message(418, "{}"), "Something went wrong (418).");
    }

    #[test]
    fn pastes_are_links_or_recipes() {
        assert_eq!(classify_import("   "), None);
        assert_eq!(
            classify_import("Best pie https://example.com/pie."),
            Some(ImportInput::Link("https://example.com/pie".into()))
        );
        assert_eq!(
            classify_import("2 eggs\nWhisk."),
            Some(ImportInput::Text("2 eggs\nWhisk.".into()))
        );
        assert!(matches!(
            classify_import("https://a.com and https://b.com"),
            Some(ImportInput::Text(_))
        ));
        let long = format!("{} https://example.com/pie", "word ".repeat(120));
        assert!(matches!(classify_import(&long), Some(ImportInput::Text(_))));
    }

    #[test]
    fn offline_search_needs_every_word() {
        assert!(matches_search(
            "pie apple",
            "Apple Pie",
            Some("Dessert"),
            None
        ));
        assert!(matches_search("british", "Scones", None, Some("British")));
        assert!(!matches_search("cherry pie", "Apple Pie", None, None));
        assert!(matches_search("", "Anything", None, None));
    }

    #[test]
    fn durations_display_like_the_server() {
        assert_eq!(display_duration(Some("PT1H30M")).as_deref(), Some("1h 30m"));
        assert_eq!(display_duration(Some("PT2H")).as_deref(), Some("2h"));
        assert_eq!(display_duration(Some("PT45M")).as_deref(), Some("45m"));
        assert_eq!(display_duration(Some("PT20S")).as_deref(), Some("1m"));
        assert_eq!(display_duration(Some("PT0M")), None);
        assert_eq!(
            display_duration(Some("about 40 minutes")).as_deref(),
            Some("about 40 minutes")
        );
        assert_eq!(display_duration(Some(" ")), None);
        assert_eq!(display_duration(None), None);
    }

    #[test]
    fn clocks_and_cook_counts() {
        assert_eq!(format_clock(245.0), "4:05");
        assert_eq!(format_clock(3723.0), "1:02:03");
        assert_eq!(format_clock(-5.0), "0:00");
        let now = 1_700_000_000_000;
        let day = 86_400_000;
        assert_eq!(cooked_line(0, None, now), None);
        assert_eq!(cooked_line(1, None, now).as_deref(), Some("Cooked once"));
        assert_eq!(
            cooked_line(3, Some(now - 3_600_000), now).as_deref(),
            Some("Cooked 3 times · last today")
        );
        assert_eq!(
            cooked_line(2, Some(now - day), now).as_deref(),
            Some("Cooked 2 times · last yesterday")
        );
        assert_eq!(
            cooked_line(2, Some(now - 15 * day), now).as_deref(),
            Some("Cooked 2 times · last 2 weeks ago")
        );
        assert_eq!(
            cooked_line(2, Some(now - 90 * day), now).as_deref(),
            Some("Cooked 2 times · last 3 months ago")
        );
        assert_eq!(
            cooked_line(2, Some(now - 800 * day), now).as_deref(),
            Some("Cooked 2 times · last 2 years ago")
        );
    }

    #[test]
    fn cook_steps_skip_blanks_and_keep_sections() {
        let recipe: Recipe = serde_json::from_str(
            r#"{"id":1,"url":null,"source":"manual","title":"Pie","description":null,
            "image":null,"author":null,"prepTime":null,"cookTime":null,"totalTime":null,
            "freezeTime":null,"recipeYield":null,"recipeCategory":null,"recipeCuisine":null,
            "ingredients":[],"instructions":[{"name":null,"items":["Mix.",""]},
            {"name":"For the sauce","items":["Stir."]},{"name":" ","items":["Serve."]}],
            "nutrition":null,"notes":null,"originalUrl":null,
            "createdAt":"2026-09-26T16:30:33.000Z","updatedAt":"2026-09-26T16:30:33.000Z"}"#,
        )
        .unwrap();
        let steps = cook_steps(&recipe);
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].section, None);
        assert_eq!(steps[1].section.as_deref(), Some("For the sauce"));
        assert_eq!(steps[2].section, None);
        assert_eq!(steps[2].text, "Serve.");
    }
}
