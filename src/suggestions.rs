//! Try next and Surprise me as a service: reads the library and the cook log, ranks it
//! with `suggest`, and, when an AI API is configured, has the model re-rank the top
//! candidates and write one-line blurbs. The AI call runs in the background and is
//! cached per day, so it never slows a page down; the algorithm's list always stands in.

use axum::http::HeaderMap;
use rusqlite::Connection;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::Duration;

use crate::AppState;
use crate::error::AppResult;
use crate::model::{RecipeSummary, normalize_sections_value, now_secs};
use crate::recipes::summaries_by_ids;
use crate::suggest::{self, Ctx, DAY, Features, History, Pick, ReasonKind, RecipeInput};

/// Candidates sent to the AI.
const AI_CANDIDATES: usize = 12;
const AI_CALLS_PER_DAY: u32 = 3;
const AI_FAILURE_BACKOFF_SECS: i64 = 3600;
const BLURB_MAX_CHARS: usize = 120;

const SYSTEM: &str = "You help one home cook choose what to cook next from their own recipe box.
Pick 4 recipes from `candidates` only, by id. Favour variety (different main ingredients and cuisines), things they haven't made, and what suits today.
Use their recent cooking as a taste signal, but don't repeat it.
For each pick, write one blurb of at most 90 characters, in the second person and concrete (why now, or why it fits them).
Use only facts in the data, with no invented ingredients, times or claims, and no exclamation marks.";

// ─── The cook's time zone ───────────────────────────────────────────────────

/// The browser's UTC offset and hemisphere, from the `crumb_tz`/`crumb_zone` cookies.
/// The last one seen is kept for requests without cookies (the MCP connector).
#[derive(Default)]
pub struct Zone {
    offset_min: AtomicI32,
    southern: AtomicBool,
}

/// IANA zones south of the equator, where the seasons are flipped. Tropical zones near
/// the equator are left northern: they barely have seasons to flip.
const SOUTHERN_PREFIXES: [&str; 14] = [
    "Australia/",
    "Antarctica/",
    "America/Argentina/",
    "Pacific/Auckland",
    "Pacific/Chatham",
    "Pacific/Fiji",
    "Pacific/Tongatapu",
    "Pacific/Noumea",
    "Pacific/Efate",
    "Pacific/Apia",
    "Pacific/Easter",
    "Indian/Mauritius",
    "Indian/Reunion",
    "Indian/Antananarivo",
];
const SOUTHERN_ZONES: [&str; 21] = [
    "America/Santiago",
    "America/Punta_Arenas",
    "America/Buenos_Aires",
    "America/Montevideo",
    "America/Asuncion",
    "America/Sao_Paulo",
    "America/Lima",
    "America/La_Paz",
    "Atlantic/Stanley",
    "Africa/Johannesburg",
    "Africa/Maputo",
    "Africa/Harare",
    "Africa/Lusaka",
    "Africa/Windhoek",
    "Africa/Gaborone",
    "Africa/Maseru",
    "Africa/Mbabane",
    "Africa/Blantyre",
    "Africa/Lubumbashi",
    "Africa/Luanda",
    "Africa/Dar_es_Salaam",
];

pub fn is_southern(zone: &str) -> bool {
    SOUTHERN_PREFIXES.iter().any(|p| zone.starts_with(p)) || SOUTHERN_ZONES.contains(&zone)
}

/// (minutes east of UTC, southern hemisphere) for this request.
pub fn zone(state: &AppState, headers: &HeaderMap) -> (i32, bool) {
    let z = &state.zone;
    if let Some(offset) =
        crate::auth::cookie_value(headers, "crumb_tz").and_then(|v| v.trim().parse::<i32>().ok())
    {
        z.offset_min
            .store(offset.clamp(-840, 840), Ordering::Relaxed);
        let name = crate::auth::cookie_value(headers, "crumb_zone")
            .map(|v| v.replace("%2F", "/").replace("%2f", "/"))
            .unwrap_or_default();
        z.southern.store(is_southern(&name), Ordering::Relaxed);
    }
    (
        z.offset_min.load(Ordering::Relaxed),
        z.southern.load(Ordering::Relaxed),
    )
}

