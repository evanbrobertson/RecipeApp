//! Turns uploaded files into recipes. Supports:
//! - Just the Recipe PDFs (its only export: Print → Save as PDF, one recipe per file)
//! - Paprika (.paprikarecipes zip of gzipped JSON, or a single .paprikarecipe)
//! - schema.org Recipe JSON (Mealie, Tandoor, Nextcloud Cookbook...) and this app's own backup
//! - Saved web pages (.html) and plain text / Markdown (recipes separated by "---")
//! - .zip archives containing any of the above

use regex::Regex;
use serde_json::{Map, Value};
use std::io::{Cursor, Read};
use std::sync::LazyLock;

use crate::AppState;
use crate::model::{RecipeFields, Section, normalize_sections};
use crate::scraper::{parse_recipe_html, recipe_from_json_ld};
use crate::text_parser::parse_recipe_text;

pub struct ImportedRecipe {
    pub fields: RecipeFields,
    pub cookbooks: Vec<String>,
    /// When it was cooked (unix seconds), from a Crumb backup's cook log.
    pub cooked: Vec<i64>,
    /// From a Crumb backup: already the cook's own, so saved as it is, with no clean-up or
    /// Wee Chef check on the way in. "Check all" checks it later and then only suggests,
    /// apart from the small undoable clean-up (checkbox glyphs, web codes, float
    /// quantities, raw ISO times) it gives every recipe already in the box.
    pub restored: bool,
    /// From a Crumb backup: the original source of a recipe saved from a share.
    pub original_url: Option<String>,
}

impl From<RecipeFields> for ImportedRecipe {
    fn from(fields: RecipeFields) -> Self {
        Self {
            fields,
            cookbooks: Vec::new(),
            cooked: Vec::new(),
            restored: false,
            original_url: None,
        }
    }
}

fn lines(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => s
            .split('\n')
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect(),
        _ => Vec::new(),
    }
}

fn str_of(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn http_url(v: Option<&Value>) -> Option<String> {
    str_of(v).filter(|s| {
        let l = s.to_ascii_lowercase();
        l.starts_with("http://") || l.starts_with("https://")
    })
}

static SECTION_HEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:#+\s*)?([^:\d]{2,50}):$").unwrap());
static LIST_MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:[-*•]|\d+[.)])\s+").unwrap());

/// Splits flat lines into sections, treating short "Header:" lines as section names.
fn sections_from_lines(items: Vec<String>) -> Vec<Section> {
    let mut sections = vec![Section::default()];
    for item in items {
        if let Some(c) = SECTION_HEADER.captures(&item) {
            sections.push(Section {
                name: Some(c[1].trim().to_string()),
                items: vec![],
            });
        } else {
            let cleaned = LIST_MARKER.replace(&item, "").into_owned();
            sections.last_mut().unwrap().items.push(cleaned);
        }
    }
    normalize_sections(sections)
}

// ─── Paprika ────────────────────────────────────────────────────────────────

fn is_paprika(o: &Map<String, Value>) -> bool {
    o.contains_key("directions") && o.get("ingredients").is_some_and(Value::is_string)
}

