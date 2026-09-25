//! "Try next" ranking and "Surprise me" picks. Pure functions over recipe features and
//! the cook/view log, so they can be tested without a database. See
//! `suggestions.rs` for the service around them (DB inputs, AI re-ranking, caching).

use chrono::{DateTime, Datelike, NaiveDateTime, Timelike, Weekday};
use rand::Rng;
use rand::seq::SliceRandom;
use regex::Regex;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

pub const DAY: i64 = 86_400;
/// Recipes cooked this recently are left out of Try next and Surprise me.
pub const COOLDOWN_DAYS: i64 = 14;

const W_NOVELTY: f64 = 0.30;
const W_AFFINITY: f64 = 0.25;
const W_INTEREST: f64 = 0.15;
const W_CONTEXT: f64 = 0.15;
const W_FRESH: f64 = 0.15;
const JITTER: f64 = 0.10;
const PENALTY: f64 = 0.05;
/// How strongly a pick is pushed away from ones already picked.
const MMR_LAMBDA: f64 = 0.5;
/// Only the top of the ranking is considered for the diverse set.
const POOL: usize = 20;

// ─── Features ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Bucket {
    Main,
    Side,
    Soup,
    Salad,
    Breakfast,
    Dessert,
    Baking,
    Snack,
    Drink,
    Sauce,
    Other,
}

impl Bucket {
    fn is_sweet(self) -> bool {
        matches!(self, Self::Dessert | Self::Baking)
    }
}

/// Keyword rules, checked in order: the first that matches the category wins.
const BUCKET_WORDS: [(Bucket, &[&str]); 10] = [
    (
        Bucket::Sauce,
        &[
            "sauce",
            "condiment",
            "dressing",
            "dip",
            "gravy",
            "marinade",
            "seasoning",
            "spice",
        ],
    ),
    (
        Bucket::Drink,
        &["drink", "beverage", "cocktail", "smoothie"],
    ),
    (Bucket::Breakfast, &["breakfast", "brunch"]),
    (Bucket::Soup, &["soup", "stew", "chili", "chowder"]),
    (Bucket::Salad, &["salad"]),
    (
        Bucket::Baking,
        &[
            "baking",
            "baked good",
            "cookie",
            "cake",
            "bread",
            "muffin",
            "pastry",
            "brownie",
        ],
    ),
    (Bucket::Dessert, &["dessert", "sweet", "treat"]),
    (Bucket::Side, &["side"]),
    (Bucket::Snack, &["snack", "appetizer", "starter"]),
    (
        Bucket::Main,
        &["main", "dinner", "entree", "entrée", "lunch", "supper"],
    ),
];

/// The broad kind of dish, from the free-text category (or the title when there's none).
pub fn bucket(category: Option<&str>, title: &str) -> Bucket {
    let find = |text: &str| {
        let text = text.to_lowercase();
        BUCKET_WORDS
            .iter()
            .find(|(_, words)| words.iter().any(|w| text.contains(w)))
            .map(|(b, _)| *b)
    };
    if let Some(b) = category.and_then(find) {
        return b;
    }
    // Titles only settle the unambiguous dish types
    match find(title) {
        Some(b @ (Bucket::Soup | Bucket::Salad)) => b,
        _ => Bucket::Other,
    }
}

struct Hero {
    name: &'static str,
    protein: bool,
    /// Matched before the noise words are removed (e.g. "sweet potato").
    raw: bool,
    re: Regex,
}

