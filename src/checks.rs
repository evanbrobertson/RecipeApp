//! Wee Chef's import checks.
//!
//! Two layers, both only for recipes that came from outside (a URL, pasted text, a file
//! or a photo; never ones the cook or Claude wrote), unless the cook asks for one recipe
//! ([`check_one`]). Backup restores are saved as they were and marked `skipped`: nothing
//! runs on the way in, and a later check of one only suggests, apart from the
//! deterministic clean-up below.
//!
//! 1. [`tidy`]: deterministic clean-up before the recipe is saved. No AI, always on.
//!    Anything it drops (a step repeating the one before, or a photo credit or ad line
//!    repeated through a scraped page) is remembered with the lines as they came in, so
//!    the recipe page can offer Undo.
//! 2. [`queue`]: after the save, one background request to TypeSafe's Jev classifier asks
//!    what kind of line every ingredient and step is. For a fresh import that nobody has
//!    edited, confident, structural answers are applied in code ([`plan`]: headings become
//!    section names, split steps are joined, tips move to the notes, junk goes), with the
//!    original kept for Undo. Anything less certain becomes a "review" flag the editor
//!    shows. "Check all recipes" ([`check_all`]) and "Check with Wee Chef" on one recipe
//!    only suggest: on recipes already in the box (restored, edited, or checked before)
//!    every Jev fix becomes a review flag, and only the deterministic clean-up of
//!    [`TidyScope::Saved`] is applied (checkbox glyphs, web codes, float quantities as
//!    fractions, raw ISO times), under the same Undo. Once the cook has undone a Wee
//!    Chef fix on a recipe, no check tidies it again. Undo is offered only while the recipe
//!    still holds exactly what the fix wrote (compared by a content hash); once the cook
//!    changes it, the fix is superseded and no longer claimed. Off without
//!    `TYPESAFE_API_KEY`.

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use regex::Regex;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value, json};
use tokio::sync::Semaphore;

use crate::AppState;
use crate::config::TypesafeConfig;
use crate::error::{AppError, AppResult};
use crate::model::{Recipe, RecipeFields, RecipePatch, Section, now_secs};

/// Checks running at once; a big file import queues instead of bursting.
const CONCURRENCY: usize = 2;
/// Per request. Measured at 0.6 s at most for the largest real recipe.
const TIMEOUT: Duration = Duration::from_secs(20);
/// Questions per request (about 224 tokens each, well inside Jev's 64k context).
const MAX_QUESTIONS: usize = 150;
/// A failed check is retried by "Check all recipes" until it has had this many tries.
const MAX_ATTEMPTS: i64 = 3;

/// An item is flagged when the OK label is below this...
const OK_BELOW: f64 = 0.2;
/// ...and the top label is at least this.
const FLAG_FROM: f64 = 0.6;
/// Only answers this confident are applied; the rest are left for the cook to review.
const FIX_FROM: f64 = 0.9;

/// Recipe sources that came from outside and are worth checking.
pub const CHECKED_SOURCES: [&str; 4] = ["url", "text", "import", "photo"];

/// Why a recipe was queued, which decides whether a check may change it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Just imported: confident fixes are applied (if nobody edited it meanwhile).
    Import,
    /// Queued by "Check all" or "Check with Wee Chef" for a recipe already in the box
    /// (restored, edited or checked before): suggestions only.
    Review,
}

impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Import => "import",
            Self::Review => "review",
        }
    }
}

/// The queue: a semaphore for concurrency and the recipes already queued or running.
pub struct Checks {
    sem: Semaphore,
    queued: Mutex<HashSet<i64>>,
}

impl Default for Checks {
    fn default() -> Self {
        Self {
            sem: Semaphore::new(CONCURRENCY),
            queued: Mutex::new(HashSet::new()),
        }
    }
}

pub fn enabled(state: &AppState) -> bool {
    state.config.typesafe.is_some()
}

// ─── Deterministic clean-up ──────────────────────────────────────────────────

/// Checkbox and bullet glyphs some recipe plugins put in front of every line.
static GLYPHS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:[▢☐□■◻◽◾▪▫☑☒✓✔✅•◦●○∙]\s*|\*\s+)+").unwrap());
static ENTITY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"&(#[0-9]{1,7}|#[xX][0-9a-fA-F]{1,6}|[a-zA-Z][a-zA-Z0-9]{1,9});").unwrap()
});

fn named_entity(name: &str) -> Option<&'static str> {
    Some(match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => " ",
        "ndash" => "–",
        "mdash" => "—",
        "hellip" => "…",
        "lsquo" => "‘",
        "rsquo" => "’",
        "ldquo" => "“",
        "rdquo" => "”",
        "laquo" => "«",
        "raquo" => "»",
        "bull" => "•",
        "middot" => "·",
        "deg" => "°",
        "times" => "×",
        "frasl" => "⁄",
        "frac12" => "½",
        "frac14" => "¼",
        "frac34" => "¾",
        "frac13" => "⅓",
        "frac23" => "⅔",
        "frac18" => "⅛",
        "eacute" => "é",
        "egrave" => "è",
        "ecirc" => "ê",
        "euml" => "ë",
        "aacute" => "á",
        "agrave" => "à",
        "acirc" => "â",
        "auml" => "ä",
        "iacute" => "í",
        "iuml" => "ï",
        "oacute" => "ó",
        "ocirc" => "ô",
        "ouml" => "ö",
        "uacute" => "ú",
        "uuml" => "ü",
        "ntilde" => "ñ",
        "ccedil" => "ç",
        "szlig" => "ß",
        "reg" => "®",
        "trade" => "™",
        "copy" => "©",
        "cent" => "¢",
        "pound" => "£",
        "euro" => "€",
        "ordm" => "º",
        "shy" => "",
        _ => return None,
    })
}

fn decode_once(s: &str) -> String {
    ENTITY
        .replace_all(s, |c: &regex::Captures| {
            let body = &c[1];
            let decoded = if let Some(hex) = body.strip_prefix("#x").or(body.strip_prefix("#X")) {
                u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
            } else if let Some(dec) = body.strip_prefix('#') {
                dec.parse().ok().and_then(char::from_u32)
            } else {
                return named_entity(body).map_or_else(|| c[0].to_string(), String::from);
            };
            match decoded {
                Some('\u{a0}') => " ".into(),
                Some(ch) if ch != '\0' => ch.to_string(),
                _ => c[0].to_string(),
            }
        })
        .into_owned()
}

/// Decodes HTML entities left in scraped text, including double-encoded ones
/// (`&amp;#039;`). Unknown entities are kept as written.
pub fn decode_entities(s: &str) -> String {
    let mut out = s.to_string();
    for _ in 0..3 {
        if !out.contains('&') {
            break;
        }
        let next = decode_once(&out);
        if next == out {
            break;
        }
        out = next;
    }
    out
}

/// A time as written ("1h 10m", "40 mins", "1 hour 30 minutes", "PT1H10M",
/// "P0Y0M0DT0H10M0.000S") in minutes. None for anything else ("Overnight", "Chill 1h")
/// and for an ISO duration under a minute.
pub fn parse_minutes(s: &str) -> Option<i64> {
    static PART: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*(hours?|hrs?|h|minutes?|mins?|m)\b").unwrap()
    });
    let s = s.trim();
    if let Some(minutes) = crate::scraper::iso_duration_minutes(s) {
        return (minutes >= 1.0).then(|| minutes.round() as i64);
    }
    let mut total = 0.0;
    let mut found = false;
    let mut rest = s.to_string();
    for c in PART.captures_iter(s) {
        let n: f64 = c[1].parse().ok()?;
        let hours = c[2].to_ascii_lowercase().starts_with('h');
        total += if hours { n * 60.0 } else { n };
        found = true;
        rest = rest.replacen(&c[0], "", 1);
    }
    // Only numbers and units: "Chill 1h" or "1-2 hours" aren't a plain duration
    let leftover = rest.replace(" and ", " ");
    if !found
        || leftover
            .chars()
            .any(|ch| !(ch.is_whitespace() || ch == ','))
    {
        return None;
    }
    Some(total.round() as i64)
}

pub fn format_minutes(minutes: i64) -> String {
    let (h, m) = (minutes / 60, minutes % 60);
    match (h, m) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

/// Lowercase words only, for comparing steps.
fn step_key(s: &str) -> String {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

/// An ingredient line or step without entities or a leading checkbox glyph.
pub fn clean_line(s: &str) -> String {
    GLYPHS.replace(&decode_entities(s), "").trim().to_string()
}

/// How much [`tidy`] may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TidyScope {
    /// A scraped web page: everything in [`TidyScope::Import`], plus photo credit and ad
    /// lines that turn up again and again through the steps.
    Scrape,
    /// Other outside text (pasted, a file, a photo): checkbox glyphs, HTML entities, a
    /// step repeating the one before, and a missing total time.
    Import,
    /// A recipe already in the box: checkbox glyphs, HTML entities, decimal quantities
    /// that are plainly fractions, and raw ISO times; nothing is dropped or filled in.
    Saved,
}

impl TidyScope {
    /// The scope for a fresh import from `source`.
    pub fn for_source(source: &str) -> Self {
        if source == "url" {
            Self::Scrape
        } else {
            Self::Import
        }
    }
}

/// What [`tidy`] did: how many things changed, and the steps it dropped.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tidied {
    pub changes: usize,
    pub dropped: Vec<String>,
}

/// A short line that reads like page furniture rather than a step: a photo credit, an
/// image caption, a copyright line or an ad.
fn looks_like_credit(step: &str) -> bool {
    static CREDIT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)©|\(c\)|\bcopyright\b|\bphoto(?:graph)?s?\b|\bimages?\s*:|\bpictured?\b|\bcredits?\b|\badvert(?:isement)?s?\b|\bsponsored\b|\bpin it\b|\bpinterest\b|\bjump to\b",
        )
        .unwrap()
    });
    CREDIT.is_match(step)
}