fn from_paprika(p: &Map<String, Value>) -> Option<ImportedRecipe> {
    let title = str_of(p.get("name"))?;
    let notes = [
        str_of(p.get("notes")),
        str_of(p.get("nutritional_info")).map(|n| format!("Nutrition: {n}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n\n");
    let categories: Vec<String> = p
        .get("categories")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    Some(ImportedRecipe {
        fields: RecipeFields {
            title,
            description: str_of(p.get("description")),
            url: http_url(p.get("source_url")),
            image: http_url(p.get("image_url")),
            author: str_of(p.get("source")),
            prep_time: str_of(p.get("prep_time")),
            cook_time: str_of(p.get("cook_time")),
            total_time: str_of(p.get("total_time")),
            recipe_yield: str_of(p.get("servings")),
            recipe_category: p
                .get("categories")
                .and_then(Value::as_array)
                .and_then(|a| str_of(a.first())),
            ingredients: sections_from_lines(lines(p.get("ingredients"))),
            instructions: sections_from_lines(lines(p.get("directions"))),
            notes: Some(notes).filter(|n| !n.is_empty()),
            ..Default::default()
        },
        cookbooks: categories,
        cooked: Vec::new(),
        restored: false,
        original_url: None,
    })
}

// ─── JSON (schema.org, Mealie, our backup) ─────────────────────────────────

/// Mealie stores ingredients as objects and steps as { text } without @type.
fn from_mealie(o: &Map<String, Value>) -> Option<ImportedRecipe> {
    let title = str_of(o.get("name"))?;
    let ingredients: Vec<String> = o
        .get("recipeIngredient")?
        .as_array()?
        .iter()
        .map(|i| match i {
            Value::String(s) => s.clone(),
            other => str_of(other.get("display"))
                .or_else(|| str_of(other.get("originalText")))
                .or_else(|| str_of(other.get("note")))
                .unwrap_or_default(),
        })
        .filter(|s| !s.is_empty())
        .collect();
    let steps: Vec<String> = match o.get("recipeInstructions") {
        Some(Value::Array(a)) => a
            .iter()
            .map(|s| match s {
                Value::String(s) => s.clone(),
                other => str_of(other.get("text")).unwrap_or_default(),
            })
            .filter(|s| !s.is_empty())
            .collect(),
        other => lines(other),
    };
    let notes = match o.get("notes") {
        Some(Value::Array(a)) => {
            let joined = a
                .iter()
                .map(|n| {
                    [n.get("title"), n.get("text")]
                        .into_iter()
                        .filter_map(|v| v.and_then(Value::as_str).filter(|s| !s.is_empty()))
                        .collect::<Vec<_>>()
                        .join(": ")
                })
                .collect::<Vec<_>>()
                .join("\n");
            Some(joined).filter(|s| !s.is_empty())
        }
        _ => None,
    };
    Some(
        RecipeFields {
            title,
            description: str_of(o.get("description")),
            url: http_url(o.get("orgURL")).or_else(|| http_url(o.get("url"))),
            prep_time: str_of(o.get("prepTime")),
            cook_time: str_of(o.get("performTime")).or_else(|| str_of(o.get("cookTime"))),
            total_time: str_of(o.get("totalTime")),
            recipe_yield: str_of(o.get("recipeYield")),
            ingredients: normalize_sections(vec![Section::unnamed(ingredients)]),
            instructions: normalize_sections(vec![Section::unnamed(steps)]),
            notes,
            ..Default::default()
        }
        .into(),
    )
}

/// A backed-up (or shared) recipe with any link that isn't http(s) dropped, rather than the
/// whole recipe refused: a `javascript:` "source" must never reach a page as a link.
fn without_bad_links(r: &Value) -> Value {
    let mut r = r.clone();
    if let Some(o) = r.as_object_mut() {
        for key in ["url", "image"] {
            if o.get(key)
                .and_then(Value::as_str)
                .is_some_and(|u| !crate::model::is_valid_url(u))
            {
                o.insert(key.into(), Value::Null);
            }
        }
    }
    r
}

/// A recipe from a shared cookbook's export carries its own share link (`shareUrl`): that's
/// the link it's kept under (as when the link itself is saved, so either way dedupes), and
/// the `url` it names becomes where it came from.
fn with_share_url(r: &Value, mut item: ImportedRecipe) -> ImportedRecipe {
    let Some(share) = r
        .get("shareUrl")
        .and_then(Value::as_str)
        .filter(|u| crate::model::is_valid_url(u))
    else {
        return item;
    };
    let named = item.original_url.take().or(item.fields.url.take());
    item.original_url = named.filter(|u| !crate::recipes::same_host(u, share));
    item.fields.url = Some(share.to_string());
    item
}

fn from_backup(o: &Map<String, Value>) -> Vec<ImportedRecipe> {
    let Some(list) = o.get("recipes").and_then(Value::as_array) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|r| match RecipeFields::from_json(&without_bad_links(r)) {
            Ok(fields) => Some(with_share_url(
                r,
                ImportedRecipe {
                    fields,
                    cookbooks: r
                        .get("cookbooks")
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(Value::as_str)
                                .map(String::from)
                                .collect()
                        })
                        .unwrap_or_default(),
                    cooked: r
                        .get("cookedAt")
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(Value::as_str)
                                .filter_map(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                                .map(|t| t.timestamp())
                                .collect()
                        })
                        .unwrap_or_default(),
                    restored: true,
                    original_url: r
                        .get("originalUrl")
                        .and_then(Value::as_str)
                        .filter(|u| crate::model::is_valid_url(u))
                        .map(String::from),
                },
            )),
            Err(err) => {
                tracing::warn!("[import] skipped a backup recipe: {err}");
                None
            }
        })
        .collect()
}

