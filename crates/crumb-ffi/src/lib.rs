//! crumb-core for the native apps (Kotlin on Android, Swift on iOS): scaling, mise en place,
//! step timers, step ingredients, durations, text export, photo URLs, server addresses,
//! error wording and the sunrise theme all come from here, so they match the web and
//! desktop apps exactly. Plain functions and records only; recipes cross as the API's JSON.

use crumb_core::{
    add, books, categories, checks, client, duration, editor, format, fractions, home, ingredients,
    model, prep, recipe_page, source, sun,
};

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CoreError {
    #[error("{0}")]
    BadRecipe(String),
}

/// A recipe from an app's JSON. Apps may leave out what they never show (the timestamps, a
/// "manual" source, blank lists), so those get neutral defaults instead of failing the call.
fn recipe(json: &str) -> Result<model::Recipe, CoreError> {
    let bad = |e: serde_json::Error| CoreError::BadRecipe(e.to_string());
    let mut value: serde_json::Value = serde_json::from_str(json).map_err(bad)?;
    if let Some(fields) = value.as_object_mut() {
        let defaults = [
            ("id", serde_json::json!(0)),
            ("source", serde_json::json!("manual")),
            ("ingredients", serde_json::json!([])),
            ("instructions", serde_json::json!([])),
            ("createdAt", serde_json::json!("1970-01-01T00:00:00Z")),
            ("updatedAt", serde_json::json!("1970-01-01T00:00:00Z")),
        ];
        for (key, default) in defaults {
            if fields.get(key).is_none_or(serde_json::Value::is_null) {
                fields.insert(key.into(), default);
            }
        }
    }
    serde_json::from_value(value).map_err(bad)
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

impl From<Vessel> for ingredients::Vessel {
    fn from(v: Vessel) -> Self {
        match v {
            Vessel::Pinch => Self::Pinch,
            Vessel::Ramekin => Self::Ramekin,
            Vessel::Small => Self::Small,
            Vessel::Medium => Self::Medium,
            Vessel::Large => Self::Large,
            Vessel::Board => Self::Board,
            Vessel::Jar => Self::Jar,
        }
    }
}

impl From<ingredients::MiseItem> for MiseItem {
    fn from(m: ingredients::MiseItem) -> Self {
        Self {
            raw: m.raw,
            quantity: m.quantity,
            quantity_max: m.quantity_max,
            unit: m.unit,
            name: m.name,
            prep: m.prep,
            vessel: m.vessel.into(),
            volume: m.volume,
            task: m.task,
        }
    }
}

impl From<MiseItem> for ingredients::MiseItem {
    fn from(m: MiseItem) -> Self {
        Self {
            raw: m.raw,
            quantity: m.quantity,
            quantity_max: m.quantity_max,
            unit: m.unit,
            name: m.name,
            prep: m.prep,
            vessel: m.vessel.into(),
            volume: m.volume,
            task: m.task,
        }
    }
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
        .map(MiseItem::from)
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

/// Whether a link is a cooking video an import watches (TikTok, an Instagram reel, a YouTube
/// video or Short): the server answers its import with a job to poll.
#[uniffi::export]
pub fn is_video_url(url: String) -> bool {
    source::is_video_url(&url)
}

/// How a recipe page plays its video (see `crumb_core::embed::VideoEmbed`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct VideoEmbed {
    pub provider: String,
    pub label: String,
    pub embed_url: String,
    pub watch_url: String,
    pub thumbnail: Option<String>,
    pub vertical: bool,
}

/// How to play the video at a recipe's `video` link in place, or null when it's on a site
/// Crumb can't embed (link to it instead). The API sends the same as `videoEmbed`.
#[uniffi::export]
pub fn video_embed(url: String) -> Option<VideoEmbed> {
    crumb_core::embed::video_embed(&url).map(|e| VideoEmbed {
        provider: e.provider,
        label: e.label,
        embed_url: e.embed_url,
        watch_url: e.watch_url,
        thumbnail: e.thumbnail,
        vertical: e.vertical,
    })
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

/// Today's sunrise and sunset, or the sun never setting or rising.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum SunTimes {
    /// Unix milliseconds.
    RiseSet {
        rise: f64,
        set: f64,
    },
    PolarDay,
    PolarNight,
}

/// Sunrise and sunset for the day containing `now_ms` (the More page's "Dark from … until …").
#[uniffi::export]
pub fn sun_times(now_ms: f64, location: SunLocation) -> SunTimes {
    match sun::sun_times(now_ms, location.lat, location.lng) {
        sun::SunTimes::RiseSet { rise, set } => SunTimes::RiseSet { rise, set },
        sun::SunTimes::PolarDay => SunTimes::PolarDay,
        sun::SunTimes::PolarNight => SunTimes::PolarNight,
    }
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

/// A line quoted in a sentence (“…”), shortened when longer than `max` characters.
#[uniffi::export]
pub fn quote(text: String, max: u32) -> String {
    checks::quote(&text, max as usize)
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

// ─── The Add box ───────────────────────────────────────────────────────────

/// What the Add box thinks it was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AddMode {
    /// Nothing typed yet.
    Auto,
    /// One or more links.
    Link,
    /// A recipe's text.
    Text,
    /// A short name: open a new recipe with it as the title.
    Scratch,
}

impl From<add::AddMode> for AddMode {
    fn from(m: add::AddMode) -> Self {
        match m {
            add::AddMode::Auto => Self::Auto,
            add::AddMode::Link => Self::Link,
            add::AddMode::Text => Self::Text,
            add::AddMode::Scratch => Self::Scratch,
        }
    }
}

/// The mode and a short summary for the box's footer ("bbcgoodfood.com", "3 links").
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Detected {
    pub mode: AddMode,
    pub summary: String,
}

/// What was pasted into the Add box, and a short summary of it (core's `add::detect`).
#[uniffi::export]
pub fn detect_add(raw: String) -> Detected {
    let d = add::detect(&raw);
    Detected {
        mode: d.mode.into(),
        summary: d.summary,
    }
}

/// Every http(s) link in a paste, in order and without repeats, for "Lots of links".
#[uniffi::export]
pub fn links_in(raw: String) -> Vec<String> {
    add::links_in(&raw)
}

/// Every http(s) link anywhere in some text, without trailing punctuation, in order and
/// without repeats.
#[uniffi::export]
pub fn links_in_text(raw: String) -> Vec<String> {
    add::links_in_text(&raw)
}

/// "1st", "2nd", "3rd", "11th".
#[uniffi::export]
pub fn ordinal(n: u32) -> String {
    add::ordinal(n)
}

/// What the Add box says while a video waits or is watched; `status` is the job's `queued`
/// or `running`, `position` is 1 for next.
#[uniffi::export]
pub fn job_progress(status: String, position: Option<u32>) -> String {
    add::job_progress(&status, position)
}

// ─── The shelf ─────────────────────────────────────────────────────────────

/// What the shelf needs to know about a book.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ShelfBook {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
    pub recipe_count: i64,
}

impl From<ShelfBook> for books::ShelfBook {
    fn from(b: ShelfBook) -> Self {
        Self {
            id: b.id,
            name: b.name,
            color: b.color,
            recipe_count: b.recipe_count,
        }
    }
}

/// One cover colour's look, as hex colours.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BookLook {
    /// Cover cloth.
    pub cloth: String,
    /// Darker cloth for the hidden faces.
    pub shade: String,
    /// Title text on the cloth.
    pub foil: String,
    /// A thin inset edge for a cover that would vanish against a light page (cream only),
    /// drawn at `book_edge_alpha()`.
    pub edge: Option<String>,
    /// Contrasting colours for the optional spine bands.
    pub bands: Vec<String>,
}

