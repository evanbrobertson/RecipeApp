//! Recognising the platform a recipe site runs on, and reading what the platform exposes
//! (see [`super::fallbacks`] for the fetching).
//!
//! Today that is WordPress, which most recipe blogs run. A page gives its API away in three
//! places, and all three name the same thing:
//! - the `Link` header: `<https://x/wp-json/wp/v2/posts/12>; rel="alternate";
//!   type="application/json"` (the post's own endpoint, so its exact type and id) and
//!   `<https://x/wp-json/>; rel="https://api.w.org/"` (the API root, which is not always at the
//!   site's root: a blog in a sub-folder has `/blog/wp-json/`)
//! - the same two as `<link>` tags in the page's head
//! - a site that has `pretty permalinks` off serves the API at `/?rest_route=/wp/v2/...` and
//!   says so in the same two places
//!
//! The recipe plugins are told apart by their markup or their id in the post's content:
//! WP Recipe Maker, Mediavine Create, Tasty Recipes and WPZOOM's Recipe Card blocks.

use regex::Regex;
use scraper::{ElementRef, Html};
use std::sync::LazyLock;

use super::{block_lines, finish, format_minutes, notes_from_html, sel, text_of, video_from_html};
use crate::model::{RecipeFields, Section};

/// A recipe platform with an API worth asking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    WordPress,
}

impl Platform {
    /// The name kept in site memory.
    pub fn as_str(self) -> &'static str {
        match self {
            Platform::WordPress => "wordpress",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "wordpress" => Some(Platform::WordPress),
            _ => None,
        }
    }
}

/// The platform a link gives away before anything is fetched: only what is unmistakable
/// (a WordPress upload or API path, `?p=12`). Site memory and the page itself do the rest.
pub fn platform_from_url(url: &url::Url) -> Option<Platform> {
    let path = url.path();
    let plain_permalink = url
        .query_pairs()
        .any(|(k, v)| matches!(&*k, "p" | "page_id") && v.bytes().all(|b| b.is_ascii_digit()));
    (path.contains("/wp-content/") || path.contains("/wp-json/") || plain_permalink)
        .then_some(Platform::WordPress)
}

/// The platform a page's HTML is built on.
pub fn detect_platform(html: &str) -> Option<Platform> {
    const MARKERS: [&str; 4] = [
        "/wp-content/",
        "/wp-includes/",
        "api.w.org",
        "content=\"WordPress",
    ];
    MARKERS
        .iter()
        .any(|m| html.contains(m))
        .then_some(Platform::WordPress)
}

// ---------- WordPress: where the API is ----------

/// Where a WordPress site's REST API lives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WpRoot {
    /// `https://x.test/wp-json` (no trailing slash), or for the query form the site's own
    /// address with its trailing slash (`https://x.test/blog/`).
    base: String,
    /// `/?rest_route=/wp/v2/...` instead of `/wp-json/wp/v2/...`.
    query: bool,
}

impl WpRoot {
    /// The usual place for a site: `{origin}/wp-json`.
    pub fn at_origin(origin: &str) -> Self {
        Self {
            base: format!("{}/wp-json", origin.trim_end_matches('/')),
            query: false,
        }
    }

    /// The same site, asked the other way (for a site whose `/wp-json` is not served).
    pub fn other_form(&self) -> Self {
        match (self.query, self.base.strip_suffix("/wp-json")) {
            (false, Some(origin)) => Self {
                base: format!("{origin}/"),
                query: true,
            },
            (false, None) => Self {
                base: self.base.clone(),
                query: true,
            },
            (true, _) => Self {
                base: format!("{}/wp-json", self.base.trim_end_matches('/')),
                query: false,
            },
        }
    }

    pub fn is_query_form(&self) -> bool {
        self.query
    }

    /// The root as an address, as kept in site memory and as the page names it.
    pub fn as_address(&self) -> String {
        if self.query {
            format!("{}?rest_route=/", self.base)
        } else {
            format!("{}/", self.base)
        }
    }

