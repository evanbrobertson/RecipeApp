//! Recipe scraping: fetch the page, then JSON-LD first, then recipe-plugin markup and microdata.
//!
//! Pages are fetched with `wreq`, which sends a real browser's TLS and HTTP/2 fingerprint and
//! headers. Many recipe sites (behind Cloudflare, Akamai, PerimeterX and the like) refuse a
//! plain Rust client on its fingerprint alone, whatever its User-Agent says. The order is
//! Firefox, then Safari when the site blocks it, then headless Chromium when installed.

use http_body_util::BodyExt;
use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use serde_json::{Map, Value};
use std::future::Future;
use std::sync::{LazyLock, OnceLock};
use std::time::Duration;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::model::{RecipeFields, Section, normalize_sections};

/// The User-Agent for the plain `reqwest` image fallback (wreq's profiles set their own).
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

const PASTE_HINT: &str = "Try copying the recipe text and pasting it instead.";

const PAGE_TIMEOUT: Duration = Duration::from_secs(15);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Largest recipe page read; real ones are well under 2 MB.
pub const MAX_PAGE_BYTES: usize = 10 * 1024 * 1024;

/// How a page was fetched, in the order they're tried.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Firefox,
    Safari,
    Browser,
}

impl Method {
    /// The name in the log line for an import.
    pub fn label(self) -> &'static str {
        match self {
            Method::Firefox => "wreq-firefox",
            Method::Safari => "wreq-safari",
            Method::Browser => "browser",
        }
    }
}

/// What one fetch attempt got back.
#[derive(Debug)]
pub enum Fetched {
    /// A response. The body is only read for a 2xx.
    Page { status: u16, html: String },
    /// No response (DNS, connection, TLS, timeout); the reason is for the log.
    Unreachable(String),
}

/// The shared browser-profile client for `method` (Firefox or Safari). Built on first use;
/// `None` if it can't be built (logged once), and then that step is skipped.
pub fn wreq_client(method: Method) -> Option<&'static wreq::Client> {
    static FIREFOX: OnceLock<Option<wreq::Client>> = OnceLock::new();
    static SAFARI: OnceLock<Option<wreq::Client>> = OnceLock::new();
    let (cell, emulation) = match method {
        Method::Firefox => (&FIREFOX, wreq_util::Emulation::Firefox151),
        Method::Safari => (&SAFARI, wreq_util::Emulation::Safari26_4),
        Method::Browser => return None,
    };
    cell.get_or_init(|| {
        wreq::Client::builder()
            .emulation(emulation)
            .cookie_store(true)
            // wreq doesn't follow redirects unless told to
            .redirect(wreq::redirect::Policy::limited(10))
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(PAGE_TIMEOUT)
            .pool_idle_timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(2)
            .build()
            .inspect_err(|e| {
                tracing::error!(
                    "[scraper] couldn't build the {} client: {e}",
                    method.label()
                )
            })
            .ok()
    })
    .as_ref()
}

/// Why a body read stopped.
#[derive(Debug, PartialEq, Eq)]
pub enum ReadError {
    TooLarge,
    Failed(String),
}

/// Reads a wreq response body, giving up once it passes `cap` bytes.
pub async fn read_capped(mut res: wreq::Response, cap: usize) -> Result<Vec<u8>, ReadError> {
    if res.content_length().is_some_and(|n| n > cap as u64) {
        return Err(ReadError::TooLarge);
    }
    let mut body = Vec::new();
    while let Some(frame) = res.frame().await {
        let frame = frame.map_err(|e| ReadError::Failed(e.to_string()))?;
        if let Ok(chunk) = frame.into_data() {
            if body.len() + chunk.len() > cap {
                return Err(ReadError::TooLarge);
            }
            body.extend_from_slice(&chunk);
        }
    }
    Ok(body)
}