/// A book lying flat.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct BookSize {
    /// Px: thicker books hold more recipes.
    pub thickness: u32,
    /// A fraction of the tower's length.
    pub length: f64,
    /// Px the spine needs for the whole title.
    pub title: u32,
}

/// How untidily a book sits in its stack.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct BookLean {
    /// Degrees, one decimal.
    pub tilt: f64,
    /// Px sideways.
    pub nudge: i32,
}

/// Any stored colour name as one of the six current ones (unknown names are `tile`), unlike
/// `book_color`, which returns null for a name it doesn't know. Core's `books::book_color`.
#[uniffi::export]
pub fn book_cover_color(color: Option<String>) -> String {
    books::book_color(color.as_deref()).to_string()
}

/// The look of a cover colour (core's `books::book_look`).
#[uniffi::export]
pub fn book_look(color: Option<String>) -> BookLook {
    let l = books::book_look(color.as_deref());
    BookLook {
        cloth: l.cloth.to_string(),
        shade: l.shade.to_string(),
        foil: l.foil.to_string(),
        edge: l.edge.map(str::to_string),
        bands: l.bands.iter().map(|b| b.to_string()).collect(),
    }
}

/// The opacity of a cover's inset edge.
#[uniffi::export]
pub fn book_edge_alpha() -> f64 {
    books::EDGE_ALPHA
}