/// Hero ingredients: what a dish is "about". Proteins set the main protein.
static HEROES: LazyLock<Vec<Hero>> = LazyLock::new(|| {
    let list: [(&str, bool, bool, &str); 32] = [
        ("chicken", true, false, r"\bchicken\b"),
        (
            "beef",
            true,
            false,
            r"\b(beef|steaks?|sirloin|brisket|ribeye|filet mignon|flank|chuck)\b",
        ),
        ("pork", true, false, r"\b(pork|ham|prosciutto|pancetta)\b"),
        ("lamb", true, false, r"\b(lamb|mutton)\b"),
        ("turkey", true, false, r"\bturkey\b"),
        (
            "sausage",
            true,
            false,
            r"\b(sausages?|chorizo|bratwurst|kielbasa)\b",
        ),
        ("bacon", true, false, r"\bbacon\b"),
        ("salmon", true, false, r"\bsalmon\b"),
        ("tuna", true, false, r"\btuna\b"),
        (
            "white fish",
            true,
            false,
            r"\b(cod|halibut|tilapia|haddock|white ?fish|pollock|snapper)\b",
        ),
        ("shrimp", true, false, r"\b(shrimps?|prawns?)\b"),
        ("crab", true, false, r"\b(crab|lobster)\b"),
        ("tofu", true, false, r"\btofu\b"),
        ("tempeh", true, false, r"\btempeh\b"),
        ("chickpea", true, false, r"\b(chickpeas?|garbanzo)"),
        ("lentil", true, false, r"\blentils?\b"),
        (
            "beans",
            true,
            false,
            r"\b(black|kidney|pinto|cannellini|navy|white|refried) beans?\b",
        ),
        ("egg", true, false, r"\beggs?\b"),
        (
            "pasta",
            false,
            false,
            r"\b(pasta|spaghetti|penne|linguine|fettuccine|macaroni|rigatoni|ziti|lasagna|orzo|tagliatelle)\b",
        ),
        ("noodles", false, false, r"\b(noodles?|ramen|udon|soba)\b"),
        ("rice", false, false, r"\brice\b"),
        ("sweet potato", false, true, r"\bsweet potato(es)?\b"),
        ("potato", false, false, r"\bpotato(es)?\b"),
        ("gnocchi", false, false, r"\bgnocchi\b"),
        ("mushroom", false, false, r"\bmushrooms?\b"),
        ("cauliflower", false, false, r"\bcauliflower\b"),
        ("eggplant", false, false, r"\b(eggplants?|aubergines?)\b"),
        ("squash", false, false, r"\b(squash|pumpkin)\b"),
        ("zucchini", false, false, r"\b(zucchinis?|courgettes?)\b"),
        ("corn", false, false, r"\bcorn\b"),
        (
            "berries",
            false,
            false,
            r"\b(strawberr\w*|blueberr\w*|raspberr\w*|blackberr\w*|berries)\b",
        ),
        ("tomato", false, false, r"\btomato(es)?\b"),
    ];
    list.iter()
        .map(|(name, protein, raw, re)| Hero {
            name,
            protein: *protein,
            raw: *raw,
            re: Regex::new(re).unwrap(),
        })
        .collect()
});

/// Phrases that mention a hero without being it ("chicken stock", "rice vinegar").
static NOISE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b(chicken|beef|vegetable|veggie|fish|pork|bone|turkey|ham)\s+(broth|stock|bouillon|base|fat|seasoning)\b|\brice\s+(vinegar|wine|flour|paper)\b|\bsteak\s+(spice|seasoning|sauce)\b|\bcorn\s+(syrup|starch|flour|meal)\b|\bsweet potato(es)?\b",
    )
    .unwrap()
});

/// Heroes found in the ingredient lines, and the first protein in ingredient order.
/// Eggs only count for breakfasts, since they're in every cake.
pub fn heroes(lines: &[String], bucket: Bucket) -> (Vec<&'static str>, Option<&'static str>) {
    let mut found: Vec<&'static str> = Vec::new();
    let mut main_protein = None;
    for line in lines {
        let raw = line.to_lowercase();
        let cleaned = NOISE.replace_all(&raw, " ");
        for hero in HEROES.iter() {
            if hero.name == "egg" && bucket != Bucket::Breakfast {
                continue;
            }
            let text: &str = if hero.raw { &raw } else { &cleaned };
            if hero.re.is_match(text) {
                if !found.contains(&hero.name) {
                    found.push(hero.name);
                }
                if hero.protein && main_protein.is_none() {
                    main_protein = Some(hero.name);
                }
            }
        }
    }
    (found, main_protein)
}

static ISO_DURATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^P(?:(\d+)D)?(?:T(?:(\d+)H)?(?:(\d+)M)?(?:\d+S)?)?$").unwrap()
});
static TEXT_DURATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*(days?|d|hours?|hrs?|h|minutes?|mins?|m)\b").unwrap()
});

/// Minutes in a free-text duration: "PT1H30M", "1h 30m", "1 hr 30 min", "45 mins",
/// "1 hour 15 minutes", or a bare "90". None for "overnight", "" and zero.
pub fn parse_minutes(s: &str) -> Option<u32> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let total = if let Some(c) = ISO_DURATION.captures(s) {
        let n = |i: usize| {
            c.get(i)
                .and_then(|m| m.as_str().parse::<f64>().ok())
                .unwrap_or(0.0)
        };
        n(1) * 1440.0 + n(2) * 60.0 + n(3)
    } else if let Ok(bare) = s.parse::<f64>() {
        bare
    } else {
        TEXT_DURATION
            .captures_iter(s)
            .map(|c| {
                let n: f64 = c[1].parse().unwrap_or(0.0);
                match c[2].to_lowercase().chars().next() {
                    Some('d') => n * 1440.0,
                    Some('h') => n * 60.0,
                    _ => n,
                }
            })
            .sum()
    };
    let minutes = total.round();
    (1.0..100_000.0)
        .contains(&minutes)
        .then_some(minutes as u32)
}

/// A recipe row, as read from the database.
pub struct RecipeInput {
    pub id: i64,
    pub title: String,
    pub image: Option<String>,
    pub prep_time: Option<String>,
    pub cook_time: Option<String>,
    pub total_time: Option<String>,
    pub category: Option<String>,
    pub cuisine: Option<String>,
    pub ingredient_lines: Vec<String>,
    pub step_count: usize,
    pub books: Vec<i64>,
    pub created_at: i64,
}