    /// A root from an address a page or site memory gave: `https://x.test/wp-json/`, or
    /// `https://x.test/?rest_route=/`.
    pub fn parse(address: &str) -> Option<Self> {
        let url = url::Url::parse(address).ok()?;
        if !matches!(url.scheme(), "http" | "https") {
            return None;
        }
        if url.query_pairs().any(|(k, _)| k == "rest_route") {
            let mut site = url;
            site.set_query(None);
            site.set_fragment(None);
            return Some(Self {
                base: site.to_string(),
                query: true,
            });
        }
        let mut root = url;
        root.set_query(None);
        root.set_fragment(None);
        Some(Self {
            base: root.as_str().trim_end_matches('/').to_string(),
            query: false,
        })
    }

    /// The address of `route` (`/wp/v2/posts`) with a query (`slug=x&_fields=y`, maybe empty).
    pub fn endpoint(&self, route: &str, query: &str) -> String {
        match (self.query, query.is_empty()) {
            (false, true) => format!("{}{route}", self.base),
            (false, false) => format!("{}{route}?{query}", self.base),
            (true, true) => format!("{}?rest_route={route}", self.base),
            (true, false) => format!("{}?rest_route={route}&{query}", self.base),
        }
    }

    /// The same root, with the page's scheme (a site that has http baked into its links).
    fn with_scheme_of(mut self, page: &url::Url) -> Self {
        if let Some(rest) = self.base.split_once("://").map(|(_, r)| r.to_string()) {
            self.base = format!("{}://{rest}", page.scheme());
        }
        self
    }
}

fn bare_host(host: &str) -> &str {
    host.strip_prefix("www.").unwrap_or(host)
}

/// What a page said about its WordPress API.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WpLead {
    pub root: Option<WpRoot>,
    /// The post's type in the API (`posts`, or a custom type's `rest_base`) and its id, when
    /// the page named its own endpoint.
    pub post: Option<(String, u64)>,
}

impl WpLead {
    /// Just a root (from site memory).
    pub fn at(root: WpRoot) -> Self {
        Self {
            root: Some(root),
            post: None,
        }
    }

    /// Whether the page named the post itself (so no search by slug is needed).
    pub fn is_specific(&self) -> bool {
        self.post.is_some()
    }
}

/// One entry of a `Link` header.
#[derive(Debug, PartialEq, Eq)]
pub struct HeaderLink {
    pub target: String,
    pub rel: String,
    pub kind: Option<String>,
}

/// The entries of a `Link` header (several headers joined with ", " are fine):
/// `<https://x/wp-json/>; rel="https://api.w.org/", <https://x/?p=1>; rel=shortlink`.
pub fn parse_link_header(header: &str) -> Vec<HeaderLink> {
    static ENTRY: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"<([^>]*)>((?:\s*;\s*[^;<,]+(?:"[^"]*")?)*)"#).unwrap());
    ENTRY
        .captures_iter(header)
        .filter_map(|c| {
            let mut link = HeaderLink {
                target: c[1].trim().to_string(),
                rel: String::new(),
                kind: None,
            };
            for param in c[2].split(';') {
                let Some((name, value)) = param.split_once('=') else {
                    continue;
                };
                let value = value.trim().trim_matches('"').to_string();
                match name.trim().to_ascii_lowercase().as_str() {
                    "rel" => link.rel = value,
                    "type" => link.kind = Some(value),
                    _ => {}
                }
            }
            (!link.target.is_empty() && !link.rel.is_empty()).then_some(link)
        })
        .collect()
}

/// `<link rel href type>` tags of a page's head.
fn html_links(html: &str) -> Vec<HeaderLink> {
    // The head is all that's wanted: stop at the body so a big page isn't parsed for this
    let head = html
        .find("</head>")
        .map_or(html, |end| &html[..end + "</head>".len()]);
    Html::parse_document(head)
        .select(&sel("link[rel][href]"))
        .filter_map(|l| {
            Some(HeaderLink {
                target: l.value().attr("href")?.trim().to_string(),
                rel: l.value().attr("rel")?.trim().to_string(),
                kind: l.value().attr("type").map(str::to_string),
            })
        })
        .collect()
}