/// A pseudo-random number in [0, 1) from an id, so a book keeps its size.
#[uniffi::export]
pub fn seeded(id: i64, salt: i64) -> f64 {
    books::seeded(id, salt)
}

/// How thick, long and wide a book's spine is lying flat on the shelf.
#[uniffi::export]
pub fn book_size(book: ShelfBook) -> BookSize {
    let s = books::book_size(&book.into());
    BookSize {
        thickness: s.thickness,
        length: s.length,
        title: s.title,
    }
}

/// Books at the foot of a tower sit flatter.
#[uniffi::export]
pub fn book_lean(book: ShelfBook, at_foot: bool) -> BookLean {
    let l = books::book_lean(&book.into(), at_foot);
    BookLean {
        tilt: l.tilt,
        nudge: l.nudge,
    }
}

/// The spine's two bands' colour, or null: about half the books get them.
#[uniffi::export]
pub fn spine_band(book: ShelfBook) -> Option<String> {
    books::spine_band(&book.into()).map(str::to_string)
}

/// Splits books into `towers` stacks of roughly equal height, keeping their order. Each
/// tower lists indexes into `books`, top to bottom.
#[uniffi::export]
pub fn stack_books(books: Vec<ShelfBook>, towers: u32) -> Vec<Vec<u32>> {
    let core: Vec<books::ShelfBook> = books.into_iter().map(Into::into).collect();
    books::stack_books(&core, towers as usize)
        .into_iter()
        .map(|tower| tower.into_iter().map(|i| i as u32).collect())
        .collect()
}

// ─── Home ──────────────────────────────────────────────────────────────────

/// The handwritten greeting for a local hour (0–23).
#[uniffi::export]
pub fn greeting(hour: u32) -> String {
    home::greeting(hour).to_string()
}

/// "Today", "Yesterday", a weekday within the last week, else "Sep 20". Times are Unix ms;
/// `offset_minutes` is the viewer's UTC offset, east positive.
#[uniffi::export]
pub fn day_label(ms: i64, now_ms: i64, offset_minutes: i32) -> String {
    home::day_label(ms, now_ms, offset_minutes)
}

/// A full date as the web prints one ("Mar 11, 2025").
#[uniffi::export]
pub fn date_label(ms: i64, offset_minutes: i32) -> String {
    home::date_label(ms, offset_minutes)
}

/// How long a recipe has left in the trash ("12 days left", "Goes for good today").
#[uniffi::export]
pub fn trash_left(purge_ms: i64, now_ms: i64) -> String {
    home::trash_left(purge_ms, now_ms)
}

/// How a recipe got into the box ("from a link"), from its `source`; null when unknown.
#[uniffi::export]
pub fn source_label(source: String) -> Option<String> {
    home::source_label(&source).map(str::to_string)
}