/// What the ranking knows about one recipe.
#[derive(Debug, Clone)]
pub struct Features {
    pub id: i64,
    pub title: String,
    pub has_image: bool,
    pub minutes: Option<u32>,
    pub bucket: Bucket,
    /// Lowercased, split on commas ("Korean, American" → ["korean", "american"]).
    pub cuisines: Vec<String>,
    pub heroes: Vec<&'static str>,
    pub main_protein: Option<&'static str>,
    pub books: Vec<i64>,
    pub ingredient_count: usize,
    pub step_count: usize,
    pub created_at: i64,
}

impl Features {
    pub fn new(r: RecipeInput) -> Self {
        let opt = |s: &Option<String>| s.as_deref().and_then(parse_minutes);
        let minutes = opt(&r.total_time).or_else(|| match (opt(&r.prep_time), opt(&r.cook_time)) {
            (None, None) => None,
            (p, c) => Some(p.unwrap_or(0) + c.unwrap_or(0)),
        });
        let bucket = bucket(r.category.as_deref(), &r.title);
        let (heroes, main_protein) = heroes(&r.ingredient_lines, bucket);
        let cuisines = r
            .cuisine
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(|c| c.trim().to_lowercase())
            .filter(|c| !c.is_empty())
            .collect();
        Self {
            id: r.id,
            title: r.title,
            has_image: r.image.is_some_and(|i| !i.trim().is_empty()),
            minutes,
            bucket,
            cuisines,
            heroes,
            main_protein,
            books: r.books,
            ingredient_count: r.ingredient_lines.len(),
            step_count: r.step_count,
            created_at: r.created_at,
        }
    }

    fn cuisine(&self) -> Option<&str> {
        self.cuisines.first().map(String::as_str)
    }
}

// ─── History ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub cooks: i64,
    pub last_cooked: Option<i64>,
    /// View timestamps, newest last.
    pub views: Vec<i64>,
}

/// The cook and view log, per recipe.
#[derive(Debug, Clone, Default)]
pub struct History(pub HashMap<i64, Stats>);

impl History {
    /// From (recipe id, kind, unix seconds) rows.
    pub fn from_events<'a>(events: impl IntoIterator<Item = (i64, &'a str, i64)>) -> Self {
        let mut map: HashMap<i64, Stats> = HashMap::new();
        for (id, kind, at) in events {
            let s = map.entry(id).or_default();
            match kind {
                "cooked" => {
                    s.cooks += 1;
                    s.last_cooked = Some(s.last_cooked.map_or(at, |t| t.max(at)));
                }
                "viewed" => s.views.push(at),
                _ => {}
            }
        }
        for s in map.values_mut() {
            s.views.sort_unstable();
        }
        Self(map)
    }

    pub fn get(&self, id: i64) -> Option<&Stats> {
        self.0.get(&id)
    }

    fn days_since_cooked(&self, id: i64, now: i64) -> Option<f64> {
        self.get(id)?
            .last_cooked
            .map(|t| ((now - t) as f64 / DAY as f64).max(0.0))
    }

    fn views_since(&self, id: i64, since: i64) -> usize {
        self.get(id)
            .map_or(0, |s| s.views.iter().filter(|t| **t >= since).count())
    }

    fn last_viewed(&self, id: i64) -> Option<i64> {
        self.get(id)?.views.last().copied()
    }

    pub fn cooked_within(&self, id: i64, now: i64, days: i64) -> bool {
        self.get(id)
            .and_then(|s| s.last_cooked)
            .is_some_and(|t| now - t < days * DAY)
    }
}

// ─── Context ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Season {
    Winter,
    Spring,
    Summer,
    Autumn,
}

/// When the ranking runs: the time, the cook's time zone, and the shuffle seed.
#[derive(Debug, Clone, Copy)]
pub struct Ctx {
    pub now: i64,
    /// Minutes east of UTC.
    pub tz_offset_min: i32,
    pub southern: bool,
    /// 0 for the day's list; each Shuffle asks for the next seed.
    pub seed: u64,
}

impl Ctx {
    pub fn local(&self) -> NaiveDateTime {
        DateTime::from_timestamp(self.now + self.tz_offset_min as i64 * 60, 0)
            .unwrap_or_default()
            .naive_utc()
    }

    /// Days since the epoch in local time: the key for "today".
    pub fn local_day(&self) -> i64 {
        (self.now + self.tz_offset_min as i64 * 60).div_euclid(DAY)
    }

    fn weeknight(&self) -> bool {
        matches!(
            self.local().weekday(),
            Weekday::Mon | Weekday::Tue | Weekday::Wed | Weekday::Thu
        )
    }

    pub fn season(&self) -> Season {
        let month = self.local().month();
        // Shift the southern hemisphere by six months
        let m = if self.southern {
            (month + 5) % 12 + 1
        } else {
            month
        };
        match m {
            12 | 1 | 2 => Season::Winter,
            3..=5 => Season::Spring,
            6..=8 => Season::Summer,
            _ => Season::Autumn,
        }
    }