// ─── Inputs ─────────────────────────────────────────────────────────────────

struct Inputs {
    cands: Vec<Features>,
    hist: History,
    books: HashMap<i64, String>,
    /// Changes whenever a recipe is added, edited or cooked: part of the AI cache key.
    version: (i64, i64, i64),
}

fn load(conn: &Connection) -> AppResult<Inputs> {
    let mut stmt = conn.prepare("SELECT recipe_id, cookbook_id FROM cookbook_recipes")?;
    let mut links: HashMap<i64, Vec<i64>> = HashMap::new();
    for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
        let (recipe, book) = row?;
        links.entry(recipe).or_default().push(book);
    }
    let mut stmt = conn.prepare("SELECT id, name FROM cookbooks")?;
    let books: HashMap<i64, String> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut stmt = conn.prepare(
        "SELECT id, title, image, prep_time, cook_time, total_time, recipe_category, recipe_cuisine,
                ingredients, instructions, created_at FROM recipes",
    )?;
    let rows = stmt.query_map([], |r| {
        let sections = |i: usize| -> rusqlite::Result<Value> {
            let raw: Option<String> = r.get(i)?;
            Ok(raw
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(Value::Null))
        };
        let ingredients = normalize_sections_value(&sections(8)?);
        let instructions = normalize_sections_value(&sections(9)?);
        let id: i64 = r.get(0)?;
        Ok(RecipeInput {
            id,
            title: r.get(1)?,
            image: r.get(2)?,
            prep_time: r.get(3)?,
            cook_time: r.get(4)?,
            total_time: r.get(5)?,
            category: r.get(6)?,
            cuisine: r.get(7)?,
            ingredient_lines: ingredients.into_iter().flat_map(|s| s.items).collect(),
            step_count: instructions.iter().map(|s| s.items.len()).sum(),
            books: links.get(&id).cloned().unwrap_or_default(),
            created_at: r.get(10)?,
        })
    })?;
    let cands: Vec<Features> = rows
        .map(|r| r.map(Features::new))
        .collect::<rusqlite::Result<_>>()?;

    let mut stmt = conn.prepare("SELECT recipe_id, kind, created_at FROM recipe_events")?;
    let events: Vec<(i64, String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let hist = History::from_events(events.iter().map(|(id, k, at)| (*id, k.as_str(), *at)));

    let version = conn.query_row(
        "SELECT (SELECT count(*) FROM recipes), (SELECT coalesce(max(updated_at), 0) FROM recipes),
                (SELECT coalesce(max(id), 0) FROM recipe_events WHERE kind = 'cooked')",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    Ok(Inputs {
        cands,
        hist,
        books,
        version,
    })
}