/// "Today · from a link" under a new recipe.
#[uniffi::export]
pub fn fresh_meta(created_ms: i64, source: String, now_ms: i64, offset_minutes: i32) -> String {
    home::fresh_meta(created_ms, &source, now_ms, offset_minutes)
}

/// "Viewed yesterday" under a recently viewed recipe, or "Viewed recently" without a time.
#[uniffi::export]
pub fn viewed_line(viewed_ms: Option<i64>, now_ms: i64, offset_minutes: i32) -> String {
    home::viewed_line(viewed_ms, now_ms, offset_minutes)
}

/// Where a cook left off, both counted from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct CookProgress {
    pub step: i64,
    pub of: i64,
}

/// Where a cook left off for "Pick up where you left off"; null when not started or on the
/// last step.
#[uniffi::export]
pub fn cook_progress(step_index: Option<i64>, steps: Option<i64>) -> Option<CookProgress> {
    home::cook_progress(step_index, steps).map(|(step, of)| CookProgress { step, of })
}

// ─── The recipe editor ─────────────────────────────────────────────────────

/// One section as the editor holds it: a name and one item per line.
#[derive(Debug, Clone, Default, PartialEq, Eq, uniffi::Record)]
pub struct DraftSection {
    pub name: String,
    pub text: String,
}

impl From<editor::DraftSection> for DraftSection {
    fn from(s: editor::DraftSection) -> Self {
        Self {
            name: s.name,
            text: s.text,
        }
    }
}

impl From<DraftSection> for editor::DraftSection {
    fn from(s: DraftSection) -> Self {
        Self {
            name: s.name,
            text: s.text,
        }
    }
}

/// A section of ingredients or steps as a recipe holds it.
#[derive(Debug, Clone, Default, PartialEq, Eq, uniffi::Record)]
pub struct Section {
    pub name: Option<String>,
    pub items: Vec<String>,
}

/// The whole editor form as text.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RecipeDraft {
    pub title: String,
    pub description: String,
    pub author: String,
    pub prep_time: String,
    pub cook_time: String,
    pub freeze_time: String,
    pub total_time: String,
    pub recipe_yield: String,
    pub recipe_category: String,
    pub recipe_cuisine: String,
    pub url: String,
    pub image: String,
    pub video: String,
    pub notes: String,
    pub nutrition: String,
    pub ingredients: Vec<DraftSection>,
    pub instructions: Vec<DraftSection>,
}

impl From<editor::RecipeDraft> for RecipeDraft {
    fn from(d: editor::RecipeDraft) -> Self {
        Self {
            title: d.title,
            description: d.description,
            author: d.author,
            prep_time: d.prep_time,
            cook_time: d.cook_time,
            freeze_time: d.freeze_time,
            total_time: d.total_time,
            recipe_yield: d.recipe_yield,
            recipe_category: d.recipe_category,
            recipe_cuisine: d.recipe_cuisine,
            url: d.url,
            image: d.image,
            video: d.video,
            notes: d.notes,
            nutrition: d.nutrition,
            ingredients: d.ingredients.into_iter().map(Into::into).collect(),
            instructions: d.instructions.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<RecipeDraft> for editor::RecipeDraft {
    fn from(d: RecipeDraft) -> Self {
        Self {
            title: d.title,
            description: d.description,
            author: d.author,
            prep_time: d.prep_time,
            cook_time: d.cook_time,
            freeze_time: d.freeze_time,
            total_time: d.total_time,
            recipe_yield: d.recipe_yield,
            recipe_category: d.recipe_category,
            recipe_cuisine: d.recipe_cuisine,
            url: d.url,
            image: d.image,
            video: d.video,
            notes: d.notes,
            nutrition: d.nutrition,
            ingredients: d.ingredients.into_iter().map(Into::into).collect(),
            instructions: d.instructions.into_iter().map(Into::into).collect(),
        }
    }
}

/// Which of the two section lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SectionKind {
    Ingredients,
    Instructions,
}

impl From<SectionKind> for editor::SectionKind {
    fn from(k: SectionKind) -> Self {
        match k {
            SectionKind::Ingredients => Self::Ingredients,
            SectionKind::Instructions => Self::Instructions,
        }
    }
}

/// A one-tap fix: the button's label and the form it leaves.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct EditorFix {
    pub label: String,
    pub draft: RecipeDraft,
}

/// One of the editor's time, yield and label fields.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MetaField {
    pub key: String,
    pub label: String,
    pub placeholder: String,
}

