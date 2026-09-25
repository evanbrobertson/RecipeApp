//! Service layer shared by the REST API and the MCP connector.

use rand::seq::SliceRandom;
use regex::Regex;
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::sync::LazyLock;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::importers::{self, ImportedRecipe};
use crate::model::*;

const SUMMARY_COLUMNS: &str = "r.id, r.title, r.image, r.total_time, r.recipe_yield, r.recipe_category, r.recipe_cuisine, r.source, r.created_at";

fn summary_from_row(r: &Row) -> rusqlite::Result<RecipeSummary> {
    Ok(RecipeSummary {
        id: r.get(0)?,
        title: r.get(1)?,
        image: r.get(2)?,
        total_time: r.get(3)?,
        recipe_yield: r.get(4)?,
        recipe_category: r.get(5)?,
        recipe_cuisine: r.get(6)?,
        source: r.get(7)?,
        created_at: r.get(8)?,
    })
}

fn json_column(r: &Row, name: &str) -> rusqlite::Result<Value> {
    let raw: Option<String> = r.get(name)?;
    Ok(raw
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null))
}

fn recipe_from_row(r: &Row) -> rusqlite::Result<Recipe> {
    let nutrition = json_column(r, "nutrition")?;
    Ok(Recipe {
        id: r.get("id")?,
        url: r.get("url")?,
        source: r.get("source")?,
        title: r.get("title")?,
        description: r.get("description")?,
        image: r.get("image")?,
        author: r.get("author")?,
        prep_time: r.get("prep_time")?,
        cook_time: r.get("cook_time")?,
        total_time: r.get("total_time")?,
        freeze_time: r.get("freeze_time")?,
        recipe_yield: r.get("recipe_yield")?,
        recipe_category: r.get("recipe_category")?,
        recipe_cuisine: r.get("recipe_cuisine")?,
        ingredients: normalize_sections_value(&json_column(r, "ingredients")?),
        instructions: normalize_sections_value(&json_column(r, "instructions")?),
        nutrition: if nutrition.is_null() {
            None
        } else {
            Some(nutrition)
        },
        notes: r.get("notes")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}

fn like_pattern(term: &str) -> String {
    let mut escaped = String::with_capacity(term.len() + 2);
    escaped.push('%');
    for c in term.chars() {
        if matches!(c, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped.push('%');
    escaped
}

/// Things a recipe can be missing, as (json key, SQL condition on `r`), for `search_recipes`.
pub const MISSING_FILTERS: [(&str, &str); 15] = [
    ("image", "coalesce(trim(r.image), '') = ''"),
    ("description", "coalesce(trim(r.description), '') = ''"),
    ("url", "coalesce(trim(r.url), '') = ''"),
    ("author", "coalesce(trim(r.author), '') = ''"),
    ("prepTime", "coalesce(trim(r.prep_time), '') = ''"),
    ("cookTime", "coalesce(trim(r.cook_time), '') = ''"),
    ("totalTime", "coalesce(trim(r.total_time), '') = ''"),
    ("recipeYield", "coalesce(trim(r.recipe_yield), '') = ''"),
    (
        "recipeCategory",
        "coalesce(trim(r.recipe_category), '') = ''",
    ),
    ("recipeCuisine", "coalesce(trim(r.recipe_cuisine), '') = ''"),
    ("notes", "coalesce(trim(r.notes), '') = ''"),
    ("ingredients", "coalesce(r.ingredients, '') IN ('', '[]')"),
    ("instructions", "coalesce(r.instructions, '') IN ('', '[]')"),
    (
        "nutrition",
        "coalesce(r.nutrition, '') IN ('', 'null', '{}')",
    ),
    (
        "cookbook",
        "NOT EXISTS (SELECT 1 FROM cookbook_recipes cr WHERE cr.recipe_id = r.id)",
    ),
];

/// Every whitespace-separated term must match the title, ingredients, category or cuisine.
pub fn list_recipes(
    conn: &Connection,
    q: Option<&str>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> AppResult<Vec<RecipeSummary>> {
    search_recipes(conn, q, &[], limit, offset)
}

/// [`list_recipes`] that also keeps only recipes missing every field in `missing`
/// (keys of [`MISSING_FILTERS`]; unknown keys are an error).
pub fn search_recipes(
    conn: &Connection,
    q: Option<&str>,
    missing: &[&str],
    limit: Option<i64>,
    offset: Option<i64>,
) -> AppResult<Vec<RecipeSummary>> {
    let terms: Vec<String> = q
        .unwrap_or("")
        .split_whitespace()
        .map(like_pattern)
        .collect();
    let mut clauses: Vec<String> = (1..=terms.len())
        .map(|i| {
            format!(
                "(r.title LIKE ?{i} ESCAPE '\\' OR r.ingredients LIKE ?{i} ESCAPE '\\' \
                 OR r.recipe_category LIKE ?{i} ESCAPE '\\' OR r.recipe_cuisine LIKE ?{i} ESCAPE '\\')"
            )
        })
        .collect();
    for key in missing {
        let (_, condition) = MISSING_FILTERS
            .iter()
            .find(|(k, _)| k == key)
            .ok_or_else(|| AppError::bad_request(format!("missing: unknown field \"{key}\"")))?;
        clauses.push((*condition).to_string());
    }
    let mut sql = format!("SELECT {SUMMARY_COLUMNS} FROM recipes r");
    if !clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }
    sql.push_str(&format!(
        " ORDER BY r.created_at DESC, r.id DESC LIMIT {} OFFSET {}",
        limit.unwrap_or(500),
        offset.unwrap_or(0)
    ));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(terms.iter()), summary_from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get_recipe(conn: &Connection, id: i64) -> AppResult<Option<Recipe>> {
    Ok(conn
        .query_row("SELECT * FROM recipes WHERE id = ?1", [id], recipe_from_row)
        .optional()?)
}

pub fn require_recipe(conn: &Connection, id: i64) -> AppResult<Recipe> {
    get_recipe(conn, id)?.ok_or_else(|| AppError::not_found("Recipe not found"))
}

fn find_by_url(conn: &Connection, url: &str) -> AppResult<Option<i64>> {
    Ok(conn
        .query_row("SELECT id FROM recipes WHERE url = ?1", [url], |r| r.get(0))
        .optional()?)
}

fn to_json_text<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "[]".into())
}

pub fn create_recipe(
    conn: &Connection,
    fields: RecipeFields,
    source: &str,
) -> AppResult<(Recipe, bool)> {
    let fields = fields.validate()?;
    if let Some(url) = &fields.url
        && let Some(id) = find_by_url(conn, url)?
    {
        return Ok((require_recipe(conn, id)?, false));
    }
    let now = now_secs();
    conn.execute(
        "INSERT INTO recipes (url, source, title, description, image, author, prep_time, cook_time,
           total_time, freeze_time, recipe_yield, recipe_category, recipe_cuisine, ingredients,
           instructions, nutrition, notes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?18)",
        params![
            fields.url,
            source,
            fields.title,
            fields.description,
            fields.image,
            fields.author,
            fields.prep_time,
            fields.cook_time,
            fields.total_time,
            fields.freeze_time,
            fields.recipe_yield,
            fields.recipe_category,
            fields.recipe_cuisine,
            to_json_text(&normalize_sections(fields.ingredients)),
            to_json_text(&normalize_sections(fields.instructions)),
            fields.nutrition.as_ref().map(to_json_text),
            fields.notes,
            now,
        ],
    )?;
    let id = conn.last_insert_rowid();
    Ok((require_recipe(conn, id)?, true))
}

pub fn update_recipe(conn: &Connection, id: i64, patch: RecipePatch) -> AppResult<Recipe> {
    if let Some(Some(url)) = &patch.url
        && let Some(existing) = find_by_url(conn, url)?
        && existing != id
    {
        return Err(AppError::new(409, "Another recipe already uses that URL"));
    }

    let mut sets: Vec<&str> = Vec::new();
    let mut values: Vec<SqlValue> = Vec::new();
    let text = |v: Option<String>| v.map(SqlValue::Text).unwrap_or(SqlValue::Null);
    let mut push = |col: &'static str, v: SqlValue| {
        sets.push(col);
        values.push(v);
    };
    if let Some(t) = patch.title {
        push("title", SqlValue::Text(t));
    }
    for (col, v) in [
        ("description", patch.description),
        ("url", patch.url),
        ("image", patch.image),
        ("author", patch.author),
        ("prep_time", patch.prep_time),
        ("cook_time", patch.cook_time),
        ("total_time", patch.total_time),
        ("freeze_time", patch.freeze_time),
        ("recipe_yield", patch.recipe_yield),
        ("recipe_category", patch.recipe_category),
        ("recipe_cuisine", patch.recipe_cuisine),
        ("notes", patch.notes),
    ] {
        if let Some(v) = v {
            push(col, text(v));
        }
    }
    if let Some(s) = patch.ingredients {
        push(
            "ingredients",
            SqlValue::Text(to_json_text(&normalize_sections(s))),
        );
    }
    if let Some(s) = patch.instructions {
        push(
            "instructions",
            SqlValue::Text(to_json_text(&normalize_sections(s))),
        );
    }
    if let Some(n) = patch.nutrition {
        push("nutrition", text(n.as_ref().map(to_json_text)));
    }
    push("updated_at", SqlValue::Integer(now_secs()));

    let assignments = sets
        .iter()
        .enumerate()
        .map(|(i, col)| format!("{col} = ?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    values.push(SqlValue::Integer(id));
    let changed = conn.execute(
        &format!(
            "UPDATE recipes SET {assignments} WHERE id = ?{}",
            values.len()
        ),
        params_from_iter(values),
    )?;
    if changed == 0 {
        return Err(AppError::not_found("Recipe not found"));
    }
    require_recipe(conn, id)
}

pub fn delete_recipes(conn: &Connection, ids: &[i64]) -> AppResult<usize> {
    if ids.is_empty() {
        return Ok(0);
    }
    let placeholders = vec!["?"; ids.len()].join(", ");
    Ok(conn.execute(
        &format!("DELETE FROM recipes WHERE id IN ({placeholders})"),
        params_from_iter(ids.iter()),
    )?)
}

fn is_http(s: &str) -> bool {
    let lower = s.get(..8).unwrap_or(s).to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Share links from Just the Recipe (and similar readers) wrap the original page URL,
/// e.g. https://www.justtherecipe.com/?url=https://site.com/recipe. Import the original.
pub fn unwrap_share_link(input: &str) -> String {
    let Ok(url) = url::Url::parse(input) else {
        return input.to_string();
    };
    for key in ["url", "u", "link", "recipe"] {
        if let Some((_, inner)) = url.query_pairs().find(|(k, _)| k == key)
            && is_http(&inner)
        {
            return inner.into_owned();
        }
    }
    let path = url.path().strip_prefix('/').unwrap_or(url.path());
    let decoded = percent_encoding::percent_decode_str(path).decode_utf8_lossy();
    let host = url.host_str().unwrap_or("").to_ascii_lowercase();
    if host.contains("justtherecipe") && is_http(&decoded) {
        return decoded.into_owned();
    }
    input.to_string()
}

pub async fn import_from_url(state: &AppState, raw_url: &str) -> AppResult<(Recipe, bool)> {
    let url = unwrap_share_link(raw_url);
    {
        let conn = state.db.lock();
        if let Some(id) = find_by_url(&conn, &url)? {
            return Ok((require_recipe(&conn, id)?, false));
        }
    }
    let fields = crate::scraper::scrape_recipe(state, &url).await?;
    create_recipe(&state.db.lock(), fields, "url")
}

/// Fields [`refresh_from_source`] can fill or overwrite, as json keys.
pub const REFRESHABLE: [&str; 15] = [
    "title",
    "description",
    "image",
    "author",
    "prepTime",
    "cookTime",
    "totalTime",
    "freezeTime",
    "recipeYield",
    "recipeCategory",
    "recipeCuisine",
    "ingredients",
    "instructions",
    "nutrition",
    "notes",
];

/// Re-scrapes a recipe's source URL and fills in fields that are empty, keeping everything
/// the user has already set. Fields named in `overwrite` are replaced even when set (the
/// title only ever changes this way). Returns the recipe and the json keys that changed.
pub async fn refresh_from_source(
    state: &AppState,
    id: i64,
    overwrite: &[&str],
) -> AppResult<(Recipe, Vec<&'static str>)> {
    if let Some(bad) = overwrite.iter().find(|k| !REFRESHABLE.contains(k)) {
        return Err(AppError::bad_request(format!(
            "overwrite: unknown field \"{bad}\""
        )));
    }
    let current = require_recipe(&state.db.lock(), id)?;
    let url =
        current.url.clone().filter(|u| is_http(u)).ok_or_else(|| {
            AppError::bad_request("This recipe has no source URL to refresh from")
        })?;
    let scraped = crate::scraper::scrape_recipe(state, &url).await?;

    let wants = |key: &str| overwrite.contains(&key);
    let blank = |v: &Option<String>| v.as_deref().is_none_or(|s| s.trim().is_empty());
    let mut changed = Vec::new();
    let mut patch = RecipePatch::default();

    if wants("title") && !scraped.title.trim().is_empty() && scraped.title != current.title {
        patch.title = Some(scraped.title);
        changed.push("title");
    }
    for (key, have, got, slot) in [
        (
            "description",
            &current.description,
            scraped.description,
            &mut patch.description,
        ),
        ("image", &current.image, scraped.image, &mut patch.image),
        ("author", &current.author, scraped.author, &mut patch.author),
        (
            "prepTime",
            &current.prep_time,
            scraped.prep_time,
            &mut patch.prep_time,
        ),
        (
            "cookTime",
            &current.cook_time,
            scraped.cook_time,
            &mut patch.cook_time,
        ),
        (
            "totalTime",
            &current.total_time,
            scraped.total_time,
            &mut patch.total_time,
        ),
        (
            "freezeTime",
            &current.freeze_time,
            scraped.freeze_time,
            &mut patch.freeze_time,
        ),
        (
            "recipeYield",
            &current.recipe_yield,
            scraped.recipe_yield,
            &mut patch.recipe_yield,
        ),
        (
            "recipeCategory",
            &current.recipe_category,
            scraped.recipe_category,
            &mut patch.recipe_category,
        ),
        (
            "recipeCuisine",
            &current.recipe_cuisine,
            scraped.recipe_cuisine,
            &mut patch.recipe_cuisine,
        ),
        ("notes", &current.notes, scraped.notes, &mut patch.notes),
    ] {
        if !blank(&got) && (blank(have) || wants(key)) && got != *have {
            *slot = Some(got);
            changed.push(key);
        }
    }
    for (key, have, got, slot) in [
        (
            "ingredients",
            &current.ingredients,
            scraped.ingredients,
            &mut patch.ingredients,
        ),
        (
            "instructions",
            &current.instructions,
            scraped.instructions,
            &mut patch.instructions,
        ),
    ] {
        if !got.is_empty() && (have.is_empty() || wants(key)) && got != *have {
            *slot = Some(got);
            changed.push(key);
        }
    }
    if scraped.nutrition.is_some()
        && (current.nutrition.is_none() || wants("nutrition"))
        && scraped.nutrition.clone().map(Value::Object) != current.nutrition
    {
        patch.nutrition = Some(scraped.nutrition);
        changed.push("nutrition");
    }

    if changed.is_empty() {
        return Ok((current, changed));
    }
    Ok((update_recipe(&state.db.lock(), id, patch)?, changed))
}

static URL_ONLY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*(https?://\S+)\s*$").unwrap());

/// Imports a recipe from pasted text. A lone URL is scraped; otherwise Claude parses
/// the text when configured, with the built-in heuristic parser as the fallback.
pub async fn import_from_text(
    state: &AppState,
    text: &str,
    use_claude: bool,
) -> AppResult<(Recipe, bool)> {
    if let Some(m) = URL_ONLY.captures(text) {
        return import_from_url(state, &m[1]).await;
    }
    let mut fields = None;
    if use_claude {
        fields = crate::llm::extract_recipe(state, text).await;
    }
    let mut fields = fields.unwrap_or_else(|| crate::text_parser::parse_recipe_text(text).recipe);

    if fields.ingredients.is_empty() && fields.instructions.is_empty() {
        return Err(AppError::new(
            422,
            "Couldn't find ingredients or steps in that text. Add “Ingredients” and “Instructions” headings and try again.",
        ));
    }

    let conn = state.db.lock();
    // Keep a source link if one was pasted alongside the text, unless it's already saved
    if let Some(url) = &fields.url
        && find_by_url(&conn, url)?.is_some()
    {
        fields.url = None;
    }
    create_recipe(&conn, fields, "text")
}

// ─── Files & backups ─────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct CreatedRef {
    pub id: i64,
    pub title: String,
}

#[derive(Debug, Serialize)]
pub struct ImportSummary {
    pub file: String,
    pub created: Vec<CreatedRef>,
    pub duplicates: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ImportSummary {
    pub fn failed(file: &str, error: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            created: vec![],
            duplicates: 0,
            error: Some(error.into()),
        }
    }
}

fn find_or_create_cookbook(conn: &Connection, name: &str) -> AppResult<i64> {
    let trimmed = name.trim();
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM cookbooks WHERE lower(name) = lower(?1)",
            [trimmed],
            |r| r.get(0),
        )
        .optional()?;
    match existing {
        Some(id) => Ok(id),
        None => Ok(create_cookbook(conn, trimmed, None, None)?.id),
    }
}

pub async fn import_file(state: &AppState, name: &str, bytes: Vec<u8>) -> ImportSummary {
    let found: Vec<ImportedRecipe> = match importers::recipes_from_file(state, name, bytes).await {
        Ok(found) => found,
        Err(err) => return ImportSummary::failed(name, format!("Couldn't read this file ({err})")),
    };
    if found.is_empty() {
        return ImportSummary::failed(name, "No recipes found in this file");
    }

    let mut summary = ImportSummary {
        file: name.into(),
        created: vec![],
        duplicates: 0,
        error: None,
    };
    let conn = state.db.lock();
    for item in found {
        let result = (|| -> AppResult<()> {
            let (recipe, is_new) = create_recipe(&conn, item.fields, "import")?;
            if is_new {
                summary.created.push(CreatedRef {
                    id: recipe.id,
                    title: recipe.title.clone(),
                });
            } else {
                summary.duplicates += 1;
            }
            for book in item.cookbooks.iter().filter(|b| !b.trim().is_empty()) {
                let id = find_or_create_cookbook(&conn, book)?;
                add_to_cookbook(&conn, id, &[recipe.id])?;
            }
            // Restoring the same backup twice doesn't double the cook log
            for at in &item.cooked {
                conn.execute(
                    "INSERT INTO recipe_events (recipe_id, kind, created_at)
                     SELECT ?1, 'cooked', ?2 WHERE NOT EXISTS (
                       SELECT 1 FROM recipe_events WHERE recipe_id = ?1 AND kind = 'cooked' AND created_at = ?2)",
                    params![recipe.id, at],
                )?;
            }
            Ok(())
        })();
        if let Err(err) = result {
            tracing::warn!("[import] skipped a recipe in {name}: {err}");
        }
    }
    summary
}

/// Everything in one JSON file that `import_file` can read back.
pub fn export_backup(conn: &Connection) -> AppResult<Value> {
    let mut stmt = conn.prepare("SELECT * FROM recipes ORDER BY id")?;
    let recipes: Vec<Recipe> = stmt
        .query_map([], recipe_from_row)?
        .collect::<rusqlite::Result<_>>()?;
    let mut stmt = conn.prepare("SELECT id, name, description FROM cookbooks ORDER BY name")?;
    let books: Vec<(i64, String, Option<String>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut stmt = conn.prepare("SELECT cookbook_id, recipe_id FROM cookbook_recipes")?;
    let links: Vec<(i64, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut stmt = conn.prepare(
        "SELECT recipe_id, created_at FROM recipe_events WHERE kind = 'cooked' ORDER BY created_at",
    )?;
    let cooks: Vec<(i64, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let book_name = |id: i64| books.iter().find(|b| b.0 == id).map(|b| b.1.clone());
    let recipes: Vec<Value> = recipes
        .iter()
        .map(|r| {
            let mut m = r.fields().to_json();
            let names: Vec<String> = links
                .iter()
                .filter(|l| l.1 == r.id)
                .filter_map(|l| book_name(l.0))
                .collect();
            m.insert("cookbooks".into(), json!(names));
            let cooked: Vec<String> = cooks
                .iter()
                .filter(|c| c.0 == r.id)
                .map(|c| iso(c.1))
                .collect();
            if !cooked.is_empty() {
                m.insert("cookedAt".into(), json!(cooked));
            }
            Value::Object(m)
        })
        .collect();
    Ok(json!({
        "format": "crumb",
        "version": 1,
        "cookbooks": books.iter().map(|b| json!({"name": b.1, "description": b.2})).collect::<Vec<_>>(),
        "recipes": recipes,
    }))
}

// ─── Cook and view log ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Viewed,
    Cooked,
}

impl EventKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Viewed => "viewed",
            Self::Cooked => "cooked",
        }
    }

    /// Repeat events closer together than this count once (a reload, "start over").
    fn dedupe_secs(self) -> i64 {
        match self {
            Self::Viewed => 30 * 60,
            Self::Cooked => 6 * 3600,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CookStats {
    pub count: i64,
    pub last_cooked_at: Option<String>,
}

fn require_exists(conn: &Connection, id: i64) -> AppResult<()> {
    let found: Option<i64> = conn
        .query_row("SELECT id FROM recipes WHERE id = ?1", [id], |r| r.get(0))
        .optional()?;
    found
        .map(|_| ())
        .ok_or_else(|| AppError::not_found("Recipe not found"))
}

/// Logs a view or a cook at `at` (unix seconds), unless one was logged near that time.
/// Returns the new event's id, or None when a matching one was already logged.
pub fn log_event(conn: &Connection, id: i64, kind: EventKind, at: i64) -> AppResult<Option<i64>> {
    require_exists(conn, id)?;
    let written = conn.execute(
        "INSERT INTO recipe_events (recipe_id, kind, created_at)
         SELECT ?1, ?2, ?3 WHERE NOT EXISTS (
           SELECT 1 FROM recipe_events WHERE recipe_id = ?1 AND kind = ?2 AND abs(created_at - ?3) < ?4)",
        params![id, kind.as_str(), at, kind.dedupe_secs()],
    )?;
    Ok((written > 0).then(|| conn.last_insert_rowid()))
}

/// The "Undo" on "Marked as cooked": removes that cook (`event`, from [`log_event`]),
/// or without one the most recently logged cook of the recipe.
pub fn undo_cooked(conn: &Connection, id: i64, event: Option<i64>) -> AppResult<CookStats> {
    require_exists(conn, id)?;
    match event {
        Some(event) => conn.execute(
            "DELETE FROM recipe_events WHERE id = ?1 AND recipe_id = ?2 AND kind = 'cooked'",
            params![event, id],
        )?,
        None => conn.execute(
            "DELETE FROM recipe_events WHERE id = (
               SELECT max(id) FROM recipe_events WHERE recipe_id = ?1 AND kind = 'cooked')",
            [id],
        )?,
    };
    cook_stats(conn, id)
}

pub fn cook_stats(conn: &Connection, id: i64) -> AppResult<CookStats> {
    let (count, last): (i64, Option<i64>) = conn.query_row(
        "SELECT count(*), max(created_at) FROM recipe_events WHERE recipe_id = ?1 AND kind = 'cooked'",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(CookStats {
        count,
        last_cooked_at: last.map(iso),
    })
}

/// Summaries for these ids, in the order given (unknown ids are skipped).
pub fn summaries_by_ids(conn: &Connection, ids: &[i64]) -> AppResult<Vec<RecipeSummary>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let marks = vec!["?"; ids.len()].join(",");
    let mut stmt = conn.prepare(&format!(
        "SELECT {SUMMARY_COLUMNS} FROM recipes r WHERE r.id IN ({marks})"
    ))?;
    let rows: Vec<RecipeSummary> = stmt
        .query_map(params_from_iter(ids.iter()), summary_from_row)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ids
        .iter()
        .filter_map(|id| rows.iter().find(|r| r.id == *id).cloned())
        .collect())
}

// ─── Cookbooks ──────────────────────────────────────────────────────────────

pub fn list_cookbooks(conn: &Connection) -> AppResult<Vec<CookbookListItem>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, c.description, c.color, c.created_at, count(cr.id)
         FROM cookbooks c LEFT JOIN cookbook_recipes cr ON c.id = cr.cookbook_id
         GROUP BY c.id ORDER BY c.name",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(CookbookListItem {
            id: r.get(0)?,
            name: r.get(1)?,
            description: r.get(2)?,
            color: r.get(3)?,
            created_at: r.get(4)?,
            recipe_count: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn cookbook_from_row(r: &Row) -> rusqlite::Result<Cookbook> {
    Ok(Cookbook {
        id: r.get("id")?,
        name: r.get("name")?,
        description: r.get("description")?,
        color: r.get("color")?,
        created_at: r.get("created_at")?,
    })
}

fn find_cookbook(conn: &Connection, id: i64) -> AppResult<Option<Cookbook>> {
    Ok(conn
        .query_row(
            "SELECT id, name, description, color, created_at FROM cookbooks WHERE id = ?1",
            [id],
            cookbook_from_row,
        )
        .optional()?)
}

fn cookbook_not_found() -> AppError {
    AppError::not_found("Cookbook not found")
}

pub fn get_cookbook(conn: &Connection, id: i64) -> AppResult<CookbookWithRecipes> {
    let book = find_cookbook(conn, id)?.ok_or_else(cookbook_not_found)?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {SUMMARY_COLUMNS} FROM cookbook_recipes cr
         INNER JOIN recipes r ON cr.recipe_id = r.id
         WHERE cr.cookbook_id = ?1 ORDER BY r.title"
    ))?;
    let recipes = stmt
        .query_map([id], summary_from_row)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(CookbookWithRecipes {
        id: book.id,
        name: book.name,
        description: book.description,
        color: book.color,
        created_at: book.created_at,
        recipes,
    })
}

pub fn create_cookbook(
    conn: &Connection,
    name: &str,
    description: Option<&str>,
    color: Option<&str>,
) -> AppResult<Cookbook> {
    let pick = *BOOK_COLORS
        .choose(&mut rand::thread_rng())
        .unwrap_or(&"tomato");
    let description = description.map(str::trim).filter(|d| !d.is_empty());
    conn.execute(
        "INSERT INTO cookbooks (name, description, color, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![name.trim(), description, color.unwrap_or(pick), now_secs()],
    )?;
    find_cookbook(conn, conn.last_insert_rowid())?.ok_or_else(cookbook_not_found)
}

#[derive(Default)]
pub struct CookbookPatch {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub color: Option<String>,
}

pub fn update_cookbook(conn: &Connection, id: i64, patch: CookbookPatch) -> AppResult<Cookbook> {
    let mut sets = Vec::new();
    let mut values: Vec<SqlValue> = Vec::new();
    if let Some(n) = patch.name {
        sets.push("name");
        values.push(SqlValue::Text(n));
    }
    if let Some(d) = patch.description {
        sets.push("description");
        values.push(d.map(SqlValue::Text).unwrap_or(SqlValue::Null));
    }
    if let Some(c) = patch.color {
        sets.push("color");
        values.push(SqlValue::Text(c));
    }
    if !sets.is_empty() {
        let assignments = sets
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{c} = ?{}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");
        values.push(SqlValue::Integer(id));
        conn.execute(
            &format!(
                "UPDATE cookbooks SET {assignments} WHERE id = ?{}",
                values.len()
            ),
            params_from_iter(values),
        )?;
    }
    find_cookbook(conn, id)?.ok_or_else(cookbook_not_found)
}

pub fn delete_cookbook(conn: &Connection, id: i64) -> AppResult<()> {
    if conn.execute("DELETE FROM cookbooks WHERE id = ?1", [id])? == 0 {
        return Err(cookbook_not_found());
    }
    Ok(())
}

pub fn add_to_cookbook(
    conn: &Connection,
    cookbook_id: i64,
    recipe_ids: &[i64],
) -> AppResult<usize> {
    if find_cookbook(conn, cookbook_id)?.is_none() {
        return Err(cookbook_not_found());
    }
    let mut added = 0;
    let mut stmt = conn.prepare(
        "INSERT INTO cookbook_recipes (cookbook_id, recipe_id)
         SELECT ?1, id FROM recipes WHERE id = ?2
         ON CONFLICT DO NOTHING",
    )?;
    for id in recipe_ids {
        added += stmt.execute(params![cookbook_id, id])?;
    }
    Ok(added)
}

/// Returns how many links were removed (0 if the recipe wasn't in the cookbook).
pub fn remove_from_cookbook(
    conn: &Connection,
    cookbook_id: i64,
    recipe_id: i64,
) -> AppResult<usize> {
    Ok(conn.execute(
        "DELETE FROM cookbook_recipes WHERE cookbook_id = ?1 AND recipe_id = ?2",
        params![cookbook_id, recipe_id],
    )?)
}

pub fn recipe_cookbook_ids(conn: &Connection, recipe_id: i64) -> AppResult<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT cookbook_id FROM cookbook_recipes WHERE recipe_id = ?1")?;
    let rows = stmt.query_map([recipe_id], |r| r.get(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Serializes anything to a JSON value (for page data).
pub fn to_value<T: Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

pub fn with_is_new(recipe: &Recipe, is_new: bool) -> Value {
    let mut v = to_value(recipe);
    if let Value::Object(m) = &mut v {
        m.insert("isNew".into(), Value::Bool(is_new));
    }
    v
}

pub fn empty_map() -> Map<String, Value> {
    Map::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwraps_share_links() {
        assert_eq!(
            unwrap_share_link("https://www.justtherecipe.com/?url=https://site.com/recipe"),
            "https://site.com/recipe"
        );
        assert_eq!(
            unwrap_share_link("https://www.justtherecipe.com/https%3A%2F%2Fsite.com%2Fr"),
            "https://site.com/r"
        );
        assert_eq!(
            unwrap_share_link("https://reader.test/view?u=http://a.test/x&z=1"),
            "http://a.test/x"
        );
        assert_eq!(
            unwrap_share_link("https://site.com/recipe?x=1"),
            "https://site.com/recipe?x=1"
        );
        assert_eq!(unwrap_share_link("not a url"), "not a url");
    }

    #[test]
    fn like_escapes_wildcards() {
        assert_eq!(like_pattern("50%_off\\"), "%50\\%\\_off\\\\%");
    }
}
