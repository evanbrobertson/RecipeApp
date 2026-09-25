//! Recipe data shapes shared by the REST API, the MCP connector and the importers,
//! plus the validation rules the old zod schemas enforced.

use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Section {
    pub name: Option<String>,
    pub items: Vec<String>,
}

impl Section {
    pub fn unnamed(items: Vec<String>) -> Self {
        Self { name: None, items }
    }
}

pub type Nutrition = Map<String, Value>;

/// Cloth colours for cookbooks on the shelf.
pub const BOOK_COLORS: [&str; 10] = [
    "tomato",
    "sage",
    "mustard",
    "plum",
    "ocean",
    "terracotta",
    "forest",
    "navy",
    "rose",
    "charcoal",
];

pub const SOURCES: [&str; 5] = ["url", "text", "claude", "manual", "import"];

/// Normalizes raw JSON from the database (a legacy flat string[] or a Section[])
/// into sections, dropping blank items and empty sections.
pub fn normalize_sections_value(data: &Value) -> Vec<Section> {
    let Some(list) = data.as_array() else {
        return Vec::new();
    };
    if list.is_empty() {
        return Vec::new();
    }
    if list.iter().all(Value::is_string) {
        let items: Vec<String> = list
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        return if items.is_empty() {
            Vec::new()
        } else {
            vec![Section::unnamed(items)]
        };
    }
    list.iter()
        .map(|s| {
            let name = s
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(String::from);
            let items = s
                .get("items")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::trim)
                        .filter(|i| !i.is_empty())
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();
            Section { name, items }
        })
        .filter(|s| !s.items.is_empty())
        .collect()
}

/// Same as [`normalize_sections_value`] for already-typed sections.
pub fn normalize_sections(sections: Vec<Section>) -> Vec<Section> {
    sections
        .into_iter()
        .map(|s| Section {
            name: s
                .name
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty()),
            items: s
                .items
                .into_iter()
                .map(|i| i.trim().to_string())
                .filter(|i| !i.is_empty())
                .collect(),
        })
        .filter(|s| !s.items.is_empty())
        .collect()
}

pub fn count_items(sections: &[Section]) -> usize {
    sections.iter().map(|s| s.items.len()).sum()
}

/// Unix seconds (how drizzle stored `mode: "timestamp"`) as JS `Date.toJSON()` would print it.
pub fn iso(secs: i64) -> String {
    // Guard against rows written in milliseconds
    let secs = if secs > 100_000_000_000 {
        secs / 1000
    } else {
        secs
    };
    chrono::DateTime::from_timestamp(secs, 0)
        .unwrap_or_default()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn ser_iso<S: Serializer>(secs: &i64, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&iso(*secs))
}

/// Every settable recipe field, validated.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RecipeFields {
    pub title: String,
    pub description: Option<String>,
    pub url: Option<String>,
    pub image: Option<String>,
    pub author: Option<String>,
    pub prep_time: Option<String>,
    pub cook_time: Option<String>,
    pub total_time: Option<String>,
    pub freeze_time: Option<String>,
    pub recipe_yield: Option<String>,
    pub recipe_category: Option<String>,
    pub recipe_cuisine: Option<String>,
    pub ingredients: Vec<Section>,
    pub instructions: Vec<Section>,
    pub nutrition: Option<Nutrition>,
    pub notes: Option<String>,
}

/// A partial update: `None` = leave alone, `Some(None)` = clear.
#[derive(Debug, Clone, Default)]
pub struct RecipePatch {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub url: Option<Option<String>>,
    pub image: Option<Option<String>>,
    pub author: Option<Option<String>>,
    pub prep_time: Option<Option<String>>,
    pub cook_time: Option<Option<String>>,
    pub total_time: Option<Option<String>>,
    pub freeze_time: Option<Option<String>>,
    pub recipe_yield: Option<Option<String>>,
    pub recipe_category: Option<Option<String>>,
    pub recipe_cuisine: Option<Option<String>>,
    pub ingredients: Option<Vec<Section>>,
    pub instructions: Option<Vec<Section>>,
    pub nutrition: Option<Option<Nutrition>>,
    pub notes: Option<Option<String>>,
}

/// Text columns other than title/url/image, as (json key, column).
pub const TEXT_FIELDS: [(&str, &str); 10] = [
    ("description", "description"),
    ("author", "author"),
    ("prepTime", "prep_time"),
    ("cookTime", "cook_time"),
    ("totalTime", "total_time"),
    ("freezeTime", "freeze_time"),
    ("recipeYield", "recipe_yield"),
    ("recipeCategory", "recipe_category"),
    ("recipeCuisine", "recipe_cuisine"),
    ("notes", "notes"),
];

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn expected(field: &str, what: &str, got: &Value) -> AppError {
    AppError::bad_request(format!(
        "{field}: Invalid input: expected {what}, received {}",
        type_name(got)
    ))
}