/// Fetches a page with one of the wreq browser profiles. The profile sets every header.
pub async fn fetch_wreq(method: Method, url: &str) -> Fetched {
    let Some(client) = wreq_client(method) else {
        return Fetched::Unreachable("client unavailable".into());
    };
    let res = match client.get(url).send().await {
        Ok(res) => res,
        Err(e) => return Fetched::Unreachable(e.to_string()),
    };
    let status = res.status().as_u16();
    if !res.status().is_success() {
        return Fetched::Page {
            status,
            html: String::new(),
        };
    }
    match read_capped(res, MAX_PAGE_BYTES).await {
        Ok(body) => Fetched::Page {
            status,
            html: String::from_utf8_lossy(&body).into_owned(),
        },
        Err(ReadError::TooLarge) => Fetched::Unreachable("page too large".into()),
        Err(ReadError::Failed(e)) => Fetched::Unreachable(e),
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
    fn from_page(html: &str, url: &str) -> Option<Self> {
        Some(Self {
            recipe: parse_recipe_html(html, url)?,
            crumb: crumb_alternate(html, url),
        })
    }
}

/// The type a Crumb share page names its lossless export by.
pub const CRUMB_JSON_TYPE: &str = "application/vnd.crumb+json";

/// `<link rel="alternate" type="application/vnd.crumb+json" href>` on a page: another Crumb's
/// share page offering the recipe as a Crumb export. Only an http(s) address on the page's
/// own host is taken.
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
    let same_host = target.host_str().is_some() && target.host_str() == base.host_str();
    (matches!(target.scheme(), "http" | "https") && same_host).then(|| target.to_string())
}

enum Verdict {
    Recipe(Box<Scraped>),
    /// Refused or challenged: worth another profile.
    Blocked(String),
    /// Anything else that didn't give a recipe.
    Failed(String),
}