// ─── Options and results ────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub limit: usize,
    pub seed: u64,
    pub exclude: HashSet<i64>,
    pub max_minutes: Option<u32>,
    /// Only these recipes (e.g. those matching a search), when set.
    pub only: Option<HashSet<i64>>,
    /// Whether this request may start an AI call (the home page and its API may; the
    /// MCP connector only reads what's cached).
    pub may_call_ai: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AiStatus {
    /// The items carry the AI's order and blurbs.
    Ready,
    /// A call is running; ask again in a few seconds.
    Pending,
    Off,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub recipe: RecipeSummary,
    pub reason: String,
    pub reason_kind: ReasonKind,
    pub ai: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Suggestions {
    pub items: Vec<Item>,
    pub ai: AiStatus,
}

fn filtered(cands: Vec<Features>, opts: &Options) -> Vec<Features> {
    cands
        .into_iter()
        .filter(|f| opts.only.as_ref().is_none_or(|ids| ids.contains(&f.id)))
        .filter(|f| {
            opts.max_minutes
                .is_none_or(|max| f.minutes.is_some_and(|m| m <= max))
        })
        .collect()
}

// ─── AI cache ───────────────────────────────────────────────────────────────

/// (local day, recipe count, last edit, last cook, tz offset, southern)
type Key = (i64, i64, i64, i64, i32, bool);

#[derive(Default)]
struct AiInner {
    cached: Option<(Key, Vec<(i64, String)>)>,
    failed: Option<(Key, i64)>,
    in_flight: Option<Key>,
    day: i64,
    calls: u32,
}

#[derive(Default)]
pub struct AiState(Mutex<AiInner>);

impl AiState {
    fn lock(&self) -> std::sync::MutexGuard<'_, AiInner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The AI's picks in its order, limited to recipes still eligible now, then filled up
/// from the algorithm's order.
pub fn apply_ai(
    ai: &[(i64, String)],
    eligible: &HashSet<i64>,
    algo: &[Pick],
    n: usize,
) -> Vec<(i64, String, ReasonKind, bool)> {
    let mut out: Vec<(i64, String, ReasonKind, bool)> = Vec::new();
    for (id, blurb) in ai {
        if out.len() < n && eligible.contains(id) && !out.iter().any(|o| o.0 == *id) {
            out.push((*id, blurb.clone(), ReasonKind::Ai, true));
        }
    }
    for p in algo {
        if out.len() < n && !out.iter().any(|o| o.0 == p.id) {
            out.push((p.id, p.reason.clone(), p.reason_kind, false));
        }
    }
    out
}

/// Valid picks from the model's reply: known candidate ids, no repeats, short blurbs.
pub fn parse_ai(reply: &Value, candidates: &HashSet<i64>) -> Vec<(i64, String)> {
    let mut out: Vec<(i64, String)> = Vec::new();
    for p in reply
        .get("picks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let (Some(id), Some(blurb)) = (
            p.get("id").and_then(Value::as_i64),
            p.get("blurb").and_then(Value::as_str),
        ) else {
            continue;
        };
        let blurb: String = blurb.trim().chars().take(BLURB_MAX_CHARS).collect();
        if candidates.contains(&id) && !blurb.is_empty() && !out.iter().any(|o| o.0 == id) {
            out.push((id, blurb.trim_end().to_string()));
        }
    }
    out
}

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "picks": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {"id": {"type": "integer"}, "blurb": {"type": "string"}},
                    "required": ["id", "blurb"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["picks"],
        "additionalProperties": false
    })
}

/// What the model sees: no ingredient lists, steps, notes, links or images.
fn payload(inputs: &Inputs, ctx: &Ctx, candidates: &[Pick]) -> Value {
    let by_id: HashMap<i64, &Features> = inputs.cands.iter().map(|f| (f.id, f)).collect();
    let now = ctx.now;
    let mut recent: Vec<(&Features, i64)> = inputs
        .cands
        .iter()
        .filter_map(|f| {
            let t = inputs.hist.get(f.id)?.last_cooked?;
            (now - t <= 180 * DAY).then_some((f, t))
        })
        .collect();
    recent.sort_by_key(|(_, t)| -t);
    let recent: Vec<Value> = recent
        .iter()
        .take(10)
        .map(|(f, t)| {
            json!({"title": f.title, "cuisine": f.cuisines.first(), "bucket": f.bucket, "daysAgo": (now - t) / DAY})
        })
        .collect();
    let mut opened: Vec<(&Features, usize)> = inputs
        .cands
        .iter()
        .filter_map(|f| {
            let s = inputs.hist.get(f.id)?;
            let views = s.views.iter().filter(|t| now - **t <= 60 * DAY).count();
            (s.cooks == 0 && views >= 2).then_some((f, views))
        })
        .collect();
    opened.sort_by_key(|(_, v)| std::cmp::Reverse(*v));
    let opened: Vec<Value> = opened
        .iter()
        .take(5)
        .map(|(f, v)| json!({"title": f.title, "views": v}))
        .collect();
    let candidates: Vec<Value> = candidates
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let f = by_id.get(&p.id)?;
            let books: Vec<&String> = f.books.iter().filter_map(|b| inputs.books.get(b)).collect();
            Some(json!({
                "id": f.id,
                "title": f.title,
                "bucket": f.bucket,
                "cuisine": f.cuisines.first(),
                "minutes": f.minutes,
                "heroes": f.heroes.iter().take(4).collect::<Vec<_>>(),
                "cookbooks": books,
                "cookedTimes": inputs.hist.get(f.id).map_or(0, |s| s.cooks),
                "algoRank": i + 1,
            }))
        })
        .collect();
    json!({
        "today": {"weekday": ctx.weekday_name(), "timeOfDay": ctx.time_of_day(), "season": ctx.season()},
        "recentlyCooked": recent,
        "openedNotCooked": opened,
        "candidates": candidates,
    })
}