pub fn from_json_value(data: &Value) -> Vec<ImportedRecipe> {
    match data {
        Value::Array(items) => items.iter().flat_map(from_json_value).collect(),
        Value::Object(o) => {
            let format = o.get("format").and_then(Value::as_str);
            if matches!(format, Some("just-the-recipe" | "crumb"))
                && o.get("recipes").is_some_and(Value::is_array)
            {
                return from_backup(o);
            }
            if is_paprika(o) {
                return from_paprika(o).into_iter().collect();
            }
            if let Some(schema) = recipe_from_json_ld(data, "")
                && (!schema.ingredients.is_empty() || !schema.instructions.is_empty())
            {
                return vec![schema.into()];
            }
            if let Some(mealie) = from_mealie(o) {
                return vec![mealie];
            }
            // Containers: { recipes: [...] } or { items: [...] }
            for key in ["recipes", "items", "data"] {
                if let Some(inner @ Value::Array(_)) = o.get(key) {
                    return from_json_value(inner);
                }
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

// ─── Text & PDF ─────────────────────────────────────────────────────────────

static CHUNK_SEPARATOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*(?:-{3,}|={3,}|\x0c)\s*$").unwrap());

async fn fields_from_text(state: &AppState, text: &str) -> RecipeFields {
    match crate::llm::extract_recipe(state, text).await {
        Some(fields) => fields,
        None => parse_recipe_text(text).recipe,
    }
}

async fn from_text(state: &AppState, text: &str) -> Vec<ImportedRecipe> {
    let chunks: Vec<String> = CHUNK_SEPARATOR
        .split(text)
        .map(|c| c.trim().to_string())
        .filter(|c| c.chars().count() > 20)
        .collect();
    let mut out = Vec::new();
    for chunk in chunks {
        let fields = fields_from_text(state, &chunk).await;
        if !fields.ingredients.is_empty() || !fields.instructions.is_empty() {
            out.push(fields.into());
        }
    }
    out
}

static PDF_FURNITURE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(page \d+( of \d+)?|justtherecipe\.com.*|made with just the recipe.*)$")
        .unwrap()
});

fn pdf_text(bytes: Vec<u8>) -> Result<String, String> {
    // pdf-extract panics on some malformed files; contain it
    std::panic::catch_unwind(move || pdf_extract::extract_text_from_mem(&bytes))
        .map_err(|_| "unreadable PDF".to_string())?
        .map_err(|e| e.to_string())
}

async fn from_pdf(state: &AppState, bytes: Vec<u8>) -> Result<Vec<ImportedRecipe>, String> {
    let text = tokio::task::spawn_blocking(move || pdf_text(bytes))
        .await
        .map_err(|e| e.to_string())??;
    // JTR PDFs are one recipe per file, often with a footer; drop page furniture
    let cleaned = text
        .split('\n')
        .filter(|l| !PDF_FURNITURE.is_match(l.trim()))
        .collect::<Vec<_>>()
        .join("\n");
    let fields = fields_from_text(state, &cleaned).await;
    Ok(
        if fields.ingredients.is_empty() && fields.instructions.is_empty() {
            Vec::new()
        } else {
            vec![fields.into()]
        },
    )
}

// ─── Dispatcher ─────────────────────────────────────────────────────────────

fn ext(name: &str) -> String {
    let lower = name.to_lowercase();
    match lower.rsplit_once('.') {
        Some((_, e)) if !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric()) => {
            e.to_string()
        }
        _ => String::new(),
    }
}

static SCRIPT_STYLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<script.*?</script>|<style.*?</style>").unwrap());
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
static HTML_START: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*<(!doctype|html)").unwrap());

fn unzip(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name().to_string();
        if name.ends_with('/') || name.starts_with("__MACOSX") {
            continue;
        }
        let mut data = Vec::new();
        file.read_to_end(&mut data).map_err(|e| e.to_string())?;
        out.push((name, data));
    }
    Ok(out)
}

fn gunzip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_end(&mut out)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

pub async fn recipes_from_file(
    state: &AppState,
    name: &str,
    bytes: Vec<u8>,
) -> Result<Vec<ImportedRecipe>, String> {
    recipes_from_file_at(state, name.to_string(), bytes, 0).await
}