/// Cleans up a recipe in place: strips checkbox glyphs, decodes leftover HTML entities,
/// turns float quantities into fractions ([`crate::fractions`]) and raw ISO times of a
/// minute or more into readable ones (every scope); with [`TidyScope::Import`] or [`TidyScope::Scrape`] it
/// also drops a step repeating the one before and fills in a missing total time from the
/// prep and cook times, and with [`TidyScope::Scrape`] a short photo credit or ad line
/// that turns up three or more times is kept only the first time.
pub fn tidy(fields: &mut RecipeFields, scope: TidyScope) -> Tidied {
    let mut changes = 0;
    let mut text = |s: &mut String, glyphs: bool| {
        let next = if glyphs {
            clean_line(s)
        } else {
            decode_entities(s)
        };
        if next != *s {
            *s = next;
            changes += 1;
        }
    };
    text(&mut fields.title, false);
    for v in [
        &mut fields.description,
        &mut fields.author,
        &mut fields.prep_time,
        &mut fields.cook_time,
        &mut fields.total_time,
        &mut fields.freeze_time,
        &mut fields.recipe_yield,
        &mut fields.recipe_category,
        &mut fields.recipe_cuisine,
        &mut fields.notes,
    ]
    .into_iter()
    .flatten()
    {
        text(v, false);
    }
    for section in fields
        .ingredients
        .iter_mut()
        .chain(fields.instructions.iter_mut())
    {
        if let Some(name) = &mut section.name {
            text(name, true);
        }
        for item in &mut section.items {
            text(item, true);
        }
    }
    // Raw ISO durations some sites publish ("P0Y0M0DT0H10M0.000S") read as "10m". Only
    // what parses as one, so anything the cook typed is left alone, and only a minute or
    // more: "PT30S" or "P0D" would read as no time at all, and a time is never cleared.
    for v in [
        &mut fields.prep_time,
        &mut fields.cook_time,
        &mut fields.total_time,
        &mut fields.freeze_time,
    ] {
        if let Some(t) = v.as_deref()
            && t.trim_start().starts_with(['P', 'p'])
            && crate::scraper::iso_duration_minutes(t).is_some_and(|m| m >= 1.0)
        {
            let next = crate::scraper::format_duration(Some(t));
            if next.as_deref() != Some(t) {
                *v = next;
                changes += 1;
            }
        }
    }
    // Decimal quantities from a unit conversion ("0.33333334 cup") read as fractions
    changes += crate::fractions::fractionize_sections(&mut fields.ingredients);
    let mut out = Tidied {
        changes,
        dropped: Vec::new(),
    };
    if scope == TidyScope::Saved {
        return out;
    }

    // Duplicated steps: a step repeating the one before it, or (on a scraped page) a
    // short unpunctuated credit or ad line that turns up three or more times
    let mut times: std::collections::HashMap<String, usize> = Default::default();
    for step in fields.instructions.iter().flat_map(|s| &s.items) {
        *times.entry(step_key(step)).or_default() += 1;
    }
    let mut seen: HashSet<String> = HashSet::new();
    for section in &mut fields.instructions {
        let mut previous: Option<String> = None;
        let mut kept = Vec::with_capacity(section.items.len());
        for step in std::mem::take(&mut section.items) {
            let key = step_key(&step);
            if key.is_empty() {
                kept.push(step);
                continue;
            }
            let filler = scope == TidyScope::Scrape
                && key.split(' ').count() <= 8
                && !ends_sentence(&step)
                && looks_like_credit(&step)
                && times[&key] >= 3;
            let repeat =
                previous.as_deref() == Some(key.as_str()) || (filler && seen.contains(&key));
            previous = Some(key.clone());
            seen.insert(key);
            if repeat {
                out.dropped.push(step);
            } else {
                kept.push(step);
            }
        }
        section.items = kept;
    }
    out.changes += out.dropped.len();

    // Total = prep + cook (+ extra) when every part is a plain duration
    if fields
        .total_time
        .as_deref()
        .is_none_or(|t| t.trim().is_empty())
        && let (Some(prep), Some(cook)) = (
            fields.prep_time.as_deref().and_then(parse_minutes),
            fields.cook_time.as_deref().and_then(parse_minutes),
        )
    {
        let extra = match fields
            .freeze_time
            .as_deref()
            .filter(|t| !t.trim().is_empty())
        {
            None => Some(0),
            Some(t) => parse_minutes(t),
        };
        if let Some(extra) = extra
            && prep + cook + extra > 0
        {
            fields.total_time = Some(format_minutes(prep + cook + extra));
            out.changes += 1;
        }
    }
    out
}

/// The lists a check or tidy may rewrite, as they were, for Undo.
fn original_of(
    ingredients: &[Section],
    instructions: &[Section],
    notes: &Option<String>,
    [prep, cook, total, freeze]: [&Option<String>; 4],
) -> Value {
    json!({
        "ingredients": ingredients,
        "instructions": instructions,
        "notes": notes,
        "prepTime": prep,
        "cookTime": cook,
        "totalTime": total,
        "freezeTime": freeze,
    })
}

/// The four times, in [`original_of`]'s order.
fn times_of(r: &Recipe) -> [&Option<String>; 4] {
    [&r.prep_time, &r.cook_time, &r.total_time, &r.freeze_time]
}

fn fnv1a(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// FNV-1a over the parts of a recipe a fix writes. Stored with a fix so Undo can tell
/// whether the recipe still holds exactly what the fix wrote. Stable across builds.
/// `2:` marks the version that covers every time; see [`hash_holds`].
pub fn content_hash(r: &Recipe) -> String {
    let text = to_text(&original_of(
        &r.ingredients,
        &r.instructions,
        &r.notes,
        times_of(r),
    ));
    format!("2:{}", fnv1a(&text))
}

/// Whether a stored [`content_hash`] matches the recipe. Hashes from before the `2:`
/// version covered the lists, the notes and the total time only.
fn hash_holds(stored: &str, r: &Recipe) -> bool {
    match stored.strip_prefix("2:") {
        Some(_) => stored == content_hash(r),
        None => {
            let legacy = json!({
                "ingredients": r.ingredients,
                "instructions": r.instructions,
                "notes": r.notes,
                "totalTime": r.total_time,
            });
            stored == fnv1a(&to_text(&legacy))
        }
    }
}

/// What an import's [`tidy`] dropped, and the lists as they came in, for Undo.
pub struct TidyUndo {
    original: Value,
    dropped: Vec<String>,
}

/// [`tidy`] for a fresh import from `source`. When it drops steps, returns what
/// [`remember_tidy`] needs to make that undoable once the recipe is saved.
pub fn tidy_import(fields: &mut RecipeFields, source: &str) -> Option<TidyUndo> {
    let original = original_of(
        &fields.ingredients,
        &fields.instructions,
        &fields.notes,
        [
            &fields.prep_time,
            &fields.cook_time,
            &fields.total_time,
            &fields.freeze_time,
        ],
    );
    let t = tidy(fields, TidyScope::for_source(source));
    (!t.dropped.is_empty()).then_some(TidyUndo {
        original,
        dropped: t.dropped,
    })
}

/// Records the steps an import's tidy dropped as fixes with Undo, for a recipe that was
/// just created. The row is `tidied` until a check runs (or for good without one).
pub fn remember_tidy(conn: &Connection, recipe: &Recipe, undo: TidyUndo) -> AppResult<()> {
    let now = now_secs();
    let added = conn.execute(
        "INSERT INTO recipe_checks (recipe_id, status, queued_at, original, fixed_at, fixed_hash)
         VALUES (?1, 'tidied', ?2, ?3, ?4, ?5) ON CONFLICT(recipe_id) DO NOTHING",
        params![
            recipe.id,
            now,
            to_text(&undo.original),
            recipe.updated_at,
            content_hash(recipe)
        ],
    )?;
    if added == 0 {
        return Ok(());
    }
    for line in undo.dropped {
        let flag = Flag {
            field: "instructions",
            item_text: line,
            kind: "repeat".into(),
            detail: json!({"p": 1.0, "fix": "removed"}),
        };
        insert_flag(conn, recipe.id, &flag, "fixed", now)?;
    }
    Ok(())
}

/// A backup restore: saved as it was, not checked on the way in. "Check all" (or the
/// cook) checks it later, and then only suggests.
pub fn mark_restored(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute(
        "INSERT INTO recipe_checks (recipe_id, status, queued_at) VALUES (?1, 'skipped', ?2)
         ON CONFLICT(recipe_id) DO NOTHING",
        params![id, now_secs()],
    )?;
    Ok(())
}

// ─── The questions ───────────────────────────────────────────────────────────

const ING_QUESTION: &str = "This `line` was extracted from the ingredient list of the recipe in the state. What kind of line is it?";
const STEP_QUESTION: &str =
    "`step` was extracted as one item of the recipe's method. What kind of item is `step`?";
const WAIT_QUESTION: &str = "Do the recipe's steps include unattended waiting outside prep and cooking, such as marinating, chilling, rising, soaking, resting, cooling or overnight setting?";

fn ing_criteria() -> Value {
    json!({
        "ingredient": "One food item or product the cook needs, with or without a quantity, unit, preparation or a short aside (for example \"2 onions, diced\", \"salt to taste\", \"8 cups broth (recipe link below)\").",
        "heading": "A label naming a group of the ingredients that follow, such as \"For the sauce:\" or \"Filling\". It names no single food to buy.",
        "step": "A cooking action or method sentence (preheat, whisk, bake, rest) rather than something to buy or measure.",
        "merged": "Two or more separate ingredients, each with its own quantity, run together on one line (\"1 cup sugar 2 eggs\"). A pair the cook normally measures together, such as \"salt and pepper\", is not merged.",
        "junk": "Website text, a nutrition label, a rating, an ad or navigation text; nothing a cook uses.",
    })
}

fn step_criteria() -> Value {
    json!({
        "step": "A complete instruction the cook follows, of any length. It may be long and contain several actions.",
        "heading": "A short title for the steps that follow (\"Chashu pork belly\", \"For the sauce\", \"Day before:\"), not an action.",
        "fragment": "An incomplete piece of a sentence: it starts mid-sentence (for example with a lowercase continuation like \"add flour mixture and stir\" that finishes the sentence in `previous_step`) or it stops mid-sentence and continues in `next_step`.",
        "ingredient": "An ingredient or list of ingredients with quantities, not an action.",
        "not_instruction": "A tip, story, storage note or serving suggestion rather than a cooking step.",
        "junk": "Website text such as a photo credit, rating, ad, link or navigation; nothing a cook uses.",
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Ingredients,
    Instructions,
}

impl Field {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ingredients => "ingredients",
            Self::Instructions => "instructions",
        }
    }

    fn ok_label(self) -> &'static str {
        match self {
            Self::Ingredients => "ingredient",
            Self::Instructions => "step",
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Self::Ingredients => "ing",
            Self::Instructions => "step",
        }
    }
}

/// One ingredient line or step, by position.
#[derive(Debug, Clone, PartialEq)]
struct Item {
    field: Field,
    section: usize,
    index: usize,
    text: String,
}

fn items(field: Field, sections: &[Section]) -> Vec<Item> {
    sections
        .iter()
        .enumerate()
        .flat_map(|(si, s)| {
            s.items.iter().enumerate().map(move |(ii, text)| Item {
                field,
                section: si,
                index: ii,
                text: text.clone(),
            })
        })
        .collect()
}

/// The recipe only: irrelevant state (URL, image, nutrition) lowers accuracy.
fn jev_state(r: &Recipe) -> Value {
    let sections = |list: &[Section]| -> Value {
        list.iter()
            .map(|s| json!({"section": s.name, "items": s.items}))
            .collect()
    };
    json!({
        "title": r.title,
        "description": r.description,
        "prepTime": r.prep_time,
        "cookTime": r.cook_time,
        "totalTime": r.total_time,
        "servings": r.recipe_yield,
        "ingredients": sections(&r.ingredients),
        "steps": sections(&r.instructions),
        "notes": r.notes,
    })
}

fn questions(r: &Recipe) -> Map<String, Value> {
    let mut q = Map::new();
    for (i, it) in items(Field::Ingredients, &r.ingredients).iter().enumerate() {
        q.insert(
            format!("ing_{i}"),
            json!({"type": "choice",
                "instructions": {"line": it.text, "question": ING_QUESTION},
                "criteria": ing_criteria()}),
        );
    }
    let steps = items(Field::Instructions, &r.instructions);
    for (i, it) in steps.iter().enumerate() {
        let near = |j: Option<usize>| j.and_then(|j| steps.get(j)).map(|s| s.text.clone());
        q.insert(
            format!("step_{i}"),
            json!({"type": "choice",
                "instructions": {
                    "previous_step": near(i.checked_sub(1)),
                    "step": it.text,
                    "next_step": near(Some(i + 1)),
                    "question": STEP_QUESTION,
                },
                "criteria": step_criteria()}),
        );
    }
    if !steps.is_empty() {
        q.insert(
            "has_wait".into(),
            json!({"type": "noul", "instructions": WAIT_QUESTION}),
        );
    }
    q
}

// ─── Reading the answers ─────────────────────────────────────────────────────

/// A line Jev thinks is not what it should be.
#[derive(Debug, Clone, PartialEq)]
pub struct Judgment {
    item: Item,
    label: String,
    p: f64,
    p_ok: f64,
}

/// Applies the flag rule to the raw answers: the top label isn't the OK one, the OK
/// label is below [`OK_BELOW`] and the top label is at least [`FLAG_FROM`].
fn judge(answers: &Map<String, Value>, r: &Recipe) -> Vec<Judgment> {
    let mut out = Vec::new();
    for field in [Field::Ingredients, Field::Instructions] {
        let list = match field {
            Field::Ingredients => &r.ingredients,
            Field::Instructions => &r.instructions,
        };
        for (i, item) in items(field, list).into_iter().enumerate() {
            let Some(a) = answers.get(&format!("{}_{i}", field.prefix())) else {
                continue;
            };
            let probs = a.get("probabilities").and_then(Value::as_object);
            let prob = |label: &str| {
                probs
                    .and_then(|p| p.get(label))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0)
            };
            let Some(label) = a.get("choice").and_then(Value::as_str) else {
                continue;
            };
            let (p, p_ok) = (prob(label), prob(field.ok_label()));
            if label != field.ok_label() && p_ok < OK_BELOW && p >= FLAG_FROM {
                out.push(Judgment {
                    item,
                    label: label.to_string(),
                    p,
                    p_ok,
                });
            }
        }
    }
    out
}