/// The editor's time, yield and label fields, in the web's order.
#[uniffi::export]
pub fn meta_fields() -> Vec<MetaField> {
    editor::META_FIELDS
        .iter()
        .map(|(key, label, placeholder)| MetaField {
            key: key.to_string(),
            label: label.to_string(),
            placeholder: placeholder.to_string(),
        })
        .collect()
}

/// Sections as drafts, or one blank section when there are none.
#[uniffi::export]
pub fn draft_sections(sections: Vec<Section>) -> Vec<DraftSection> {
    let core: Vec<model::Section> = sections
        .into_iter()
        .map(|s| model::Section {
            name: s.name,
            items: s.items,
        })
        .collect();
    editor::draft_sections(&core)
        .into_iter()
        .map(Into::into)
        .collect()
}

/// Drafts as sections: bullets and numbering gone, blank lines dropped, empty sections gone.
#[uniffi::export]
pub fn section_list(sections: Vec<DraftSection>) -> Vec<Section> {
    let core: Vec<editor::DraftSection> = sections.into_iter().map(Into::into).collect();
    editor::section_list(&core)
        .into_iter()
        .map(|s| Section {
            name: s.name,
            items: s.items,
        })
        .collect()
}

/// A text field's value trimmed, or null when it's blank (core's `editor::nullable`).
#[uniffi::export]
pub fn nullable_text(value: String) -> Option<String> {
    editor::nullable(&value)
}

/// Nutrition (a recipe's JSON object, or null) as "key: value" lines.
#[uniffi::export]
pub fn nutrition_text(nutrition_json: Option<String>) -> Result<String, CoreError> {
    let value = match nutrition_json {
        Some(json) => Some(
            serde_json::from_str::<serde_json::Value>(&json)
                .map_err(|e| CoreError::BadRecipe(e.to_string()))?,
        ),
        None => None,
    };
    Ok(editor::nutrition_text(value.as_ref()))
}

/// "calories: 320" lines as a nutrition JSON object.
#[uniffi::export]
pub fn parse_nutrition(text: String) -> Result<String, CoreError> {
    serde_json::to_string(&editor::parse_nutrition(&text))
        .map_err(|e| CoreError::BadRecipe(e.to_string()))
}

/// A new recipe's form, with the title "From scratch" was given (`RecipeDraft::new`).
#[uniffi::export]
pub fn recipe_draft_new(title: String) -> RecipeDraft {
    editor::RecipeDraft::new(&title).into()
}

/// A saved recipe as a form (`RecipeDraft::from_recipe`).
#[uniffi::export]
pub fn recipe_draft_from_recipe(recipe_json: String) -> Result<RecipeDraft, CoreError> {
    Ok(editor::RecipeDraft::from_recipe(&recipe(&recipe_json)?).into())
}

/// Why the form can't be saved yet, as the editor says it; null when it can.
#[uniffi::export]
pub fn recipe_draft_problem(draft: RecipeDraft) -> Option<String> {
    editor::RecipeDraft::from(draft)
        .problem()
        .map(str::to_string)
}

/// The form as the create/update JSON body, camelCase as the API takes it (blank fields
/// null, never ""); covers `RecipeDraft::to_fields` too.
#[uniffi::export]
pub fn recipe_draft_to_json(draft: RecipeDraft) -> Result<String, CoreError> {
    serde_json::to_string(&editor::RecipeDraft::from(draft).to_json())
        .map_err(|e| CoreError::BadRecipe(e.to_string()))
}

/// A category from before the fixed list; null for a current one or none.
#[uniffi::export]
pub fn legacy_category(category: String) -> Option<String> {
    editor::legacy_category(&category)
}