    pub fn weekday_name(&self) -> String {
        self.local().format("%A").to_string()
    }

    pub fn time_of_day(&self) -> &'static str {
        match self.local().hour() {
            5..=10 => "morning",
            11..=13 => "midday",
            14..=16 => "afternoon",
            17..=21 => "evening",
            _ => "night",
        }
    }
}

static WINTER_TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(stew|braise[ds]?|chili|roast|curry|soup)").unwrap());
static SUMMER_TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(grill\w*|tomato\w*|zucchini|corn|berr\w*)").unwrap());

/// Whether a recipe suits the season: comfort food from November to February,
/// fresh and grilled things in June to August (both mirrored in the south).
fn in_season(f: &Features, ctx: &Ctx) -> bool {
    let month = ctx.local().month();
    let m = if ctx.southern {
        (month + 5) % 12 + 1
    } else {
        month
    };
    match m {
        11 | 12 | 1 | 2 => f.bucket == Bucket::Soup || WINTER_TITLE.is_match(&f.title),
        6..=8 => {
            f.bucket == Bucket::Salad
                || SUMMER_TITLE.is_match(&f.title)
                || f.heroes
                    .iter()
                    .any(|h| matches!(*h, "zucchini" | "corn" | "berries"))
        }
        _ => false,
    }
}

fn context(f: &Features, ctx: &Ctx) -> (f64, bool) {
    let time_fit: f64 = match (ctx.weeknight(), f.minutes) {
        (_, None) => 0.35,
        (true, Some(m)) if m <= 40 => 0.7,
        (true, Some(m)) if m <= 60 => 0.45,
        (true, Some(_)) => 0.1,
        (false, Some(m)) if m <= 60 => 0.4,
        (false, Some(_)) => 0.5,
    };
    let meal_fit = if ctx.local().hour() < 11 {
        if f.bucket == Bucket::Breakfast {
            0.3
        } else {
            0.0
        }
    } else {
        match f.bucket {
            Bucket::Breakfast => -0.3,
            Bucket::Dessert | Bucket::Baking => -0.15,
            Bucket::Main | Bucket::Soup | Bucket::Salad => 0.1,
            _ => 0.0,
        }
    };
    let season = in_season(f, ctx);
    let bonus = if season { 0.2 } else { 0.0 };
    ((time_fit + meal_fit + bonus).clamp(0.0, 1.0), season)
}

// ─── Similarity ─────────────────────────────────────────────────────────────

fn jaccard<T: PartialEq>(a: &[T], b: &[T]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let shared = a.iter().filter(|x| b.contains(x)).count() as f64;
    shared / ((a.len() + b.len()) as f64 - shared)
}

/// How alike two recipes are, in [0, 1]: shared cookbooks and hero ingredients,
/// same cuisine, same kind of dish.
pub fn similarity(a: &Features, b: &Features) -> f64 {
    let same_cuisine = a.cuisines.iter().any(|c| b.cuisines.contains(c));
    0.35 * jaccard(&a.books, &b.books)
        + 0.35 * jaccard(&a.heroes, &b.heroes)
        + if same_cuisine { 0.15 } else { 0.0 }
        + if a.bucket == b.bucket { 0.15 } else { 0.0 }
}

// ─── Ranking ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasonKind {
    Like,
    Interest,
    Rediscover,
    Quick,
    Project,
    New,
    Season,
    Kicker,
    /// Written by the AI layer.
    Ai,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pick {
    pub id: i64,
    pub score: f64,
    pub reason: String,
    pub reason_kind: ReasonKind,
}

/// FNV-1a over the seed, the local day and the id: a stable nudge in [0, 1).
fn jitter(seed: u64, day: i64, id: i64) -> f64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in seed
        .to_le_bytes()
        .iter()
        .chain(day.to_le_bytes().iter())
        .chain(id.to_le_bytes().iter())
    {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// Recipes worth suggesting at all: a real dish with ingredients and steps.
fn is_dish(f: &Features) -> bool {
    f.ingredient_count >= 2
        && f.step_count >= 1
        && !matches!(f.bucket, Bucket::Sauce | Bucket::Drink)
}

fn fmt_minutes(m: u32) -> String {
    match (m / 60, m % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

struct Scored<'a> {
    f: &'a Features,
    score: f64,
    reason: String,
    kind: ReasonKind,
}

/// Recipes Try next may show: dishes not cooked lately and not in `exclude`. Something
/// opened today is already in "Pick up where you left off", so it's left out too, unless
/// that would leave too little to choose from.
pub fn eligible<'a>(
    cands: &'a [Features],
    hist: &History,
    ctx: &Ctx,
    exclude: &HashSet<i64>,
) -> Vec<&'a Features> {
    let now = ctx.now;
    let base: Vec<&Features> = cands
        .iter()
        .filter(|f| is_dish(f) && !exclude.contains(&f.id))
        .filter(|f| !hist.cooked_within(f.id, now, COOLDOWN_DAYS))
        .collect();
    let not_viewed_today: Vec<&Features> = base
        .iter()
        .copied()
        .filter(|f| hist.last_viewed(f.id).is_none_or(|t| now - t >= DAY))
        .collect();
    if not_viewed_today.len() >= 4 {
        not_viewed_today
    } else {
        base
    }
}