/// Clears the in-flight marker when a call ends, even by panic, unless a newer call
/// (for a newer key) has taken over.
struct InFlight {
    state: AppState,
    key: Key,
}

impl Drop for InFlight {
    fn drop(&mut self) {
        let mut inner = self.state.ai.lock();
        if inner.in_flight == Some(self.key) {
            inner.in_flight = None;
        }
    }
}

async fn run_ai(state: AppState, key: Key, body: Value, candidates: HashSet<i64>) {
    let _flight = InFlight {
        state: state.clone(),
        key,
    };
    let model = state
        .config
        .suggest_model
        .clone()
        .or_else(|| state.config.llm.as_ref().map(|l| l.model.clone()))
        .unwrap_or_default();
    let reply = crate::llm::ask(
        &state,
        crate::llm::Ask {
            tag: "suggest",
            model: &model,
            system: SYSTEM,
            user: &body.to_string(),
            schema: schema(),
            // Thinking models count their reasoning against this too
            max_tokens: 4000,
            timeout: Duration::from_secs(20),
        },
    )
    .await;
    let picks = reply.map(|r| parse_ai(&r, &candidates)).unwrap_or_default();
    let mut inner = state.ai.lock();
    // A newer call (the library changed meanwhile) owns the result now
    if inner.in_flight != Some(key) {
        return;
    }
    if picks.is_empty() {
        inner.failed = Some((key, now_secs() + AI_FAILURE_BACKOFF_SECS));
    } else {
        inner.cached = Some((key, picks));
    }
}

// ─── Entry points ───────────────────────────────────────────────────────────

pub fn context(state: &AppState, headers: &HeaderMap, seed: u64) -> Ctx {
    let (tz_offset_min, southern) = zone(state, headers);
    Ctx {
        now: now_secs(),
        tz_offset_min,
        southern,
        seed,
    }
}

/// Try next: the day's list (seed 0), or a shuffle of it.
pub fn suggestions(state: &AppState, ctx: Ctx, opts: &Options) -> AppResult<Suggestions> {
    let inputs = load(&state.db.lock())?;
    let cands = filtered(inputs.cands.clone(), opts);
    let limit = opts.limit.max(1);
    let algo = suggest::rank(&cands, &inputs.hist, &ctx, &opts.exclude, limit);

    let plain = ctx.seed == 0
        && opts.exclude.is_empty()
        && opts.only.is_none()
        && opts.max_minutes.is_none();
    let mut status = AiStatus::Off;
    let mut chosen: Vec<(i64, String, ReasonKind, bool)> = algo
        .iter()
        .map(|p| (p.id, p.reason.clone(), p.reason_kind, false))
        .collect();

    if plain && state.config.suggestions_ai && state.config.llm.is_some() && algo.len() >= 2 {
        let (count, edited, cooked) = inputs.version;
        let key: Key = (
            ctx.local_day(),
            count,
            edited,
            cooked,
            ctx.tz_offset_min,
            ctx.southern,
        );
        let mut inner = state.ai.lock();
        if let Some((k, picks)) = &inner.cached
            && *k == key
        {
            let eligible: HashSet<i64> =
                suggest::eligible(&cands, &inputs.hist, &ctx, &HashSet::new())
                    .iter()
                    .map(|f| f.id)
                    .collect();
            chosen = apply_ai(picks, &eligible, &algo, limit);
            status = AiStatus::Ready;
        } else if inner.in_flight == Some(key) {
            status = AiStatus::Pending;
        } else if inner
            .failed
            .is_some_and(|(k, until)| k == key && ctx.now < until)
        {
            status = AiStatus::Off;
        } else if opts.may_call_ai {
            if inner.day != key.0 {
                inner.day = key.0;
                inner.calls = 0;
            }
            if inner.calls < AI_CALLS_PER_DAY {
                inner.calls += 1;
                inner.in_flight = Some(key);
                let pool =
                    suggest::rank(&cands, &inputs.hist, &ctx, &HashSet::new(), AI_CANDIDATES);
                let ids: HashSet<i64> = pool.iter().map(|p| p.id).collect();
                let body = payload(&inputs, &ctx, &pool);
                tokio::spawn(run_ai(state.clone(), key, body, ids));
                status = AiStatus::Pending;
            }
        }
    }

    let ids: Vec<i64> = chosen.iter().map(|c| c.0).collect();
    let summaries = summaries_by_ids(&state.db.lock(), &ids)?;
    let items = chosen
        .into_iter()
        .filter_map(|(id, reason, reason_kind, ai)| {
            let recipe = summaries.iter().find(|s| s.id == id)?.clone();
            Some(Item {
                recipe,
                reason,
                reason_kind,
                ai,
            })
        })
        .collect();
    Ok(Suggestions { items, ai: status })
}

