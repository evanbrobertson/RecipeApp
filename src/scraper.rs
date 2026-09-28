//! Recipe scraping: fetch the page, then JSON-LD first, then recipe-plugin markup and microdata.
//!
//! Pages are fetched with `wreq` (see `crumb-fetch`), which sends a real browser's TLS and
//! HTTP/2 fingerprint and headers. Many recipe sites (behind Cloudflare, Akamai, PerimeterX and
//! the like) refuse a plain Rust client on its fingerprint alone, whatever its User-Agent says.
//! The order is Firefox, then Safari when the site blocks it. When both were refused or
//! challenged (not for a page that simply has no recipe), the ways round the block come next:
//! the site's own WordPress API, a relay on another network (`crumb-relay`, see `crate::relay`),
//! then the Internet Archive's copy ([`fallbacks`]). Last is headless Chromium when installed,
//! the dearest, which also takes pages that need JavaScript.

use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use serde_json::{Map, Value};
use std::future::Future;
use std::sync::LazyLock;

use crumb_fetch::Profile;
pub use crumb_fetch::{MAX_PAGE_BYTES, ReadError, read_capped};

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::model::{RecipeFields, Section, normalize_sections};

pub mod fallbacks;

/// The User-Agent for the plain `reqwest` image fallback (wreq's profiles set their own).
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

const PASTE_HINT: &str = "Try copying the recipe text and pasting it instead.";

/// What a cook is told when the site's bot check turned the server away. The extension reads
/// the page from their own browser, which is already past the check.
const BLOCKED_MESSAGE: &str = "This site asked for a human check, so Crumb couldn't read it from here. The Crumb extension can read it from your browser, which is already past the check.";

/// The `AppError` code for [`BLOCKED_MESSAGE`]; the web and MCP key their nudges on it.
pub const SITE_BLOCKED: &str = "site_blocked";

/// How a page was fetched, in the order they're tried.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Firefox,
    Safari,
    /// The site's own WordPress REST API, for a blog that blocks its pages.
    WordPress,
    /// A `crumb-relay` on another network (see `crate::relay`).
    Relay,
    /// The Internet Archive's copy of the page.
    Archive,
    Browser,
}

impl Method {
    /// The name in the log line for an import.
    pub fn label(self) -> &'static str {
        match self {
            Method::Firefox => Profile::Firefox.label(),
            Method::Safari => Profile::Safari.label(),
            Method::WordPress => "wordpress-api",
            Method::Relay => "relay",
            Method::Archive => "wayback",
            Method::Browser => "browser",
        }
    }

    /// The browser profile this method fetches with itself, if it does.
    fn profile(self) -> Option<Profile> {
        match self {
            Method::Firefox => Some(Profile::Firefox),
            Method::Safari => Some(Profile::Safari),
            _ => None,
        }
    }
}

/// What one step of a scrape got back: a fetch's answer (see [`crumb_fetch::Fetched`]), or a
/// recipe read without a page.
#[derive(Debug)]
pub enum Fetched {
    /// A response. The body is only read for a 2xx.
    Page { status: u16, html: String },
    /// No response (DNS, connection, TLS, timeout); the reason is for the log.
    Unreachable(String),
    /// A recipe read without a page (see [`fallbacks::fetch_wordpress`]).
    Recipe(Box<Scraped>),
}

impl From<crumb_fetch::Fetched> for Fetched {
    fn from(fetched: crumb_fetch::Fetched) -> Self {
        match fetched {
            crumb_fetch::Fetched::Page { status, html } => Fetched::Page { status, html },
            crumb_fetch::Fetched::Unreachable(why) => Fetched::Unreachable(why),
        }
    }
}

/// The shared browser-profile client for `method` (Firefox or Safari). Built on first use;
/// `None` if it can't be built (logged once), and then that step is skipped.
pub fn wreq_client(method: Method) -> Option<&'static wreq::Client> {
    crumb_fetch::client(method.profile()?)
}

/// Fetches a page with one of the wreq browser profiles. The profile sets every header.
pub async fn fetch_wreq(method: Method, url: &str) -> Fetched {
    match method.profile() {
        Some(profile) => crumb_fetch::fetch(profile, url).await.into(),
        None => Fetched::Unreachable("client unavailable".into()),
    }
}

static CHALLENGE_TITLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)just a moment|attention required|access denied|access to this page has been denied|verify you are human|are you a robot|not a robot|pardon our interruption|security check",
    )
    .unwrap()
});

static TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<title[^>]*>(.*?)</title>").unwrap());

/// Page markers that only bot-check and block pages carry.
const CHALLENGE_MARKERS: [&str; 6] = [
    "px-captcha",              // PerimeterX / HUMAN
    "_Incapsula_Resource",     // Imperva
    "window._cf_chl_opt",      // Cloudflare challenge
    "cf-browser-verification", // Cloudflare (older)
    "captcha-delivery.com",    // DataDome
    "errors.edgesuite.net",    // Akamai "Access Denied" reference
];

/// Whether a page title is a bot check ("Just a moment...") rather than the page.
pub fn is_challenge_title(title: &str) -> bool {
    CHALLENGE_TITLE.is_match(title)
}

/// Whether HTML is a bot check or block page. Only asked of pages with no recipe, so a
/// false positive costs one retry, never a lost recipe.
pub fn is_challenge_page(html: &str) -> bool {
    TITLE
        .captures(html)
        .is_some_and(|c| is_challenge_title(&c[1]))
        || CHALLENGE_MARKERS.iter().any(|m| html.contains(m))
}

/// Statuses bot protection answers with; another browser profile may get through.
fn is_block_status(status: u16) -> bool {
    matches!(status, 403 | 429 | 503)
}

/// What a recipe page gave: the recipe as scraped, and the address of its Crumb export
/// when the page is another Crumb's share page (see [`crumb_alternate`]).
#[derive(Debug, Clone)]
pub struct Scraped {
    pub recipe: RecipeFields,
    pub crumb: Option<String>,
}

impl Scraped {
    /// The recipe on the page. Another Crumb's shared cookbook has none of its own, only
    /// its export: then the recipe is empty (no title) and `crumb` says where to look.
    fn from_page(html: &str, url: &str) -> Option<Self> {
        let crumb = crumb_alternate(html, url);
        let recipe = match parse_recipe_html(html, url) {
            Some(recipe) => recipe,
            None if crumb.is_some() => RecipeFields::default(),
            None => return None,
        };
        Some(Self { recipe, crumb })
    }
}

/// The type a Crumb share page names its lossless export by.
pub const CRUMB_JSON_TYPE: &str = "application/vnd.crumb+json";