/// Whether a review flag belongs under this section.
#[uniffi::export]
pub fn flag_is_here(
    kind: SectionKind,
    section: DraftSection,
    field: String,
    state: String,
    item_text: String,
) -> bool {
    editor::flag_is_here(kind.into(), &section.into(), &field, &state, &item_text)
}

/// Whether a review flag on the photo still applies: the draft still has that link.
#[uniffi::export]
pub fn photo_flag_is_here(
    draft: RecipeDraft,
    field: String,
    state: String,
    item_text: String,
) -> bool {
    editor::photo_flag_is_here(&draft.into(), &field, &state, &item_text)
}

/// The fix offered for a flagged line in section `index` of the list, or null when the line
/// is gone or nothing fits.
#[uniffi::export]
pub fn fix_for(
    draft: RecipeDraft,
    list: SectionKind,
    index: u32,
    kind: String,
    item_text: String,
) -> Option<EditorFix> {
    editor::fix_for(
        &draft.into(),
        list.into(),
        index as usize,
        &kind,
        &item_text,
    )
    .map(|f| EditorFix {
        label: f.label.to_string(),
        draft: f.draft.into(),
    })
}

// ─── Prep ──────────────────────────────────────────────────────────────────

/// One of the prep page's three groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PrepGroupKind {
    /// Chop & prep: everything bound for the board.
    Chop,
    /// Measure into bowls, biggest first.
    Measure,
    /// Keep within reach: seasoning and extras.
    Reach,
}

impl From<prep::PrepGroupKind> for PrepGroupKind {
    fn from(k: prep::PrepGroupKind) -> Self {
        match k {
            prep::PrepGroupKind::Chop => Self::Chop,
            prep::PrepGroupKind::Measure => Self::Measure,
            prep::PrepGroupKind::Reach => Self::Reach,
        }
    }
}

impl From<PrepGroupKind> for prep::PrepGroupKind {
    fn from(k: PrepGroupKind) -> Self {
        match k {
            PrepGroupKind::Chop => Self::Chop,
            PrepGroupKind::Measure => Self::Measure,
            PrepGroupKind::Reach => Self::Reach,
        }
    }
}

/// An ingredient in a prep group; `key` is its index in `mise_en_place`'s list, which is
/// what the page remembers as ready.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PrepItem {
    pub key: u32,
    pub item: MiseItem,
}

/// A group and its items.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PrepGroup {
    pub kind: PrepGroupKind,
    pub items: Vec<PrepItem>,
}

/// How many of a vessel to get out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct VesselCount {
    pub vessel: Vessel,
    pub count: u32,
}

/// A group's heading ("Chop & prep").
#[uniffi::export]
pub fn prep_group_title(kind: PrepGroupKind) -> String {
    prep::PrepGroupKind::from(kind).title().to_string()
}

/// A group's hint under its heading.
#[uniffi::export]
pub fn prep_group_hint(kind: PrepGroupKind) -> String {
    prep::PrepGroupKind::from(kind).hint().to_string()
}

/// The prep page's groups for a recipe's ingredient lines, leaving out empty ones.
#[uniffi::export]
pub fn prep_groups(lines: Vec<String>) -> Vec<PrepGroup> {
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    prep::prep_groups(&refs)
        .into_iter()
        .map(|g| PrepGroup {
            kind: g.kind.into(),
            items: g
                .items
                .into_iter()
                .map(|(key, item)| PrepItem {
                    key: key as u32,
                    item: item.into(),
                })
                .collect(),
        })
        .collect()
}

/// "Large bowl", "Pinch bowl", "Board", "On hand".
#[uniffi::export]
pub fn vessel_label(vessel: Vessel) -> String {
    prep::vessel_label(vessel.into()).to_string()
}

/// How much to measure at `scale`: "1½–2 cups", "a pinch", or "" when there's no amount.
#[uniffi::export]
pub fn prep_amount(item: MiseItem, scale: f64) -> String {
    prep::prep_amount(&item.into(), scale)
}