fn judge(fetched: Fetched, url: &str) -> Verdict {
    match fetched {
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

/// The fetch order, with the fetchers passed in (so it's testable without a network):
/// Firefox; Safari if Firefox was blocked; then the browser (when `browser` is true) if
/// neither gave a recipe. Returns the method that worked, or the message for the cook.
pub async fn scrape_with<F, Fut>(
    url: &str,
    browser: bool,
    mut fetch: F,
) -> Result<(Method, Scraped), String>
where
    F: FnMut(Method) -> Fut,
    Fut: Future<Output = Fetched>,
{
    let mut problem = match judge(fetch(Method::Firefox).await, url) {
        Verdict::Recipe(recipe) => return Ok((Method::Firefox, *recipe)),
        Verdict::Blocked(_) => match judge(fetch(Method::Safari).await, url) {
            Verdict::Recipe(recipe) => return Ok((Method::Safari, *recipe)),
            Verdict::Blocked(p) | Verdict::Failed(p) => p,
        },
        Verdict::Failed(p) => p,
    };

    if browser {
        match fetch(Method::Browser).await {
            Fetched::Page { html, .. } => match Scraped::from_page(&html, url) {
                Some(scraped) => return Ok((Method::Browser, scraped)),
                None => {
                    problem = "Couldn't find a recipe on that page, even in a real browser.".into()
                }
            },
            Fetched::Unreachable(err) => {
                tracing::warn!(
                    "[scraper] browser fallback failed for {}: {err}",
                    crate::telemetry::host_of(url)
                );
                problem = format!("{problem} A real browser was blocked too.");
            }
        }
    }

    Err(format!("{problem} {PASTE_HINT}"))
}

/// Scrapes a recipe page (see [`scrape_with`] for the order it tries).
pub async fn scrape_recipe(state: &AppState, url: &str) -> AppResult<RecipeFields> {
    Ok(scrape_page(state, url).await?.recipe)
}

/// [`scrape_recipe`], also saying whether the page offers a Crumb export.
pub async fn scrape_page(state: &AppState, url: &str) -> AppResult<Scraped> {
    let parsed =
        url::Url::parse(url).map_err(|_| AppError::bad_request("Please enter a valid URL"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::bad_request("Only http(s) links are supported."));
    }

    let browser = state.browser.clone();
    let result = scrape_with(url, browser.available(), |method| {
        let browser = browser.clone();
        async move {
            match method {
                Method::Browser => match browser.fetch(url).await {
                    Ok(html) => Fetched::Page { status: 200, html },
                    Err(err) => Fetched::Unreachable(err),
                },
                wreq => fetch_wreq(wreq, url).await,
            }
        }
    })
    .await;

    let host = crate::telemetry::host_of(url);
    match result {
        Ok((method, scraped)) => {
            tracing::info!("[scraper] {host}: {}", method.label());
            Ok(scraped)
        }
        Err(message) => {
            tracing::info!("[scraper] {host}: failed");
            Err(AppError::new(422, message))
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
        notes: None,
    }
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

/// ISO 8601 durations in full: "PT1H10M", "P1DT2H", and the long form some sites emit,
/// "P0Y0M0DT0H10M0.000S". Any unit may have decimals; weeks count as 7 days.
static DURATION: LazyLock<Regex> = LazyLock::new(|| {
    let n = r"(\d+(?:[.,]\d+)?)";
    Regex::new(&format!(
        r"(?i)^P(?:{n}Y)?(?:{n}M)?(?:{n}W)?(?:{n}D)?(?:T(?:{n}H)?(?:{n}M)?(?:{n}S)?)?$"
    ))
    .unwrap()
});

/// Minutes in an ISO 8601 duration, or None if it isn't one. Years and months have no
/// fixed length and never describe cooking, so a duration using them is not parsed.
pub fn iso_duration_minutes(iso: &str) -> Option<f64> {
    let iso = iso.trim();
    // "P" or "PT" alone would otherwise match as zero
    if iso.len() < 3 {
        return None;
    }
    let c = DURATION.captures(iso)?;
    let n = |i: usize| {
        c.get(i)
            .and_then(|m| m.as_str().replace(',', ".").parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    if n(1) != 0.0 || n(2) != 0.0 {
        return None;
    }
    Some(n(3) * 10_080.0 + n(4) * 1440.0 + n(5) * 60.0 + n(6) + n(7) / 60.0)
}

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
        notes: None,
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

    /// Runs `scrape_with` against scripted responses, returning the result and the methods asked.
    fn scripted(
        browser: bool,
        mut responses: Vec<(Method, Fetched)>,
    ) -> (Result<(Method, Scraped), String>, Vec<Method>) {
        let asked = std::cell::RefCell::new(Vec::new());
        let result = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(scrape_with("https://food.test/r", browser, |method| {
                asked.borrow_mut().push(method);
                let at = responses
                    .iter()
                    .position(|(m, _)| *m == method)
                    .unwrap_or_else(|| panic!("unexpected fetch with {method:?}"));
                let fetched = responses.remove(at).1;
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
                .starts_with("The site responded with 404.")
        );
        assert_eq!(asked, vec![Method::Firefox]);

        let (result, asked) = scripted(
            false,
            vec![(Method::Firefox, Fetched::Unreachable("dns".into()))],
        );
        assert!(result.unwrap_err().starts_with("Couldn't reach that site."));
        assert_eq!(asked, vec![Method::Firefox]);

        let (result, _) = scripted(false, vec![(Method::Firefox, ok("<p>no recipe</p>"))]);
        assert!(result.unwrap_err().contains("Couldn't find a recipe"));
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
            vec![Method::Firefox, Method::Safari, Method::Browser]
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
        let message = result.unwrap_err();
        assert!(
            message.starts_with("The site responded with 403."),
            "{message}"
        );
        assert!(message.ends_with(PASTE_HINT));
        assert_eq!(asked, vec![Method::Firefox, Method::Safari]);

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
        assert!(
            result
                .unwrap_err()
                .contains("A real browser was blocked too.")
        );
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
