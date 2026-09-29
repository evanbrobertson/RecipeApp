//! `crumb-core` for QML: every piece of wording and arithmetic the web works out in the
//! browser, as one `Core` singleton, so no page re-implements it in JavaScript.
//!
//! Structured values cross as JSON strings (QML reads them with `JSON.parse`); a missing
//! value is "" or, for numbers, a negative. The functions below the bridge are plain Rust
//! over those strings, which is what the tests call.

use crumb_core::model::Recipe;
use crumb_core::{add, books, checks, client, editor, embed, format, home, ingredients, prep};
use crumb_core::{categories, recipe_page, trash};
use cxx_qt_lib::QString;
use serde::Serialize;
use serde_json::{Value, json};

fn out(value: impl Serialize) -> String {
    serde_json::to_string(&value).unwrap_or_else(|_| "null".into())
}

fn parse<T: serde::de::DeserializeOwned>(json: &str) -> Option<T> {
    serde_json::from_str(json).ok()
}

fn some(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

fn opt_ms(ms: f64) -> Option<i64> {
    (ms.is_finite() && ms > 0.0).then_some(ms as i64)
}

fn lines(json: &str) -> Vec<String> {
    parse(json).unwrap_or_default()
}

/// A shelf book from `{id, name, color, recipeCount}`.
fn shelf_book(json: &str) -> Option<books::ShelfBook> {
    parse(json)
}

pub fn book_json(color: &str) -> String {
    let look = books::book_look(some(color));
    out(json!({
        "name": books::book_color(some(color)),
        "cloth": look.cloth,
        "shade": look.shade,
        "foil": look.foil,
        "edge": look.edge,
        "edgeAlpha": books::EDGE_ALPHA,
        "bands": look.bands,
    }))
}

/// A book's size, lean and spine band together: `{thickness, length, title, tilt, nudge, band}`.
pub fn book_shape(book: &str, at_foot: bool) -> String {
    let Some(book) = shelf_book(book) else {
        return "null".into();
    };
    let size = books::book_size(&book);
    let lean = books::book_lean(&book, at_foot);
    out(json!({
        "thickness": size.thickness,
        "length": size.length,
        "title": size.title,
        "tilt": lean.tilt,
        "nudge": lean.nudge,
        "band": books::spine_band(&book),
    }))
}

pub fn stack_books(books_json: &str, towers: i32) -> String {
    let list: Vec<books::ShelfBook> = parse(books_json).unwrap_or_default();
    out(books::stack_books(&list, towers.max(1) as usize))
}

pub fn prep_groups(lines_json: &str, scale: f64) -> String {
    let lines = lines(lines_json);
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let groups: Vec<Value> = prep::prep_groups(&refs)
        .into_iter()
        .map(|g| {
            json!({
                "kind": g.kind,
                "title": g.kind.title(),
                "hint": g.kind.hint(),
                "items": g.items.iter().map(|(key, item)| json!({
                    "key": key,
                    "name": item.name,
                    "raw": item.raw,
                    "prep": item.prep,
                    "task": item.task,
                    "vessel": item.vessel,
                    "vesselLabel": prep::vessel_label(item.vessel),
                    "volume": item.volume,
                    "amount": prep::prep_amount(item, scale),
                    "color": prep::ingredient_color(&item.name),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    out(groups)
}

pub fn vessel_counts(lines_json: &str) -> String {
    let lines = lines(lines_json);
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let items = ingredients::mise_en_place(&refs);
    out(prep::vessel_counts(&items)
        .into_iter()
        .map(|(v, n)| json!({"vessel": v, "count": n, "label": prep::vessel_count_label(v, n)}))
        .collect::<Vec<_>>())
}

pub fn draft_from_recipe(recipe_json: &str) -> String {
    match parse::<Recipe>(recipe_json) {
        Some(r) => out(editor::RecipeDraft::from_recipe(&r)),
        None => out(editor::RecipeDraft::default()),
    }
}

/// The save body for a draft, or `{"error": …}` when it can't be saved yet.
pub fn draft_to_fields(draft_json: &str) -> String {
    let Some(draft) = parse::<editor::RecipeDraft>(draft_json) else {
        return out(json!({"error": "The form didn't make sense"}));
    };
    match draft.problem() {
        Some(problem) => out(json!({ "error": problem })),
        None => out(draft.to_json()),
    }
}

pub fn fix_for(draft_json: &str, list: &str, index: i32, kind: &str, item_text: &str) -> String {
    let (Some(draft), Ok(list)) = (
        parse::<editor::RecipeDraft>(draft_json),
        serde_json::from_value::<editor::SectionKind>(json!(list)),
    ) else {
        return String::new();
    };
    match editor::fix_for(&draft, list, index.max(0) as usize, kind, item_text) {
        Some(fix) => out(fix),
        None => String::new(),
    }
}

pub fn cook_steps(recipe_json: &str) -> String {
    let Some(recipe) = parse::<Recipe>(recipe_json) else {
        return "[]".into();
    };
    out(client::cook_steps(&recipe)
        .into_iter()
        .map(|s| {
            let timers = ingredients::find_timers(&s.text);
            json!({"section": s.section, "text": s.text, "timers": timers})
        })
        .collect::<Vec<_>>())
}

/// The ingredient lines a step uses, scaled: `[{raw, section, scaled}]`.
pub fn step_ingredients(step: &str, section: &str, recipe_json: &str, scale: f64) -> String {
    let Some(recipe) = parse::<Recipe>(recipe_json) else {
        return "[]".into();
    };
    let all: Vec<ingredients::IngredientLine> = recipe
        .ingredients
        .iter()
        .flat_map(|s| {
            s.items.iter().map(|raw| ingredients::IngredientLine {
                raw: raw.clone(),
                section: s.name.clone(),
            })
        })
        .collect();
    out(
        ingredients::ingredients_for_step_in(step, some(section), &all)
            .into_iter()
            .map(|l| {
                json!({
                    "raw": l.raw,
                    "section": l.section,
                    "scaled": ingredients::scale_ingredient(&l.raw, scale),
                })
            })
            .collect::<Vec<_>>(),
    )
}

pub fn checks_status_text(counts_json: &str, run: i32) -> String {
    let c: Value = parse(counts_json).unwrap_or(Value::Null);
    let n = |k: &str| c.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
    checks::checks_status_text(
        checks::ChecksCounts {
            eligible: n("eligible"),
            checked: n("checked"),
            pending: n("pending"),
            tidied: n("tidied"),
            to_check: n("toCheck"),
            due: n("due"),
            restored: n("restored"),
            edited: n("edited"),
        },
        (run >= 0).then_some(run as u32),
    )
}

/// The review list's summary from a `{field: count}` object, keeping the server's order.
pub fn review_summary(fields_json: &str) -> String {
    let fields: serde_json::Map<String, Value> = parse(fields_json).unwrap_or_default();
    let pairs: Vec<(String, u32)> = fields
        .into_iter()
        .map(|(k, v)| (k, v.as_u64().unwrap_or(0) as u32))
        .collect();
    checks::review_summary(&pairs)
}

pub fn fix_text(detail_json: &str, item_text: &str) -> String {
    let d: Value = parse(detail_json).unwrap_or(Value::Null);
    let s = |k: &str| d.get(k).and_then(Value::as_str);
    checks::fix_text(s("fix"), item_text, s("category"), s("was"))
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        type Core = super::CoreRust;

        // ─── Home ───
        #[qinvokable]
        fn greeting(self: &Core, hour: i32) -> QString;
        #[qinvokable]
        #[cxx_name = "dayLabel"]
        fn day_label(self: &Core, ms: f64, now_ms: f64, offset_minutes: i32) -> QString;
        #[qinvokable]
        #[cxx_name = "freshMeta"]
        fn fresh_meta(
            self: &Core,
            created_ms: f64,
            source: QString,
            now_ms: f64,
            offset_minutes: i32,
        ) -> QString;
        #[qinvokable]
        #[cxx_name = "viewedLine"]
        fn viewed_line(self: &Core, viewed_ms: f64, now_ms: f64, offset_minutes: i32) -> QString;
        /// `[step, of]`, or "null" when there's nothing to pick up.
        #[qinvokable]
        #[cxx_name = "cookProgress"]
        fn cook_progress(self: &Core, step: i32, steps: i32) -> QString;
        /// "Mar 11, 2025": a local calendar date, for a shared link or a connected app.
        #[qinvokable]
        #[cxx_name = "dateLabel"]
        fn date_label(self: &Core, ms: f64, offset_minutes: i32) -> QString;
        // ─── Trash ───
        /// "12 days left" / "Goes for good today": a trashed recipe's time left.
        #[qinvokable]
        #[cxx_name = "trashLeft"]
        fn trash_left(self: &Core, purge_ms: f64, now_ms: f64) -> QString;
        /// The single delete's confirmation.
        #[qinvokable]
        #[cxx_name = "trashDeleteOne"]
        fn trash_delete_one(self: &Core) -> QString;
        /// The bulk delete's confirmation.
        #[qinvokable]
        #[cxx_name = "trashDeleteMany"]
        fn trash_delete_many(self: &Core, n: i32) -> QString;
        /// "Moved 3 recipes to the trash".
        #[qinvokable]
        #[cxx_name = "movedToTrash"]
        fn moved_to_trash(self: &Core, n: i32) -> QString;
        /// "Put back", or "Put back 3 recipes".
        #[qinvokable]
        #[cxx_name = "putBack"]
        fn put_back(self: &Core, n: i32) -> QString;
        /// "3 recipes will be deleted for good. This can't be undone."
        #[qinvokable]
        #[cxx_name = "emptyTrash"]
        fn empty_trash(self: &Core, n: i32) -> QString;

        // ─── Add box ───
        /// `{mode, summary}`: mode is auto, link, text or scratch.
        #[qinvokable]
        fn detect(self: &Core, text: QString) -> QString;
        #[qinvokable]
        #[cxx_name = "linksIn"]
        fn links_in(self: &Core, text: QString) -> QString;
        /// Every link anywhere in some text, as the Import page finds them (JSON array).
        #[qinvokable]
        #[cxx_name = "linksInText"]
        fn links_in_text(self: &Core, text: QString) -> QString;
        #[qinvokable]
        #[cxx_name = "jobProgress"]
        fn job_progress(self: &Core, status: QString, position: i32) -> QString;
        #[qinvokable]
        #[cxx_name = "bookImportedTitle"]
        fn book_imported_title(
            self: &Core,
            name: QString,
            added: i32,
            duplicates: i32,
            skipped: i32,
        ) -> QString;

        // ─── Shelf ───
        #[qinvokable]
        #[cxx_name = "bookColors"]
        fn book_colors(self: &Core) -> QString;
        /// `{name, cloth, shade, foil, edge, edgeAlpha, bands}` for a stored colour name.
        #[qinvokable]
        #[cxx_name = "bookLook"]
        fn book_look(self: &Core, color: QString) -> QString;
        #[qinvokable]
        #[cxx_name = "bookShape"]
        fn book_shape(self: &Core, book: QString, at_foot: bool) -> QString;
        /// Towers of indexes into `books`, top to bottom.
        #[qinvokable]
        #[cxx_name = "stackBooks"]
        fn stack_books(self: &Core, books: QString, towers: i32) -> QString;

        // ─── Recipe ───
        #[qinvokable]
        fn kicker(self: &Core, category: QString, cuisine: QString) -> QString;
        #[qinvokable]
        #[cxx_name = "hostOf"]
        fn host_of(self: &Core, url: QString) -> QString;
        #[qinvokable]
        #[cxx_name = "webLink"]
        fn web_link(self: &Core, url: QString) -> QString;
        #[qinvokable]
        #[cxx_name = "displayDuration"]
        fn display_duration(self: &Core, raw: QString) -> QString;
        #[qinvokable]
        #[cxx_name = "scaleIngredient"]
        fn scale_ingredient(self: &Core, line: QString, factor: f64) -> QString;
        #[qinvokable]
        #[cxx_name = "formatQuantity"]
        fn format_quantity(self: &Core, n: f64) -> QString;
        #[qinvokable]
        #[cxx_name = "cookedLine"]
        fn cooked_line(self: &Core, count: i32, last_ms: f64, now_ms: f64) -> QString;
        #[qinvokable]
        #[cxx_name = "nutritionLabel"]
        fn nutrition_label(self: &Core, key: QString) -> QString;
        #[qinvokable]
        #[cxx_name = "recipeText"]
        fn recipe_text(self: &Core, recipe: QString, link: QString) -> QString;
        #[qinvokable]
        #[cxx_name = "recipeMarkdown"]
        fn recipe_markdown(self: &Core, recipe: QString) -> QString;
        #[qinvokable]
        #[cxx_name = "bookText"]
        fn book_text(self: &Core, name: QString, titles: QString, link: QString) -> QString;
        /// `{provider, label, embedUrl, …}` or "null".
        #[qinvokable]
        #[cxx_name = "videoEmbed"]
        fn video_embed(self: &Core, url: QString) -> QString;
        #[qinvokable]
        #[cxx_name = "matchesSearch"]
        fn matches_search(
            self: &Core,
            query: QString,
            title: QString,
            category: QString,
            cuisine: QString,
        ) -> bool;
        #[qinvokable]
        fn categories(self: &Core) -> QString;

        // ─── Wee Chef ───
        #[qinvokable]
        #[cxx_name = "fixText"]
        fn fix_text(self: &Core, detail: QString, item_text: QString) -> QString;
        #[qinvokable]
        #[cxx_name = "reviewText"]
        fn review_text(self: &Core, kind: QString) -> QString;
        #[qinvokable]
        fn quote(self: &Core, text: QString, max: i32) -> QString;
        #[qinvokable]
        fn plural(self: &Core, n: i32, one: QString) -> QString;
        #[qinvokable]
        #[cxx_name = "checksStatusText"]
        fn checks_status_text(self: &Core, counts: QString, run: i32) -> QString;
        /// "2 steps · 1 ingredient" for a review-list recipe's `fields` (a JSON object).
        #[qinvokable]
        #[cxx_name = "reviewSummary"]
        fn review_summary(self: &Core, fields: QString) -> QString;
        #[qinvokable]
        #[cxx_name = "tidiedTitle"]
        fn tidied_title(self: &Core, n: i32) -> QString;
        #[qinvokable]
        #[cxx_name = "lookTitle"]
        fn look_title(self: &Core, n: i32) -> QString;
        #[qinvokable]
        #[cxx_name = "fixWeight"]
        fn fix_weight(self: &Core, detail: QString) -> i32;
        /// `[title, description]` (description "" when none).
        #[qinvokable]
        #[cxx_name = "checkDoneToast"]
        fn check_done_toast(self: &Core, status: QString, fixed: i32, review: i32) -> QString;

        // ─── Cook and prep ───
        /// `[{section, text, timers: [{label, seconds}]}]`.
        #[qinvokable]
        #[cxx_name = "cookSteps"]
        fn cook_steps(self: &Core, recipe: QString) -> QString;
        #[qinvokable]
        #[cxx_name = "stepIngredients"]
        fn step_ingredients(
            self: &Core,
            step: QString,
            section: QString,
            recipe: QString,
            scale: f64,
        ) -> QString;
        #[qinvokable]
        #[cxx_name = "findTimers"]
        fn find_timers(self: &Core, step: QString) -> QString;
        #[qinvokable]
        #[cxx_name = "formatClock"]
        fn format_clock(self: &Core, seconds: f64) -> QString;
        /// The prep page's groups for ingredient lines (a JSON array of strings).
        #[qinvokable]
        #[cxx_name = "prepGroups"]
        fn prep_groups(self: &Core, lines: QString, scale: f64) -> QString;
        /// The Get out line: `[{vessel, count, label}]`.
        #[qinvokable]
        #[cxx_name = "vesselCounts"]
        fn vessel_counts(self: &Core, lines: QString) -> QString;
        #[qinvokable]
        #[cxx_name = "ingredientColor"]
        fn ingredient_color(self: &Core, name: QString) -> QString;

        // ─── Editor ───
        #[qinvokable]
        #[cxx_name = "newDraft"]
        fn new_draft(self: &Core, title: QString) -> QString;
        #[qinvokable]
        #[cxx_name = "draftFromRecipe"]
        fn draft_from_recipe(self: &Core, recipe: QString) -> QString;
        /// The create/update body, or `{error}`.
        #[qinvokable]
        #[cxx_name = "draftToFields"]
        fn draft_to_fields(self: &Core, draft: QString) -> QString;
        /// `{label, draft}` or "" when no fix fits. `list` is ingredients or instructions.
        #[qinvokable]
        #[cxx_name = "fixFor"]
        fn fix_for(
            self: &Core,
            draft: QString,
            list: QString,
            index: i32,
            kind: QString,
            item_text: QString,
        ) -> QString;
        #[qinvokable]
        #[cxx_name = "legacyCategory"]
        fn legacy_category(self: &Core, category: QString) -> QString;
        /// `[[key, label, placeholder]]`.
        #[qinvokable]
        #[cxx_name = "metaFields"]
        fn meta_fields(self: &Core) -> QString;
        /// A device's user agent as "Firefox on Linux" (`""` for an unknown one).
        #[qinvokable]
        #[cxx_name = "deviceName"]
        fn device_name(self: &Core, agent: QString) -> QString;
    }
}

/// The `Core` singleton holds nothing.
#[derive(Default)]
pub struct CoreRust;

fn q(s: impl AsRef<str>) -> QString {
    QString::from(s.as_ref())
}

impl qobject::Core {
    pub fn greeting(&self, hour: i32) -> QString {
        q(home::greeting(hour.clamp(0, 23) as u32))
    }
    pub fn day_label(&self, ms: f64, now_ms: f64, offset: i32) -> QString {
        q(home::day_label(ms as i64, now_ms as i64, offset))
    }
    pub fn fresh_meta(&self, created: f64, source: QString, now: f64, offset: i32) -> QString {
        q(home::fresh_meta(
            created as i64,
            &source.to_string(),
            now as i64,
            offset,
        ))
    }
    pub fn viewed_line(&self, viewed: f64, now: f64, offset: i32) -> QString {
        q(home::viewed_line(opt_ms(viewed), now as i64, offset))
    }
    pub fn cook_progress(&self, step: i32, steps: i32) -> QString {
        q(out(home::cook_progress(
            (step >= 0).then_some(i64::from(step)),
            (steps >= 0).then_some(i64::from(steps)),
        )))
    }
    pub fn date_label(&self, ms: f64, offset: i32) -> QString {
        q(home::date_label(ms as i64, offset))
    }

    pub fn trash_left(&self, purge_ms: f64, now_ms: f64) -> QString {
        q(trash::days_left(purge_ms as i64, now_ms as i64))
    }
    pub fn trash_delete_one(&self) -> QString {
        q(trash::DELETE_ONE)
    }
    pub fn trash_delete_many(&self, n: i32) -> QString {
        q(trash::delete_many(n.max(0) as u32))
    }
    pub fn moved_to_trash(&self, n: i32) -> QString {
        q(trash::moved_to_trash(n.max(0) as u32))
    }
    pub fn put_back(&self, n: i32) -> QString {
        q(trash::put_back(n.max(0) as u32))
    }
    pub fn empty_trash(&self, n: i32) -> QString {
        q(trash::empty_trash(n.max(0) as u32))
    }

    pub fn detect(&self, text: QString) -> QString {
        q(out(add::detect(&text.to_string())))
    }
    pub fn links_in(&self, text: QString) -> QString {
        q(out(add::links_in(&text.to_string())))
    }
    pub fn links_in_text(&self, text: QString) -> QString {
        q(out(add::links_in_text(&text.to_string())))
    }
    pub fn job_progress(&self, status: QString, position: i32) -> QString {
        q(add::job_progress(
            &status.to_string(),
            (position > 0).then_some(position as u32),
        ))
    }
    pub fn book_imported_title(
        &self,
        name: QString,
        added: i32,
        dup: i32,
        skipped: i32,
    ) -> QString {
        q(format::book_imported_title(
            &name.to_string(),
            added.max(0) as usize,
            dup.max(0) as usize,
            (skipped >= 0).then_some(skipped as usize),
        ))
    }

    pub fn book_colors(&self) -> QString {
        q(out(crumb_core::model::BOOK_COLORS))
    }
    pub fn book_look(&self, color: QString) -> QString {
        q(book_json(&color.to_string()))
    }
    pub fn book_shape(&self, book: QString, at_foot: bool) -> QString {
        q(book_shape(&book.to_string(), at_foot))
    }
    pub fn stack_books(&self, books: QString, towers: i32) -> QString {
        q(stack_books(&books.to_string(), towers))
    }

    pub fn kicker(&self, category: QString, cuisine: QString) -> QString {
        q(format::kicker(
            some(&category.to_string()),
            some(&cuisine.to_string()),
        ))
    }
    pub fn host_of(&self, url: QString) -> QString {
        q(format::host_of(some(&url.to_string())).unwrap_or_default())
    }
    pub fn web_link(&self, url: QString) -> QString {
        q(format::web_link(some(&url.to_string())).unwrap_or_default())
    }
    pub fn display_duration(&self, raw: QString) -> QString {
        q(client::display_duration(some(&raw.to_string())).unwrap_or_default())
    }
    pub fn scale_ingredient(&self, line: QString, factor: f64) -> QString {
        q(ingredients::scale_ingredient(&line.to_string(), factor))
    }
    pub fn format_quantity(&self, n: f64) -> QString {
        q(ingredients::format_quantity(n))
    }
    pub fn cooked_line(&self, count: i32, last_ms: f64, now_ms: f64) -> QString {
        q(
            client::cooked_line(count.max(0) as u32, opt_ms(last_ms), now_ms as i64)
                .unwrap_or_default(),
        )
    }
    pub fn nutrition_label(&self, key: QString) -> QString {
        q(recipe_page::nutrition_label(&key.to_string()))
    }
    pub fn recipe_text(&self, recipe: QString, link: QString) -> QString {
        parse::<Recipe>(&recipe.to_string())
            .map(|r| q(format::recipe_to_text(&r, some(&link.to_string()))))
            .unwrap_or_default()
    }
    pub fn recipe_markdown(&self, recipe: QString) -> QString {
        parse::<Recipe>(&recipe.to_string())
            .map(|r| q(format::recipe_to_markdown(&r)))
            .unwrap_or_default()
    }
    pub fn book_text(&self, name: QString, titles: QString, link: QString) -> QString {
        let titles = lines(&titles.to_string());
        let refs: Vec<&str> = titles.iter().map(String::as_str).collect();
        q(format::book_to_text(
            &name.to_string(),
            &refs,
            some(&link.to_string()),
        ))
    }
    pub fn device_name(&self, agent: QString) -> QString {
        q(client::device_name(some(&agent.to_string())))
    }
    pub fn video_embed(&self, url: QString) -> QString {
        q(out(embed::video_embed(&url.to_string())))
    }
    pub fn matches_search(
        &self,
        query: QString,
        title: QString,
        cat: QString,
        cui: QString,
    ) -> bool {
        client::matches_search(
            &query.to_string(),
            &title.to_string(),
            some(&cat.to_string()),
            some(&cui.to_string()),
        )
    }
    pub fn categories(&self) -> QString {
        q(out(categories::LIST))
    }

    pub fn fix_text(&self, detail: QString, item_text: QString) -> QString {
        q(fix_text(&detail.to_string(), &item_text.to_string()))
    }
    pub fn review_text(&self, kind: QString) -> QString {
        q(checks::review_text(&kind.to_string()))
    }
    pub fn quote(&self, text: QString, max: i32) -> QString {
        q(checks::quote(&text.to_string(), max.max(2) as usize))
    }
    pub fn plural(&self, n: i32, one: QString) -> QString {
        q(checks::plural(n.max(0) as u32, &one.to_string()))
    }
    pub fn checks_status_text(&self, counts: QString, run: i32) -> QString {
        q(checks_status_text(&counts.to_string(), run))
    }
    pub fn review_summary(&self, fields: QString) -> QString {
        q(review_summary(&fields.to_string()))
    }
    pub fn tidied_title(&self, n: i32) -> QString {
        q(recipe_page::tidied_title(n.max(0) as u32))
    }
    pub fn look_title(&self, n: i32) -> QString {
        q(recipe_page::look_title(n.max(0) as u32))
    }
    pub fn fix_weight(&self, detail: QString) -> i32 {
        recipe_page::fix_weight(&parse(&detail.to_string()).unwrap_or(Value::Null)) as i32
    }
    pub fn check_done_toast(&self, status: QString, fixed: i32, review: i32) -> QString {
        let (title, description) = recipe_page::check_done_toast(
            &status.to_string(),
            fixed.max(0) as u32,
            review.max(0) as u32,
        );
        q(out([title, description.unwrap_or_default()]))
    }

    pub fn cook_steps(&self, recipe: QString) -> QString {
        q(cook_steps(&recipe.to_string()))
    }
    pub fn step_ingredients(
        &self,
        step: QString,
        section: QString,
        recipe: QString,
        scale: f64,
    ) -> QString {
        q(step_ingredients(
            &step.to_string(),
            &section.to_string(),
            &recipe.to_string(),
            scale,
        ))
    }
    pub fn find_timers(&self, step: QString) -> QString {
        q(out(ingredients::find_timers(&step.to_string())))
    }
    pub fn format_clock(&self, seconds: f64) -> QString {
        q(client::format_clock(seconds))
    }
    pub fn prep_groups(&self, lines: QString, scale: f64) -> QString {
        q(prep_groups(&lines.to_string(), scale))
    }
    pub fn vessel_counts(&self, lines: QString) -> QString {
        q(vessel_counts(&lines.to_string()))
    }
    pub fn ingredient_color(&self, name: QString) -> QString {
        q(prep::ingredient_color(&name.to_string()))
    }

    pub fn new_draft(&self, title: QString) -> QString {
        q(out(editor::RecipeDraft::new(&title.to_string())))
    }
    pub fn draft_from_recipe(&self, recipe: QString) -> QString {
        q(draft_from_recipe(&recipe.to_string()))
    }
    pub fn draft_to_fields(&self, draft: QString) -> QString {
        q(draft_to_fields(&draft.to_string()))
    }
    pub fn fix_for(
        &self,
        draft: QString,
        list: QString,
        index: i32,
        kind: QString,
        item: QString,
    ) -> QString {
        q(fix_for(
            &draft.to_string(),
            &list.to_string(),
            index,
            &kind.to_string(),
            &item.to_string(),
        ))
    }
    pub fn legacy_category(&self, category: QString) -> QString {
        q(editor::legacy_category(&category.to_string()).unwrap_or_default())
    }
    pub fn meta_fields(&self) -> QString {
        q(out(editor::META_FIELDS))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_cross_as_the_webs_en_us() {
        // 2025-03-11T00:00:00Z
        assert_eq!(home::date_label(1_741_651_200_000, 0), "Mar 11, 2025");
        // 2025-03-11T23:30:00Z: +120 minutes rolls into the 12th
        assert_eq!(home::date_label(1_741_735_800_000, 120), "Mar 12, 2025");
        // A leap day
        assert_eq!(home::date_label(1_709_208_000_000, 0), "Feb 29, 2024");
    }

    #[test]
    fn books_cross_as_json() {
        let look: Value = serde_json::from_str(&book_json("cream")).unwrap();
        assert_eq!(look["name"], "cream");
        assert!(look["edge"].is_string());
        let shape: Value = serde_json::from_str(&book_shape(
            r#"{"id": 3, "name": "Weeknights", "color": "sage", "recipeCount": 5}"#,
            false,
        ))
        .unwrap();
        assert_eq!(shape["thickness"], 42);
        assert_eq!(
            stack_books(
                r#"[{"id":1,"name":"a","recipeCount":1},{"id":2,"name":"b"}]"#,
                2
            ),
            "[[0],[1]]"
        );
    }

    #[test]
    fn review_summaries_keep_the_servers_order() {
        assert_eq!(
            review_summary(r#"{"notes": 1, "instructions": 2, "ingredients": 1}"#),
            "2 steps · 1 note · 1 ingredient"
        );
    }

    #[test]
    fn prep_crosses_as_json() {
        let groups: Value =
            serde_json::from_str(&prep_groups(r#"["2 onions, diced", "1 cup flour"]"#, 2.0))
                .unwrap();
        let measure = groups
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["kind"] == "measure")
            .unwrap();
        assert_eq!(measure["items"][0]["amount"], "2 cups");
        assert_eq!(measure["items"][0]["color"], "#faf5e8");
    }

    #[test]
    fn drafts_round_trip() {
        let draft = serde_json::from_str::<Value>(&draft_from_recipe("{}")).unwrap();
        assert_eq!(draft["title"], "");
        let fields: Value = serde_json::from_str(&draft_to_fields(&draft.to_string())).unwrap();
        assert_eq!(fields["error"], "Give the recipe a title.");
        let mut named = draft.clone();
        named["title"] = json!("Soup");
        named["instructions"] = json!([{"name": "", "text": "Boil.\nand stir."}]);
        let fields: Value = serde_json::from_str(&draft_to_fields(&named.to_string())).unwrap();
        assert_eq!(fields["title"], "Soup");
        let fix: Value = serde_json::from_str(&fix_for(
            &named.to_string(),
            "instructions",
            0,
            "fragment",
            "and stir.",
        ))
        .unwrap();
        assert_eq!(fix["label"], "Join with the step above");
        assert_eq!(fix_for(&named.to_string(), "nope", 0, "junk", "x"), "");
    }
}