/// `<link rel="alternate" type="application/vnd.crumb+json" href>` on a page: another Crumb's
/// share page offering the recipe as a Crumb export. Only an http(s) address on the page's
/// own origin (scheme, host and port) is taken, so a page can't point the fetch elsewhere.
pub fn crumb_alternate(html: &str, page_url: &str) -> Option<String> {
    if !html.contains(CRUMB_JSON_TYPE) {
        return None;
    }
    let base = url::Url::parse(page_url).ok()?;
    let doc = Html::parse_document(html);
    let link = sel(r#"link[rel~="alternate"]"#);
    let href = doc
        .select(&link)
        .find(|l| l.value().attr("type") == Some(CRUMB_JSON_TYPE))?
        .value()
        .attr("href")?;
    let target = base.join(href.trim()).ok()?;
    (matches!(target.scheme(), "http" | "https") && same_origin(&target, &base))
        .then(|| target.to_string())
}

/// Same scheme, host and port (a missing port is the scheme's default).
pub fn same_origin(a: &url::Url, b: &url::Url) -> bool {
    a.host_str().is_some()
        && a.scheme() == b.scheme()
        && a.host_str().map(str::to_ascii_lowercase) == b.host_str().map(str::to_ascii_lowercase)
        && a.port_or_known_default() == b.port_or_known_default()
}

enum Verdict {
    Recipe(Box<Scraped>),
    /// Refused or challenged: worth another profile.
    Blocked(String),
    /// Anything else that didn't give a recipe.
    Failed(String),
}

/// Why a scrape gave nothing: the message for the cook, and whether the last thing that
/// happened was the site's bot protection turning the server away.
#[derive(Debug)]
pub struct ScrapeError {
    pub message: String,
    pub blocked: bool,
}

fn judge(fetched: Fetched, url: &str) -> Verdict {
    match fetched {
        Fetched::Recipe(scraped) => Verdict::Recipe(scraped),
        Fetched::Unreachable(_) => Verdict::Failed("Couldn't reach that site.".into()),
        Fetched::Page { status, .. } if is_block_status(status) => {
            Verdict::Blocked(format!("The site responded with {status}."))
        }
        Fetched::Page { status, .. } if !(200..300).contains(&status) => {
            Verdict::Failed(format!("The site responded with {status}."))
        }
        Fetched::Page { html, .. } => match Scraped::from_page(&html, url) {
            Some(scraped) => Verdict::Recipe(Box::new(scraped)),
            None if is_challenge_page(&html) => {
                Verdict::Blocked("The site showed a bot check instead of the recipe.".into())
            }
            None => Verdict::Failed("Couldn't find a recipe on that page.".into()),
        },
    }
}

/// Which steps after Firefox and Safari are on.
#[derive(Clone, Copy, Debug)]
pub struct Steps {
    /// Headless Chromium: only if it's installed and not switched off.
    pub browser: bool,
    /// The site's WordPress API; `SCRAPE_WORDPRESS=off` turns it off.
    pub wordpress: bool,
    /// The Internet Archive's copy; `SCRAPE_ARCHIVE=off` turns it off.
    pub archive: bool,
    /// The relays: only if some are set up (`SCRAPE_RELAYS`).
    pub relay: bool,
}

impl Steps {
    /// Every step on, less the ones the environment switches off; the browser is on only
    /// if it can run (`browser`).
    pub fn from_env(browser: bool, relay: bool) -> Self {
        Self {
            browser,
            wordpress: !crate::config::switched_off("SCRAPE_WORDPRESS"),
            archive: !crate::config::switched_off("SCRAPE_ARCHIVE"),
            relay,
        }
    }

    fn on(self, method: Method) -> bool {
        match method {
            Method::Firefox | Method::Safari => true,
            Method::WordPress => self.wordpress,
            Method::Relay => self.relay,
            Method::Archive => self.archive,
            Method::Browser => self.browser,
        }
    }
}

/// The ways round a block, in order: tried when Firefox and Safari were both refused, and
/// before the browser. A new one goes in here.
const BLOCKED_STEPS: [Method; 3] = [Method::WordPress, Method::Relay, Method::Archive];

/// The fetch order, with the fetchers passed in (so it's testable without a network):
/// Firefox; Safari if Firefox was blocked; if Safari was blocked too, the [`BLOCKED_STEPS`];
/// then the browser (when `steps.browser` is true) if nothing gave a recipe. Returns the
/// method that worked, or the message for the cook.
pub async fn scrape_with<F, Fut>(
    url: &str,
    steps: Steps,
    mut fetch: F,
) -> Result<(Method, Scraped), ScrapeError>
where
    F: FnMut(Method) -> Fut,
    Fut: Future<Output = Fetched>,
{
    let (mut problem, mut blocked) = match judge(fetch(Method::Firefox).await, url) {
        Verdict::Recipe(recipe) => return Ok((Method::Firefox, *recipe)),
        Verdict::Blocked(_) => match judge(fetch(Method::Safari).await, url) {
            Verdict::Recipe(recipe) => return Ok((Method::Safari, *recipe)),
            Verdict::Blocked(p) => (p, true),
            Verdict::Failed(p) => (p, false),
        },
        Verdict::Failed(p) => (p, false),
    };

    // A step that gets nothing leaves the block's own message as the problem
    if blocked {
        for method in BLOCKED_STEPS.into_iter().filter(|m| steps.on(*m)) {
            if let Verdict::Recipe(recipe) = judge(fetch(method).await, url) {
                return Ok((method, *recipe));
            }
        }
    }

    if steps.browser {
        match fetch(Method::Browser).await {
            Fetched::Page { html, .. } => match Scraped::from_page(&html, url) {
                Some(scraped) => return Ok((Method::Browser, scraped)),
                None => {
                    problem = "Couldn't find a recipe on that page, even in a real browser.".into();
                    blocked = is_challenge_page(&html);
                }
            },
            Fetched::Recipe(scraped) => return Ok((Method::Browser, *scraped)),
            Fetched::Unreachable(err) => {
                tracing::warn!(
                    "[scraper] browser fallback failed for {}: {err}",
                    crate::telemetry::host_of(url)
                );
                problem = format!("{problem} A real browser was blocked too.");
            }
        }
    }

    Err(ScrapeError {
        message: format!("{problem} {PASTE_HINT}"),
        blocked,
    })
}

/// The page the first relay that can gives, or "unreachable" (also with no relay set up).
async fn fetch_relay(relays: &crate::relay::Relays, url: &str) -> Fetched {
    let got = relays
        .fetch(url, |fetched| match fetched {
            crumb_fetch::Fetched::Page { html, .. } => Scraped::from_page(html, url).is_some(),
            crumb_fetch::Fetched::Unreachable(_) => false,
        })
        .await;
    got.map_or_else(
        || Fetched::Unreachable("no relay gave the page".into()),
        Into::into,
    )
}

/// Scrapes a recipe page (see [`scrape_with`] for the order it tries).
pub async fn scrape_recipe(state: &AppState, url: &str) -> AppResult<RecipeFields> {
    let recipe = scrape_page(state, url).await?.recipe;
    if recipe.title.trim().is_empty() {
        return Err(AppError::new(422, "Couldn't find a recipe on that page."));
    }
    Ok(recipe)
}

/// [`scrape_recipe`], also saying whether the page offers a Crumb export.
pub async fn scrape_page(state: &AppState, url: &str) -> AppResult<Scraped> {
    let parsed =
        url::Url::parse(url).map_err(|_| AppError::bad_request("Please enter a valid URL"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::bad_request("Only http(s) links are supported."));
    }

    let browser = state.browser.clone();
    let result = scrape_with(
        url,
        Steps::from_env(browser.available(), !state.relays.is_empty()),
        |method| {
            let browser = browser.clone();
            let relays = state.relays.clone();
            async move {
                match method {
                    Method::Browser => match browser.fetch(url).await {
                        Ok(html) => Fetched::Page { status: 200, html },
                        Err(err) => Fetched::Unreachable(err),
                    },
                    Method::WordPress => fallbacks::fetch_wordpress(url).await,
                    Method::Relay => fetch_relay(&relays, url).await,
                    Method::Archive => fallbacks::fetch_archive(url).await,
                    wreq => fetch_wreq(wreq, url).await,
                }
            }
        },
    )
    .await;

    let host = crate::telemetry::host_of(url);
    match result {
        Ok((method, scraped)) => {
            tracing::info!("[scraper] {host}: {}", method.label());
            Ok(scraped)
        }
        Err(err) => {
            tracing::info!("[scraper] {host}: failed");
            Err(if err.blocked {
                AppError::new(422, BLOCKED_MESSAGE).with_code(SITE_BLOCKED)
            } else {
                AppError::new(422, err.message)
            })
        }
    }
}

fn sel(s: &str) -> Selector {
    Selector::parse(s).expect("valid selector")
}

fn text_of(el: ElementRef) -> String {
    el.text().collect::<String>().trim().to_string()
}

/// Extracts a recipe from page HTML (JSON-LD first, then microdata/selectors).
pub fn parse_recipe_html(html: &str, url: &str) -> Option<RecipeFields> {
    let doc = Html::parse_document(html);
    let recipe = match extract_json_ld(&doc) {
        Some(ld) => normalize_json_ld(&ld, url, Some(&doc)),
        None => extract_from_html(&doc, url),
    };
    finish(recipe, url)
}

/// Converts a schema.org Recipe object (e.g. from an export file) into recipe fields.
pub fn recipe_from_json_ld(data: &Value, url: &str) -> Option<RecipeFields> {
    let ld = find_recipe_in_json_ld(data)?;
    let source = if !url.is_empty() {
        url.to_string()
    } else {
        ld.get("url")
            .and_then(Value::as_str)
            .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
            .unwrap_or("")
            .to_string()
    };
    let mut recipe = finish(normalize_json_ld(ld, &source, None), &source)?;
    if source.is_empty() {
        recipe.url = None;
    }
    Some(recipe)
}

fn finish(mut recipe: RecipeFields, url: &str) -> Option<RecipeFields> {
    recipe.ingredients =
        normalize_sections(recipe.ingredients.into_iter().map(decode_section).collect());
    // Some sites' JSON-LD has "0.33333334 cup" where the page shows "⅓ cup"
    crate::fractions::fractionize_sections(&mut recipe.ingredients);
    recipe.instructions = normalize_sections(
        recipe
            .instructions
            .into_iter()
            .map(decode_section)
            .collect(),
    );
    let title = decode_text(&recipe.title);
    recipe.title = if title.is_empty() {
        "Untitled recipe".into()
    } else {
        title
    };
    recipe.description = recipe
        .description
        .map(|d| decode_text(&d))
        .filter(|d| !d.is_empty());
    recipe.image = if url.is_empty() {
        recipe.image.filter(|i| !i.is_empty())
    } else {
        absolute_url(recipe.image.as_deref(), url)
    };
    if recipe.url.as_deref() == Some("") {
        recipe.url = None;
    }
    // The site's own wording ("Dinner, Entree, Sandwich") filed under Crumb's list
    recipe.recipe_category = crate::categories::for_import(
        recipe
            .recipe_category
            .as_deref()
            .map(decode_text)
            .as_deref(),
    );
    if recipe.ingredients.is_empty() && recipe.instructions.is_empty() {
        return None;
    }
    Some(recipe)
}

static WS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

fn collapse(s: &str) -> String {
    WS.replace_all(s, " ").trim().to_string()
}

/// Decodes HTML entities and strips any stray markup from scraped strings.
pub fn decode_text(value: &str) -> String {
    if !value.contains(['<', '&']) {
        return collapse(value);
    }
    let frag = Html::parse_fragment(value);
    collapse(&frag.root_element().text().collect::<String>())
}

fn decode_section(s: Section) -> Section {
    Section {
        name: s.name.map(|n| decode_text(&n)),
        items: s.items.iter().map(|i| decode_text(i)).collect(),
    }
}

fn absolute_url(value: Option<&str>, base: &str) -> Option<String> {
    let value = value.filter(|v| !v.is_empty())?;
    url::Url::parse(base)
        .ok()?
        .join(value)
        .ok()
        .map(|u| u.to_string())
}

fn extract_json_ld(doc: &Html) -> Option<Map<String, Value>> {
    for script in doc.select(&sel(r#"script[type="application/ld+json"]"#)) {
        let text: String = script.text().collect();
        if text.trim().is_empty() {
            continue;
        }
        let Ok(data) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        if let Some(recipe) = find_recipe_in_json_ld(&data) {
            return Some(recipe.clone());
        }
    }
    None
}

fn is_type(v: Option<&Value>, name: &str) -> bool {
    match v {
        Some(Value::String(s)) => s == name,
        Some(Value::Array(a)) => a.iter().any(|t| t.as_str() == Some(name)),
        _ => false,
    }
}

pub fn find_recipe_in_json_ld(data: &Value) -> Option<&Map<String, Value>> {
    match data {
        Value::Array(items) => items.iter().find_map(find_recipe_in_json_ld),
        Value::Object(obj) => {
            if is_type(obj.get("@type"), "Recipe") {
                return Some(obj);
            }
            match obj.get("@graph") {
                Some(graph @ Value::Array(_)) => find_recipe_in_json_ld(graph),
                _ => None,
            }
        }
        _ => None,
    }
}

fn scalar_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn normalize_json_ld(ld: &Map<String, Value>, url: &str, doc: Option<&Html>) -> RecipeFields {
    // JSON-LD flat list first; fall back to HTML ingredient groups
    let mut ingredients = normalize_ingredient_sections(&strings(ld.get("recipeIngredient")));
    if ingredients.len() == 1
        && ingredients[0].name.is_none()
        && let Some(doc) = doc
    {
        let groups = extract_ingredient_groups_from_html(doc);
        if !groups.is_empty() {
            ingredients = groups;
        }
    }
    let str_field = |k: &str| ld.get(k).and_then(Value::as_str).map(String::from);
    let time = |k: &str| format_duration(ld.get(k).and_then(Value::as_str));

    RecipeFields {
        url: Some(url.to_string()),
        title: str_field("name")
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Untitled recipe".into()),
        description: str_field("description").filter(|d| !d.is_empty()),
        image: normalize_image(ld.get("image")),
        author: normalize_author(ld.get("author")),
        prep_time: time("prepTime"),
        cook_time: time("cookTime"),
        total_time: time("totalTime"),
        freeze_time: compute_additional_time(
            ld.get("prepTime").and_then(Value::as_str),
            ld.get("cookTime").and_then(Value::as_str),
            ld.get("totalTime").and_then(Value::as_str),
        ),
        recipe_yield: string_or_array(ld.get("recipeYield")),
        recipe_category: string_or_array(ld.get("recipeCategory")),
        recipe_cuisine: string_or_array(ld.get("recipeCuisine")),
        ingredients,
        instructions: normalize_instructions(ld.get("recipeInstructions")),
        nutrition: normalize_nutrition(ld.get("nutrition")),
        notes: doc.and_then(notes_from_html),
        video: json_ld_video(ld, url, doc).or_else(|| doc.and_then(|d| video_from_html(d, url))),
    }
}

/// The recipe's video from its JSON-LD: a `VideoObject` (or a list of them, or an `@id`
/// pointing at one elsewhere on the page), as a link Crumb can play.
fn json_ld_video(ld: &Map<String, Value>, url: &str, doc: Option<&Html>) -> Option<String> {
    let videos = match ld.get("video")? {
        Value::Array(a) => a.iter().collect(),
        v => vec![v],
    };
    videos.into_iter().find_map(|v| match v {
        Value::String(s) => playable(s, url),
        Value::Object(o) => video_object_link(o, url).or_else(|| {
            let id = o.get("@id").and_then(Value::as_str)?;
            let node = json_ld_node(doc?, id)?;
            video_object_link(&node, url)
        }),
        _ => None,
    })
}

fn video_object_link(o: &Map<String, Value>, url: &str) -> Option<String> {
    ["embedUrl", "contentUrl", "url"]
        .iter()
        .filter_map(|k| o.get(*k).and_then(Value::as_str))
        .find_map(|u| playable(u, url))
}

/// The JSON-LD node on the page with this `@id`.
fn json_ld_node(doc: &Html, id: &str) -> Option<Map<String, Value>> {
    fn find(v: &Value, id: &str) -> Option<Map<String, Value>> {
        match v {
            Value::Array(a) => a.iter().find_map(|v| find(v, id)),
            Value::Object(o) if o.get("@id").and_then(Value::as_str) == Some(id) && o.len() > 1 => {
                Some(o.clone())
            }
            Value::Object(o) => o.get("@graph").and_then(|g| find(g, id)),
            _ => None,
        }
    }
    doc.select(&sel(r#"script[type="application/ld+json"]"#))
        .filter_map(|s| serde_json::from_str::<Value>(&s.text().collect::<String>()).ok())
        .find_map(|data| find(&data, id))
}

/// A video address found on a page, made absolute, as the link Crumb keeps: only videos
/// it can play (an ad network's player script is no use to the cook).
fn playable(value: &str, base: &str) -> Option<String> {
    crumb_core::embed::video_link(&absolute_url(Some(value.trim()), base)?)
}

/// Where a player element keeps its video: lazy loaders park an iframe's address in a
/// data attribute until it scrolls into view, and YouTube facades keep just the id.
fn element_video(el: ElementRef, base: &str) -> Option<String> {
    let e = el.value();
    if let Some(id) = e.attr("videoid").or_else(|| {
        e.has_class(
            "rll-youtube-player",
            scraper::CaseSensitivity::AsciiCaseInsensitive,
        )
        .then(|| e.attr("data-id"))
        .flatten()
    }) {
        return playable(&format!("https://www.youtube.com/watch?v={id}"), base);
    }
    [
        "src",
        "data-src",
        "data-lazy-src",
        "data-litespeed-src",
        "data-rocket-src",
    ]
    .iter()
    .filter_map(|a| e.attr(a))
    .find_map(|u| playable(u, base))
}

/// The recipe's video from the page itself: the recipe card's player first, then the
/// page's declared video, then the first one in the post. Never the sidebar or footer,
/// where a site shows its other videos.
fn video_from_html(doc: &Html, url: &str) -> Option<String> {
    let players = sel("iframe, lite-youtube, .rll-youtube-player, video, video source");
    let within = |containers: &str| {
        doc.select(&sel(containers))
            .flat_map(|c| std::iter::once(c).chain(c.select(&players)))
            .find_map(|el| element_video(el, url))
    };
    within(
        ".wprm-recipe-video, .wprm-recipe-video-container, .tasty-recipes-video-embed, \
         .tasty-recipe-video-embed, .mv-create-video, .recipe-video, [class*='recipe-video']",
    )
    .or_else(|| {
        doc.select(&sel(
            r#"meta[property="og:video:secure_url"], meta[property="og:video:url"],
               meta[property="og:video"], meta[name="twitter:player"]"#,
        ))
        .filter_map(|m| m.value().attr("content"))
        .find_map(|u| playable(u, url))
    })
    .or_else(|| within("article, .entry-content, .post-content, main"))
}

/// A notes block's text, a line per paragraph or list item.
fn block_lines(el: ElementRef) -> Vec<String> {
    const BLOCKS: [&str; 16] = [
        "p",
        "div",
        "li",
        "ul",
        "ol",
        "br",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "blockquote",
        "tr",
        "section",
        "dd",
    ];
    fn flush(lines: &mut Vec<String>, line: &mut String) {
        let text = collapse(line);
        if !text.is_empty() && text != "•" {
            lines.push(text);
        }
        line.clear();
    }
    fn walk(el: ElementRef, lines: &mut Vec<String>, line: &mut String) {
        for child in el.children() {
            if let Some(text) = child.value().as_text() {
                line.push_str(text);
                continue;
            }
            let Some(child) = ElementRef::wrap(child) else {
                continue;
            };
            let e = child.value();
            let name = e.name();
            if matches!(
                name,
                "script" | "style" | "noscript" | "svg" | "button" | "template" | "img"
            ) {
                continue;
            }
            // WPRM writes each line of its notes as a <span style="display: block">
            let block = BLOCKS.contains(&name)
                || e.attr("style")
                    .is_some_and(|s| s.replace(' ', "").contains("display:block"));
            if block {
                flush(lines, line);
            }
            if name == "li" {
                line.push_str("• ");
            }
            walk(child, lines, line);
            if block {
                flush(lines, line);
            }
        }
    }
    let (mut lines, mut line) = (Vec::new(), String::new());
    walk(el, &mut lines, &mut line);
    flush(&mut lines, &mut line);
    lines
}

/// The recipe card's notes (tips, substitutions, storage), which JSON-LD has no place for.
fn notes_from_html(doc: &Html) -> Option<String> {
    static HEADER: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^(recipe |cook'?s? )?notes?:?$").unwrap());
    [
        ".wprm-recipe-notes",
        ".tasty-recipes-notes-body",
        ".tasty-recipes-notes",
        ".mv-create-notes-content",
        ".mv-create-notes",
        ".recipe-notes, .recipe-card-notes, .wpzoom-rcb-note, .ERSNotes",
    ]
    .iter()
    .find_map(|s| {
        doc.select(&sel(s)).find_map(|el| {
            let mut lines = block_lines(el);
            if lines.first().is_some_and(|l| HEADER.is_match(l)) {
                lines.remove(0);
            }
            Some(lines.join("\n")).filter(|n| !n.is_empty())
        })
    })
}

fn strings(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

fn normalize_image(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Array(a) => normalize_image(a.first()),
        Value::Object(o) => o.get("url").and_then(Value::as_str).map(String::from),
        _ => None,
    }
}

fn normalize_author(v: Option<&Value>) -> Option<String> {
    let name_of = |a: &Value| match a {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => o.get("name").and_then(Value::as_str).map(String::from),
        _ => None,
    };
    match v? {
        Value::Array(a) => {
            let names: Vec<String> = a.iter().filter_map(name_of).collect();
            if names.is_empty() {
                None
            } else {
                Some(names.join(", "))
            }
        }
        other => name_of(other).filter(|s| !s.is_empty()),
    }
}

fn string_or_array(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::Array(a) => {
            let parts: Vec<String> = a.iter().filter_map(scalar_string).collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join(", "))
            }
        }
        other => scalar_string(other).filter(|s| !s.is_empty()),
    }
}

fn heading_level(el: &ElementRef, levels: &[&str]) -> bool {
    levels.contains(&el.value().name())
}

/// Items from the lists that follow a heading, up to the next heading.
fn items_after_heading(heading: ElementRef, levels: &[&str]) -> Vec<String> {
    let li = sel("li");
    let mut items = Vec::new();
    for node in heading.next_siblings() {
        let Some(el) = ElementRef::wrap(node) else {
            continue;
        };
        if heading_level(&el, levels) {
            break;
        }
        if matches!(el.value().name(), "ul" | "ol") {
            items.extend(el.select(&li).map(text_of).filter(|t| !t.is_empty()));
        }
    }
    items
}

/// Extracts ingredient groups from recipe-plugin markup (WPRM, Tasty Recipes, generic
/// headed lists). Returns an empty list if no grouped structure is found.
fn extract_ingredient_groups_from_html(doc: &Html) -> Vec<Section> {
    let mut sections = Vec::new();

    // WPRM (WP Recipe Maker), the most common WordPress recipe plugin
    let groups: Vec<_> = doc.select(&sel(".wprm-recipe-ingredient-group")).collect();
    if !groups.is_empty() {
        let name_sel = sel(".wprm-recipe-group-name");
        let item_sel = sel(".wprm-recipe-ingredient");
        for group in groups {
            let name = group
                .select(&name_sel)
                .next()
                .map(text_of)
                .filter(|n| !n.is_empty());
            let items: Vec<String> = group
                .select(&item_sel)
                .map(text_of)
                .filter(|t| !t.is_empty())
                .collect();
            if !items.is_empty() {
                sections.push(Section { name, items });
            }
        }
        if sections.len() > 1 || (sections.len() == 1 && sections[0].name.is_some()) {
            return sections;
        }
        sections.clear();
    }

    // Tasty Recipes plugin
    let tasty: Vec<_> = doc
        .select(&sel(".tasty-recipes-ingredients-body"))
        .collect();
    if !tasty.is_empty() {
        let levels = ["h4", "h3", "h2"];
        let heading_sel = sel("h4, h3, h2");
        for body in &tasty {
            for heading in body.select(&heading_sel) {
                let name = Some(text_of(heading)).filter(|n| !n.is_empty());
                let items = items_after_heading(heading, &levels);
                if !items.is_empty() {
                    sections.push(Section { name, items });
                }
            }
        }
        if !sections.is_empty() {
            return sections;
        }
    }

    // Generic: ingredient containers with internal headings
    let levels = ["h2", "h3", "h4", "h5"];
    let heading_sel = sel("h2, h3, h4, h5");
    for container in doc.select(&sel(
        ".recipe-ingredients, .ingredients-section, [class*='ingredient-group']",
    )) {
        for heading in container.select(&heading_sel) {
            let name = Some(text_of(heading)).filter(|n| !n.is_empty());
            let items = items_after_heading(heading, &levels);
            if !items.is_empty() {
                sections.push(Section { name, items });
            }
        }
    }
    sections
}

/// A flat ingredient string that is really a header: "For the sauce:", "Cake:".
fn is_ingredient_section_header(text: &str) -> bool {
    let t = text.trim();
    t.ends_with(':') && t.chars().count() < 60 && !t.chars().any(|c| c.is_ascii_digit())
}

/// Splits a flat ingredient list into sections by detecting header items.
fn normalize_ingredient_sections(ingredients: &[String]) -> Vec<Section> {
    if ingredients.is_empty() {
        return vec![Section::default()];
    }
    let mut sections = Vec::new();
    let mut current = Section::default();
    for item in ingredients {
        if is_ingredient_section_header(item) {
            if !current.items.is_empty() {
                sections.push(current);
            }
            let name = item.trim().trim_end_matches(':').trim().to_string();
            current = Section {
                name: Some(name),
                items: vec![],
            };
        } else {
            current.items.push(item.clone());
        }
    }
    if !current.items.is_empty() || !sections.is_empty() {
        sections.push(current);
    }
    if sections.is_empty() {
        vec![Section::default()]
    } else {
        sections
    }
}

/// A HowToStep's text (schema.org steps sometimes omit @type).
fn step_text(item: &Map<String, Value>) -> Option<String> {
    let typed = item.get("@type");
    if typed.is_some() && !is_type(typed, "HowToStep") {
        return None;
    }
    item.get("text")
        .and_then(Value::as_str)
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

fn normalize_instructions(v: Option<&Value>) -> Vec<Section> {
    let items = match v {
        Some(Value::String(s)) => {
            return vec![Section::unnamed(
                s.split('\n')
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(String::from)
                    .collect(),
            )];
        }
        Some(Value::Array(items)) => items,
        _ => return vec![Section::default()],
    };

    let has_sections = items.iter().any(|i| {
        i.as_object()
            .is_some_and(|o| is_type(o.get("@type"), "HowToSection"))
    });

    if !has_sections {
        let steps = items
            .iter()
            .filter_map(|i| match i {
                Value::String(s) => Some(s.trim().to_string()),
                Value::Object(o) => step_text(o),
                _ => None,
            })
            .collect();
        return vec![Section::unnamed(steps)];
    }

    let mut sections = Vec::new();
    let mut current = Section::default();
    for item in items {
        match item {
            Value::String(s) => current.items.push(s.trim().to_string()),
            Value::Object(o) if is_type(o.get("@type"), "HowToSection") => {
                if !current.items.is_empty() {
                    sections.push(std::mem::take(&mut current));
                }
                let steps = match o.get("itemListElement") {
                    Some(Value::Array(list)) => list
                        .iter()
                        .filter_map(|s| s.get("text").and_then(Value::as_str))
                        .map(|t| t.trim().to_string())
                        .collect(),
                    _ => Vec::new(),
                };
                let name = o
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|n| !n.is_empty());
                sections.push(Section {
                    name: name.map(String::from),
                    items: steps,
                });
            }
            Value::Object(o) => {
                if let Some(t) = step_text(o) {
                    current.items.push(t);
                }
            }
            _ => {}
        }
    }
    if !current.items.is_empty() {
        sections.push(current);
    }
    if sections.is_empty() {
        vec![Section::default()]
    } else {
        sections
    }
}

const NUTRITION_FIELDS: [&str; 12] = [
    "calories",
    "fatContent",
    "saturatedFatContent",
    "unsaturatedFatContent",
    "transFatContent",
    "carbohydrateContent",
    "sugarContent",
    "fiberContent",
    "proteinContent",
    "cholesterolContent",
    "sodiumContent",
    "servingSize",
];

fn normalize_nutrition(v: Option<&Value>) -> Option<Map<String, Value>> {
    let o = v?.as_object()?;
    let mut out = Map::new();
    for f in NUTRITION_FIELDS {
        if let Some(Value::String(s)) = o.get(f)
            && !s.is_empty()
        {
            out.insert(f.to_string(), Value::String(s.clone()));
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

pub use crumb_core::duration::iso_duration_minutes;

fn parse_duration_minutes(iso: Option<&str>) -> Option<i64> {
    iso_duration_minutes(iso?).map(|m| m.round() as i64)
}

fn format_minutes(minutes: i64) -> String {
    let (h, m) = (minutes / 60, minutes % 60);
    let mut parts = Vec::new();
    if h > 0 {
        parts.push(format!("{h}h"));
    }
    if m > 0 {
        parts.push(format!("{m}m"));
    }
    parts.join(" ")
}

fn compute_additional_time(
    prep: Option<&str>,
    cook: Option<&str>,
    total: Option<&str>,
) -> Option<String> {
    // With neither prep nor cook known, the whole total would wrongly count as extra time
    if prep.is_none_or(str::is_empty) && cook.is_none_or(str::is_empty) {
        return None;
    }
    let total = parse_duration_minutes(total).filter(|t| *t != 0)?;
    let additional = total
        - parse_duration_minutes(prep).unwrap_or(0)
        - parse_duration_minutes(cook).unwrap_or(0);
    if additional <= 0 {
        return None;
    }
    Some(format_minutes(additional)).filter(|s| !s.is_empty())
}

/// ISO 8601 durations ("PT1H10M", "P0Y0M0DT0H10M0.000S") as "1h 10m"; anything else is
/// kept as written. A duration under a minute is dropped.
pub fn format_duration(iso: Option<&str>) -> Option<String> {
    let iso = iso.filter(|s| !s.trim().is_empty())?;
    match iso_duration_minutes(iso) {
        Some(minutes) if minutes < 1.0 => None,
        Some(minutes) => Some(format_minutes(minutes.round() as i64)),
        None => Some(iso.to_string()),
    }
}

fn extract_from_html(doc: &Html, url: &str) -> RecipeFields {
    let first_text = |s: &str| {
        doc.select(&sel(s))
            .next()
            .map(text_of)
            .filter(|t| !t.is_empty())
    };
    let first_attr = |s: &str, attr: &str| {
        doc.select(&sel(s))
            .next()
            .and_then(|e| e.value().attr(attr))
            .filter(|v| !v.is_empty())
            .map(String::from)
    };
    let title_tag: String = doc.select(&sel("title")).map(text_of).collect::<String>();

    let title = first_text(r#"[itemprop="name"]"#)
        .or_else(|| first_text("h1"))
        .or_else(|| Some(title_tag.trim().to_string()).filter(|t| !t.is_empty()))
        .unwrap_or_else(|| "Untitled recipe".into());

    let image = first_attr(r#"[itemprop="image"]"#, "src")
        .or_else(|| first_attr(r#"[itemprop="image"]"#, "content"))
        .or_else(|| first_attr(r#"meta[property="og:image"]"#, "content"));

    let ingredients: Vec<String> = doc
        .select(&sel(
            r#"[itemprop="recipeIngredient"], [itemprop="ingredients"]"#,
        ))
        .map(text_of)
        .filter(|t| !t.is_empty())
        .collect();

    let li = sel("li");
    let mut instructions = Vec::new();
    for el in doc.select(&sel(r#"[itemprop="recipeInstructions"]"#)) {
        if matches!(el.value().name(), "ol" | "ul") {
            instructions.extend(el.select(&li).map(text_of).filter(|t| !t.is_empty()));
        } else {
            let text = text_of(el);
            instructions.extend(
                text.split('\n')
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(String::from),
            );
        }
    }

    let time = |prop: &str| {
        format_duration(first_attr(&format!(r#"[itemprop="{prop}"]"#), "content").as_deref())
    };

    RecipeFields {
        url: Some(url.to_string()),
        title,
        description: first_text(r#"[itemprop="description"]"#),
        image,
        author: first_text(r#"[itemprop="author"]"#),
        prep_time: time("prepTime"),
        cook_time: time("cookTime"),
        total_time: time("totalTime"),
        freeze_time: time("freezeTime"),
        recipe_yield: first_text(r#"[itemprop="recipeYield"]"#),
        recipe_category: first_text(r#"[itemprop="recipeCategory"]"#),
        recipe_cuisine: first_text(r#"[itemprop="recipeCuisine"]"#),
        ingredients: normalize_ingredient_sections(&ingredients),
        instructions: vec![Section::unnamed(instructions)],
        nutrition: None,
        notes: notes_from_html(doc),
        video: video_from_html(doc, url),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRAPH: &str = r#"<html><head><script type="application/ld+json">
    {"@context":"https://schema.org","@graph":[
      {"@type":"WebPage","name":"Page"},
      {"@type":["Recipe"],"name":"Lemon &amp; Herb Chicken","description":"<p>Bright and <b>easy</b>.</p>",
       "image":[{"url":"/img/chicken.jpg"}],"author":[{"@type":"Person","name":"Sam"},"Alex"],
       "prepTime":"PT15M","cookTime":"PT1H","totalTime":"PT1H30M","recipeYield":["4","4 servings"],
       "recipeCategory":"Dinner","recipeCuisine":["Greek"],
       "recipeIngredient":["For the marinade:","2 lemons","3 cloves garlic","Chicken:","1 kg chicken thighs"],
       "recipeInstructions":[
         {"@type":"HowToSection","name":"Marinate","itemListElement":[{"@type":"HowToStep","text":"Mix the marinade."}]},
         {"@type":"HowToSection","name":"Cook","itemListElement":[{"@type":"HowToStep","text":"Roast at 200&deg;C for 1 hour."}]}
       ],
       "nutrition":{"@type":"NutritionInformation","calories":"420 kcal","proteinContent":"38 g","bogus":"x"}}
    ]}</script></head><body></body></html>"#;

    #[test]
    fn spots_challenge_pages() {
        let cloudflare = "<html><head><title>Just a moment...</title></head><body></body></html>";
        assert!(is_challenge_page(cloudflare));
        let akamai = "<HTML><HEAD>\n<TITLE>Access Denied</TITLE>\n</HEAD><BODY>Reference errors.edgesuite.net</BODY></HTML>";
        assert!(is_challenge_page(akamai));
        let perimeterx = r#"<html><head><title>Food Site</title></head><body><div id="px-captcha"></div></body></html>"#;
        assert!(is_challenge_page(perimeterx));
        let plain = "<html><head><title>Easy Weeknight Pasta</title></head><body>Access denied to nobody.</body></html>";
        assert!(!is_challenge_page(plain));
        assert!(!is_challenge_page(GRAPH));
    }

    /// [`scripted_steps`] with the WordPress and archive steps off (just Firefox, Safari, a
    /// relay and the browser).
    fn scripted(
        browser: bool,
        responses: Vec<(Method, Fetched)>,
    ) -> (Result<(Method, Scraped), ScrapeError>, Vec<Method>) {
        let steps = Steps {
            browser,
            wordpress: false,
            archive: false,
            relay: true,
        };
        scripted_steps(steps, responses)
    }

    /// Runs `scrape_with` against scripted responses, returning the result and the methods asked.
    fn scripted_steps(
        steps: Steps,
        mut responses: Vec<(Method, Fetched)>,
    ) -> (Result<(Method, Scraped), ScrapeError>, Vec<Method>) {
        let asked = std::cell::RefCell::new(Vec::new());
        let result = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(scrape_with("https://food.test/r", steps, |method| {
                asked.borrow_mut().push(method);
                let fetched = match responses.iter().position(|(m, _)| *m == method) {
                    Some(at) => responses.remove(at).1,
                    None if method == Method::Relay => Fetched::Unreachable("no relays".into()),
                    None => panic!("unexpected fetch with {method:?}"),
                };
                async move { fetched }
            }));
        (result, asked.into_inner())
    }

    fn ok(html: &str) -> Fetched {
        Fetched::Page {
            status: 200,
            html: html.into(),
        }
    }

    fn status(code: u16) -> Fetched {
        Fetched::Page {
            status: code,
            html: String::new(),
        }
    }

    const CHALLENGE: &str = "<html><head><title>Just a moment...</title></head></html>";

    #[test]
    fn firefox_first_and_alone_when_it_works() {
        let (result, asked) = scripted(true, vec![(Method::Firefox, ok(GRAPH))]);
        assert_eq!(result.unwrap().0, Method::Firefox);
        assert_eq!(asked, vec![Method::Firefox]);
    }

    #[test]
    fn safari_after_a_block_status_or_challenge() {
        for blocked in [status(403), status(429), status(503), ok(CHALLENGE)] {
            let (result, asked) = scripted(
                true,
                vec![(Method::Firefox, blocked), (Method::Safari, ok(GRAPH))],
            );
            assert_eq!(result.unwrap().0, Method::Safari);
            assert_eq!(asked, vec![Method::Firefox, Method::Safari]);
        }
    }

    #[test]
    fn no_safari_retry_for_other_failures() {
        let (result, asked) = scripted(false, vec![(Method::Firefox, status(404))]);
        assert!(
            result
                .unwrap_err()
                .message
                .starts_with("The site responded with 404.")
        );
        assert_eq!(asked, vec![Method::Firefox]);

        let (result, asked) = scripted(
            false,
            vec![(Method::Firefox, Fetched::Unreachable("dns".into()))],
        );
        assert!(
            result
                .unwrap_err()
                .message
                .starts_with("Couldn't reach that site.")
        );
        assert_eq!(asked, vec![Method::Firefox]);

        let (result, _) = scripted(false, vec![(Method::Firefox, ok("<p>no recipe</p>"))]);
        assert!(
            result
                .unwrap_err()
                .message
                .contains("Couldn't find a recipe")
        );
    }

    #[test]
    fn browser_last_and_only_when_available() {
        let (result, asked) = scripted(
            true,
            vec![
                (Method::Firefox, status(403)),
                (Method::Safari, ok(CHALLENGE)),
                (Method::Browser, ok(GRAPH)),
            ],
        );
        assert_eq!(result.unwrap().0, Method::Browser);
        assert_eq!(
            asked,
            vec![
                Method::Firefox,
                Method::Safari,
                Method::Relay,
                Method::Browser
            ]
        );

        // A page without a recipe (rendered by scripts) goes straight to the browser
        let (result, asked) = scripted(
            true,
            vec![
                (Method::Firefox, ok("<p>loading</p>")),
                (Method::Browser, ok(GRAPH)),
            ],
        );
        assert_eq!(result.unwrap().0, Method::Browser);
        assert_eq!(asked, vec![Method::Firefox, Method::Browser]);

        let (result, asked) = scripted(
            false,
            vec![
                (Method::Firefox, status(403)),
                (Method::Safari, status(403)),
            ],
        );
        let err = result.unwrap_err();
        let message = err.message;
        assert!(
            message.starts_with("The site responded with 403."),
            "{message}"
        );
        assert!(message.ends_with(PASTE_HINT));
        assert!(err.blocked);
        assert_eq!(asked, vec![Method::Firefox, Method::Safari, Method::Relay]);

        let (result, _) = scripted(
            true,
            vec![
                (Method::Firefox, status(403)),
                (Method::Safari, status(403)),
                (
                    Method::Browser,
                    Fetched::Unreachable("Blocked by the site".into()),
                ),
            ],
        );
        let err = result.unwrap_err();
        assert!(err.message.contains("A real browser was blocked too."));
        assert!(err.blocked);
    }

    #[test]
    fn only_a_block_is_reported_as_one() {
        let blocked = |responses| scripted(true, responses).0.unwrap_err().blocked;
        // The last word was a bot check, from the wreq profiles or in the browser
        assert!(blocked(vec![
            (Method::Firefox, ok(CHALLENGE)),
            (Method::Safari, status(403)),
            (Method::Browser, ok(CHALLENGE)),
        ]));
        // A block the browser got past, only to find no recipe, isn't one
        assert!(!blocked(vec![
            (Method::Firefox, status(403)),
            (Method::Safari, status(403)),
            (Method::Browser, ok("<p>no recipe</p>")),
        ]));
        assert!(!blocked(vec![
            (Method::Firefox, status(404)),
            (Method::Browser, ok("<p>no recipe</p>")),
        ]));
        assert!(!blocked(vec![
            (Method::Firefox, Fetched::Unreachable("dns".into())),
            (Method::Browser, Fetched::Unreachable("dns".into())),
        ]));
    }

    fn all_steps() -> Steps {
        Steps {
            browser: true,
            wordpress: true,
            archive: true,
            relay: true,
        }
    }

    /// A recipe as a fallback step hands it over.
    fn recipe_from_step() -> Fetched {
        let recipe = parse_recipe_html(GRAPH, "https://food.test/r").unwrap();
        Fetched::Recipe(Box::new(Scraped {
            recipe,
            crumb: None,
        }))
    }

    /// Firefox and Safari both refused.
    fn refused() -> Vec<(Method, Fetched)> {
        vec![
            (Method::Firefox, status(403)),
            (Method::Safari, ok(CHALLENGE)),
        ]
    }

    #[test]
    fn wordpress_after_both_profiles_are_blocked() {
        let mut script = refused();
        script.push((Method::WordPress, recipe_from_step()));
        let (result, asked) = scripted_steps(all_steps(), script);
        let (method, scraped) = result.unwrap();
        assert_eq!(method, Method::WordPress);
        assert_eq!(scraped.recipe.title, "Lemon & Herb Chicken");
        assert_eq!(
            asked,
            vec![Method::Firefox, Method::Safari, Method::WordPress]
        );
    }

    #[test]
    fn archive_when_wordpress_has_nothing() {
        for nothing in [
            Fetched::Unreachable("no post at that link".into()),
            status(404),
            ok("<p>not a recipe</p>"),
        ] {
            let mut script = refused();
            script.push((Method::WordPress, nothing));
            script.push((Method::Archive, ok(GRAPH)));
            let (result, asked) = scripted_steps(all_steps(), script);
            assert_eq!(result.unwrap().0, Method::Archive);
            assert_eq!(
                asked,
                vec![
                    Method::Firefox,
                    Method::Safari,
                    Method::WordPress,
                    Method::Relay,
                    Method::Archive
                ]
            );
        }
    }

    #[test]
    fn browser_when_neither_has_a_recipe() {
        let mut script = refused();
        script.push((Method::WordPress, Fetched::Unreachable("no post".into())));
        script.push((Method::Archive, status(404)));
        script.push((Method::Browser, ok(GRAPH)));
        let (result, asked) = scripted_steps(all_steps(), script);
        assert_eq!(result.unwrap().0, Method::Browser);
        assert_eq!(
            asked,
            vec![
                Method::Firefox,
                Method::Safari,
                Method::WordPress,
                Method::Relay,
                Method::Archive,
                Method::Browser
            ]
        );

        // With nothing working, the block is what the cook is told
        let mut script = refused();
        script.push((Method::WordPress, Fetched::Unreachable("no post".into())));
        script.push((Method::Archive, status(404)));
        let steps = Steps {
            browser: false,
            ..all_steps()
        };
        let (result, _) = scripted_steps(steps, script);
        let message = result.unwrap_err().message;
        assert!(
            message.starts_with("The site showed a bot check"),
            "{message}"
        );
    }

    #[test]
    fn a_plain_failure_skips_the_ways_round_a_block() {
        // A page rendered by scripts, a 404, or Safari failing differently from Firefox
        let cases = [
            vec![(Method::Firefox, ok("<p>loading</p>"))],
            vec![(Method::Firefox, status(404))],
            vec![
                (Method::Firefox, status(403)),
                (Method::Safari, status(404)),
            ],
        ];
        for mut script in cases {
            script.push((Method::Browser, ok(GRAPH)));
            let (result, asked) = scripted_steps(all_steps(), script);
            assert_eq!(result.unwrap().0, Method::Browser);
            assert!(!asked.contains(&Method::WordPress), "{asked:?}");
            assert!(!asked.contains(&Method::Archive), "{asked:?}");
        }
    }

    #[test]
    fn switched_off_steps_are_skipped() {
        let mut script = refused();
        script.push((Method::Archive, ok(GRAPH)));
        // No relays set up counts as off
        let steps = Steps {
            wordpress: false,
            relay: false,
            ..all_steps()
        };
        let (result, asked) = scripted_steps(steps, script);
        assert_eq!(result.unwrap().0, Method::Archive);
        assert_eq!(
            asked,
            vec![Method::Firefox, Method::Safari, Method::Archive]
        );

        let mut script = refused();
        script.push((Method::WordPress, Fetched::Unreachable("no post".into())));
        script.push((Method::Browser, ok(GRAPH)));
        let steps = Steps {
            archive: false,
            ..all_steps()
        };
        let (result, asked) = scripted_steps(steps, script);
        assert_eq!(result.unwrap().0, Method::Browser);
        assert_eq!(
            asked,
            vec![
                Method::Firefox,
                Method::Safari,
                Method::WordPress,
                Method::Relay,
                Method::Browser
            ]
        );
    }

    #[test]
    fn relay_after_both_profiles_were_blocked() {
        let (result, asked) = scripted(
            true,
            vec![
                (Method::Firefox, status(403)),
                (Method::Safari, ok(CHALLENGE)),
                (Method::Relay, ok(GRAPH)),
            ],
        );
        assert_eq!(result.unwrap().0, Method::Relay);
        assert_eq!(asked, vec![Method::Firefox, Method::Safari, Method::Relay]);
    }

    #[test]
    fn browser_after_a_relay_that_failed() {
        for relayed in [
            Fetched::Unreachable("down".into()),
            status(403),
            ok(CHALLENGE),
            ok("<p>no recipe</p>"),
        ] {
            let (result, asked) = scripted(
                true,
                vec![
                    (Method::Firefox, status(403)),
                    (Method::Safari, status(403)),
                    (Method::Relay, relayed),
                    (Method::Browser, ok(GRAPH)),
                ],
            );
            assert_eq!(result.unwrap().0, Method::Browser);
            assert_eq!(
                asked,
                vec![
                    Method::Firefox,
                    Method::Safari,
                    Method::Relay,
                    Method::Browser
                ]
            );
        }

        // A relay that gave nothing leaves the site's own answer as the reason
        let (result, _) = scripted(
            false,
            vec![
                (Method::Firefox, status(403)),
                (Method::Safari, status(403)),
                (Method::Relay, ok("<p>no recipe</p>")),
            ],
        );
        assert!(
            result
                .unwrap_err()
                .message
                .starts_with("The site responded with 403.")
        );
    }

    #[test]
    fn no_relay_unless_both_profiles_were_blocked() {
        // Firefox got a recipe, or Safari did, or a failure that isn't a block
        let (_, asked) = scripted(true, vec![(Method::Firefox, ok(GRAPH))]);
        assert!(!asked.contains(&Method::Relay));
        let (_, asked) = scripted(
            true,
            vec![(Method::Firefox, status(403)), (Method::Safari, ok(GRAPH))],
        );
        assert!(!asked.contains(&Method::Relay));
        for failed in [
            status(404),
            status(500),
            Fetched::Unreachable("dns".into()),
            ok("<p>no recipe</p>"),
        ] {
            let (_, asked) = scripted(false, vec![(Method::Firefox, failed)]);
            assert_eq!(asked, vec![Method::Firefox]);
        }
        // Firefox blocked, then Safari failed some other way
        let (_, asked) = scripted(
            false,
            vec![
                (Method::Firefox, status(403)),
                (Method::Safari, status(404)),
            ],
        );
        assert_eq!(asked, vec![Method::Firefox, Method::Safari]);
    }

    #[test]
    fn parses_json_ld_graph_with_sections() {
        let r = parse_recipe_html(GRAPH, "https://food.test/recipes/chicken").unwrap();
        assert_eq!(r.title, "Lemon & Herb Chicken");
        assert_eq!(r.description.as_deref(), Some("Bright and easy."));
        assert_eq!(
            r.image.as_deref(),
            Some("https://food.test/img/chicken.jpg")
        );
        assert_eq!(r.author.as_deref(), Some("Sam, Alex"));
        assert_eq!(r.prep_time.as_deref(), Some("15m"));
        assert_eq!(r.cook_time.as_deref(), Some("1h"));
        assert_eq!(r.total_time.as_deref(), Some("1h 30m"));
        assert_eq!(r.freeze_time.as_deref(), Some("15m"));
        assert_eq!(r.recipe_yield.as_deref(), Some("4, 4 servings"));
        assert_eq!(r.recipe_cuisine.as_deref(), Some("Greek"));
        assert_eq!(r.ingredients.len(), 2);
        assert_eq!(r.ingredients[0].name.as_deref(), Some("For the marinade"));
        assert_eq!(r.ingredients[1].items, vec!["1 kg chicken thighs"]);
        assert_eq!(r.instructions[1].name.as_deref(), Some("Cook"));
        assert_eq!(r.instructions[1].items, vec!["Roast at 200°C for 1 hour."]);
        let n = r.nutrition.unwrap();
        assert_eq!(n.len(), 2);
        assert_eq!(n["calories"], "420 kcal");
    }

    #[test]
    fn turns_float_quantities_back_into_fractions() {
        let html = r#"<script type="application/ld+json">{"@type":"Recipe","name":"Casserole",
          "recipeIngredient":["0.33333334326744 cup olive oil","1.5 teaspoons salt","0.4 kg potatoes"],
          "recipeInstructions":"Bake."}</script>"#;
        let r = parse_recipe_html(html, "https://x.test/casserole").unwrap();
        assert_eq!(
            r.ingredients[0].items,
            ["⅓ cup olive oil", "1 ½ teaspoons salt", "0.4 kg potatoes"]
        );
    }

    #[test]
    fn prefers_wprm_groups_over_flat_json_ld() {
        let html = r#"<script type="application/ld+json">{"@type":"Recipe","name":"Cake",
          "recipeIngredient":["2 cups flour","1 cup sugar","1 cup cream"],
          "recipeInstructions":"Mix.\nBake."}</script>
          <div class="wprm-recipe-ingredient-group"><h4 class="wprm-recipe-group-name">Cake</h4>
            <ul><li class="wprm-recipe-ingredient">2 cups flour</li><li class="wprm-recipe-ingredient">1 cup sugar</li></ul></div>
          <div class="wprm-recipe-ingredient-group"><h4 class="wprm-recipe-group-name">Frosting</h4>
            <ul><li class="wprm-recipe-ingredient">1 cup cream</li></ul></div>"#;
        let r = parse_recipe_html(html, "https://x.test/cake").unwrap();
        assert_eq!(r.ingredients.len(), 2);
        assert_eq!(r.ingredients[1].name.as_deref(), Some("Frosting"));
        assert_eq!(r.instructions[0].items, vec!["Mix.", "Bake."]);
        assert!(r.image.is_none());
    }

    #[test]
    fn falls_back_to_microdata() {
        let html = r#"<html><head><title>Site</title>
          <meta property="og:image" content="https://cdn.test/p.jpg"></head><body>
          <div itemscope itemtype="https://schema.org/Recipe">
            <h1 itemprop="name">Pancakes</h1>
            <meta itemprop="prepTime" content="PT10M">
            <span itemprop="recipeYield">8 pancakes</span>
            <ul><li itemprop="recipeIngredient">1 cup flour</li><li itemprop="recipeIngredient">1 egg</li></ul>
            <ol itemprop="recipeInstructions"><li>Whisk.</li><li>Fry.</li></ol>
          </div></body></html>"#;
        let r = parse_recipe_html(html, "https://x.test/p").unwrap();
        assert_eq!(r.title, "Pancakes");
        assert_eq!(r.prep_time.as_deref(), Some("10m"));
        assert_eq!(r.recipe_yield.as_deref(), Some("8 pancakes"));
        assert_eq!(r.image.as_deref(), Some("https://cdn.test/p.jpg"));
        assert_eq!(r.ingredients[0].items, vec!["1 cup flour", "1 egg"]);
        assert_eq!(r.instructions[0].items, vec!["Whisk.", "Fry."]);
    }

    // As Natasha's Kitchen writes them: WPRM's notes, with WordPress block comments in
    // "display: block" spans, and a YouTube video in the JSON-LD
    const WPRM: &str = r#"<html><head><script type="application/ld+json">{"@type":"Recipe",
      "name":"Beef and Broccoli","recipeIngredient":["1 lb flank steak"],"recipeInstructions":"Sear.",
      "video":{"@type":"VideoObject","name":"How To Make Beef and Broccoli",
        "embedUrl":"https:\/\/www.youtube.com\/embed\/8eITNSfct3Q?feature=oembed",
        "contentUrl":"https:\/\/www.youtube.com\/watch?v=8eITNSfct3Q"}}</script></head><body>
      <div id="recipe-60463-notes" class="wprm-recipe-notes-container wprm-block-text-normal">
        <h3 class="wprm-recipe-header wprm-recipe-notes-header">Notes</h3>
        <div class="wprm-recipe-notes"><span style="display: block;"><strong>Variations:&nbsp;</strong></span><div class="wprm-spacer"></div>
        <span style="display: block;"><!-- wp:list --></span><div class="wprm-spacer"></div>
        <ul class="wp-block-list"><!-- wp:list-item --><div class="wprm-spacer"></div>
        <li><strong>More steak options include:</strong> Top Sirloin Steak, or Ribeye</li>
        <span style="display: block;"><!-- /wp:list-item --> <!-- wp:list-item --></span><div class="wprm-spacer"></div>
        <li><strong>Vegetables:</strong>&nbsp;use fresh vegetables, not frozen.</li>
        <span style="display: block;"><!-- /wp:list-item --></span></ul>
        <span style="display: block;"><!-- /wp:list --></span></div></div>
      </body></html>"#;

    #[test]
    fn reads_wprm_notes_and_json_ld_video() {
        let r = parse_recipe_html(WPRM, "https://natashaskitchen.com/beef-and-broccoli/").unwrap();
        assert_eq!(
            r.notes.as_deref(),
            Some(
                "Variations:\n\
                 • More steak options include: Top Sirloin Steak, or Ribeye\n\
                 • Vegetables: use fresh vegetables, not frozen."
            )
        );
        assert_eq!(
            r.video.as_deref(),
            Some("https://www.youtube.com/watch?v=8eITNSfct3Q")
        );
    }

    #[test]
    fn keeps_a_jw_player_file_from_json_ld() {
        // Allrecipes: a Recipe/NewsArticle whose video is JW Player's mp4
        let html = r#"<script type="application/ld+json">[{"@type":["Recipe","NewsArticle"],
          "name":"Slow Cooker Asian Zing Chicken Noodles","recipeIngredient":["1 jar sauce"],
          "recipeInstructions":[{"@type":"HowToStep","text":"Cook."}],
          "video":{"@type":"VideoObject","contentUrl":"https://cdn.jwplayer.com/videos/9QcFPcvu-K3AjnAEN.mp4",
            "name":"How to Make Slow Cooker Asian Zing Chicken Noodles",
            "thumbnailUrl":"https://cdn.jwplayer.com/v2/media/9QcFPcvu/thumbnails/g43V12F6.jpg?width=1280"}}]
          </script>"#;
        let r =
            parse_recipe_html(html, "https://www.allrecipes.com/zing-noodles-11725006").unwrap();
        assert_eq!(
            r.video.as_deref(),
            Some("https://cdn.jwplayer.com/videos/9QcFPcvu-K3AjnAEN.mp4")
        );
    }

    #[test]
    fn video_from_the_recipe_card_or_post_but_not_the_sidebar() {
        let page = |body: &str| {
            format!(
                r#"<script type="application/ld+json">{{"@type":"Recipe","name":"Soup",
                  "recipeIngredient":["1 leek"],"recipeInstructions":"Simmer."}}</script>{body}"#
            )
        };
        let url = "https://x.test/soup";
        // WP Rocket's lazy YouTube player, in the WPRM card
        let r = parse_recipe_html(
            &page(r#"<aside><iframe src="https://www.youtube.com/embed/sidebar0001"></iframe></aside>
              <div class="wprm-recipe-video"><div class="rll-youtube-player" data-src="https://www.youtube.com/embed/cardVideo01" data-id="cardVideo01"></div></div>"#),
            url,
        )
        .unwrap();
        assert_eq!(
            r.video.as_deref(),
            Some("https://www.youtube.com/watch?v=cardVideo01")
        );
        // A lazy-loaded Vimeo iframe in the post
        let r = parse_recipe_html(
            &page(r#"<article><iframe data-lazy-src="//player.vimeo.com/video/76979871"></iframe></article>"#),
            url,
        )
        .unwrap();
        assert_eq!(r.video.as_deref(), Some("https://vimeo.com/76979871"));
        // Only a sidebar video, or an ad network's player: none
        let r = parse_recipe_html(
            &page(r#"<aside><lite-youtube videoid="sidebar0001"></lite-youtube></aside>
              <article><div class="mv-video"><script src="https://video.mediavine.com/videos/abc.js"></script></div></article>"#),
            url,
        )
        .unwrap();
        assert_eq!(r.video, None);
    }

    #[test]
    fn reads_tasty_notes_without_their_heading() {
        let html = r#"<div itemscope><h1 itemprop="name">Scones</h1>
          <ul><li itemprop="recipeIngredient">2 cups flour</li></ul>
          <ol itemprop="recipeInstructions"><li>Bake.</li></ol>
          <div class="tasty-recipes-notes"><h3>Notes</h3><p>Freeze unbaked.</p><p>Best warm.</p></div></div>"#;
        let r = parse_recipe_html(html, "https://x.test/scones").unwrap();
        assert_eq!(r.notes.as_deref(), Some("Freeze unbaked.\nBest warm."));
    }

    #[test]
    fn no_recipe_is_none() {
        assert!(
            parse_recipe_html("<html><h1>Blog</h1><p>Hi</p></html>", "https://x.test").is_none()
        );
    }

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(Some("PT1H10M")).as_deref(), Some("1h 10m"));
        assert_eq!(format_duration(Some("pt45m")).as_deref(), Some("45m"));
        assert_eq!(format_duration(Some("PT30S")), None);
        assert_eq!(format_duration(Some("20 mins")).as_deref(), Some("20 mins"));
        assert_eq!(format_duration(None), None);
        // The long form Food Network uses, with years, months, days and decimal seconds
        assert_eq!(
            format_duration(Some("P0Y0M0DT0H10M0.000S")).as_deref(),
            Some("10m")
        );
        assert_eq!(
            format_duration(Some("P0Y0M0DT0H34M0.000S")).as_deref(),
            Some("34m")
        );
        assert_eq!(format_duration(Some("P1DT2H")).as_deref(), Some("26h"));
        assert_eq!(format_duration(Some("PT90M")).as_deref(), Some("1h 30m"));
        assert_eq!(format_duration(Some("PT1.5H")).as_deref(), Some("1h 30m"));
        // Months are not minutes; a year-long "duration" is kept as written
        assert_eq!(format_duration(Some("P1M")).as_deref(), Some("P1M"));
        assert_eq!(format_duration(Some("PT")).as_deref(), Some("PT"));
        assert_eq!(
            compute_additional_time(
                Some("P0Y0M0DT0H10M0.000S"),
                Some("P0Y0M0DT0H20M0.000S"),
                Some("P0Y0M0DT0H34M0.000S")
            )
            .as_deref(),
            Some("4m")
        );
        assert_eq!(
            compute_additional_time(Some("PT10M"), Some("PT20M"), Some("PT2H")).as_deref(),
            Some("1h 30m")
        );
        assert_eq!(
            compute_additional_time(Some("PT10M"), None, Some("PT10M")),
            None
        );
        assert_eq!(compute_additional_time(None, None, Some("PT40M")), None);
    }

    #[test]
    fn json_ld_from_export_keeps_its_own_url() {
        let v: Value = serde_json::from_str(
            r#"{"@type":"Recipe","name":"Soup","url":"https://soup.test/s","recipeIngredient":["water"]}"#,
        )
        .unwrap();
        let r = recipe_from_json_ld(&v, "").unwrap();
        assert_eq!(r.url.as_deref(), Some("https://soup.test/s"));
        let v: Value = serde_json::from_str(
            r#"{"@type":"Recipe","name":"Soup","recipeIngredient":["water"]}"#,
        )
        .unwrap();
        assert_eq!(recipe_from_json_ld(&v, "").unwrap().url, None);
    }
}