/// The Try next list: up to `n` recipes, ranked and then picked for variety.
pub fn rank(
    cands: &[Features],
    hist: &History,
    ctx: &Ctx,
    exclude: &HashSet<i64>,
    n: usize,
) -> Vec<Pick> {
    let now = ctx.now;
    let eligible = eligible(cands, hist, ctx, exclude);

    // Taste profile: what was cooked lately (fading over months), plus things opened
    // repeatedly but never made
    let profile: Vec<(&Features, f64, bool)> = cands
        .iter()
        .filter_map(|p| {
            if let Some(dc) = hist.days_since_cooked(p.id, now)
                && dc <= 180.0
            {
                return Some((p, 0.5_f64.powf(dc / 60.0), true));
            }
            let never_cooked = hist.get(p.id).is_none_or(|s| s.cooks == 0);
            (never_cooked && hist.views_since(p.id, now - 60 * DAY) >= 2).then_some((p, 0.4, false))
        })
        .collect();

    let day = ctx.local_day();
    let mut scored: Vec<Scored> = eligible
        .into_iter()
        .map(|f| {
            let stats = hist.get(f.id);
            let cooks = stats.map_or(0, |s| s.cooks);
            let dc = hist.days_since_cooked(f.id, now);
            let views90 = hist.views_since(f.id, now - 90 * DAY);
            let never_touched = cooks == 0 && stats.is_none_or(|s| s.views.is_empty());

            let novelty = match dc {
                None => 1.0,
                Some(dc) => 0.8 * ((dc - COOLDOWN_DAYS as f64) / 90.0).clamp(0.0, 1.0),
            };
            let (affinity, because) = profile
                .iter()
                .filter(|(p, _, _)| p.id != f.id)
                .map(|(p, w, cooked)| (w * similarity(f, p), Some((*p, *cooked))))
                .fold(
                    (0.0, None),
                    |best, cur| if cur.0 > best.0 { cur } else { best },
                );
            let interest = if cooks == 0 {
                views90.min(4) as f64 / 4.0
            } else {
                0.0
            };
            let (ctx_score, season) = context(f, ctx);
            let fresh = if never_touched {
                let age = ((now - f.created_at) as f64 / DAY as f64).max(0.0);
                0.5 + 0.5 * (-age / 30.0).exp()
            } else {
                0.0
            };
            let penalty = if f.has_image { 0.0 } else { PENALTY }
                + if f.minutes.is_some() { 0.0 } else { PENALTY };
            let score = W_NOVELTY * novelty
                + W_AFFINITY * affinity
                + W_INTEREST * interest
                + W_CONTEXT * ctx_score
                + W_FRESH * fresh
                + JITTER * jitter(ctx.seed, day, f.id)
                - penalty;

            let (kind, reason) = match (because, f.minutes) {
                (Some((p, true)), _) if W_AFFINITY * affinity >= 0.10 => {
                    (ReasonKind::Like, format!("Because you made {}", p.title))
                }
                _ if interest >= 0.5 => (
                    ReasonKind::Interest,
                    format!("You've opened this {views90} times"),
                ),
                _ if dc.is_some_and(|d| d >= 60.0) => {
                    let months = (dc.unwrap_or(0.0) / 30.0).round() as i64;
                    (
                        ReasonKind::Rediscover,
                        format!("Last made {months} months ago"),
                    )
                }
                (_, Some(m)) if ctx.weeknight() && m <= 40 => (
                    ReasonKind::Quick,
                    format!("Weeknight-quick: {}", fmt_minutes(m)),
                ),
                (_, Some(m)) if !ctx.weeknight() && m >= 120 => (
                    ReasonKind::Project,
                    format!("A weekend project: {}", fmt_minutes(m)),
                ),
                _ if fresh > 0.0 => (ReasonKind::New, "New in the box, not tried yet".into()),
                _ if season => (ReasonKind::Season, "Good for the season".into()),
                // The card already shows category and cuisine; no reason line
                _ => (ReasonKind::Kicker, String::new()),
            };
            Scored {
                f,
                score,
                reason,
                kind,
            }
        })
        .collect();

    scored.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.f.id.cmp(&b.f.id)));
    scored.truncate(POOL.max(n));
    diversify(scored, n)
}