// ─── Planning the fixes ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Action {
    Keep,
    Remove,
    Heading(String),
    ToNotes,
    JoinPrev,
}

/// A flag to store: what Jev saw, and what was done about it.
#[derive(Debug, Clone, PartialEq)]
pub struct Flag {
    pub field: &'static str,
    pub item_text: String,
    pub kind: String,
    pub detail: Value,
}

/// The recipe after the confident fixes, plus what was fixed and what's left to review.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Plan {
    pub ingredients: Vec<Section>,
    pub instructions: Vec<Section>,
    pub notes: Option<String>,
    pub fixed: Vec<Flag>,
    pub review: Vec<Flag>,
}

/// "choux pastry ingredients" → "Choux pastry", "Sauce:" → "Sauce".
pub fn heading_name(line: &str) -> String {
    let mut s = line
        .trim()
        .trim_end_matches([':', '.', ' '])
        .trim()
        .to_string();
    let lower = s.to_lowercase();
    for suffix in [" ingredients", " ingredient"] {
        if lower.ends_with(suffix) && s.len() > suffix.len() {
            s.truncate(s.len() - suffix.len());
            break;
        }
    }
    let mut chars = s.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => line.trim().to_string(),
    }
}

fn ends_sentence(s: &str) -> bool {
    s.trim_end()
        .ends_with(['.', '!', '?', ')', '"', '”', ':', ';', '…'])
}

fn starts_lowercase(s: &str) -> bool {
    s.trim_start()
        .chars()
        .next()
        .is_some_and(char::is_lowercase)
}

fn rebuild(
    sections: &[Section],
    flat: &[Item],
    actions: &[Action],
    notes: &mut Vec<String>,
) -> Vec<Section> {
    let mut out = Vec::new();
    let mut k = 0;
    for s in sections {
        let mut cur = Section {
            name: s.name.clone(),
            items: Vec::new(),
        };
        for text in &s.items {
            debug_assert_eq!(&flat[k].text, text);
            match &actions[k] {
                Action::Keep => cur.items.push(text.clone()),
                Action::Remove => {}
                Action::ToNotes => notes.push(text.clone()),
                Action::Heading(name) => {
                    if !cur.items.is_empty() {
                        out.push(cur);
                    }
                    cur = Section {
                        name: Some(name.clone()),
                        items: Vec::new(),
                    };
                }
                Action::JoinPrev => match cur.items.last_mut() {
                    Some(last) => *last = format!("{} {}", last.trim_end(), text.trim_start()),
                    None => cur.items.push(text.clone()),
                },
            }
            k += 1;
        }
        if !cur.items.is_empty() {
            out.push(cur);
        }
    }
    out
}

fn plan_list(
    field: Field,
    sections: &[Section],
    judgments: &[&Judgment],
    fixed: &mut Vec<Flag>,
    review: &mut Vec<Flag>,
    notes: &mut Vec<String>,
) -> Vec<Section> {
    let flat = items(field, sections);
    let mut actions = vec![Action::Keep; flat.len()];
    let pos = |j: &Judgment| {
        flat.iter()
            .position(|it| it.section == j.item.section && it.index == j.item.index)
    };
    let judged: HashSet<usize> = judgments.iter().filter_map(|j| pos(j)).collect();
    let headings: HashSet<usize> = judgments
        .iter()
        .filter(|j| j.label == "heading")
        .filter_map(|j| pos(j))
        .collect();
    let same_section =
        |a: usize, b: usize| flat.get(b).is_some_and(|x| x.section == flat[a].section);

    for j in judgments {
        let Some(k) = pos(j) else { continue };
        let text = &flat[k].text;
        let flag = |detail: Value| Flag {
            field: field.as_str(),
            item_text: text.clone(),
            kind: j.label.clone(),
            detail,
        };
        let confident = j.p >= FIX_FROM;
        let mut fix: Option<(Action, Value)> = None;
        if confident {
            match j.label.as_str() {
                "heading" => {
                    let has_next = same_section(k, k + 1) && !headings.contains(&(k + 1));
                    let named_first =
                        flat[k].index == 0 && sections[flat[k].section].name.is_some();
                    if has_next && !named_first {
                        let name = heading_name(text);
                        fix = Some((
                            Action::Heading(name.clone()),
                            json!({"p": j.p, "fix": "heading", "heading": name}),
                        ));
                    }
                }
                "junk" => fix = Some((Action::Remove, json!({"p": j.p, "fix": "removed"}))),
                "not_instruction" if field == Field::Instructions => {
                    fix = Some((Action::ToNotes, json!({"p": j.p, "fix": "notes"})))
                }
                "fragment" if field == Field::Instructions => {
                    let prev_ok = k > 0
                        && same_section(k, k - 1)
                        && matches!(actions[k - 1], Action::Keep | Action::JoinPrev);
                    let starts_mid =
                        starts_lowercase(text) || (k > 0 && !ends_sentence(&flat[k - 1].text));
                    let next_ok = same_section(k, k + 1) && !judged.contains(&(k + 1));
                    if starts_mid && prev_ok {
                        fix = Some((
                            Action::JoinPrev,
                            json!({"p": j.p, "fix": "joined", "with": flat[k - 1].text}),
                        ));
                    } else if !ends_sentence(text) && next_ok && starts_lowercase(&flat[k + 1].text)
                    {
                        actions[k + 1] = Action::JoinPrev;
                        fixed.push(flag(
                            json!({"p": j.p, "fix": "joined", "with": flat[k + 1].text}),
                        ));
                        continue;
                    }
                }
                _ => {}
            }
        }
        match fix {
            Some((action, detail)) => {
                actions[k] = action;
                fixed.push(flag(detail));
            }
            None => review.push(flag(json!({"p": j.p}))),
        }
    }
    rebuild(sections, &flat, &actions, notes)
}

/// Works out the fixes for `judgments` on the recipe as it was checked.
fn plan(r: &Recipe, judgments: &[Judgment]) -> Plan {
    let mut p = Plan::default();
    let mut moved = Vec::new();
    let of = |f: Field| {
        judgments
            .iter()
            .filter(|j| j.item.field == f)
            .collect::<Vec<_>>()
    };
    p.ingredients = plan_list(
        Field::Ingredients,
        &r.ingredients,
        &of(Field::Ingredients),
        &mut p.fixed,
        &mut p.review,
        &mut moved,
    );
    p.instructions = plan_list(
        Field::Instructions,
        &r.instructions,
        &of(Field::Instructions),
        &mut p.fixed,
        &mut p.review,
        &mut moved,
    );
    p.notes = r.notes.clone();
    if !moved.is_empty() {
        let tips = moved.join("\n");
        p.notes = Some(match r.notes.as_deref().map(str::trim) {
            Some(n) if !n.is_empty() => format!("{n}\n\n{tips}"),
            _ => tips,
        });
    }
    p
}

// ─── Calling Jev ─────────────────────────────────────────────────────────────

struct Reply {
    answers: Map<String, Value>,
    input_tokens: i64,
    model: String,
}

async fn ask_jev(
    state: &AppState,
    ts: &TypesafeConfig,
    jev_state: &Value,
    questions: Map<String, Value>,
) -> Result<Reply, String> {
    let body = json!({"model": ts.model, "state": jev_state, "questions": questions});
    let url = format!("{}/v1/systemone", ts.base_url);
    let mut attempt = 0;
    loop {
        let res = state
            .http
            .post(&url)
            .bearer_auth(&ts.api_key)
            .timeout(TIMEOUT)
            .json(&body)
            .send()
            .await;
        let retry = match res {
            Ok(res) if res.status().is_success() => {
                let v: Value = res.json().await.map_err(|e| format!("bad reply: {e}"))?;
                let answers = v
                    .get("answers")
                    .and_then(Value::as_object)
                    .cloned()
                    .ok_or("reply has no answers")?;
                return Ok(Reply {
                    answers,
                    input_tokens: v["usage"]["input_tokens"].as_i64().unwrap_or(0),
                    model: v["model"].as_str().unwrap_or(&ts.model).to_string(),
                });
            }
            Ok(res) => {
                let status = res.status().as_u16();
                let text = res.text().await.unwrap_or_default();
                let err = format!("HTTP {status}: {}", crate::telemetry::api_error_text(&text));
                if !(status == 429 || status >= 500) {
                    return Err(err);
                }
                err
            }
            Err(e) => format!("request failed: {e}"),
        };
        if attempt >= 2 {
            return Err(retry);
        }
        attempt += 1;
        tokio::time::sleep(Duration::from_millis(1000 * 3u64.pow(attempt - 1))).await;
    }
}

/// Every question for `r`, in requests of at most [`MAX_QUESTIONS`], answers merged.
async fn check_recipe(state: &AppState, ts: &TypesafeConfig, r: &Recipe) -> Result<Reply, String> {
    let all = questions(r);
    let jev_state = jev_state(r);
    let mut merged = Reply {
        answers: Map::new(),
        input_tokens: 0,
        model: ts.model.clone(),
    };
    let entries: Vec<(String, Value)> = all.into_iter().collect();
    for chunk in entries.chunks(MAX_QUESTIONS) {
        let reply = ask_jev(state, ts, &jev_state, chunk.iter().cloned().collect()).await?;
        merged.answers.extend(reply.answers);
        merged.input_tokens += reply.input_tokens;
        merged.model = reply.model;
    }
    Ok(merged)
}

// ─── Queue and run ───────────────────────────────────────────────────────────

/// Marks `ids` as waiting for a check and starts them in the background (at most
/// [`CONCURRENCY`] at once). `mode` decides whether the check may change them. Does
/// nothing when the checks aren't configured.
pub fn queue(state: &AppState, conn: &Connection, ids: &[i64], mode: Mode) {
    if !enabled(state) {
        return;
    }
    let now = now_secs();
    for &id in ids {
        if !state
            .checks
            .queued
            .lock()
            .map(|mut q| q.insert(id))
            .unwrap_or(false)
        {
            continue;
        }
        if let Err(err) = conn.execute(
            "INSERT INTO recipe_checks (recipe_id, status, queued_at, mode) VALUES (?1, 'pending', ?2, ?3)
             ON CONFLICT(recipe_id) DO UPDATE SET status = 'pending', queued_at = ?2, mode = ?3,
               error = NULL, seen_at = NULL",
            params![id, now, mode.as_str()],
        ) {
            tracing::warn!("[checks] couldn't queue recipe {id}: {err}");
            forget(state, id);
            continue;
        }
        let state = state.clone();
        tokio::spawn(async move {
            run(&state, id).await;
            forget(&state, id);
        });
    }
}

fn forget(state: &AppState, id: i64) {
    if let Ok(mut q) = state.checks.queued.lock() {
        q.remove(&id);
    }
}

async fn run(state: &AppState, id: i64) {
    let Some(ts) = state.config.typesafe.clone() else {
        return;
    };
    let Ok(_permit) = state.checks.sem.acquire().await else {
        return;
    };
    let (snapshot, mode) = {
        let conn = state.db.lock();
        let _ = conn.execute(
            "UPDATE recipe_checks SET attempts = attempts + 1 WHERE recipe_id = ?1",
            [id],
        );
        let mode = conn
            .query_row(
                "SELECT mode FROM recipe_checks WHERE recipe_id = ?1",
                [id],
                |r| r.get::<_, Option<String>>(0),
            )
            .ok()
            .flatten();
        // Anything not queued by an import is only ever suggested to
        let mode = if mode.as_deref() == Some("import") {
            Mode::Import
        } else {
            Mode::Review
        };
        match crate::recipes::get_recipe(&conn, id) {
            Ok(Some(r)) => (r, mode),
            _ => return,
        }
    };
    let started = std::time::Instant::now();
    let reply = if snapshot.ingredients.is_empty() && snapshot.instructions.is_empty() {
        Ok(Reply {
            answers: Map::new(),
            input_tokens: 0,
            model: ts.model.clone(),
        })
    } else {
        check_recipe(state, &ts, &snapshot).await
    };
    let result = match reply {
        Ok(reply) => apply(&state.db.lock(), &snapshot, reply, mode).map(|(fixed, review)| {
            tracing::info!(
                "[checks] recipe {id}: {fixed} tidied, {review} to review ({:.2}s)",
                started.elapsed().as_secs_f64()
            );
        }),
        Err(err) => {
            tracing::warn!("[checks] recipe {id} failed: {err}");
            let conn = state.db.lock();
            conn.execute(
                "UPDATE recipe_checks SET status = 'failed', error = ?2, checked_at = ?3,
                   seen_at = CASE WHEN seen_at = 0 THEN 0 ELSE ?4 END
                 WHERE recipe_id = ?1",
                params![id, err, now_secs(), snapshot.updated_at],
            )
            .map(|_| ())
            .map_err(AppError::from)
        }
    };
    if let Err(err) = result {
        tracing::warn!("[checks] recipe {id}: couldn't save the check: {err}");
    }
}