/// A post's own endpoint (`.../wp-json/wp/v2/posts/12` or `/?rest_route=/wp/v2/posts/12`)
/// as its API root, type and id.
fn post_endpoint(target: &url::Url) -> Option<(WpRoot, String, u64)> {
    let (root, route) =
        if let Some((_, route)) = target.query_pairs().find(|(k, _)| k == "rest_route") {
            let mut site = target.clone();
            site.set_query(None);
            site.set_fragment(None);
            let root = WpRoot::parse(&format!("{site}?rest_route=/"))?;
            (root, route.to_string())
        } else {
            let at = target.path().find("/wp/v2/")?;
            let mut base = target.clone();
            base.set_query(None);
            base.set_fragment(None);
            base.set_path(&target.path()[..at]);
            (
                WpRoot::parse(base.as_str())?,
                target.path()[at..].to_string(),
            )
        };
    let mut parts = route.strip_prefix("/wp/v2/")?.split('/');
    let rest_base = parts.next().filter(|b| is_route_name(b))?;
    let id = parts.next()?.parse().ok()?;
    Some((root, rest_base.to_string(), id))
}

/// A REST base is a word; anything else in a page's link is not put into a request.
fn is_route_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

/// What a page (its `Link` header and its head) says about its WordPress API. Only an address
/// on the page's own site is taken, so a page can't point the fetch elsewhere.
pub fn wp_lead(link_header: Option<&str>, html: &str, page: &url::Url) -> Option<WpLead> {
    let mut links = link_header.map(parse_link_header).unwrap_or_default();
    if html.contains("api.w.org") || html.contains("wp-json") || html.contains("rest_route") {
        links.extend(html_links(html));
    }
    let page_host = bare_host(&page.host_str()?.to_ascii_lowercase()).to_string();
    let mut lead = WpLead::default();
    for link in links {
        let Ok(target) = page.join(&link.target) else {
            continue;
        };
        if !matches!(target.scheme(), "http" | "https")
            || target
                .host_str()
                .map(|h| bare_host(&h.to_ascii_lowercase()).to_string())
                != Some(page_host.clone())
        {
            continue;
        }
        if link
            .rel
            .split_whitespace()
            .any(|r| r == "https://api.w.org/")
        {
            if lead.root.is_none() {
                lead.root = WpRoot::parse(target.as_str()).map(|r| r.with_scheme_of(page));
            }
        } else if link.rel.split_whitespace().any(|r| r == "alternate")
            && link.kind.as_deref() == Some("application/json")
            && lead.post.is_none()
            && let Some((root, base, id)) = post_endpoint(&target)
        {
            lead.post = Some((base, id));
            lead.root = Some(root.with_scheme_of(page));
        }
    }
    (lead.root.is_some() || lead.post.is_some()).then_some(lead)
}

// ---------- WordPress: recipe plugins in a post's content ----------

/// The id of the WP Recipe Maker card in a post's content: from the card's own markup, or
/// from the marker WPRM leaves where a card sits (a comment in the rendered content, or the
/// block's comment): `<!--WPRM Recipe 12-->`, `<!-- wp:wp-recipe-maker/recipe {"id":12} -->`.
pub fn wprm_recipe_id(content: &str) -> Option<u64> {
    static ID: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r#"(?:wprm-recipe-container-|data-recipe-id=["']|<!--\s*WPRM Recipe\s+|wp:wp-recipe-maker/recipe\s*\{\s*"id"\s*:\s*)(\d+)"#,
        )
        .unwrap()
    });
    ID.captures(content)?[1].parse().ok()
}

/// The id of the Mediavine Create card in a post's content.
pub fn mediavine_id(content: &str) -> Option<u64> {
    static ID: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?:mv-creation-|data-mv-create-id=["'])(\d+)"#).unwrap());
    ID.captures(content)?[1].parse().ok()
}

/// Sections from a body of headed lists: an `h2`-`h5` names the lists after it; a list
/// before any heading is unnamed.
fn headed_lists(body: ElementRef) -> Vec<Section> {
    let li = sel("li");
    let mut sections: Vec<Section> = Vec::new();
    let mut name: Option<String> = None;
    for child in body.children().filter_map(ElementRef::wrap) {
        match child.value().name() {
            "h2" | "h3" | "h4" | "h5" | "h6" => {
                name = Some(text_of(child)).filter(|n| !n.is_empty());
            }
            "ul" | "ol" => {
                let items: Vec<String> = child
                    .select(&li)
                    .map(text_of)
                    .filter(|t| !t.is_empty())
                    .collect();
                if !items.is_empty() {
                    sections.push(Section {
                        name: name.take(),
                        items,
                    });
                }
            }
            // A wrapper around the lists (some themes add one)
            _ => {
                if child.select(&li).next().is_some() {
                    sections.extend(headed_lists(child));
                }
            }
        }
    }
    sections
}

/// "1 hour 15 minutes", "45 mins", "2 hrs" as "1h 15m".
fn minutes_in_text(text: &str) -> Option<String> {
    static HOURS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)(\d+)\s*(?:hours?|hrs?|h)\b").unwrap());
    static MINUTES: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)(\d+)\s*(?:minutes?|mins?|m)\b").unwrap());
    let get = |re: &Regex| {
        re.captures(text)
            .and_then(|c| c[1].parse::<i64>().ok())
            .unwrap_or(0)
    };
    let minutes = get(&HOURS) * 60 + get(&MINUTES);
    (minutes > 0).then(|| format_minutes(minutes))
}