/// Greedy maximal-marginal-relevance picking with caps: at most one per main protein,
/// one sweet thing, and two per cuisine. Caps relax (cuisine first) when they'd block
/// every remaining recipe.
fn diversify(mut pool: Vec<Scored>, n: usize) -> Vec<Pick> {
    let mut picked: Vec<Scored> = Vec::new();
    while picked.len() < n && !pool.is_empty() {
        let allowed = |s: &Scored, level: u8| {
            let protein_ok = level < 1
                || s.f
                    .main_protein
                    .is_none_or(|p| !picked.iter().any(|q| q.f.main_protein == Some(p)));
            let sweet_ok = level < 2
                || !s.f.bucket.is_sweet()
                || !picked.iter().any(|q| q.f.bucket.is_sweet());
            let cuisine_ok = level < 3
                || s.f
                    .cuisine()
                    .is_none_or(|c| picked.iter().filter(|q| q.f.cuisine() == Some(c)).count() < 2);
            protein_ok && sweet_ok && cuisine_ok
        };
        let best = (0..=3u8).rev().find_map(|level| {
            pool.iter()
                .enumerate()
                .filter(|(_, s)| allowed(s, level))
                .map(|(i, s)| {
                    let overlap = picked
                        .iter()
                        .map(|q| similarity(s.f, q.f))
                        .fold(0.0, f64::max);
                    (i, s.score - MMR_LAMBDA * overlap)
                })
                .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
                .map(|(i, _)| i)
        });
        let Some(i) = best else { break };
        picked.push(pool.remove(i));
    }
    picked
        .into_iter()
        .map(|s| Pick {
            id: s.f.id,
            score: s.score,
            reason: s.reason,
            reason_kind: s.kind,
        })
        .collect()
}

// ─── Random ─────────────────────────────────────────────────────────────────