/// A present key: `Ok(None)` for null, `Ok(Some(trimmed))` for strings.
fn nullable_text(field: &str, v: &Value) -> AppResult<Option<String>> {
    match v {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.trim().to_string())),
        other => Err(expected(field, "string", other)),
    }
}

pub fn is_valid_url(value: &str) -> bool {
    url::Url::parse(value).is_ok()
}

fn nullable_url(field: &str, v: &Value) -> AppResult<Option<String>> {
    match v {
        Value::Null => Ok(None),
        Value::String(s) if is_valid_url(s) => Ok(Some(s.clone())),
        Value::String(_) => Err(AppError::bad_request(format!("{field}: Invalid URL"))),
        other => Err(expected(field, "string", other)),
    }
}

fn title(v: &Value) -> AppResult<String> {
    let Value::String(s) = v else {
        return Err(expected("title", "string", v));
    };
    let t = s.trim();
    if t.is_empty() {
        return Err(AppError::bad_request("title: Title is required"));
    }
    if t.chars().count() > 300 {
        return Err(AppError::bad_request(
            "title: Title is too long (300 characters max)",
        ));
    }
    Ok(t.to_string())
}

pub fn parse_sections(field: &str, v: &Value) -> AppResult<Vec<Section>> {
    let Value::Array(list) = v else {
        return Err(expected(field, "array", v));
    };
    list.iter()
        .map(|s| {
            let Value::Object(o) = s else {
                return Err(expected(field, "object", s));
            };
            let name = match o.get("name") {
                None | Some(Value::Null) => None,
                Some(Value::String(n)) => Some(n.clone()),
                Some(other) => return Err(expected(&format!("{field}.name"), "string", other)),
            };
            let items = match o.get("items") {
                Some(Value::Array(items)) => items
                    .iter()
                    .map(|i| match i {
                        Value::String(s) => Ok(s.clone()),
                        other => Err(expected(&format!("{field}.items"), "string", other)),
                    })
                    .collect::<AppResult<Vec<_>>>()?,
                Some(other) => return Err(expected(&format!("{field}.items"), "array", other)),
                None => return Err(expected(&format!("{field}.items"), "array", &Value::Null)),
            };
            Ok(Section { name, items })
        })
        .collect()
}

fn nutrition(v: &Value) -> AppResult<Option<Nutrition>> {
    match v {
        Value::Null => Ok(None),
        Value::Object(o) => {
            for (k, val) in o {
                if !val.is_string() {
                    return Err(expected(&format!("nutrition.{k}"), "string", val));
                }
            }
            Ok(Some(o.clone()))
        }
        other => Err(expected("nutrition", "record", other)),
    }
}

fn object(v: &Value) -> AppResult<&Map<String, Value>> {
    v.as_object()
        .ok_or_else(|| AppError::bad_request("Invalid input: expected object"))
}

impl RecipePatch {
    /// Parses a partial update (zod `recipePatchSchema`); unknown keys are ignored.
    pub fn from_json(v: &Value) -> AppResult<Self> {
        let o = object(v)?;
        let mut p = RecipePatch::default();
        let text = |key: &str| o.get(key).map(|v| nullable_text(key, v)).transpose();
        if let Some(v) = o.get("title") {
            p.title = Some(title(v)?);
        }
        p.description = text("description")?;
        p.author = text("author")?;
        p.prep_time = text("prepTime")?;
        p.cook_time = text("cookTime")?;
        p.total_time = text("totalTime")?;
        p.freeze_time = text("freezeTime")?;
        p.recipe_yield = text("recipeYield")?;
        p.recipe_category = text("recipeCategory")?;
        p.recipe_cuisine = text("recipeCuisine")?;
        p.notes = text("notes")?;
        p.url = o.get("url").map(|v| nullable_url("url", v)).transpose()?;
        p.image = o
            .get("image")
            .map(|v| nullable_url("image", v))
            .transpose()?;
        p.ingredients = o
            .get("ingredients")
            .map(|v| parse_sections("ingredients", v))
            .transpose()?;
        p.instructions = o
            .get("instructions")
            .map(|v| parse_sections("instructions", v))
            .transpose()?;
        p.nutrition = o.get("nutrition").map(nutrition).transpose()?;
        Ok(p)
    }
}