fn first_text(doc: &Html, selector: &str) -> Option<String> {
    doc.select(&sel(selector))
        .next()
        .map(text_of)
        .filter(|t| !t.is_empty())
}

fn lazy_image(doc: &Html, selector: &str) -> Option<String> {
    let img = doc.select(&sel(selector)).next()?;
    ["data-lazy-src", "data-src", "data-lazy-srcset", "src"]
        .iter()
        .filter_map(|a| img.value().attr(a))
        .find(|v| !v.is_empty() && !v.starts_with("data:") && !v.contains(' '))
        .map(String::from)
}

/// A Tasty Recipes card in a post's content. Its ingredients are lists (with the amount and
/// unit also as `data-amount` / `data-unit`, which the text already carries) under headings,
/// its steps an ordered list.
pub fn recipe_from_tasty_markup(doc: &Html, url: &str) -> Option<RecipeFields> {
    let card = doc.select(&sel(".tasty-recipes")).next().is_some()
        || doc
            .select(&sel(".tasty-recipes-ingredients-body"))
            .next()
            .is_some();
    if !card {
        return None;
    }
    let sections = |selector: &str| -> Vec<Section> {
        doc.select(&sel(selector))
            .flat_map(headed_lists)
            .collect::<Vec<_>>()
    };
    let time = |part: &str| {
        first_text(doc, &format!(".tasty-recipes-{part}-time")).and_then(|t| minutes_in_text(&t))
    };
    finish(
        RecipeFields {
            url: Some(url.to_string()),
            title: first_text(doc, ".tasty-recipes-title").unwrap_or_default(),
            description: first_text(doc, ".tasty-recipes-description-body")
                .or_else(|| first_text(doc, ".tasty-recipes-description")),
            image: lazy_image(doc, ".tasty-recipes-image img"),
            author: first_text(doc, ".tasty-recipes-author-name"),
            prep_time: time("prep"),
            cook_time: time("cook"),
            total_time: time("total"),
            recipe_yield: first_text(doc, ".tasty-recipes-yield"),
            ingredients: sections(".tasty-recipes-ingredients-body"),
            instructions: sections(".tasty-recipes-instructions-body"),
            notes: notes_from_html(doc),
            video: video_from_html(doc, url),
            ..Default::default()
        },
        url,
    )
}