/// A uniformly random recipe with steps. It avoids `exclude` (seen this session or
/// recently viewed) and anything cooked lately, dropping those rules in that order
/// when they'd leave nothing, and avoids `current` whenever there's an alternative.
pub fn random(
    cands: &[Features],
    hist: &History,
    now: i64,
    exclude: &HashSet<i64>,
    current: Option<i64>,
    rng: &mut impl Rng,
) -> Option<i64> {
    let pool: Vec<&Features> = cands.iter().filter(|f| f.step_count >= 1).collect();
    let pool = if pool.is_empty() {
        cands.iter().collect()
    } else {
        pool
    };
    let not_current = |f: &&&Features| current != Some(f.id);
    let levels: [&dyn Fn(&&&Features) -> bool; 3] = [
        &|f| {
            not_current(f)
                && !exclude.contains(&f.id)
                && !hist.cooked_within(f.id, now, COOLDOWN_DAYS)
        },
        &|f| not_current(f) && !hist.cooked_within(f.id, now, COOLDOWN_DAYS),
        &not_current,
    ];
    for keep in levels {
        let choices: Vec<&&Features> = pool.iter().filter(|f| keep(f)).collect();
        if let Some(f) = choices.choose(rng) {
            return Some(f.id);
        }
    }
    pool.first().map(|f| f.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    // Tuesday 2026-09-22 18:00 UTC
    const TUE_EVENING: i64 = 1_790_100_000;

    fn ctx(now: i64) -> Ctx {
        Ctx {
            now,
            tz_offset_min: 0,
            southern: false,
            seed: 0,
        }
    }

    fn recipe(id: i64, title: &str, category: &str, lines: &[&str]) -> RecipeInput {
        RecipeInput {
            id,
            title: title.into(),
            image: Some("https://img.test/x.jpg".into()),
            prep_time: None,
            cook_time: None,
            total_time: Some("45m".into()),
            category: Some(category.into()),
            cuisine: None,
            ingredient_lines: lines.iter().map(|s| s.to_string()).collect(),
            step_count: 3,
            books: vec![],
            created_at: TUE_EVENING - 400 * DAY,
        }
    }

    fn feat(id: i64, title: &str, category: &str, lines: &[&str]) -> Features {
        Features::new(recipe(id, title, category, lines))
    }

    fn cooked(pairs: &[(i64, i64)]) -> History {
        History::from_events(pairs.iter().map(|(id, at)| (*id, "cooked", *at)))
    }

    #[test]
    fn day_constant_is_a_tuesday_evening() {
        let c = ctx(TUE_EVENING);
        assert_eq!(c.local().weekday(), Weekday::Tue);
        assert_eq!(c.local().hour(), 18);
    }

    #[test]
    fn parses_durations() {
        for (s, m) in [
            ("PT1H30M", Some(90)),
            ("1h 30m", Some(90)),
            ("45 mins", Some(45)),
            ("1 hour 15 minutes", Some(75)),
            ("1 hr 30 min", Some(90)),
            ("90", Some(90)),
            ("P1DT2H", Some(1560)),
            ("overnight", None),
            ("", None),
            ("PT0M", None),
        ] {
            assert_eq!(parse_minutes(s), m, "{s}");
        }
    }

    #[test]
    fn finds_heroes_and_buckets() {
        let (h, p) = heroes(
            &[
                "1 cup chicken stock".into(),
                "2 lb boneless chicken thighs".into(),
                "200g chorizo".into(),
            ],
            Bucket::Main,
        );
        assert_eq!(h, vec!["chicken", "sausage"]);
        assert_eq!(p, Some("chicken"));
        let (h, _) = heroes(
            &["3 eggs".into(), "1 tsp rice vinegar".into()],
            Bucket::Baking,
        );
        assert!(h.is_empty());
        let (h, p) = heroes(
            &["1 can chickpeas".into(), "2 sweet potatoes".into()],
            Bucket::Main,
        );
        assert_eq!(h, vec!["chickpea", "sweet potato"]);
        assert_eq!(p, Some("chickpea"));
        let (h, _) = heroes(&["1 tbsp montreal steak spice".into()], Bucket::Main);
        assert!(h.is_empty());

        assert_eq!(bucket(Some("Main Course"), ""), Bucket::Main);
        assert_eq!(bucket(Some("Dinner"), ""), Bucket::Main);
        assert_eq!(bucket(Some("Cookies, Dessert"), ""), Bucket::Baking);
        assert_eq!(bucket(Some("Dressing"), ""), Bucket::Sauce);
        assert_eq!(bucket(None, "French Onion Soup"), Bucket::Soup);
        assert_eq!(bucket(None, "Fried Rice"), Bucket::Other);
    }

    #[test]
    fn leaves_out_recent_cooks_and_todays_views() {
        let cands: Vec<Features> = (1..=6)
            .map(|i| feat(i, &format!("Dish {i}"), "Main", &["a", "b"]))
            .collect();
        let mut hist = cooked(&[(1, TUE_EVENING - 3 * DAY)]);
        hist.0
            .entry(2)
            .or_default()
            .views
            .push(TUE_EVENING - 2 * 3600);
        let ids: Vec<i64> = rank(&cands, &hist, &ctx(TUE_EVENING), &HashSet::new(), 6)
            .iter()
            .map(|p| p.id)
            .collect();
        assert!(!ids.contains(&1));
        assert!(!ids.contains(&2));
        assert_eq!(ids.len(), 4);

        // With too few left, today's views come back (recent cooks never do)
        let few = &cands[..3];
        let ids: Vec<i64> = rank(few, &hist, &ctx(TUE_EVENING), &HashSet::new(), 4)
            .iter()
            .map(|p| p.id)
            .collect();
        assert!(ids.contains(&2));
        assert!(!ids.contains(&1));
    }

    #[test]
    fn untried_beats_long_ago_beats_recent() {
        let cands: Vec<Features> = (1..=3)
            .map(|i| feat(i, "Same", "Other", &["a", "b"]))
            .collect();
        let hist = cooked(&[(2, TUE_EVENING - 120 * DAY), (3, TUE_EVENING - 20 * DAY)]);
        // Views keep freshness out of it so novelty decides
        let mut hist = hist;
        hist.0
            .entry(1)
            .or_default()
            .views
            .push(TUE_EVENING - 200 * DAY);
        let picks = rank(&cands, &hist, &ctx(TUE_EVENING), &HashSet::new(), 3);
        let score = |id| picks.iter().find(|p| p.id == id).unwrap().score;
        assert!(score(1) > score(2));
        assert!(score(2) > score(3));
    }

    #[test]
    fn likes_what_you_cook() {
        let mut tinga = feat(1, "Chicken Tinga", "Main", &["2 lb chicken", "chipotle"]);
        tinga.cuisines = vec!["mexican".into()];
        tinga.books = vec![7];
        let mut enchiladas = feat(2, "Enchiladas", "Main", &["1 lb chicken", "tortillas"]);
        enchiladas.cuisines = vec!["mexican".into()];
        enchiladas.books = vec![7];
        let other = feat(3, "Lemon Tart", "Dessert", &["lemons", "butter"]);
        let hist = cooked(&[(1, TUE_EVENING - 20 * DAY)]);
        let picks = rank(
            &[tinga, enchiladas, other],
            &hist,
            &ctx(TUE_EVENING),
            &HashSet::new(),
            2,
        );
        assert_eq!(picks[0].id, 2);
        assert_eq!(picks[0].reason_kind, ReasonKind::Like);
        assert_eq!(picks[0].reason, "Because you made Chicken Tinga");
    }

    #[test]
    fn picks_for_variety() {
        let mut cands: Vec<Features> = (1..=5)
            .map(|i| {
                feat(
                    i,
                    &format!("Chicken {i}"),
                    "Main",
                    &["1 lb chicken", "salt"],
                )
            })
            .collect();
        cands.push(feat(6, "Salmon", "Main", &["salmon", "lemon"]));
        cands.push(feat(7, "Tofu", "Main", &["tofu", "soy"]));
        cands.push(feat(8, "Cake", "Dessert", &["flour", "sugar"]));
        cands.push(feat(9, "Cookies", "Baking", &["flour", "butter"]));
        let picks = rank(
            &cands,
            &History::default(),
            &ctx(TUE_EVENING),
            &HashSet::new(),
            4,
        );
        let chicken = picks.iter().filter(|p| p.id <= 5).count();
        let sweet = picks.iter().filter(|p| p.id >= 8).count();
        assert_eq!(picks.len(), 4);
        assert!(chicken <= 1, "{picks:?}");
        assert!(sweet <= 1, "{picks:?}");
    }

    #[test]
    fn fits_the_time_of_day() {
        let mut quick = feat(1, "Quick", "Main", &["a", "b"]);
        quick.minutes = Some(30);
        let mut slow = feat(2, "Slow", "Main", &["a", "b"]);
        slow.minutes = Some(120);
        let cands = [quick, slow];
        let picks = rank(
            &cands,
            &History::default(),
            &ctx(TUE_EVENING),
            &HashSet::new(),
            2,
        );
        let score = |id| picks.iter().find(|p| p.id == id).unwrap().score;
        assert!(score(1) > score(2));
        assert_eq!(
            picks.iter().find(|p| p.id == 1).unwrap().reason_kind,
            ReasonKind::Quick
        );

        let breakfast = feat(3, "Pancakes", "Breakfast", &["flour", "milk"]);
        let main = feat(4, "Stew", "Main", &["beef", "carrots"]);
        let morning = TUE_EVENING - 10 * 3600; // 08:00
        let cands = [breakfast, main];
        let picks = rank(
            &cands,
            &History::default(),
            &ctx(morning),
            &HashSet::new(),
            2,
        );
        assert_eq!(picks[0].id, 3);

        // A +10h zone turns Tuesday 18:00 UTC into Wednesday 04:00
        let east = Ctx {
            tz_offset_min: 600,
            ..ctx(TUE_EVENING)
        };
        assert_eq!(east.local().weekday(), Weekday::Wed);
        assert_eq!(east.local_day(), ctx(TUE_EVENING).local_day() + 1);
    }

    #[test]
    fn seasons_mirror_in_the_south() {
        let jan = 1_768_089_600; // 2026-01-11
        let north = ctx(jan);
        let south = Ctx {
            southern: true,
            ..north
        };
        assert_eq!(north.season(), Season::Winter);
        assert_eq!(south.season(), Season::Summer);
        let soup = feat(1, "Onion Soup", "Soup", &["onions", "stock"]);
        assert!(in_season(&soup, &north));
        assert!(!in_season(&soup, &south));
    }

    #[test]
    fn is_deterministic_per_seed_and_respects_exclude() {
        let cands: Vec<Features> = (1..=12)
            .map(|i| feat(i, &format!("Dish {i}"), "Other", &["a", "b"]))
            .collect();
        let hist = History::default();
        let ids = |c: Ctx, ex: &HashSet<i64>| -> Vec<i64> {
            rank(&cands, &hist, &c, ex, 4)
                .iter()
                .map(|p| p.id)
                .collect()
        };
        let none = HashSet::new();
        assert_eq!(ids(ctx(TUE_EVENING), &none), ids(ctx(TUE_EVENING), &none));
        let shuffled = Ctx {
            seed: 1,
            ..ctx(TUE_EVENING)
        };
        assert_ne!(ids(ctx(TUE_EVENING), &none), ids(shuffled, &none));
        let first: HashSet<i64> = ids(ctx(TUE_EVENING), &none).into_iter().collect();
        let next = ids(ctx(TUE_EVENING), &first);
        assert!(next.iter().all(|id| !first.contains(id)));
    }

    #[test]
    fn small_libraries_and_random() {
        let hist = History::default();
        assert!(rank(&[], &hist, &ctx(TUE_EVENING), &HashSet::new(), 4).is_empty());
        let one = [feat(1, "Only", "Main", &["a", "b"])];
        assert_eq!(
            rank(&one, &hist, &ctx(TUE_EVENING), &HashSet::new(), 4).len(),
            1
        );
        let mut rng = StdRng::seed_from_u64(7);
        assert_eq!(
            random(&one, &hist, TUE_EVENING, &HashSet::new(), None, &mut rng),
            Some(1)
        );
        // Only one recipe: it's returned even though it's the current one
        assert_eq!(
            random(&one, &hist, TUE_EVENING, &HashSet::new(), Some(1), &mut rng),
            Some(1)
        );

        let three: Vec<Features> = (1..=3).map(|i| feat(i, "D", "Main", &["a", "b"])).collect();
        let everything: HashSet<i64> = [1, 2, 3].into();
        for _ in 0..20 {
            let id = random(&three, &hist, TUE_EVENING, &everything, Some(2), &mut rng).unwrap();
            assert_ne!(id, 2);
        }
        let seen: HashSet<i64> = [1, 2].into();
        for _ in 0..20 {
            assert_eq!(
                random(&three, &hist, TUE_EVENING, &seen, None, &mut rng),
                Some(3)
            );
        }
    }
}
