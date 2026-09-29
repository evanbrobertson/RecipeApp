//! The Add box: what was pasted (a link, a whole recipe, a name to start from scratch), and
//! the wording while a cooking video waits in the server's queue. Ported from
//! `web/src/islands/TopBox.svelte` (`detect`) and `web/src/lib/importLink.ts`.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

/// What the Add box thinks it was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AddMode {
    /// Nothing typed yet.
    Auto,
    /// One or more links.
    Link,
    /// A recipe's text.
    Text,
    /// A short name: open a new recipe with it as the title.
    Scratch,
}

/// The mode and a short summary for the box's footer ("bbcgoodfood.com", "3 links",
/// "12 ingredients", "New recipe").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Detected {
    pub mode: AddMode,
    pub summary: String,
}

static URL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^https?://\S+$").unwrap());

// A line that reads like an ingredient: starts with an amount or a bullet, or names a unit.
// `\b` is ASCII here, as in JavaScript.
static INGREDIENT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^\s*([-•*▢□]|\d|[½¼¾⅓⅔⅛]|a (pinch|handful|few))|(?-u:\b)(cups?|tbsp|tsp|tablespoons?|teaspoons?|grams?|g|kg|ml|l|oz|ounces?|lbs?|pounds?|cloves?|pinch)(?-u:\b)",
    )
    .unwrap()
});

/// A link's host without `www.`, or "" when it doesn't parse.
fn host(link: &str) -> String {
    url::Url::parse(link)
        .ok()
        .and_then(|u| {
            u.host_str()
                .map(|h| h.trim_start_matches("www.").to_string())
        })
        .unwrap_or_default()
}

fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

/// What was pasted (`detect`). Photos and files are chosen, not typed, so they never
/// come from here.
pub fn detect(raw: &str) -> Detected {
    let text = raw.trim();
    let detected = |mode, summary: String| Detected { mode, summary };
    if text.is_empty() {
        return detected(AddMode::Auto, String::new());
    }
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let urls: Vec<&str> = tokens
        .iter()
        .copied()
        .filter(|t| URL_RE.is_match(t))
        .collect();
    if !urls.is_empty() && urls.len() == tokens.len() {
        if urls.len() > 1 {
            return detected(AddMode::Link, format!("{} links", urls.len()));
        }
        return detected(AddMode::Link, host(urls[0]));
    }
    let lines: Vec<&str> = text.split('\n').filter(|l| !l.trim().is_empty()).collect();
    // Shared text like "Best lasagna https://…": one link and a few words on one line
    if urls.len() == 1 && lines.len() == 1 {
        return detected(AddMode::Link, host(urls[0]));
    }
    if lines.len() >= 3 {
        let n = lines.iter().filter(|l| INGREDIENT_RE.is_match(l)).count();
        let summary = if n > 0 {
            plural(n, "ingredient")
        } else {
            format!("{} lines", lines.len())
        };
        return detected(AddMode::Text, summary);
    }
    // The web counts UTF-16 units
    if lines.len() == 1 && text.encode_utf16().count() <= 80 {
        return detected(AddMode::Scratch, "New recipe".into());
    }
    detected(AddMode::Text, plural(lines.len(), "line"))
}

/// Every http(s) link in a paste, in order and without repeats, for "Lots of links".
pub fn links_in(raw: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for token in raw.split_whitespace() {
        if URL_RE.is_match(token) && !out.iter().any(|seen| seen == token) {
            out.push(token.to_string());
        }
    }
    out
}

static LINK_IN_TEXT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)https?://[^\s<>"']+"#).unwrap());
static LINK_TAIL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[),.;]+$").unwrap());

/// Every http(s) link anywhere in some text, without trailing punctuation, in order and
/// without repeats: the Import page's links box ("see https://x.com/a, thanks").
pub fn links_in_text(raw: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for found in LINK_IN_TEXT_RE.find_iter(raw) {
        let link = LINK_TAIL_RE.replace(found.as_str(), "").into_owned();
        if !out.contains(&link) {
            out.push(link);
        }
    }
    out
}

/// "1st", "2nd", "3rd", "11th".
pub fn ordinal(n: u32) -> String {
    let (tens, last) = (n % 100, n % 10);
    let suffix = if (11..=13).contains(&tens) {
        "th"
    } else {
        match last {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        }
    };
    format!("{n}{suffix}")
}

/// What the Add box says while a video waits or is watched (`jobProgress`). `status` is
/// the job's `queued` or `running`; `position` is 1 for next.
pub fn job_progress(status: &str, position: Option<u32>) -> String {
    if status == "queued" {
        return match position {
            Some(p) if p > 1 => format!("Queued ({})…", ordinal(p)),
            _ => "Up next…".into(),
        };
    }
    "Watching the video… this can take a minute or two".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(raw: &str) -> (AddMode, String) {
        let d = detect(raw);
        (d.mode, d.summary)
    }

    #[test]
    fn links_and_names() {
        assert_eq!(mode("  "), (AddMode::Auto, String::new()));
        assert_eq!(
            mode("https://www.bbcgoodfood.com/recipes/x"),
            (AddMode::Link, "bbcgoodfood.com".into())
        );
        assert_eq!(
            mode("https://a.com/1 https://b.com/2"),
            (AddMode::Link, "2 links".into())
        );
        assert_eq!(
            mode("Best lasagna https://food.example/lasagna"),
            (AddMode::Link, "food.example".into())
        );
        assert_eq!(
            mode("Nan's scones"),
            (AddMode::Scratch, "New recipe".into())
        );
    }

    #[test]
    fn recipe_text_counts_ingredients() {
        assert_eq!(
            mode("Pancakes\n200g flour\n2 eggs\nMix and fry."),
            (AddMode::Text, "2 ingredients".into())
        );
        assert_eq!(mode("One\nTwo\nThree"), (AddMode::Text, "3 lines".into()));
        let long = "word ".repeat(30);
        assert_eq!(mode(&long), (AddMode::Text, "1 line".into()));
    }

    #[test]
    fn video_queue_wording() {
        assert_eq!(job_progress("queued", Some(1)), "Up next…");
        assert_eq!(job_progress("queued", Some(2)), "Queued (2nd)…");
        assert_eq!(job_progress("queued", Some(12)), "Queued (12th)…");
        assert_eq!(
            job_progress("running", None),
            "Watching the video… this can take a minute or two"
        );
        assert_eq!(ordinal(21), "21st");
        assert_eq!(ordinal(113), "113th");
    }

    #[test]
    fn links_are_found_inside_prose() {
        assert_eq!(
            links_in_text("see https://x.com/a, thanks (https://y.org/b). And https://x.com/a"),
            vec!["https://x.com/a", "https://y.org/b"]
        );
        assert_eq!(
            links_in_text("<a href=\"https://z.net/c?d=1\">"),
            vec!["https://z.net/c?d=1"]
        );
        assert!(links_in_text("no links").is_empty());
    }

    #[test]
    fn links_are_found_once_each() {
        assert_eq!(
            links_in("a https://x.com/1\nhttps://x.com/1 http://y.org"),
            vec!["https://x.com/1", "http://y.org"]
        );
    }
}