/// Surprise me: a random recipe id, or None when the library (after filters) is empty.
pub fn random(
    state: &AppState,
    exclude: &HashSet<i64>,
    current: Option<i64>,
    opts: &Options,
) -> AppResult<Option<i64>> {
    let inputs = load(&state.db.lock())?;
    let cands = filtered(inputs.cands, opts);
    Ok(suggest::random(
        &cands,
        &inputs.hist,
        now_secs(),
        exclude,
        current,
        &mut rand::thread_rng(),
    ))
}

/// Ids from a comma-separated query value; junk is ignored and the list is capped.
pub fn id_set(raw: Option<&String>) -> HashSet<i64> {
    raw.map(|s| {
        s.split(',')
            .filter_map(|p| p.trim().parse::<i64>().ok())
            .filter(|id| *id > 0)
            .take(200)
            .collect()
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn southern_zones() {
        assert!(is_southern("Australia/Sydney"));
        assert!(is_southern("Pacific/Auckland"));
        assert!(is_southern("America/Argentina/Buenos_Aires"));
        assert!(is_southern("Africa/Johannesburg"));
        assert!(!is_southern("America/Toronto"));
        assert!(!is_southern("Europe/London"));
        assert!(!is_southern(""));
    }

    #[test]
    fn validates_ai_picks() {
        let reply = json!({"picks": [
            {"id": 3, "blurb": "  Quick and bright for a Tuesday  "},
            {"id": 99, "blurb": "Not a candidate"},
            {"id": 3, "blurb": "Again"},
            {"id": 5, "blurb": "x".repeat(300)},
            {"id": 6, "blurb": "   "}
        ]});
        let cands: HashSet<i64> = [1, 2, 3, 5, 6].into();
        let picks = parse_ai(&reply, &cands);
        assert_eq!(picks.len(), 2);
        assert_eq!(picks[0], (3, "Quick and bright for a Tuesday".to_string()));
        assert_eq!(picks[1].1.chars().count(), BLURB_MAX_CHARS);

        let algo: Vec<Pick> = [1, 2, 3, 4]
            .iter()
            .map(|id| Pick {
                id: *id,
                score: 0.0,
                reason: format!("r{id}"),
                reason_kind: ReasonKind::New,
            })
            .collect();
        // 5 was cooked today: dropped, and the list is filled from the algorithm
        let eligible: HashSet<i64> = [1, 2, 3, 4].into();
        let out = apply_ai(&picks, &eligible, &algo, 4);
        let ids: Vec<i64> = out.iter().map(|o| o.0).collect();
        assert_eq!(ids, vec![3, 1, 2, 4]);
        assert!(out[0].3 && !out[1].3);
        assert_eq!(out[0].2, ReasonKind::Ai);
    }
}