/// The card WP Recipe Maker leaves in a post's content for readers without its styles
/// (`wprm-fallback-recipe-*`): a name, a summary and plain lists of ingredients and steps.
/// Matched on the class-name prefix, so its group and header variants read too.
pub fn recipe_from_wprm_fallback(doc: &Html, url: &str) -> Option<RecipeFields> {
    let list_items = |token: &str| -> Vec<String> {
        doc.select(&sel(&format!("li[class*='{token}']")))
            .map(|li| block_lines(li).join(" "))
            .filter(|t| !t.is_empty())
            .collect()
    };
    let group = |token: &str| -> Vec<Section> {
        // Groups when the card has them (a wrapper with its own name), else one list
        let groups: Vec<Section> = doc
            .select(&sel(&format!(
                "[class*='wprm-fallback-recipe-{token}-group']"
            )))
            .filter_map(|g| {
                let items: Vec<String> = g
                    .select(&sel("li"))
                    .map(|li| block_lines(li).join(" "))
                    .filter(|t| !t.is_empty())
                    .collect();
                let name = g
                    .select(&sel("[class*='group-name'], h3, h4"))
                    .next()
                    .map(text_of)
                    .filter(|n| !n.is_empty());
                (!items.is_empty()).then_some(Section { name, items })
            })
            .collect();
        if groups.is_empty() {
            vec![Section::unnamed(list_items(&format!(
                "wprm-fallback-recipe-{token}"
            )))]
        } else {
            groups
        }
    };
    let ingredients = group("ingredient");
    let instructions = group("instruction");
    if ingredients.iter().all(|s| s.items.is_empty())
        && instructions.iter().all(|s| s.items.is_empty())
    {
        return None;
    }
    finish(
        RecipeFields {
            url: Some(url.to_string()),
            title: first_text(doc, "[class*='wprm-fallback-recipe-name']").unwrap_or_default(),
            description: first_text(doc, "[class*='wprm-fallback-recipe-summary']"),
            image: lazy_image(doc, "[class*='wprm-fallback-recipe-image'] img"),
            ingredients,
            instructions,
            notes: notes_from_html(doc),
            ..Default::default()
        },
        url,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> url::Url {
        url::Url::parse(s).unwrap()
    }

    const PAGE: &str = "https://food.test/2020/11/21/ginger-cookies/";

    #[test]
    fn parses_a_link_header() {
        let header = r#"<https://food.test/wp-json/>; rel="https://api.w.org/", <https://food.test/wp-json/wp/v2/posts/12>; rel="alternate"; title="JSON"; type="application/json", <https://food.test/?p=12>; rel=shortlink"#;
        let links = parse_link_header(header);
        assert_eq!(links.len(), 3);
        assert_eq!(links[0].target, "https://food.test/wp-json/");
        assert_eq!(links[0].rel, "https://api.w.org/");
        assert_eq!(links[1].rel, "alternate");
        assert_eq!(links[1].kind.as_deref(), Some("application/json"));
        assert_eq!(links[2].rel, "shortlink");
        assert!(parse_link_header("").is_empty());
        assert!(parse_link_header("nonsense; rel=x").is_empty());
    }

    #[test]
    fn the_link_header_names_the_post() {
        let header = r#"<https://food.test/wp-json/>; rel="https://api.w.org/", <https://food.test/wp-json/wp/v2/recipe/77>; rel="alternate"; type="application/json""#;
        let lead = wp_lead(Some(header), "", &u(PAGE)).unwrap();
        assert_eq!(lead.post, Some(("recipe".into(), 77)));
        assert_eq!(
            lead.root.unwrap().endpoint("/wp/v2/recipe/77", ""),
            "https://food.test/wp-json/wp/v2/recipe/77"
        );
    }

    #[test]
    fn a_sub_path_install_keeps_its_prefix() {
        let header = r#"<https://food.test/blog/wp-json/>; rel="https://api.w.org/", <https://food.test/blog/wp-json/wp/v2/posts/5>; rel="alternate"; type="application/json""#;
        let lead = wp_lead(Some(header), "", &u("https://food.test/blog/pie/")).unwrap();
        assert_eq!(lead.post, Some(("posts".into(), 5)));
        assert_eq!(
            lead.root.unwrap().endpoint("/wp/v2/posts", "slug=pie"),
            "https://food.test/blog/wp-json/wp/v2/posts?slug=pie"
        );
    }

    #[test]
    fn the_head_says_the_same_and_the_query_form_is_understood() {
        let html = r#"<html><head>
            <link rel='https://api.w.org/' href='https://food.test/?rest_route=/' />
            <link rel="alternate" title="JSON" type="application/json" href="https://food.test/?rest_route=/wp/v2/posts/12" />
            </head><body>x</body></html>"#;
        let lead = wp_lead(None, html, &u(PAGE)).unwrap();
        assert_eq!(lead.post, Some(("posts".into(), 12)));
        let root = lead.root.unwrap();
        assert!(root.is_query_form());
        assert_eq!(
            root.endpoint("/wp/v2/posts/12", "_fields=link"),
            "https://food.test/?rest_route=/wp/v2/posts/12&_fields=link"
        );
        assert_eq!(root.as_address(), "https://food.test/?rest_route=/");
        assert_eq!(WpRoot::parse(&root.as_address()), Some(root));
    }

    #[test]
    fn an_api_elsewhere_or_a_bad_route_is_not_taken() {
        let elsewhere = r#"<https://evil.test/wp-json/>; rel="https://api.w.org/", <https://evil.test/wp-json/wp/v2/posts/1>; rel="alternate"; type="application/json""#;
        assert_eq!(wp_lead(Some(elsewhere), "", &u(PAGE)), None);
        // Not something a request path should carry
        let odd = r#"<https://food.test/wp-json/wp/v2/po%20sts/1>; rel="alternate"; type="application/json""#;
        assert_eq!(wp_lead(Some(odd), "", &u(PAGE)), None);
        // www and http/https differences are the same site; the page's scheme is used
        let www = r#"<http://www.food.test/wp-json/>; rel="https://api.w.org/""#;
        let lead = wp_lead(Some(www), "", &u(PAGE)).unwrap();
        assert_eq!(
            lead.root.unwrap().as_address(),
            "https://www.food.test/wp-json/"
        );
        assert_eq!(wp_lead(None, "<p>plain</p>", &u(PAGE)), None);
    }

    #[test]
    fn the_root_can_be_asked_the_other_way() {
        let pretty = WpRoot::at_origin("https://food.test/");
        assert_eq!(
            pretty.endpoint("/wp/v2/types", ""),
            "https://food.test/wp-json/wp/v2/types"
        );
        let query = pretty.other_form();
        assert_eq!(
            query.endpoint("/wp/v2/posts", "slug=a"),
            "https://food.test/?rest_route=/wp/v2/posts&slug=a"
        );
        assert_eq!(query.other_form(), pretty);
    }

    #[test]
    fn recognises_the_platform() {
        assert_eq!(
            detect_platform(r#"<link href="/wp-content/themes/x/style.css">"#),
            Some(Platform::WordPress)
        );
        assert_eq!(
            detect_platform(r#"<meta name="generator" content="WordPress 6.5">"#),
            Some(Platform::WordPress)
        );
        assert_eq!(detect_platform("<p>hello</p>"), None);
        assert_eq!(
            platform_from_url(&u("https://food.test/?p=12")),
            Some(Platform::WordPress)
        );
        assert_eq!(
            platform_from_url(&u("https://food.test/wp-content/uploads/a")),
            Some(Platform::WordPress)
        );
        assert_eq!(platform_from_url(&u(PAGE)), None);
        assert_eq!(Platform::parse("wordpress"), Some(Platform::WordPress));
        assert_eq!(Platform::parse("ghost"), None);
    }

    #[test]
    fn finds_the_wprm_id_wherever_it_is_left() {
        assert_eq!(
            wprm_recipe_id(r#"<div id="wprm-recipe-container-16841">"#),
            Some(16841)
        );
        assert_eq!(wprm_recipe_id(r#"<div data-recipe-id="77">"#), Some(77));
        assert_eq!(
            wprm_recipe_id("<p>x</p><!--WPRM Recipe 4521--><p>y</p>"),
            Some(4521)
        );
        assert_eq!(wprm_recipe_id("<!-- WPRM Recipe 9 -->"), Some(9));
        assert_eq!(
            wprm_recipe_id(r#"<!-- wp:wp-recipe-maker/recipe {"id":31337,"updated":1} /-->"#),
            Some(31337)
        );
        assert_eq!(wprm_recipe_id("<p>no card</p>"), None);
    }

    #[test]
    fn finds_the_mediavine_id() {
        assert_eq!(
            mediavine_id(r#"<div id="mv-creation-903" class="mv-create-wrapper">"#),
            Some(903)
        );
        assert_eq!(mediavine_id(r#"<a data-mv-create-id="12">"#), Some(12));
        assert_eq!(mediavine_id("<p>none</p>"), None);
    }

    fn doc(html: &str) -> Html {
        Html::parse_document(&format!("<html><body>{html}</body></html>"))
    }

    #[test]
    fn reads_a_tasty_card() {
        let card = r#"<div class="tasty-recipes" data-tr-id="9">
          <h2 class="tasty-recipes-title">Skillet Pie</h2>
          <div class="tasty-recipes-image"><img src="data:image/gif;base64,R0" data-lazy-src="https://food.test/pie.jpg"></div>
          <div class="tasty-recipes-description"><p>Flaky and quick.</p></div>
          <span class="tasty-recipes-prep-time">15 minutes</span>
          <span class="tasty-recipes-cook-time">1 hour 5 mins</span>
          <span class="tasty-recipes-total-time">1 hour 20 minutes</span>
          <span class="tasty-recipes-yield"><span>6</span> servings</span>
          <div class="tasty-recipes-ingredients"><div class="tasty-recipes-ingredients-body">
            <h4>Crust</h4>
            <ul><li data-amount="2" data-unit="cups"><span data-amount="2" data-unit="cups">2 cups</span> flour</li>
                <li><span data-amount="1" data-unit="stick">1 stick</span> butter</li></ul>
            <h4>Filling</h4>
            <ul><li>3 apples, sliced</li></ul>
          </div></div>
          <div class="tasty-recipes-instructions"><div class="tasty-recipes-instructions-body">
            <ol><li>Rub butter into flour.</li><li>Bake.</li></ol>
          </div></div>
          <div class="tasty-recipes-notes"><h3>Notes</h3><p>Keeps two days.</p></div>
        </div>"#;
        let r = recipe_from_tasty_markup(&doc(card), PAGE).unwrap();
        assert_eq!(r.title, "Skillet Pie");
        assert_eq!(r.image.as_deref(), Some("https://food.test/pie.jpg"));
        assert_eq!(r.description.as_deref(), Some("Flaky and quick."));
        assert_eq!(r.prep_time.as_deref(), Some("15m"));
        assert_eq!(r.cook_time.as_deref(), Some("1h 5m"));
        assert_eq!(r.total_time.as_deref(), Some("1h 20m"));
        assert_eq!(r.recipe_yield.as_deref(), Some("6 servings"));
        assert_eq!(r.ingredients.len(), 2);
        assert_eq!(r.ingredients[0].name.as_deref(), Some("Crust"));
        assert_eq!(r.ingredients[0].items, ["2 cups flour", "1 stick butter"]);
        assert_eq!(r.ingredients[1].items, ["3 apples, sliced"]);
        assert_eq!(r.instructions[0].items, ["Rub butter into flour.", "Bake."]);
        assert_eq!(r.notes.as_deref(), Some("Keeps two days."));
        assert!(recipe_from_tasty_markup(&doc("<p>no card</p>"), PAGE).is_none());
    }

    #[test]
    fn reads_the_wprm_fallback_card() {
        let card = r#"<div class="wprm-fallback-recipe">
          <h2 class="wprm-fallback-recipe-name">Miso Soup</h2>
          <div class="wprm-fallback-recipe-summary">Simple <b>and</b> quick.</div>
          <h3 class="wprm-fallback-recipe-header">Ingredients</h3>
          <ul class="wprm-fallback-recipe-ingredients">
            <li class="wprm-fallback-recipe-ingredient">4 cups dashi</li>
            <li class="wprm-fallback-recipe-ingredient">3 tbsp miso</li>
          </ul>
          <h3 class="wprm-fallback-recipe-header">Instructions</h3>
          <ol class="wprm-fallback-recipe-instructions">
            <li class="wprm-fallback-recipe-instruction">Warm the dashi.</li>
            <li class="wprm-fallback-recipe-instruction">Whisk in the miso.</li>
          </ol>
        </div>"#;
        let r = recipe_from_wprm_fallback(&doc(card), PAGE).unwrap();
        assert_eq!(r.title, "Miso Soup");
        assert_eq!(r.description.as_deref(), Some("Simple and quick."));
        assert_eq!(r.ingredients[0].items, ["4 cups dashi", "3 tbsp miso"]);
        assert_eq!(
            r.instructions[0].items,
            ["Warm the dashi.", "Whisk in the miso."]
        );
        assert!(recipe_from_wprm_fallback(&doc("<p>nothing</p>"), PAGE).is_none());
    }

    #[test]
    fn reads_times_written_out() {
        assert_eq!(minutes_in_text("45 mins").as_deref(), Some("45m"));
        assert_eq!(minutes_in_text("2 hrs").as_deref(), Some("2h"));
        assert_eq!(
            minutes_in_text("1 hour 15 minutes").as_deref(),
            Some("1h 15m")
        );
        assert_eq!(minutes_in_text("overnight"), None);
    }
}
