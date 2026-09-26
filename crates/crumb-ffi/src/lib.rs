//! crumb-core for the native apps (Kotlin on Android, Swift on iOS): scaling, mise en place,
//! step timers, step ingredients, durations, text export, photo URLs, server addresses,
//! error wording and the sunrise theme all come from here, so they match the web and
//! desktop apps exactly. Plain functions and records only; recipes cross as the API's JSON.

use crumb_core::{
    categories, checks, client, duration, format, fractions, ingredients, model, source, sun,
};

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CoreError {
    #[error("{0}")]
    BadRecipe(String),
}

fn recipe(json: &str) -> Result<model::Recipe, CoreError> {
    serde_json::from_str(json).map_err(|e| CoreError::BadRecipe(e.to_string()))
}

// ─── Ingredients ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ParsedIngredient {
    pub raw: String,
    pub quantity: Option<f64>,
    pub quantity_max: Option<f64>,
    pub unit: Option<String>,
    pub name: String,
    pub prep: Option<String>,
}

impl From<ingredients::ParsedIngredient> for ParsedIngredient {
    fn from(p: ingredients::ParsedIngredient) -> Self {
        Self {
            raw: p.raw,
            quantity: p.quantity,
            quantity_max: p.quantity_max,
            unit: p.unit,
            name: p.name,
            prep: p.prep,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Vessel {
    Pinch,
    Ramekin,
    Small,
    Medium,
    Large,
    Board,
    Jar,
}

impl From<ingredients::Vessel> for Vessel {
    fn from(v: ingredients::Vessel) -> Self {
        use ingredients::Vessel as V;
        match v {
            V::Pinch => Self::Pinch,
            V::Ramekin => Self::Ramekin,
            V::Small => Self::Small,
            V::Medium => Self::Medium,
            V::Large => Self::Large,
            V::Board => Self::Board,
            V::Jar => Self::Jar,
        }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MiseItem {
    pub raw: String,
    pub quantity: Option<f64>,
    pub quantity_max: Option<f64>,
    pub unit: Option<String>,
    pub name: String,
    pub prep: Option<String>,
    pub vessel: Vessel,
    /// Rough volume in ml, when it can be estimated.
    pub volume: Option<f64>,
    /// A short prep instruction, e.g. "dice".
    pub task: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StepTimer {
    pub label: String,
    pub seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct IngredientLine {
    pub raw: String,
    pub section: Option<String>,
}

#[uniffi::export]
pub fn parse_ingredient(raw: String) -> ParsedIngredient {
    ingredients::parse_ingredient(&raw).into()
}

#[uniffi::export]
pub fn parse_number(value: String) -> Option<f64> {
    ingredients::parse_number(&value)
}

#[uniffi::export]
pub fn format_quantity(n: f64) -> String {
    ingredients::format_quantity(n)
}

/// One ingredient line at `factor` times the quantity ("2 cups" × 1.5 → "3 cups").
#[uniffi::export]
pub fn scale_ingredient(raw: String, factor: f64) -> String {
    ingredients::scale_ingredient(&raw, factor)
}

#[uniffi::export]
pub fn plural_unit(unit: String, quantity: f64) -> String {
    ingredients::plural_unit(&unit, quantity)
}

#[uniffi::export]
pub fn mise_en_place(lines: Vec<String>) -> Vec<MiseItem> {
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    ingredients::mise_en_place(&refs)
        .into_iter()
        .map(|m| MiseItem {
            raw: m.raw,
            quantity: m.quantity,
            quantity_max: m.quantity_max,
            unit: m.unit,
            name: m.name,
            prep: m.prep,
            vessel: m.vessel.into(),
            volume: m.volume,
            task: m.task,
        })
        .collect()
}

/// Durations in a step ("bake 25–30 minutes") for one-tap timers.
#[uniffi::export]
pub fn find_timers(step: String) -> Vec<StepTimer> {
    ingredients::find_timers(&step)
        .into_iter()
        .map(|t| StepTimer {
            label: t.label,
            seconds: t.seconds,
        })
        .collect()
}

/// Indices into `ingredients` of the ones a step uses, preferring the step's own section.
#[uniffi::export]
pub fn ingredients_for_step(
    step: String,
    step_section: Option<String>,
    ingredients: Vec<IngredientLine>,
) -> Vec<u32> {
    let lines: Vec<ingredients::IngredientLine> = ingredients
        .into_iter()
        .map(|l| ingredients::IngredientLine {
            raw: l.raw,
            section: l.section,
        })
        .collect();
    ingredients::ingredients_for_step_in(&step, step_section.as_deref(), &lines)
        .into_iter()
        .filter_map(|hit| lines.iter().position(|l| std::ptr::eq(l, hit)))
        .map(|i| i as u32)
        .collect()
}

#[uniffi::export]
pub fn fractionize(line: String) -> String {
    fractions::fractionize(&line)
}

// ─── Durations, labels, links ──────────────────────────────────────────────

/// Minutes in an ISO 8601 duration ("PT1H30M" → 90), or null if it isn't one.
#[uniffi::export]
pub fn iso_duration_minutes(iso: String) -> Option<f64> {
    duration::iso_duration_minutes(&iso)
}

/// "Breakfast · British".
#[uniffi::export]
pub fn kicker(category: Option<String>, cuisine: Option<String>) -> String {
    format::kicker(category.as_deref(), cuisine.as_deref())
}

#[uniffi::export]
pub fn web_link(url: Option<String>) -> Option<String> {
    format::web_link(url.as_deref())
}

/// "bbcgoodfood.com" for a recipe's source link.
#[uniffi::export]
pub fn host_of(url: Option<String>) -> Option<String> {
    format::host_of(url.as_deref())
}

#[uniffi::export]
pub fn is_share_link(url: String) -> bool {
    source::is_share_link(&url)
}

/// Where a recipe came from, as a link the page may show (its original source first).
#[uniffi::export]
pub fn source_url(recipe_json: String) -> Result<Option<String>, CoreError> {
    Ok(source::source_url(&recipe(&recipe_json)?).map(str::to_string))
}

/// Every category, in display order ("Other" last).
#[uniffi::export]
pub fn categories() -> Vec<String> {
    categories::LIST.iter().map(|c| c.to_string()).collect()
}

/// The cookbook cloth colours, in picker order.
#[uniffi::export]
pub fn book_colors() -> Vec<String> {
    model::BOOK_COLORS.iter().map(|c| c.to_string()).collect()
}

/// A current or legacy colour name as a current one.
#[uniffi::export]
pub fn book_color(name: String) -> Option<String> {
    model::book_color(&name).map(str::to_string)
}

// ─── Text export ───────────────────────────────────────────────────────────

/// Plain text for sharing: title, "•" ingredients, numbered steps, then `link`.
#[uniffi::export]
pub fn recipe_to_text(recipe_json: String, link: Option<String>) -> Result<String, CoreError> {
    Ok(format::recipe_to_text(
        &recipe(&recipe_json)?,
        link.as_deref(),
    ))
}

#[uniffi::export]
pub fn recipe_to_markdown(recipe_json: String) -> Result<String, CoreError> {
    Ok(format::recipe_to_markdown(&recipe(&recipe_json)?))
}

#[uniffi::export]
pub fn book_to_text(name: String, titles: Vec<String>, link: Option<String>) -> String {
    let refs: Vec<&str> = titles.iter().map(String::as_str).collect();
    format::book_to_text(&name, &refs, link.as_deref())
}

#[uniffi::export]
pub fn book_imported_title(
    name: String,
    added: u32,
    duplicates: u32,
    skipped: Option<u32>,
) -> String {
    format::book_imported_title(
        &name,
        added as usize,
        duplicates as usize,
        skipped.map(|n| n as usize),
    )
}

// ─── Client helpers ────────────────────────────────────────────────────────

/// The session cookie's name, `crumb_session`.
#[uniffi::export]
pub fn session_cookie_name() -> String {
    client::SESSION_COOKIE.to_string()
}

/// FNV-1a key of a recipe's image string, the `v` of its photo URLs.
#[uniffi::export]
pub fn image_key(image: String) -> String {
    client::image_key(&image)
}

/// The smallest width the server's resizer makes that is at least `px`.
#[uniffi::export]
pub fn snap_width(px: u32) -> u32 {
    client::snap_width(px)
}

/// `img/{id}/{width}?v={key}` relative to the server's base URL, or null without an image.
#[uniffi::export]
pub fn photo_path(recipe_id: i64, image: Option<String>, px: u32) -> Option<String> {
    client::photo_path(recipe_id, image.as_deref(), px)
}

/// What someone typed as their server, as a base URL ending in "/" (HTTPS by default).
#[uniffi::export]
pub fn server_url(input: String) -> Option<String> {
    client::server_url(&input)
}

/// Whether plain HTTP is fine for this server (the device itself or the local network).
#[uniffi::export]
pub fn allows_cleartext(base_url: String) -> bool {
    client::allows_cleartext(&base_url)
}

/// The session value from a `Set-Cookie` header, unless it clears the cookie.
#[uniffi::export]
pub fn session_cookie(set_cookie: String) -> Option<String> {
    client::session_cookie(&set_cookie)
}

/// The message to show for a failed request, from its status and body.
#[uniffi::export]
pub fn error_message(status: u16, body: String) -> String {
    client::error_message(status, &body)
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ImportInput {
    Link { url: String },
    Text { text: String },
}

/// A paste or share as a link to fetch or the recipe's own text; null when blank.
#[uniffi::export]
pub fn classify_import(raw: String) -> Option<ImportInput> {
    client::classify_import(&raw).map(|input| match input {
        client::ImportInput::Link(url) => ImportInput::Link { url },
        client::ImportInput::Text(text) => ImportInput::Text { text },
    })
}

/// Offline search: every word of `query` in the title, category or cuisine.
#[uniffi::export]
pub fn matches_search(
    query: String,
    title: String,
    category: Option<String>,
    cuisine: Option<String>,
) -> bool {
    client::matches_search(&query, &title, category.as_deref(), cuisine.as_deref())
}

/// A recipe time for display: ISO 8601 as "1h 30m", anything else as written.
#[uniffi::export]
pub fn display_duration(raw: Option<String>) -> Option<String> {
    client::display_duration(raw.as_deref())
}

/// A timer's remaining time, "4:05" or "1:02:03".
#[uniffi::export]
pub fn format_clock(seconds: f64) -> String {
    client::format_clock(seconds)
}

/// "Cooked 3 times · last 2 weeks ago" (times in Unix ms); null when never cooked.
#[uniffi::export]
pub fn cooked_line(count: u32, last_cooked_ms: Option<i64>, now_ms: i64) -> Option<String> {
    client::cooked_line(count, last_cooked_ms, now_ms)
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CookStep {
    pub section: Option<String>,
    pub text: String,
}

/// Cook mode's pages: every non-blank instruction with its section.
#[uniffi::export]
pub fn cook_steps(recipe_json: String) -> Result<Vec<CookStep>, CoreError> {
    Ok(client::cook_steps(&recipe(&recipe_json)?)
        .into_iter()
        .map(|s| CookStep {
            section: s.section,
            text: s.text,
        })
        .collect())
}

// ─── Sunrise & sunset theme ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct SunState {
    pub dark: bool,
    /// Unix ms of the next sunrise or sunset (or a re-check, in polar day or night).
    pub next_change_ms: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct SunLocation {
    pub lat: f64,
    pub lng: f64,
}

/// Whether it's dark at `now_ms` where the cook is, and when that next changes.
#[uniffi::export]
pub fn sun_state(now_ms: f64, location: SunLocation) -> SunState {
    let s = sun::sun_state(now_ms, location.lat, location.lng);
    SunState {
        dark: s.dark,
        next_change_ms: s.next_change_ms,
    }
}

/// A location from the time zone alone (`standard_offset_east_secs`: the smaller of the
/// zone's January and July offsets, in seconds east of UTC).
#[uniffi::export]
pub fn estimate_location(zone_id: String, standard_offset_east_secs: i32) -> SunLocation {
    let (lat, lng) = sun::estimate_location(&zone_id, standard_offset_east_secs);
    SunLocation { lat, lng }
}

// ─── Wee Chef's checks ─────────────────────────────────────────────────────

/// What Wee Chef did to a line on import (from the flag's `detail.fix`, `detail.category`
/// and `detail.was`).
#[uniffi::export]
pub fn fix_text(
    fix: Option<String>,
    item_text: String,
    category: Option<String>,
    was: Option<String>,
) -> String {
    checks::fix_text(
        fix.as_deref(),
        &item_text,
        category.as_deref(),
        was.as_deref(),
    )
}

/// Why a flagged line might need a look, from the flag's `kind`.
#[uniffi::export]
pub fn review_text(kind: String) -> String {
    checks::review_text(&kind).to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct ChecksCounts {
    pub eligible: u32,
    pub checked: u32,
    pub pending: u32,
    pub tidied: u32,
    pub to_check: u32,
    pub due: u32,
    pub restored: u32,
    pub edited: u32,
}

/// One line on where Check all stands; `run` is how many the last Check all queued.
#[uniffi::export]
pub fn checks_status_text(counts: ChecksCounts, run: Option<u32>) -> String {
    checks::checks_status_text(
        checks::ChecksCounts {
            eligible: counts.eligible,
            checked: counts.checked,
            pending: counts.pending,
            tidied: counts.tidied,
            to_check: counts.to_check,
            due: counts.due,
            restored: counts.restored,
            edited: counts.edited,
        },
        run,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_ingredients_come_back_as_indices() {
        let lines = vec![
            IngredientLine {
                raw: "100g plain flour".into(),
                section: None,
            },
            IngredientLine {
                raw: "2 large eggs".into(),
                section: None,
            },
            IngredientLine {
                raw: "300ml milk".into(),
                section: None,
            },
            IngredientLine {
                raw: "caster sugar to serve".into(),
                section: None,
            },
        ];
        let hit = ingredients_for_step("Whisk the flour, eggs and milk".into(), None, lines);
        assert_eq!(hit, vec![0, 1, 2]);
    }

    #[test]
    fn scales_and_finds_timers() {
        assert_eq!(scale_ingredient("2 cups flour".into(), 1.5), "3 cups flour");
        let t = find_timers("Bake for 25–30 minutes".into());
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].seconds, 1800);
    }

    #[test]
    fn client_helpers_cross_the_boundary() {
        assert_eq!(
            classify_import("see https://example.com/pie".into()),
            Some(ImportInput::Link {
                url: "https://example.com/pie".into()
            })
        );
        assert_eq!(
            server_url("crumb.example.com".into()).as_deref(),
            Some("https://crumb.example.com/")
        );
        assert_eq!(
            display_duration(Some("PT90M".into())).as_deref(),
            Some("1h 30m")
        );
        let loc = estimate_location("Europe/London".into(), 0);
        assert_eq!(
            loc,
            SunLocation {
                lat: 50.0,
                lng: 0.0
            }
        );
    }

    #[test]
    fn recipe_json_round_trips() {
        let json = r#"{"id":1,"url":null,"source":"manual","title":"Toast","description":null,
          "image":null,"author":null,"prepTime":null,"cookTime":null,"totalTime":null,
          "freezeTime":null,"recipeYield":null,"recipeCategory":null,"recipeCuisine":null,
          "ingredients":[{"name":null,"items":["1 slice bread"]}],
          "instructions":[{"name":null,"items":["Toast it."]}],"nutrition":null,"notes":null,
          "originalUrl":null,"createdAt":"2026-09-26T16:30:33.000Z","updatedAt":"2026-09-26T16:30:33.000Z"}"#;
        let text = recipe_to_text(json.into(), None).unwrap();
        assert!(text.starts_with("Toast\n\nIngredients\n• 1 slice bread"));
        assert!(recipe_to_text("{}".into(), None).is_err());
    }
}