/// What to get out, in first-seen order: each vessel and how many.
#[uniffi::export]
pub fn vessel_counts(items: Vec<MiseItem>) -> Vec<VesselCount> {
    let core: Vec<ingredients::MiseItem> = items.into_iter().map(Into::into).collect();
    prep::vessel_counts(&core)
        .into_iter()
        .map(|(vessel, n)| VesselCount {
            vessel: vessel.into(),
            count: n as u32,
        })
        .collect()
}

/// "2 × small bowls" for the Get out line.
#[uniffi::export]
pub fn vessel_count_label(vessel: Vessel, count: u32) -> String {
    prep::vessel_count_label(vessel.into(), count as usize)
}

/// A plausible colour for what's in the bowl, from the ingredient's name.
#[uniffi::export]
pub fn ingredient_color(name: String) -> String {
    prep::ingredient_color(&name).to_string()
}

// ─── The recipe page ───────────────────────────────────────────────────────

/// A nutrition key as a label: "Carbs" for `carbohydrateContent`.
#[uniffi::export]
pub fn nutrition_label(key: String) -> String {
    recipe_page::nutrition_label(&key)
}

/// "Wee Chef tidied 2 things".
#[uniffi::export]
pub fn tidied_title(fixed: u32) -> String {
    recipe_page::tidied_title(fixed)
}

/// "3 lines might need a look".
#[uniffi::export]
pub fn look_title(review: u32) -> String {
    recipe_page::look_title(review)
}

/// How many things one fixed flag counts for, from the flag's `detail` JSON object.
#[uniffi::export]
pub fn fix_weight(detail_json: String) -> Result<u32, CoreError> {
    let detail: serde_json::Value =
        serde_json::from_str(&detail_json).map_err(|e| CoreError::BadRecipe(e.to_string()))?;
    Ok(recipe_page::fix_weight(&detail))
}

/// The toast when a check someone asked for is done.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CheckToast {
    pub title: String,
    pub description: Option<String>,
}

/// The toast for a finished check; `status` is the check's status, `fixed` counts only this
/// check's fixes (by `fix_weight`) and `review` leaves out the photo's flag.
#[uniffi::export]
pub fn check_done_toast(status: String, fixed: u32, review: u32) -> CheckToast {
    let (title, description) = recipe_page::check_done_toast(&status, fixed, review);
    CheckToast { title, description }
}

/// How many suggestions a check has for one recipe field.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FieldCount {
    pub field: String,
    pub count: u32,
}

/// What a recipe in the review list has to look at, most first: "2 steps · 1 ingredient".
#[uniffi::export]
pub fn review_summary(fields: Vec<FieldCount>) -> String {
    let pairs: Vec<(String, u32)> = fields.into_iter().map(|f| (f.field, f.count)).collect();
    checks::review_summary(&pairs)
}

/// "1 line", "3 lines".
#[uniffi::export]
pub fn plural(n: u32, one: String) -> String {
    checks::plural(n, &one)
}

/// A session's user agent as "Firefox on Linux", roughly; null or blank is "Unknown device".
#[uniffi::export]
pub fn device_name(agent: Option<String>) -> String {
    client::device_name(agent.as_deref())
}

#[cfg(test)]
mod tests {