fn to_text<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

fn insert_flag(conn: &Connection, id: i64, f: &Flag, state: &str, now: i64) -> AppResult<()> {
    conn.execute(
        "INSERT INTO recipe_flags (recipe_id, field, item_text, kind, state, detail, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            f.field,
            f.item_text,
            f.kind,
            state,
            to_text(&f.detail),
            now
        ],
    )?;
    Ok(())
}

/// Stores a finished check and records every flag. The confident fixes are applied
/// only to a fresh import ([`Mode::Import`]) that nobody has edited, before or during
/// the check; on anything else they become review flags, and only the deterministic
/// clean-up of [`TidyScope::Saved`] is applied (undoable): glyphs, entities, float
/// quantities as fractions and raw ISO times. No clean-up at all once the cook has undone
/// a fix on the recipe. Returns (fixed, to review).
fn apply(
    conn: &Connection,
    snapshot: &Recipe,
    reply: Reply,
    mode: Mode,
) -> AppResult<(usize, usize)> {
    let id = snapshot.id;
    let tx = conn.unchecked_transaction()?;
    let Some(current) = crate::recipes::get_recipe(&tx, id)? else {
        return Ok((0, 0)); // deleted meanwhile
    };
    // Edited while the check ran (see [`note_edit`]), even within the same second
    let edited_meanwhile = tx
        .query_row(
            "SELECT seen_at FROM recipe_checks WHERE recipe_id = ?1",
            [id],
            |r| r.get::<_, Option<i64>>(0),
        )
        .optional()?
        .flatten()
        == Some(0);
    // Only what a fix writes counts: a new photo or title meanwhile doesn't stop it
    let unchanged = current.ingredients == snapshot.ingredients
        && current.instructions == snapshot.instructions
        && current.notes == snapshot.notes
        && times_of(&current) == times_of(snapshot);
    // Fresh: queued by its import and never edited since (restores and older recipes
    // come through "Check all", edits bump updated_at past created_at)
    let fresh = mode == Mode::Import && snapshot.updated_at <= snapshot.created_at;
    // The cook undid a fix (this deploy's tidy or any earlier one: Undo has always marked
    // every fix it put back `undone`), so the recipe is theirs as it is: not tidied again
    let undone: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM recipe_flags WHERE recipe_id = ?1 AND state = 'undone')",
        [id],
        |r| r.get(0),
    )?;
    let now = now_secs();

    // "Keep as is" is remembered for the same line and kind
    let mut stmt = tx.prepare(
        "SELECT field, item_text, kind FROM recipe_flags WHERE recipe_id = ?1 AND state = 'dismissed'",
    )?;
    let dismissed: HashSet<(String, String, String)> = stmt
        .query_map([id], |r| {
            Ok((
                r.get(0)?,
                r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                r.get(2)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    drop(stmt);
    let judgments: Vec<Judgment> = judge(&reply.answers, snapshot)
        .into_iter()
        .filter(|j| {
            // Flags are stored with the line as tidied, so match either form
            let field = j.item.field.as_str().to_string();
            ![j.item.text.clone(), clean_line(&j.item.text)]
                .into_iter()
                .any(|text| dismissed.contains(&(field.clone(), text, j.label.clone())))
        })
        .collect();
    let mut plan = plan(snapshot, &judgments);
    if !(unchanged && fresh) {
        // Edited, restored or already in the box: suggest, don't touch
        let fixed = std::mem::take(&mut plan.fixed);
        plan.review.extend(fixed.into_iter().map(|mut f| {
            f.detail = json!({"p": f.detail["p"]});
            f
        }));
        plan.ingredients = snapshot.ingredients.clone();
        plan.instructions = snapshot.instructions.clone();
        plan.notes = snapshot.notes.clone();
    }

    tx.execute(
        "DELETE FROM recipe_flags WHERE recipe_id = ?1 AND state = 'review'",
        [id],
    )?;
    // The deterministic clean-up once more, as one more fix under the same Undo: the
    // import's full tidy on a fresh import (a join can repeat a step), and on anything
    // else only glyphs, entities, float quantities and raw ISO times. Skipped once the
    // cook has undone a fix. The other times are decoded first so only what's written
    // back (the lists, notes, total time and any ISO time turned readable) is counted.
    let mut total_time = snapshot.total_time.clone();
    // Prep, cook and extra time, when the tidy made an ISO one readable
    let mut other_times: [Option<Option<String>>; 3] = Default::default();
    if unchanged && !undone {
        let scope = if fresh {
            TidyScope::for_source(&snapshot.source)
        } else {
            TidyScope::Saved
        };
        let decoded = |t: &Option<String>| t.as_deref().map(decode_entities);
        let mut f = RecipeFields {
            prep_time: decoded(&snapshot.prep_time),
            cook_time: decoded(&snapshot.cook_time),
            total_time: snapshot.total_time.clone(),
            freeze_time: decoded(&snapshot.freeze_time),
            ingredients: std::mem::take(&mut plan.ingredients),
            instructions: std::mem::take(&mut plan.instructions),
            notes: plan.notes.take(),
            ..Default::default()
        };
        let tidied = tidy(&mut f, scope);
        (plan.ingredients, plan.instructions, plan.notes) =
            (f.ingredients, f.instructions, f.notes);
        let written = f.total_time != snapshot.total_time;
        if written {
            total_time = f.total_time;
        }
        for (slot, (next, was)) in other_times.iter_mut().zip([
            (f.prep_time, &snapshot.prep_time),
            (f.cook_time, &snapshot.cook_time),
            (f.freeze_time, &snapshot.freeze_time),
        ]) {
            if next != decoded(was) {
                *slot = Some(next);
            }
        }
        let count = tidied.changes;
        if count > 0 {
            for flag in plan.fixed.iter_mut().chain(plan.review.iter_mut()) {
                flag.item_text = clean_line(&flag.item_text);
            }
            plan.fixed.push(Flag {
                field: "recipe",
                item_text: String::new(),
                kind: "tidy".into(),
                detail: json!({"p": 1.0, "fix": "tidy", "count": count}),
            });
        }
    }

    // An earlier fix (the import's tidy) that the recipe still holds stays undoable
    // together with this one; one the cook has since changed is superseded
    let earlier: Option<(Option<String>, Option<String>)> = tx
        .query_row(
            "SELECT original, fixed_hash FROM recipe_checks WHERE recipe_id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (earlier_original, earlier_hash) = earlier.unwrap_or_default();
    let earlier_holds =
        earlier_original.is_some() && earlier_hash.is_some_and(|h| hash_holds(&h, &current));
    if !earlier_holds {
        tx.execute(
            "UPDATE recipe_flags SET state = 'superseded', resolved_at = ?2
             WHERE recipe_id = ?1 AND state = 'fixed'",
            params![id, now],
        )?;
        tx.execute(
            "UPDATE recipe_checks SET original = NULL, fixed_at = NULL, fixed_hash = NULL
             WHERE recipe_id = ?1",
            [id],
        )?;
    }

    let mut original: Option<String> = None;
    let mut fixed_at: Option<i64> = None;
    let mut fixed_hash: Option<String> = None;
    let final_recipe = if plan.fixed.is_empty() {
        current
    } else {
        let before = original_of(
            &snapshot.ingredients,
            &snapshot.instructions,
            &snapshot.notes,
            times_of(snapshot),
        );
        original = Some(match earlier_original.filter(|_| earlier_holds) {
            // An older original may lack some times: they're as the snapshot has them
            Some(o) => match serde_json::from_str::<Value>(&o) {
                Ok(Value::Object(mut m)) => {
                    for (k, v) in before.as_object().into_iter().flatten() {
                        m.entry(k.clone()).or_insert_with(|| v.clone());
                    }
                    to_text(&m)
                }
                _ => o,
            },
            None => to_text(&before),
        });
        let notes_changed = plan.notes != snapshot.notes;
        let patch = RecipePatch {
            ingredients: Some(std::mem::take(&mut plan.ingredients)),
            instructions: Some(std::mem::take(&mut plan.instructions)),
            notes: notes_changed.then(|| plan.notes.take()),
            total_time: (total_time != snapshot.total_time).then_some(total_time),
            prep_time: other_times[0].take(),
            cook_time: other_times[1].take(),
            freeze_time: other_times[2].take(),
            ..Default::default()
        };
        let updated = crate::recipes::update_recipe(&tx, id, patch)?;
        fixed_at = Some(updated.updated_at);
        fixed_hash = Some(content_hash(&updated));
        for f in &plan.fixed {
            insert_flag(&tx, id, f, "fixed", now)?;
        }
        updated
    };
    for f in &plan.review {
        insert_flag(&tx, id, f, "review", now)?;
    }
    // A flag on a line the fixes (or the tidy) took out is already done with
    resolve_missing(&tx, &final_recipe)?;
    // The recipe as this check leaves it; 0 when the cook edited it meanwhile, so that
    // edit is checked next time
    let seen_at = if unchanged && !edited_meanwhile {
        final_recipe.updated_at
    } else {
        0
    };
    tx.execute(
        "UPDATE recipe_checks SET status = 'done', model = ?2, answers = ?3,
           original = coalesce(?4, original), fixed_at = coalesce(?5, fixed_at),
           fixed_hash = coalesce(?6, fixed_hash),
           input_tokens = ?7, error = NULL, checked_at = ?8, seen_at = ?9
         WHERE recipe_id = ?1",
        params![
            id,
            reply.model,
            to_text(&reply.answers),
            original,
            fixed_at,
            fixed_hash,
            reply.input_tokens,
            now,
            seen_at
        ],
    )?;
    let review: usize = tx.query_row(
        "SELECT count(*) FROM recipe_flags WHERE recipe_id = ?1 AND state = 'review'",
        [id],
        |r| r.get::<_, i64>(0),
    )? as usize;
    tx.commit()?;
    Ok((plan.fixed.len(), review))
}

// ─── Reading, undoing, dismissing ────────────────────────────────────────────

/// Marks review flags resolved once their line is gone from the recipe (edited,
/// removed, or turned into a section heading). Called after every recipe update.
pub fn resolve_missing(conn: &Connection, recipe: &Recipe) -> AppResult<()> {
    let mut stmt = conn.prepare(
        "SELECT id, field, item_text FROM recipe_flags WHERE recipe_id = ?1 AND state = 'review'",
    )?;
    let open: Vec<(i64, String, Option<String>)> = stmt
        .query_map([recipe.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let now = now_secs();
    for (flag, field, text) in open {
        let list = match field.as_str() {
            "ingredients" => &recipe.ingredients,
            "instructions" => &recipe.instructions,
            _ => continue,
        };
        let text = text.unwrap_or_default();
        if !list.iter().any(|s| s.items.contains(&text)) {
            conn.execute(
                "UPDATE recipe_flags SET state = 'resolved', resolved_at = ?2 WHERE id = ?1",
                params![flag, now],
            )?;
        }
    }
    Ok(())
}

/// Called on every recipe update, with the recipe before and after it. Only an update
/// that changes what a check reads and fixes (ingredients, steps, notes, times) counts as
/// an edit: a recipe edited while its import check waits is no longer a fresh import (the
/// check only suggests), and whatever a check saw is out of date (`seen_at = 0`), so
/// "Check all" checks it again; a check or Undo that writes the recipe sets `seen_at`
/// again afterwards. (Timestamps alone miss an edit in the same second as the import or
/// the check.) Any other update (a photo, the title, a refresh that brings the same
/// lists) keeps a current check current, so it doesn't cost another check.
pub fn note_edit(conn: &Connection, before: &Recipe, after: &Recipe) -> AppResult<()> {
    let id = after.id;
    let edited = before.ingredients != after.ingredients
        || before.instructions != after.instructions
        || before.notes != after.notes
        || times_of(before) != times_of(after);
    if !edited {
        // Current before (as [`EDITED_SINCE`] reads it): still current now
        conn.execute(
            "UPDATE recipe_checks SET seen_at = ?2
             WHERE recipe_id = ?1 AND seen_at IS NOT 0
               AND ?3 <= coalesce(seen_at, max(coalesce(checked_at, 0), coalesce(fixed_at, 0)))",
            params![id, after.updated_at, before.updated_at],
        )?;
        return Ok(());
    }
    conn.execute(
        "UPDATE recipe_checks SET mode = 'review'
         WHERE recipe_id = ?1 AND status = 'pending' AND mode = 'import'",
        [id],
    )?;
    conn.execute(
        "UPDATE recipe_checks SET seen_at = 0 WHERE recipe_id = ?1",
        [id],
    )?;
    Ok(())
}

/// Whether the recipe still holds exactly what the last fix wrote (so Undo is safe and
/// the fixes are still in it). False when there's nothing to undo.
fn fix_holds(conn: &Connection, recipe: &Recipe) -> AppResult<bool> {
    let row: Option<(Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT original, fixed_hash FROM recipe_checks WHERE recipe_id = ?1",
            [recipe.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(match row {
        Some((Some(_), Some(hash))) => hash_holds(&hash, recipe),
        _ => false,
    })
}

/// The check for the recipe page and editor: null when it was never checked.
/// `{status, canUndo, flags: [{id, field, itemText, kind, state, detail}]}`. Fixes the
/// cook has since changed (the recipe no longer holds what the fix wrote) are
/// superseded: not listed, and not undoable.
pub fn for_recipe(conn: &Connection, id: i64) -> AppResult<Value> {
    let status: Option<String> = conn
        .query_row(
            "SELECT status FROM recipe_checks WHERE recipe_id = ?1",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    let (Some(status), Some(recipe)) = (status, crate::recipes::get_recipe(conn, id)?) else {
        return Ok(Value::Null);
    };
    let holds = fix_holds(conn, &recipe)?;
    let mut stmt = conn.prepare(
        "SELECT id, field, item_text, kind, state, detail FROM recipe_flags
         WHERE recipe_id = ?1 AND state IN ('fixed', 'review') ORDER BY id",
    )?;
    let flags: Vec<Value> = stmt
        .query_map([id], |r| {
            let detail: Option<String> = r.get(5)?;
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "field": r.get::<_, String>(1)?,
                "itemText": r.get::<_, Option<String>>(2)?,
                "kind": r.get::<_, String>(3)?,
                "state": r.get::<_, String>(4)?,
                "detail": detail.and_then(|d| serde_json::from_str::<Value>(&d).ok()).unwrap_or(Value::Null),
            }))
        })?
        .collect::<rusqlite::Result<Vec<Value>>>()?
        .into_iter()
        .filter(|f| holds || f["state"] != "fixed")
        .collect();
    let any_fixed = flags.iter().any(|f| f["state"] == "fixed");
    Ok(json!({"status": status, "canUndo": holds && any_fixed, "flags": flags}))
}

/// Puts back the ingredients, steps and notes as they were imported, if the recipe
/// still holds exactly what Wee Chef wrote.
pub fn undo(conn: &Connection, id: i64) -> AppResult<Value> {
    let current = crate::recipes::require_recipe(conn, id)?;
    let original: Option<String> = conn
        .query_row(
            "SELECT original FROM recipe_checks WHERE recipe_id = ?1",
            [id],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    let Some(original) = original else {
        return Err(AppError::new(409, "Wee Chef hasn't changed this recipe"));
    };
    if !fix_holds(conn, &current)? {
        return Err(AppError::new(
            409,
            "This recipe was edited after Wee Chef tidied it, so undoing would lose those edits",
        ));
    }
    let original: Value = serde_json::from_str(&original).map_err(AppError::internal)?;
    let sections = |key: &str| crate::model::normalize_sections_value(&original[key]);
    // Older originals have no prep, cook or extra time: those are left as they are
    let time = |key: &str| original.get(key).map(|t| t.as_str().map(String::from));
    let tx = conn.unchecked_transaction()?;
    let now = now_secs();
    tx.execute(
        "UPDATE recipe_flags SET state = 'undone', resolved_at = ?2 WHERE recipe_id = ?1 AND state = 'fixed'",
        params![id, now],
    )?;
    let restored = crate::recipes::update_recipe(
        &tx,
        id,
        RecipePatch {
            ingredients: Some(sections("ingredients")),
            instructions: Some(sections("instructions")),
            notes: Some(original["notes"].as_str().map(String::from)),
            total_time: time("totalTime"),
            prep_time: time("prepTime"),
            cook_time: time("cookTime"),
            freeze_time: time("freezeTime"),
            ..Default::default()
        },
    )?;
    // Undo isn't an edit of the cook's: "Check all" doesn't pick the recipe up again for it
    tx.execute(
        "UPDATE recipe_checks SET original = NULL, fixed_at = NULL, fixed_hash = NULL, seen_at = ?2
         WHERE recipe_id = ?1",
        params![id, restored.updated_at],
    )?;
    tx.commit()?;
    for_recipe(conn, id)
}

/// "Keep as is": the flag is dismissed and stays dismissed on later checks.
pub fn dismiss(conn: &Connection, id: i64, flag: i64) -> AppResult<Value> {
    let changed = conn.execute(
        "UPDATE recipe_flags SET state = 'dismissed', resolved_at = ?3
         WHERE id = ?1 AND recipe_id = ?2 AND state = 'review'",
        params![flag, id, now_secs()],
    )?;
    if changed == 0 {
        return Err(AppError::not_found(
            "Nothing to keep: that line isn't flagged",
        ));
    }
    for_recipe(conn, id)
}

/// How many recipes have suggestions waiting: decides whether the nav shows Suggestions.
pub fn review_count(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT count(DISTINCT recipe_id) FROM recipe_flags WHERE state = 'review'",
        [],
        |r| r.get(0),
    )?)
}

/// The recipes with suggestions waiting, most recent first, each with how many there are
/// per field (`{"instructions": 2, "ingredients": 1}`): the Suggestions page.
pub fn to_review(conn: &Connection) -> AppResult<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.title, r.image, f.field, count(*), max(f.created_at)
         FROM recipe_flags f JOIN recipes r ON r.id = f.recipe_id
         WHERE f.state = 'review'
         GROUP BY r.id, f.field",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, i64>(5)?,
        ))
    })?;
    let mut out: Vec<(i64, Value)> = Vec::new();
    for row in rows {
        let (id, title, image, field, n, at) = row?;
        match out.iter_mut().find(|(_, v)| v["id"] == id) {
            Some((latest, v)) => {
                *latest = (*latest).max(at);
                v["fields"][&field] = json!(n);
                v["count"] = json!(v["count"].as_i64().unwrap_or(0) + n);
            }
            None => out.push((
                at,
                json!({"id": id, "title": title, "image": image, "count": n, "fields": {field: n}}),
            )),
        }
    }
    out.sort_by_key(|r| std::cmp::Reverse(r.0));
    Ok(out.into_iter().map(|(_, v)| v).collect())
}

