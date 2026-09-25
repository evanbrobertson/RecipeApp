//! Recipe scraping: JSON-LD first, then recipe-plugin markup and microdata.

use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use serde_json::{Map, Value};
use std::sync::LazyLock;
use std::time::Duration;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::model::{RecipeFields, Section, normalize_sections};

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

const PASTE_HINT: &str = "Try copying the recipe text and pasting it instead.";

/// Scrapes a recipe page. Plain HTTP first (fast, cheap); if the site blocks it or the
/// recipe is rendered by JavaScript, retries in headless Chromium when it's installed.
pub async fn scrape_recipe(state: &AppState, url: &str) -> AppResult<RecipeFields> {
    let parsed =
        url::Url::parse(url).map_err(|_| AppError::bad_request("Please enter a valid URL"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::bad_request("Only http(s) links are supported."));
    }

    let mut problem = match fetch_html(&state.http, url).await {
        Ok(html) => match parse_recipe_html(&html, url) {
            Some(recipe) => return Ok(recipe),
            None => "Couldn't find a recipe on that page.".to_string(),
        },
        Err(Some(status)) => format!("The site responded with {status}."),
        Err(None) => "Couldn't reach that site.".to_string(),
    };

    if state.browser.available() {
        match state.browser.fetch(url).await {
            Ok(html) => match parse_recipe_html(&html, url) {
                Some(recipe) => return Ok(recipe),
                None => {
                    problem = "Couldn't find a recipe on that page, even in a real browser.".into()
                }
            },
            Err(err) => {
                tracing::warn!("[scraper] browser fallback failed for {url}: {err}");
                problem = format!("{problem} A real browser was blocked too.");
            }
        }
    }

    Err(AppError::new(422, format!("{problem} {PASTE_HINT}")))
}

/// `Err(Some(status))` for HTTP errors, `Err(None)` when the site can't be reached.
async fn fetch_html(client: &reqwest::Client, url: &str) -> Result<String, Option<u16>> {
    let response = client
        .get(url)
        .timeout(Duration::from_secs(15))
        .header("User-Agent", USER_AGENT)
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await
        .map_err(|_| None)?;
    let status = response.status();
    if !status.is_success() {
        return Err(Some(status.as_u16()));
    }
    response.text().await.map_err(|_| None)
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

static DURATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^PT(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)S)?$").unwrap());

fn parse_duration_minutes(iso: Option<&str>) -> Option<i64> {
    let c = DURATION.captures(iso?)?;
    let n = |i: usize| {
        c.get(i)
            .and_then(|m| m.as_str().parse::<i64>().ok())
            .unwrap_or(0)
    };
    Some(n(1) * 60 + n(2))
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

/// ISO-8601 durations ("PT1H10M") as "1h 10m"; anything else is kept as written.
pub fn format_duration(iso: Option<&str>) -> Option<String> {
    let iso = iso.filter(|s| !s.is_empty())?;
    let Some(c) = DURATION.captures(iso) else {
        return Some(iso.to_string());
    };
    let parts: Vec<String> = [(1, "h"), (2, "m")]
        .iter()
        .filter_map(|(i, unit)| c.get(*i).map(|m| format!("{}{unit}", m.as_str())))
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
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