fn recipes_from_file_at<'a>(
    state: &'a AppState,
    name: String,
    bytes: Vec<u8>,
    depth: u8,
) -> std::pin::Pin<Box<dyn Future<Output = Result<Vec<ImportedRecipe>, String>> + Send + 'a>> {
    Box::pin(async move {
        let kind = ext(&name);
        let is_zip = bytes.starts_with(&[0x50, 0x4b]);
        let is_gzip = bytes.starts_with(&[0x1f, 0x8b]);

        if is_zip && depth < 2 {
            let mut out = Vec::new();
            for (entry, data) in unzip(&bytes)? {
                // A bad entry shouldn't sink the whole archive
                out.extend(
                    recipes_from_file_at(state, entry, data, depth + 1)
                        .await
                        .unwrap_or_default(),
                );
            }
            return Ok(out);
        }
        if is_gzip {
            let inner = name.strip_suffix(".gz").unwrap_or(&name).to_string();
            return recipes_from_file_at(state, inner, gunzip(&bytes)?, depth + 1).await;
        }
        if kind == "pdf" {
            return from_pdf(state, bytes).await;
        }

        let text = String::from_utf8_lossy(&bytes).into_owned();
        let trimmed = text.trim_start();
        if kind == "json"
            || kind == "paprikarecipe"
            || trimmed.starts_with('{')
            || trimmed.starts_with('[')
        {
            match serde_json::from_str::<Value>(&text) {
                Ok(v) => return Ok(from_json_value(&v)),
                Err(_) if kind == "json" => return Ok(Vec::new()),
                Err(_) => {}
            }
        }
        if kind == "html" || kind == "htm" || HTML_START.is_match(&text) {
            if let Some(recipe) = parse_recipe_html(&text, "") {
                return Ok(vec![recipe.into()]);
            }
            // Treat the page's visible text as a pasted recipe
            let stripped = SCRIPT_STYLE.replace_all(&text, "");
            let visible = TAG.replace_all(&stripped, "\n");
            return Ok(from_text(state, &visible).await);
        }
        if ["txt", "md", "markdown", "text", ""].contains(&kind.as_str()) {
            return Ok(from_text(state, &text).await);
        }
        Ok(Vec::new())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backups_drop_links_that_arent_http() {
        let doc = serde_json::json!({"format": "crumb", "version": 1, "recipes": [{
            "title": "Pie",
            "url": "javascript://evil.test/%0Aalert(1)",
            "image": "data:image/png;base64,AA",
            "originalUrl": "javascript:alert(1)",
            "ingredients": [{"items": ["1 pie"]}],
            "instructions": [{"items": ["Eat."]}]
        }, {
            "title": "Tart",
            "url": "https://a.test/s/tok",
            "originalUrl": "https://food.test/tart",
            "ingredients": [], "instructions": []
        }]});
        let found = from_json_value(&doc);
        assert_eq!(found.len(), 2, "the recipe is kept, only the link goes");
        assert_eq!(found[0].fields.url, None);
        assert_eq!(found[0].fields.image, None);
        assert_eq!(found[0].original_url, None);
        assert_eq!(
            found[1].original_url.as_deref(),
            Some("https://food.test/tart")
        );
    }
    use serde_json::json;

    #[test]
    fn reads_paprika() {
        let v = json!({"name": "Scones", "ingredients": "Dough:\n2 cups flour\n- 1 tsp salt",
            "directions": "1. Mix\n2. Bake", "categories": ["Baking"], "source_url": "https://s.test/x",
            "image_url": "not-a-url", "nutritional_info": "200 kcal"});
        let out = from_json_value(&v);
        assert_eq!(out.len(), 1);
        let r = &out[0];
        assert_eq!(r.fields.ingredients[0].name.as_deref(), Some("Dough"));
        assert_eq!(
            r.fields.ingredients[0].items,
            vec!["2 cups flour", "1 tsp salt"]
        );
        assert_eq!(r.fields.instructions[0].items, vec!["Mix", "Bake"]);
        assert_eq!(r.fields.image, None);
        assert_eq!(r.fields.notes.as_deref(), Some("Nutrition: 200 kcal"));
        assert_eq!(r.cookbooks, vec!["Baking"]);
    }

    #[test]
    fn reads_backups_and_mealie() {
        let backup = json!({"format": "just-the-recipe", "version": 1, "recipes": [
            {"title": "A", "ingredients": [{"name": null, "items": ["x"]}], "cookbooks": ["Faves"]},
            {"title": ""}
        ]});
        let out = from_json_value(&backup);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].cookbooks, vec!["Faves"]);

        let mealie = json!({"name": "Stew", "recipeIngredient": [{"display": "1 onion"}, {"note": "salt"}],
            "recipeInstructions": [{"text": "Simmer"}], "orgURL": "https://m.test/s"});
        let out = from_json_value(&json!({"items": [mealie]}));
        assert_eq!(out[0].fields.ingredients[0].items, vec!["1 onion", "salt"]);
        assert_eq!(out[0].fields.url.as_deref(), Some("https://m.test/s"));
    }

    #[test]
    fn extensions() {
        assert_eq!(ext("a/b.PaprikaRecipes"), "paprikarecipes");
        assert_eq!(ext("README"), "");
    }
}