impl RecipeFields {
    /// Parses a full recipe (zod `recipeFieldsSchema`): title required, lists default to [].
    pub fn from_json(v: &Value) -> AppResult<Self> {
        let o = object(v)?;
        if !o.contains_key("title") {
            return Err(AppError::bad_request(
                "title: Invalid input: expected string, received undefined",
            ));
        }
        let p = RecipePatch::from_json(v)?;
        Ok(RecipeFields {
            title: p.title.unwrap_or_default(),
            description: p.description.flatten(),
            url: p.url.flatten(),
            image: p.image.flatten(),
            author: p.author.flatten(),
            prep_time: p.prep_time.flatten(),
            cook_time: p.cook_time.flatten(),
            total_time: p.total_time.flatten(),
            freeze_time: p.freeze_time.flatten(),
            recipe_yield: p.recipe_yield.flatten(),
            recipe_category: p.recipe_category.flatten(),
            recipe_cuisine: p.recipe_cuisine.flatten(),
            ingredients: p.ingredients.unwrap_or_default(),
            instructions: p.instructions.unwrap_or_default(),
            nutrition: p.nutrition.flatten(),
            notes: p.notes.flatten(),
        })
    }

    /// Re-applies the schema rules to fields built in code (scraper, parsers, Claude).
    pub fn validate(mut self) -> AppResult<Self> {
        self.title = title(&Value::String(self.title))?;
        let trim = |v: Option<String>| v.map(|s| s.trim().to_string());
        self.description = trim(self.description);
        self.author = trim(self.author);
        self.prep_time = trim(self.prep_time);
        self.cook_time = trim(self.cook_time);
        self.total_time = trim(self.total_time);
        self.freeze_time = trim(self.freeze_time);
        self.recipe_yield = trim(self.recipe_yield);
        self.recipe_category = trim(self.recipe_category);
        self.recipe_cuisine = trim(self.recipe_cuisine);
        self.notes = trim(self.notes);
        for (field, value) in [("url", &self.url), ("image", &self.image)] {
            if let Some(u) = value
                && !is_valid_url(u)
            {
                return Err(AppError::bad_request(format!("{field}: Invalid URL")));
            }
        }
        Ok(self)
    }

    /// The backup/export JSON shape (camelCase keys, same order as the TS export).
    pub fn to_json(&self) -> Map<String, Value> {
        let mut m = Map::new();
        let s = |v: &Option<String>| v.clone().map(Value::String).unwrap_or(Value::Null);
        m.insert("title".into(), Value::String(self.title.clone()));
        m.insert("description".into(), s(&self.description));
        m.insert("url".into(), s(&self.url));
        m.insert("image".into(), s(&self.image));
        m.insert("author".into(), s(&self.author));
        m.insert("prepTime".into(), s(&self.prep_time));
        m.insert("cookTime".into(), s(&self.cook_time));
        m.insert("totalTime".into(), s(&self.total_time));
        m.insert("freezeTime".into(), s(&self.freeze_time));
        m.insert("recipeYield".into(), s(&self.recipe_yield));
        m.insert("recipeCategory".into(), s(&self.recipe_category));
        m.insert("recipeCuisine".into(), s(&self.recipe_cuisine));
        m.insert(
            "ingredients".into(),
            serde_json::to_value(&self.ingredients).unwrap_or_default(),
        );
        m.insert(
            "instructions".into(),
            serde_json::to_value(&self.instructions).unwrap_or_default(),
        );
        m.insert(
            "nutrition".into(),
            self.nutrition
                .clone()
                .map(Value::Object)
                .unwrap_or(Value::Null),
        );
        m.insert("notes".into(), s(&self.notes));
        m
    }
}

/// A saved recipe as the API returns it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recipe {
    pub id: i64,
    pub url: Option<String>,
    pub source: String,
    pub title: String,
    pub description: Option<String>,
    pub image: Option<String>,
    pub author: Option<String>,
    pub prep_time: Option<String>,
    pub cook_time: Option<String>,
    pub total_time: Option<String>,
    pub freeze_time: Option<String>,
    pub recipe_yield: Option<String>,
    pub recipe_category: Option<String>,
    pub recipe_cuisine: Option<String>,
    pub ingredients: Vec<Section>,
    pub instructions: Vec<Section>,
    pub nutrition: Option<Value>,
    pub notes: Option<String>,
    #[serde(serialize_with = "ser_iso")]
    pub created_at: i64,
    #[serde(serialize_with = "ser_iso")]
    pub updated_at: i64,
}