/// A recipe the cook (or Claude) changed after its last check finished. `seen_at` is the
/// recipe's `updated_at` as the check (or an Undo) left it, and 0 once it's edited after
/// ([`note_edit`]); rows from before the column fall back to when the check finished or
/// its fix was written.
const EDITED_SINCE: &str =
    "r.updated_at > coalesce(c.seen_at, max(coalesce(c.checked_at, 0), coalesce(c.fixed_at, 0)))";

/// Why "Check all" would queue a recipe now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Due {
    /// Never checked (or only tidied on import), failed with tries left, or stuck waiting.
    Unchecked,
    /// Restored from a backup and never checked since.
    Restored,
    /// Edited after its last check.
    Edited,
}

/// The recipes "Check all" queues, and why (see [`check_all`]).
fn due(conn: &Connection) -> AppResult<Vec<(i64, Due)>> {
    let marks = CHECKED_SOURCES.map(|s| format!("'{s}'")).join(", ");
    let mut stmt = conn.prepare(&format!(
        "SELECT r.id, CASE
           WHEN c.status = 'skipped' THEN 1
           WHEN c.status IN ('done', 'failed') AND {EDITED_SINCE} THEN 2
           ELSE 0 END
         FROM recipes r LEFT JOIN recipe_checks c ON c.recipe_id = r.id
         WHERE r.source IN ({marks}) AND (c.recipe_id IS NULL
           OR c.status IN ('tidied', 'skipped')
           OR (c.status = 'failed' AND c.attempts < ?1)
           OR (c.status = 'pending' AND c.queued_at < ?2)
           OR (c.status IN ('done', 'failed') AND {EDITED_SINCE}))
         ORDER BY r.id"
    ))?;
    let rows = stmt
        .query_map(params![MAX_ATTEMPTS, now_secs() - 600], |r| {
            Ok((
                r.get(0)?,
                match r.get::<_, i64>(1)? {
                    1 => Due::Restored,
                    2 => Due::Edited,
                    _ => Due::Unchecked,
                },
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

fn to_check_all(conn: &Connection) -> AppResult<Vec<i64>> {
    Ok(due(conn)?.into_iter().map(|(id, _)| id).collect())
}

/// Progress for the More page: `{enabled, eligible, checked, pending, failed, tidied,
/// toCheck, due, restored, edited}`. `eligible` is every recipe from outside; `checked`
/// the ones whose last check is done and still current; `due` how many "Check all" would
/// queue now, `restored` and `edited` of which are backup restores and recipes edited
/// since their check. `toCheck` is how many recipes have suggestions waiting.
pub fn status(state: &AppState, conn: &Connection) -> AppResult<Value> {
    let count = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0));
    let marks = CHECKED_SOURCES.map(|s| format!("'{s}'")).join(", ");
    let due = due(conn)?;
    let of = |why: Due| due.iter().filter(|d| d.1 == why).count();
    Ok(json!({
        "enabled": enabled(state),
        "eligible": count(&format!("SELECT count(*) FROM recipes r WHERE r.source IN ({marks})"))?,
        "checked": count(&format!(
            "SELECT count(*) FROM recipes r JOIN recipe_checks c ON c.recipe_id = r.id
             WHERE r.source IN ({marks}) AND c.status = 'done' AND NOT ({EDITED_SINCE})"
        ))?,
        "pending": count("SELECT count(*) FROM recipe_checks WHERE status = 'pending'")?,
        "failed": count("SELECT count(*) FROM recipe_checks WHERE status = 'failed'")?,
        "tidied": count("SELECT count(*) FROM recipe_flags WHERE state = 'fixed'")?,
        "toCheck": count("SELECT count(DISTINCT recipe_id) FROM recipe_flags WHERE state = 'review'")?,
        "due": due.len(),
        "restored": of(Due::Restored),
        "edited": of(Due::Edited),
    }))
}

/// "Check all recipes": queues recipes from outside that were never checked (including
/// ones only tidied on import while the checks were off), backup restores, ones edited
/// since their last check, failed ones with tries left, and ones stuck waiting from
/// before a restart. All in [`Mode::Review`]: they're already in the box, so Wee Chef
/// only suggests (and "Keep as is" answers stay kept).
pub fn check_all(state: &AppState) -> AppResult<Value> {
    if !enabled(state) {
        return Err(not_set_up());
    }
    let conn = state.db.lock();
    let ids = to_check_all(&conn)?;
    queue(state, &conn, &ids, Mode::Review);
    let mut out = status(state, &conn)?;
    out["queued"] = json!(ids.len());
    Ok(out)
}

fn not_set_up() -> AppError {
    AppError::new(409, "Wee Chef checks aren't set up on this server")
}

/// "Check with Wee Chef" on one recipe: checks it again now, whatever came before, and
/// returns its check (pending) for the recipe page. Only suggests ([`Mode::Review`]),
/// unless it's a fresh, unedited import from outside whose own check never ran (a
/// `tidied` row, or a `pending` one its import queued), which may be fixed as its import
/// check would have. A recipe with no check row at all (saved before Wee Chef, or an old
/// restore) is already in the box: suggestions only, as "Check all" would. Already
/// waiting or running: left be.
pub fn check_one(state: &AppState, id: i64) -> AppResult<Value> {
    if !enabled(state) {
        return Err(not_set_up());
    }
    let conn = state.db.lock();
    let recipe = crate::recipes::require_recipe(&conn, id)?;
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT status, mode FROM recipe_checks WHERE recipe_id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let never_ran = row.as_ref().is_some_and(|(status, mode)| {
        status == "tidied" || (status == "pending" && mode.as_deref() == Some("import"))
    });
    let fresh = never_ran
        && CHECKED_SOURCES.contains(&recipe.source.as_str())
        && recipe.updated_at <= recipe.created_at;
    let mode = if fresh { Mode::Import } else { Mode::Review };
    // Asked for by the cook: a failure is retried by "Check all" again
    conn.execute(
        "UPDATE recipe_checks SET attempts = 0 WHERE recipe_id = ?1",
        [id],
    )?;
    queue(state, &conn, &[id], mode);
    for_recipe(&conn, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(name: Option<&str>, items: &[&str]) -> Section {
        Section {
            name: name.map(String::from),
            items: items.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn fields() -> RecipeFields {
        RecipeFields {
            title: "Pork &amp;amp; Beans".into(),
            ..Default::default()
        }
    }

    #[test]
    fn decodes_entities_including_double_encoded() {
        assert_eq!(decode_entities("Don&amp;#039;t stir"), "Don't stir");
        assert_eq!(
            decode_entities("Don&#39;t &amp; won&#x27;t"),
            "Don't & won't"
        );
        assert_eq!(decode_entities("&frac12; cup&nbsp;milk"), "½ cup milk");
        assert_eq!(
            decode_entities("salt &unknown; pepper"),
            "salt &unknown; pepper"
        );
        assert_eq!(decode_entities("fish & chips"), "fish & chips");
        assert_eq!(decode_entities("&#0; ok"), "&#0; ok");
    }

    #[test]
    fn parses_written_times() {
        assert_eq!(parse_minutes("1h 10m"), Some(70));
        assert_eq!(parse_minutes("40m"), Some(40));
        assert_eq!(parse_minutes("1 hour 30 minutes"), Some(90));
        assert_eq!(parse_minutes("45 mins"), Some(45));
        assert_eq!(parse_minutes("1.5 hours"), Some(90));
        assert_eq!(parse_minutes("2 hrs, 5 min"), Some(125));
        assert_eq!(parse_minutes("PT1H10M"), Some(70));
        assert_eq!(parse_minutes("Overnight"), None);
        assert_eq!(parse_minutes("Chill 1h"), None);
        assert_eq!(parse_minutes("1-2 hours"), None);
        assert_eq!(parse_minutes(""), None);
        assert_eq!(format_minutes(70), "1h 10m");
        assert_eq!(format_minutes(120), "2h");
        assert_eq!(format_minutes(5), "5m");
    }

    #[test]
    fn tidy_cleans_an_import() {
        let mut f = fields();
        f.notes = Some("Line one\nIt&#039;s good".into());
        f.ingredients = vec![section(
            Some("For the &quot;sauce&quot;"),
            &[
                "▢ 1/3 cup flour",
                "☐ ½ tsp salt",
                "* 2 eggs",
                "✓1 cup milk",
                "2 * 3 inch pieces",
            ],
        )];
        f.instructions = vec![
            section(
                None,
                &[
                    "Mix.",
                    "Photo credit: Studio",
                    "Bake until golden, about 25 minutes.",
                    "Bake until golden, about 25 minutes!",
                    "Photo credit: Studio",
                    "Cool.",
                ],
            ),
            section(
                Some("Glaze"),
                &["Stir it.", "Photo credit: Studio", "Cool."],
            ),
        ];
        f.prep_time = Some("40m".into());
        f.cook_time = Some("1 hour".into());
        let n = tidy(&mut f, TidyScope::Scrape);
        assert_eq!(
            n.dropped,
            [
                "Bake until golden, about 25 minutes!",
                "Photo credit: Studio",
                "Photo credit: Studio"
            ]
        );
        let n = n.changes;
        assert_eq!(f.title, "Pork & Beans");
        assert_eq!(f.notes.as_deref(), Some("Line one\nIt's good"));
        assert_eq!(f.ingredients[0].name.as_deref(), Some("For the \"sauce\""));
        assert_eq!(
            f.ingredients[0].items,
            [
                "1/3 cup flour",
                "½ tsp salt",
                "2 eggs",
                "1 cup milk",
                "2 * 3 inch pieces"
            ]
        );
        // The repeated long step is only dropped when it follows itself; a short repeated
        // line (a photo credit) goes everywhere after its first time
        assert_eq!(
            f.instructions[0].items,
            [
                "Mix.",
                "Photo credit: Studio",
                "Bake until golden, about 25 minutes.",
                "Cool."
            ]
        );
        assert_eq!(f.instructions[1].items, ["Stir it.", "Cool."]);
        assert_eq!(f.total_time.as_deref(), Some("1h 40m"));
        // title, notes, section name, 4 ingredients, 3 steps, total time
        assert_eq!(n, 11);
        // Running it again changes nothing
        assert_eq!(tidy(&mut f, TidyScope::Scrape).changes, 0);
    }

    #[test]
    fn tidy_only_drops_repeated_credits_from_scraped_pages() {
        let steps = [
            "Stir well",
            "Add the stock.",
            "Stir well",
            "Add the rice.",
            "Stir well",
            "Photo: Studio",
            "Serve.",
            "Photo: Studio",
            "Photo: Studio",
        ];
        // A short imperative repeated through the method is a real step, even scraped
        let mut f = fields();
        f.instructions = vec![section(None, &steps)];
        let t = tidy(&mut f, TidyScope::Scrape);
        assert_eq!(t.dropped, ["Photo: Studio", "Photo: Studio"]);
        assert_eq!(f.instructions[0].items.len(), 7);
        // Pasted text or a file keeps them all, bar a line repeating the one before
        let mut f = fields();
        f.instructions = vec![section(None, &steps)];
        assert_eq!(tidy(&mut f, TidyScope::Import).dropped, ["Photo: Studio"]);
        assert_eq!(f.instructions[0].items, steps[..8]);
        // A saved recipe only loses glyphs and web codes
        let mut f = fields();
        f.prep_time = Some("5m".into());
        f.cook_time = Some("5m".into());
        f.instructions = vec![section(None, &["▢ Mix.", "▢ Mix.", "It&#039;s done."])];
        let t = tidy(&mut f, TidyScope::Saved);
        assert_eq!(f.instructions[0].items, ["Mix.", "Mix.", "It's done."]);
        assert_eq!(f.total_time, None);
        assert!(t.dropped.is_empty());
        assert_eq!(t.changes, 4); // title and three steps
    }

    #[test]
    fn tidy_leaves_times_it_cant_add_up() {
        let mut f = fields();
        f.prep_time = Some("20m".into());
        f.cook_time = Some("Overnight".into());
        tidy(&mut f, TidyScope::Import);
        assert_eq!(f.total_time, None);
        f.cook_time = Some("10m".into());
        f.freeze_time = Some("Chill until set".into());
        tidy(&mut f, TidyScope::Import);
        assert_eq!(f.total_time, None);
        f.freeze_time = Some("2h".into());
        tidy(&mut f, TidyScope::Import);
        assert_eq!(f.total_time.as_deref(), Some("2h 30m"));
        let mut kept = fields();
        kept.prep_time = Some("5m".into());
        kept.cook_time = Some("5m".into());
        kept.total_time = Some("1h".into());
        tidy(&mut kept, TidyScope::Import);
        assert_eq!(kept.total_time.as_deref(), Some("1h"));
    }

    #[test]
    fn saved_tidy_turns_decimal_quantities_into_fractions() {
        let mut f = RecipeFields {
            ingredients: vec![section(None, &["0.33333334 cup sugar", "2 eggs"])],
            ..Default::default()
        };
        let t = tidy(&mut f, TidyScope::Saved);
        assert_eq!(
            f.ingredients,
            vec![section(None, &["⅓ cup sugar", "2 eggs"])]
        );
        assert_eq!(t.changes, 1);
    }

    #[test]
    fn tidy_makes_raw_iso_times_readable_and_leaves_typed_ones() {
        let mut f = RecipeFields {
            prep_time: Some("P0Y0M0DT0H10M0.000S".into()),
            cook_time: Some("PT1H30M".into()),
            total_time: Some("Plenty".into()),
            freeze_time: Some("Overnight".into()),
            ..Default::default()
        };
        let t = tidy(&mut f, TidyScope::Saved);
        assert_eq!(f.prep_time.as_deref(), Some("10m"));
        assert_eq!(f.cook_time.as_deref(), Some("1h 30m"));
        assert_eq!(f.total_time.as_deref(), Some("Plenty"));
        assert_eq!(f.freeze_time.as_deref(), Some("Overnight"));
        assert_eq!(t.changes, 2);
        assert_eq!(parse_minutes("P0Y0M0DT0H10M0.000S"), Some(10));
        assert_eq!(parse_minutes("PT20S"), None);

        // Under a minute would read as no time: kept as written, never cleared
        let mut short = RecipeFields {
            prep_time: Some("PT30S".into()),
            cook_time: Some("P0D".into()),
            ..Default::default()
        };
        for scope in [TidyScope::Saved, TidyScope::Import, TidyScope::Scrape] {
            assert_eq!(tidy(&mut short, scope).changes, 0);
            assert_eq!(short.prep_time.as_deref(), Some("PT30S"));
            assert_eq!(short.cook_time.as_deref(), Some("P0D"));
        }
    }

    #[test]
    fn a_check_repairs_stored_iso_times_under_undo() {
        let db = crate::db::open_in_memory().unwrap();
        let conn = db.lock();
        let fields = RecipeFields {
            title: "Dip".into(),
            prep_time: Some("P0Y0M0DT0H10M0.000S".into()),
            cook_time: Some("P0Y0M0DT0H20M0.000S".into()),
            ingredients: vec![section(None, &["1 cup yogurt"])],
            instructions: vec![section(None, &["Stir."])],
            ..Default::default()
        };
        let (r, _) = crate::recipes::create_recipe(&conn, fields, "url").unwrap();
        queued(&conn, r.id, Mode::Review);
        assert_eq!(
            apply(&conn, &r, reply(&r, &[]), Mode::Review).unwrap(),
            (1, 0)
        );
        let now = crate::recipes::require_recipe(&conn, r.id).unwrap();
        assert_eq!(now.prep_time.as_deref(), Some("10m"));
        assert_eq!(now.cook_time.as_deref(), Some("20m"));
        // Review mode on a saved recipe doesn't make up a total time
        assert_eq!(now.total_time, None);
        let c = for_recipe(&conn, r.id).unwrap();
        assert_eq!(c["flags"][0]["detail"]["count"], 2);
        assert_eq!(c["canUndo"], true);
        undo(&conn, r.id).unwrap();
        let back = crate::recipes::require_recipe(&conn, r.id).unwrap();
        assert_eq!(back.prep_time, r.prep_time);
        assert_eq!(back.cook_time, r.cook_time);

        // Undone: the next check leaves the times as the cook put them back
        queued(&conn, r.id, Mode::Review);
        assert_eq!(
            apply(&conn, &back, reply(&back, &[]), Mode::Review).unwrap(),
            (0, 0)
        );
        let after = crate::recipes::require_recipe(&conn, r.id).unwrap();
        assert_eq!(after.prep_time, r.prep_time);
        assert_eq!(after.cook_time, r.cook_time);
    }

    #[test]
    fn a_recipe_undone_before_this_deploy_is_not_tidied_again() {
        let db = crate::db::open_in_memory().unwrap();
        let conn = db.lock();
        let fields = RecipeFields {
            title: "Cake".into(),
            prep_time: Some("PT15M".into()),
            ingredients: vec![section(None, &["0.33333334 cup sugar", "▢ 2 eggs"])],
            instructions: vec![section(None, &["Bake."])],
            ..Default::default()
        };
        let (r, _) = crate::recipes::create_recipe(&conn, fields, "url").unwrap();
        // As an earlier build's Undo left it: the check done, its fixes marked undone
        conn.execute(
            "INSERT INTO recipe_checks (recipe_id, status, queued_at, checked_at, seen_at)
             VALUES (?1, 'done', 0, ?2, ?2)",
            params![r.id, r.updated_at],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO recipe_flags (recipe_id, field, item_text, kind, state, detail, created_at)
             VALUES (?1, 'instructions', 'Bake.', 'repeat', 'undone', '{}', 0)",
            [r.id],
        )
        .unwrap();
        queued(&conn, r.id, Mode::Review);
        assert_eq!(
            apply(&conn, &r, reply(&r, &[]), Mode::Review).unwrap(),
            (0, 0)
        );
        let after = crate::recipes::require_recipe(&conn, r.id).unwrap();
        assert_eq!(after.ingredients, r.ingredients);
        assert_eq!(after.prep_time.as_deref(), Some("PT15M"));

        // Without the undone row the same check tidies it
        conn.execute("DELETE FROM recipe_flags WHERE recipe_id = ?1", [r.id])
            .unwrap();
        queued(&conn, r.id, Mode::Review);
        assert_eq!(
            apply(&conn, &r, reply(&r, &[]), Mode::Review).unwrap(),
            (1, 0)
        );
        let tidied = crate::recipes::require_recipe(&conn, r.id).unwrap();
        assert_eq!(
            tidied.ingredients,
            vec![section(None, &["⅓ cup sugar", "2 eggs"])]
        );
        assert_eq!(tidied.prep_time.as_deref(), Some("15m"));
    }

    #[test]
    fn only_edits_to_what_a_check_reads_make_it_due_again() {
        let db = crate::db::open_in_memory().unwrap();
        let conn = db.lock();
        let fields = RecipeFields {
            title: "Soup".into(),
            ingredients: vec![section(None, &["1 onion"])],
            instructions: vec![section(None, &["Simmer."])],
            ..Default::default()
        };
        let (r, _) = crate::recipes::create_recipe(&conn, fields, "url").unwrap();
        queued(&conn, r.id, Mode::Review);
        apply(&conn, &r, reply(&r, &[]), Mode::Review).unwrap();
        assert!(due(&conn).unwrap().is_empty());

        // A photo, a title, or a refresh bringing the same lists: still checked
        for patch in [
            RecipePatch {
                image: Some(Some("https://example.com/soup.jpg".into())),
                ..Default::default()
            },
            RecipePatch {
                title: Some("Onion soup".into()),
                ..Default::default()
            },
            RecipePatch {
                ingredients: Some(r.ingredients.clone()),
                instructions: Some(r.instructions.clone()),
                notes: Some(None),
                ..Default::default()
            },
        ] {
            crate::recipes::update_recipe(&conn, r.id, patch).unwrap();
            assert!(due(&conn).unwrap().is_empty());
        }

        // A time or a step: due again
        crate::recipes::update_recipe(
            &conn,
            r.id,
            RecipePatch {
                cook_time: Some(Some("30m".into())),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(due(&conn).unwrap(), vec![(r.id, Due::Edited)]);
    }

    #[test]
    fn hashes_from_before_the_times_still_hold() {
        let r = recipe(vec![section(None, &["1 egg"])], vec![], Some("n"));
        let legacy = json!({
            "ingredients": r.ingredients,
            "instructions": r.instructions,
            "notes": r.notes,
            "totalTime": r.total_time,
        });
        assert!(hash_holds(&fnv1a(&to_text(&legacy)), &r));
        assert!(hash_holds(&content_hash(&r), &r));
        let mut other = r.clone();
        other.prep_time = Some("5m".into());
        assert!(!hash_holds(&content_hash(&r), &other));
    }

    #[test]
    fn heading_names_are_tidy() {
        assert_eq!(heading_name("special sauce:"), "Special sauce");
        assert_eq!(heading_name("choux pastry ingredients"), "Choux pastry");
        assert_eq!(heading_name("For the sauce"), "For the sauce");
        assert_eq!(heading_name("Ingredients:"), "Ingredients");
    }

    fn recipe(
        ingredients: Vec<Section>,
        instructions: Vec<Section>,
        notes: Option<&str>,
    ) -> Recipe {
        Recipe {
            id: 1,
            url: None,
            source: "url".into(),
            title: "Test".into(),
            description: None,
            image: None,
            author: None,
            prep_time: None,
            cook_time: None,
            total_time: None,
            freeze_time: None,
            recipe_yield: None,
            recipe_category: None,
            recipe_cuisine: None,
            ingredients,
            instructions,
            nutrition: None,
            notes: notes.map(String::from),
            original_url: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn answer(ok: &str, label: &str, p: f64) -> Value {
        let mut probs = Map::new();
        probs.insert(
            ok.into(),
            json!(if label == ok { p } else { (1.0 - p).min(0.1) }),
        );
        if label != ok {
            probs.insert(label.into(), json!(p));
        }
        json!({"type": "choice", "choice": label, "confidence": p, "probabilities": probs})
    }

    /// Answers for a recipe: every line OK except the (key, label, p) listed.
    fn answers(r: &Recipe, odd: &[(&str, &str, f64)]) -> Map<String, Value> {
        let mut out = Map::new();
        for (i, _) in items(Field::Ingredients, &r.ingredients).iter().enumerate() {
            out.insert(format!("ing_{i}"), answer("ingredient", "ingredient", 1.0));
        }
        for (i, _) in items(Field::Instructions, &r.instructions)
            .iter()
            .enumerate()
        {
            out.insert(format!("step_{i}"), answer("step", "step", 1.0));
        }
        for (key, label, p) in odd {
            let ok = if key.starts_with("ing") {
                "ingredient"
            } else {
                "step"
            };
            out.insert(key.to_string(), answer(ok, label, *p));
        }
        out
    }

    #[test]
    fn judges_only_clear_answers() {
        let r = recipe(vec![section(None, &["a", "b", "c", "d"])], vec![], None);
        let mut a = answers(&r, &[("ing_0", "heading", 0.95), ("ing_1", "junk", 0.55)]);
        // A close call: the OK label is still likely
        a.insert(
            "ing_2".into(),
            json!({"choice": "merged", "probabilities": {"merged": 0.62, "ingredient": 0.35}}),
        );
        let j = judge(&a, &r);
        assert_eq!(j.len(), 1);
        assert_eq!(j[0].label, "heading");
        assert_eq!(j[0].item.text, "a");
    }

    #[test]
    fn plans_structural_fixes() {
        let r = recipe(
            vec![section(
                None,
                &[
                    "special sauce:",
                    "1 cup mayo",
                    "Nutrition Facts",
                    "burger:",
                    "1 lb beef",
                    "1 cup sugar 2 eggs",
                ],
            )],
            vec![
                section(
                    None,
                    &[
                        "Sauce:",
                        "Whisk the mayo and",
                        "relish together.",
                        "Keeps for a week in the fridge.",
                        "Burgers:",
                        "Grill the patties",
                        "Serve.",
                    ],
                ),
                section(Some("Glaze"), &["Heat it.", "Cool it"]),
            ],
            Some("Mine."),
        );
        let a = answers(
            &r,
            &[
                ("ing_0", "heading", 1.0),
                ("ing_2", "junk", 0.99),
                ("ing_3", "heading", 1.0),
                ("ing_5", "merged", 1.0),
                ("step_0", "heading", 1.0),
                ("step_2", "fragment", 0.95),
                ("step_3", "not_instruction", 0.97),
                ("step_4", "heading", 0.98),
                ("step_5", "fragment", 0.92), // ends mid-sentence, but "Serve." is a new one
                ("step_8", "not_instruction", 0.7),
            ],
        );
        let p = plan(&r, &judge(&a, &r));
        assert_eq!(
            p.ingredients,
            vec![
                section(Some("Special sauce"), &["1 cup mayo"]),
                section(Some("Burger"), &["1 lb beef", "1 cup sugar 2 eggs"]),
            ]
        );
        assert_eq!(
            p.instructions,
            vec![
                section(Some("Sauce"), &["Whisk the mayo and relish together."]),
                section(Some("Burgers"), &["Grill the patties", "Serve."]),
                section(Some("Glaze"), &["Heat it.", "Cool it"]),
            ]
        );
        assert_eq!(
            p.notes.as_deref(),
            Some("Mine.\n\nKeeps for a week in the fridge.")
        );
        let kinds = |v: &[Flag]| {
            v.iter()
                .map(|f| (f.item_text.clone(), f.kind.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            kinds(&p.fixed),
            [
                ("special sauce:", "heading"),
                ("Nutrition Facts", "junk"),
                ("burger:", "heading"),
                ("Sauce:", "heading"),
                ("relish together.", "fragment"),
                ("Keeps for a week in the fridge.", "not_instruction"),
                ("Burgers:", "heading"),
            ]
            .map(|(a, b)| (a.to_string(), b.to_string()))
        );
        assert_eq!(
            kinds(&p.review),
            [
                ("1 cup sugar 2 eggs", "merged"),
                ("Grill the patties", "fragment"),
                ("Cool it", "not_instruction"),
            ]
            .map(|(a, b)| (a.to_string(), b.to_string()))
        );
    }

    #[test]
    fn joins_a_step_that_stops_mid_sentence() {
        let r = recipe(
            vec![],
            vec![section(
                None,
                &["Beat the eggs,", "then fold in flour.", "Bake."],
            )],
            None,
        );
        let a = answers(&r, &[("step_0", "fragment", 0.93)]);
        let p = plan(&r, &judge(&a, &r));
        assert_eq!(
            p.instructions,
            vec![section(
                None,
                &["Beat the eggs, then fold in flour.", "Bake."]
            )]
        );
        assert_eq!(p.fixed.len(), 1);
    }

    #[test]
    fn leaves_unsafe_headings_for_review() {
        // Last line, two headings in a row, and the first line of an already named section
        let r = recipe(
            vec![
                section(None, &["Filling", "Topping", "1 cup cream", "Extras"]),
                section(Some("Base"), &["Crust", "1 cup flour"]),
            ],
            vec![],
            None,
        );
        let a = answers(
            &r,
            &[
                ("ing_0", "heading", 1.0),
                ("ing_1", "heading", 1.0),
                ("ing_3", "heading", 1.0),
                ("ing_4", "heading", 1.0),
            ],
        );
        let p = plan(&r, &judge(&a, &r));
        assert_eq!(p.fixed.len(), 1);
        assert_eq!(p.fixed[0].item_text, "Topping");
        assert_eq!(p.review.len(), 3);
        assert_eq!(
            p.ingredients,
            vec![
                section(None, &["Filling"]),
                section(Some("Topping"), &["1 cup cream", "Extras"]),
                section(Some("Base"), &["Crust", "1 cup flour"]),
            ]
        );
    }

    #[test]
    fn asks_about_every_line_in_one_request() {
        let r = recipe(
            vec![section(None, &["1 egg", "2 cups flour"])],
            vec![section(None, &["Mix.", "Bake."])],
            None,
        );
        let q = questions(&r);
        assert_eq!(q.len(), 5);
        assert_eq!(q["ing_1"]["instructions"]["line"], "2 cups flour");
        assert_eq!(q["step_0"]["instructions"]["previous_step"], Value::Null);
        assert_eq!(q["step_0"]["instructions"]["next_step"], "Bake.");
        assert_eq!(q["has_wait"]["type"], "noul");
        let s = jev_state(&r);
        assert!(s.get("url").is_none() && s.get("image").is_none());
    }

    fn reply(r: &Recipe, odd: &[(&str, &str, f64)]) -> Reply {
        Reply {
            answers: answers(r, odd),
            input_tokens: 10,
            model: "jev-1.13.0".into(),
        }
    }

    fn queued(conn: &Connection, id: i64, mode: Mode) {
        conn.execute(
            "INSERT INTO recipe_checks (recipe_id, status, queued_at, mode) VALUES (?1, 'pending', 0, ?2)
             ON CONFLICT(recipe_id) DO UPDATE SET status = 'pending', mode = ?2, seen_at = NULL",
            params![id, mode.as_str()],
        )
        .unwrap();
    }

    fn states(conn: &Connection, id: i64) -> Vec<(String, String)> {
        let mut stmt = conn
            .prepare("SELECT kind, state FROM recipe_flags WHERE recipe_id = ?1 ORDER BY id")
            .unwrap();
        stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn suggests_instead_of_fixing_a_recipe_edited_meanwhile() {
        let db = crate::db::open_in_memory().unwrap();
        let conn = db.lock();
        let fields = RecipeFields {
            title: "Pie".into(),
            ingredients: vec![section(None, &["Filling", "2 apples", "Nutrition Facts"])],
            ..Default::default()
        };
        let (snapshot, _) = crate::recipes::create_recipe(&conn, fields, "url").unwrap();
        queued(&conn, snapshot.id, Mode::Import);
        let odd = [("ing_0", "heading", 1.0), ("ing_2", "junk", 1.0)];

        // The cook saved a change while Wee Chef was looking (in the same second)
        crate::recipes::update_recipe(
            &conn,
            snapshot.id,
            RecipePatch {
                notes: Some(Some("Mine".into())),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            apply(&conn, &snapshot, reply(&snapshot, &odd), Mode::Import).unwrap(),
            (0, 2)
        );
        let now = crate::recipes::require_recipe(&conn, snapshot.id).unwrap();
        assert_eq!(now.ingredients, snapshot.ingredients);
        let c = for_recipe(&conn, snapshot.id).unwrap();
        assert_eq!(c["canUndo"], false);
        assert_eq!(c["flags"].as_array().unwrap().len(), 2);

        // "Keep as is" on one: a later check doesn't raise it again, and the other is
        // still only a suggestion: the recipe was edited, even within the same second
        let flag = c["flags"][1]["id"].as_i64().unwrap();
        assert_eq!(c["flags"][1]["itemText"], "Nutrition Facts");
        dismiss(&conn, snapshot.id, flag).unwrap();
        let current = crate::recipes::require_recipe(&conn, snapshot.id).unwrap();
        for mode in [Mode::Import, Mode::Review] {
            queued(&conn, snapshot.id, mode);
            if mode == Mode::Import {
                // As an edit while the import check waits would leave it
                note_edit(&conn, &snapshot, &current).unwrap();
            }
            let mode: String = conn
                .query_row(
                    "SELECT mode FROM recipe_checks WHERE recipe_id = ?1",
                    [snapshot.id],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(mode, "review");
            assert_eq!(
                apply(&conn, &current, reply(&current, &odd), Mode::Review).unwrap(),
                (0, 1)
            );
        }
        let after = crate::recipes::require_recipe(&conn, snapshot.id).unwrap();
        assert_eq!(after.ingredients, snapshot.ingredients);
    }

    #[test]
    fn fixes_a_fresh_import_and_a_stale_save_supersedes_the_fix() {
        let db = crate::db::open_in_memory().unwrap();
        let conn = db.lock();
        let fields = RecipeFields {
            title: "Pie".into(),
            ingredients: vec![section(None, &["Filling", "2 apples", "Nutrition Facts"])],
            ..Default::default()
        };
        let (r, _) = crate::recipes::create_recipe(&conn, fields, "url").unwrap();
        queued(&conn, r.id, Mode::Import);
        let odd = [("ing_0", "heading", 1.0), ("ing_2", "junk", 1.0)];
        assert_eq!(
            apply(&conn, &r, reply(&r, &odd), Mode::Import).unwrap(),
            (2, 0)
        );
        let fixed = crate::recipes::require_recipe(&conn, r.id).unwrap();
        assert_eq!(
            fixed.ingredients,
            vec![section(Some("Filling"), &["2 apples"])]
        );
        let c = for_recipe(&conn, r.id).unwrap();
        assert_eq!(c["canUndo"], true);
        assert_eq!(c["flags"].as_array().unwrap().len(), 2);

        // An editor opened before the fix saves the old lists, in the same second
        crate::recipes::update_recipe(
            &conn,
            r.id,
            RecipePatch {
                ingredients: Some(r.ingredients.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        let c = for_recipe(&conn, r.id).unwrap();
        assert_eq!(c["canUndo"], false);
        assert_eq!(c["flags"].as_array().unwrap().len(), 0, "no longer claimed");
        assert_eq!(undo(&conn, r.id).unwrap_err().status, 409);
        let now = crate::recipes::require_recipe(&conn, r.id).unwrap();
        assert_eq!(now.ingredients, r.ingredients);
    }

    #[test]
    fn check_all_only_suggests_and_tidies_glyphs_under_undo() {
        let db = crate::db::open_in_memory().unwrap();
        let conn = db.lock();
        let fields = RecipeFields {
            title: "Stew".into(),
            prep_time: Some("10&nbsp;min".into()),
            cook_time: Some("1h".into()),
            ingredients: vec![section(None, &["▢ 1 cup broth", "▢ Nutrition Facts"])],
            instructions: vec![section(None, &["Don&#039;t rush it."])],
            ..Default::default()
        };
        // An older recipe (like every one the legacy upgrade marked 'url')
        let (r, _) = crate::recipes::create_recipe(&conn, fields, "url").unwrap();
        assert_eq!(to_check_all(&conn).unwrap(), [r.id]);
        queued(&conn, r.id, Mode::Review);
        assert_eq!(
            apply(
                &conn,
                &r,
                reply(&r, &[("ing_1", "junk", 0.99)]),
                Mode::Review
            )
            .unwrap(),
            (1, 1)
        );
        let now = crate::recipes::require_recipe(&conn, r.id).unwrap();
        // The junk line stays (flagged, with its glyph gone); no total time is made up
        assert_eq!(
            now.ingredients,
            vec![section(None, &["1 cup broth", "Nutrition Facts"])]
        );
        assert_eq!(now.instructions, vec![section(None, &["Don't rush it."])]);
        assert_eq!(now.total_time, None);
        assert_eq!(now.prep_time, r.prep_time);
        let c = for_recipe(&conn, r.id).unwrap();
        assert_eq!(c["flags"][0]["kind"], "tidy");
        // Only what was written back is counted: three lines, not the prep time
        assert_eq!(c["flags"][0]["detail"]["count"], 3);
        assert_eq!(c["flags"][1]["itemText"], "Nutrition Facts");
        assert_eq!(c["flags"][1]["state"], "review");
        assert_eq!(c["canUndo"], true);
        undo(&conn, r.id).unwrap();
        let back = crate::recipes::require_recipe(&conn, r.id).unwrap();
        assert_eq!(back.ingredients, r.ingredients);
        assert_eq!(back.instructions, r.instructions);
        assert!(to_check_all(&conn).unwrap().is_empty());
    }

    #[test]
    fn check_all_picks_restores_and_recipes_edited_since() {
        let db = crate::db::open_in_memory().unwrap();
        let conn = db.lock();
        let make = |title: &str| {
            let fields = RecipeFields {
                title: title.into(),
                ingredients: vec![section(None, &["1 egg"])],
                ..Default::default()
            };
            crate::recipes::create_recipe(&conn, fields, "import")
                .unwrap()
                .0
        };
        let restored = make("Restored");
        mark_restored(&conn, restored.id).unwrap();
        let older = make("Older");
        let checked = make("Checked");
        queued(&conn, checked.id, Mode::Import);
        apply(&conn, &checked, reply(&checked, &[]), Mode::Import).unwrap();
        let due_now = due(&conn).unwrap();
        assert_eq!(
            due_now,
            [(restored.id, Due::Restored), (older.id, Due::Unchecked)]
        );

        // An edit after the check makes it due again, even in the same second
        crate::recipes::update_recipe(
            &conn,
            checked.id,
            RecipePatch {
                notes: Some(Some("Mine".into())),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(due(&conn).unwrap().contains(&(checked.id, Due::Edited)));
        let current = crate::recipes::require_recipe(&conn, checked.id).unwrap();
        queued(&conn, checked.id, Mode::Review);
        apply(&conn, &current, reply(&current, &[]), Mode::Review).unwrap();
        assert!(!to_check_all(&conn).unwrap().contains(&checked.id));
    }

    #[test]
    fn an_imports_dropped_steps_can_be_undone_with_the_checks_fixes() {
        let db = crate::db::open_in_memory().unwrap();
        let conn = db.lock();
        let credit = "Photo credit: Studio";
        let mut fields = RecipeFields {
            title: "Soup".into(),
            ingredients: vec![section(None, &["1 onion", "Nutrition Facts"])],
            instructions: vec![section(
                None,
                &["Chop.", credit, "Fry.", credit, "Simmer.", credit],
            )],
            ..Default::default()
        };
        let as_scraped = fields.instructions.clone();
        let undo_info = tidy_import(&mut fields, "url").expect("dropped two credits");
        let (r, _) = crate::recipes::create_recipe(&conn, fields, "url").unwrap();
        remember_tidy(&conn, &r, undo_info).unwrap();
        let c = for_recipe(&conn, r.id).unwrap();
        assert_eq!(c["status"], "tidied");
        assert_eq!(c["canUndo"], true);
        assert_eq!(c["flags"].as_array().unwrap().len(), 2);

        // The check then removes the junk: one Undo puts back the import as it came in
        queued(&conn, r.id, Mode::Import);
        assert_eq!(
            apply(
                &conn,
                &r,
                reply(&r, &[("ing_1", "junk", 0.99)]),
                Mode::Import
            )
            .unwrap(),
            (1, 0)
        );
        let c = for_recipe(&conn, r.id).unwrap();
        assert_eq!(c["flags"].as_array().unwrap().len(), 3);
        undo(&conn, r.id).unwrap();
        let back = crate::recipes::require_recipe(&conn, r.id).unwrap();
        assert_eq!(back.instructions, as_scraped);
        assert_eq!(back.ingredients, r.ingredients);
        assert_eq!(
            states(&conn, r.id)
                .into_iter()
                .map(|(_, s)| s)
                .collect::<Vec<_>>(),
            ["undone", "undone", "undone"]
        );
    }

    #[test]
    fn a_flag_on_a_step_the_tidy_drops_is_closed() {
        let db = crate::db::open_in_memory().unwrap();
        let conn = db.lock();
        let fields = RecipeFields {
            title: "Bread".into(),
            instructions: vec![section(None, &["Bake until golden.", "Bake until golden!"])],
            ..Default::default()
        };
        let (r, _) = crate::recipes::create_recipe(&conn, fields, "text").unwrap();
        queued(&conn, r.id, Mode::Import);
        assert_eq!(
            apply(
                &conn,
                &r,
                reply(&r, &[("step_1", "not_instruction", 0.7)]),
                Mode::Import
            )
            .unwrap(),
            (1, 0)
        );
        let c = for_recipe(&conn, r.id).unwrap();
        assert_eq!(c["flags"].as_array().unwrap().len(), 1);
        assert_eq!(c["flags"][0]["kind"], "tidy");
    }

    #[test]
    fn content_hash_is_stable() {
        let r = recipe(vec![section(None, &["1 egg"])], vec![], Some("n"));
        assert_eq!(content_hash(&r), content_hash(&r.clone()));
        let mut other = r.clone();
        other.notes = None;
        assert_ne!(content_hash(&r), content_hash(&other));
        assert_eq!(content_hash(&r).len(), 18);
    }
}