    #[test]
    fn cook_steps_accept_a_recipe_without_timestamps() {
        let json =
            r#"{"title":"Toast","instructions":[{"items":["Toast the bread.","Butter it."]}]}"#;
        let steps = cook_steps(json.into()).unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[1].text, "Butter it.");
    }

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

    #[test]
    fn app_logic_wrappers_convert_both_ways() {
        let d = detect_add("Pancakes\n200g flour\n2 eggs\nMix.".into());
        assert_eq!(d.mode, AddMode::Text);
        assert_eq!(d.summary, "2 ingredients");
        assert_eq!(job_progress("queued".into(), Some(2)), "Queued (2nd)…");

        let books: Vec<ShelfBook> = (1..=5)
            .map(|id| ShelfBook {
                id,
                name: "Book".into(),
                color: None,
                recipe_count: id * 3,
            })
            .collect();
        let towers = stack_books(books.clone(), 3);
        assert_eq!(towers.iter().flatten().count(), 5);
        assert_eq!(book_size(books[0].clone()).thickness, 40);
        assert_eq!(book_cover_color(Some("purple".into())), "tile");
        assert_eq!(book_look(Some("cream".into())).bands.len(), 2);

        assert_eq!(date_label(0, 0), "Jan 1, 1970");
        assert_eq!(trash_left(86_400_000 * 3, 0), "3 days left");
        assert_eq!(
            cook_progress(Some(2), Some(8)),
            Some(CookProgress { step: 3, of: 8 })
        );
    }

    #[test]
    fn editor_drafts_round_trip() {
        let draft = recipe_draft_new("Soup".into());
        assert_eq!(recipe_draft_problem(draft.clone()), None);
        let mut blank = draft.clone();
        blank.title = " ".into();
        assert!(recipe_draft_problem(blank).is_some());

        let mut with_steps = draft;
        with_steps.instructions = vec![DraftSection {
            name: String::new(),
            text: "Boil.\nFluffy".into(),
        }];
        let fix = fix_for(
            with_steps.clone(),
            SectionKind::Instructions,
            0,
            "fragment".into(),
            "Fluffy".into(),
        )
        .unwrap();
        assert_eq!(fix.label, "Join with the step above");
        assert_eq!(fix.draft.instructions[0].text, "Boil. Fluffy");

        let json: serde_json::Value =
            serde_json::from_str(&recipe_draft_to_json(with_steps).unwrap()).unwrap();
        assert_eq!(json["instructions"][0]["items"][1], "Fluffy");

        let nutrition = parse_nutrition("calories: 320\nnope".into()).unwrap();
        assert_eq!(nutrition, r#"{"calories":"320"}"#);
        assert_eq!(nutrition_text(Some(nutrition)).unwrap(), "calories: 320");
        assert_eq!(nutrition_text(None).unwrap(), "");
        assert!(nutrition_text(Some("{".into())).is_err());

        let sections = section_list(vec![DraftSection {
            name: "Sauce".into(),
            text: "- 1 onion\n\n2) garlic".into(),
        }]);
        assert_eq!(sections[0].items, vec!["1 onion", "garlic"]);
        assert_eq!(draft_sections(vec![]).len(), 1);
        assert_eq!(meta_fields()[0].key, "prepTime");
    }

    #[test]
    fn prep_and_page_wrappers_map_tuples_and_json() {
        let groups = prep_groups(vec!["2 onions, diced".into(), "500 g flour".into()]);
        assert_eq!(groups[0].kind, PrepGroupKind::Chop);
        assert_eq!(groups[0].items[0].key, 0);
        assert_eq!(groups[1].items[0].key, 1);
        let items: Vec<MiseItem> = groups
            .iter()
            .flat_map(|g| g.items.iter().map(|i| i.item.clone()))
            .collect();
        let counts = vessel_counts(items.clone());
        assert_eq!(counts[0].vessel, Vessel::Board);
        assert_eq!(counts[0].count, 1);
        assert_eq!(vessel_count_label(Vessel::Small, 2), "2 × small bowls");
        assert_eq!(prep_amount(items[1].clone(), 2.0), "1000 g");

        assert_eq!(fix_weight(r#"{"fix":"tidy","count":4}"#.into()).unwrap(), 4);
        assert!(fix_weight("nope".into()).is_err());
        let toast = check_done_toast("done".into(), 2, 1);
        assert_eq!(toast.title, "Wee Chef tidied 2 things");
        assert_eq!(
            toast.description.as_deref(),
            Some("1 line might need a look")
        );
        assert_eq!(
            review_summary(vec![
                FieldCount {
                    field: "ingredients".into(),
                    count: 1
                },
                FieldCount {
                    field: "instructions".into(),
                    count: 2
                },
            ]),
            "2 steps · 1 ingredient"
        );
        assert_eq!(device_name(None), "Unknown device");
    }
}