impl Recipe {
    pub fn fields(&self) -> RecipeFields {
        RecipeFields {
            title: self.title.clone(),
            description: self.description.clone(),
            url: self.url.clone(),
            image: self.image.clone(),
            author: self.author.clone(),
            prep_time: self.prep_time.clone(),
            cook_time: self.cook_time.clone(),
            total_time: self.total_time.clone(),
            freeze_time: self.freeze_time.clone(),
            recipe_yield: self.recipe_yield.clone(),
            recipe_category: self.recipe_category.clone(),
            recipe_cuisine: self.recipe_cuisine.clone(),
            ingredients: self.ingredients.clone(),
            instructions: self.instructions.clone(),
            nutrition: self.nutrition.as_ref().and_then(|v| v.as_object().cloned()),
            notes: self.notes.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeSummary {
    pub id: i64,
    pub title: String,
    pub image: Option<String>,
    pub total_time: Option<String>,
    pub recipe_yield: Option<String>,
    pub recipe_category: Option<String>,
    pub recipe_cuisine: Option<String>,
    pub source: String,
    #[serde(serialize_with = "ser_iso")]
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cookbook {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
    #[serde(serialize_with = "ser_iso")]
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CookbookListItem {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
    #[serde(serialize_with = "ser_iso")]
    pub created_at: i64,
    pub recipe_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CookbookWithRecipes {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
    #[serde(serialize_with = "ser_iso")]
    pub created_at: i64,
    pub recipes: Vec<RecipeSummary>,
}

// ─── Cookbook fields ────────────────────────────────────────────────────────

pub fn cookbook_name(v: &Value, required_message: &str) -> AppResult<String> {
    let name = v
        .as_str()
        .ok_or_else(|| AppError::bad_request(format!("name: {required_message}")))?
        .trim();
    if name.is_empty() {
        return Err(AppError::bad_request(format!("name: {required_message}")));
    }
    if name.chars().count() > 100 {
        return Err(AppError::bad_request(
            "name: Name is too long (100 characters max)",
        ));
    }
    Ok(name.to_string())
}

pub fn cookbook_description(v: &Value) -> AppResult<Option<String>> {
    match v {
        Value::Null => Ok(None),
        Value::String(s) if s.trim().chars().count() <= 500 => {
            Ok(Some(s.trim().to_string()).filter(|s| !s.is_empty()))
        }
        Value::String(_) => Err(AppError::bad_request(
            "description: Too long (500 characters max)",
        )),
        _ => Err(AppError::bad_request("description: expected string")),
    }
}

pub fn cookbook_color(v: &Value) -> AppResult<String> {
    v.as_str()
        .filter(|c| BOOK_COLORS.contains(c))
        .map(String::from)
        .ok_or_else(|| {
            AppError::bad_request(format!("color: expected one of {}", BOOK_COLORS.join(", ")))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalizes_legacy_flat_lists() {
        let s = normalize_sections_value(&json!(["  1 egg ", "", "2 cups flour"]));
        assert_eq!(
            s,
            vec![Section::unnamed(vec![
                "1 egg".into(),
                "2 cups flour".into()
            ])]
        );
    }

    #[test]
    fn normalizes_sections_and_drops_empties() {
        let s = normalize_sections_value(&json!([
            {"name": "  Sauce ", "items": [" a ", ""]},
            {"name": "", "items": ["b"]},
            {"name": "Empty", "items": ["  "]},
            {"items": "nope"}
        ]));
        assert_eq!(
            s,
            vec![
                Section {
                    name: Some("Sauce".into()),
                    items: vec!["a".into()]
                },
                Section {
                    name: None,
                    items: vec!["b".into()]
                },
            ]
        );
        assert!(normalize_sections_value(&json!(null)).is_empty());
        assert!(normalize_sections_value(&json!([])).is_empty());
    }

    #[test]
    fn iso_matches_js_date_to_json() {
        assert_eq!(iso(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso(1_700_000_000), "2023-11-14T22:13:20.000Z");
        assert_eq!(iso(1_700_000_000_000), "2023-11-14T22:13:20.000Z");
    }

    #[test]
    fn validates_fields() {
        let f =
            RecipeFields::from_json(&json!({"title": "  Soup ", "url": null, "extra": 1})).unwrap();
        assert_eq!(f.title, "Soup");
        assert!(f.ingredients.is_empty());
        assert!(RecipeFields::from_json(&json!({"title": ""})).is_err());
        assert!(RecipeFields::from_json(&json!({})).is_err());
        assert!(RecipeFields::from_json(&json!({"title": "x", "url": "not a url"})).is_err());
        let p = RecipePatch::from_json(&json!({"notes": null, "cookTime": " 5m "})).unwrap();
        assert_eq!(p.notes, Some(None));
        assert_eq!(p.cook_time, Some(Some("5m".into())));
        assert!(p.title.is_none());
    }
}
