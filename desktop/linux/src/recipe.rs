//! The recipe detail view, as a small QObject for QML's `RecipePage`.
//!
//! Like the session and the recipe list, the logic is plain Rust ([`snapshot`] and the
//! helpers around it) so the integration tests can drive it without Qt. The `RecipeView`
//! class is the thin QObject: it fetches on the shared runtime and applies the result back
//! on the Qt thread.
//!
//! Ingredient, instruction and nutrition data reach QML as JSON strings, which QML reads
//! with `JSON.parse`; that is the simplest reliable path with cxx-qt. Ingredient lines are
//! already scaled. Everything that decides display text lives in `crumb-core`, never in QML.

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};

use core::pin::Pin;

use crumb_client::{Client, Error};
use crumb_core::duration::iso_duration_minutes;
use crumb_core::format;
use crumb_core::ingredients::scale_ingredient;
use crumb_core::model::{Recipe, is_valid_url};
use crumb_core::source::source_url;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::json;
use tokio::sync::Mutex;

use crate::session::{SessionCore, app_core};

/// The scale range and step. The presets (½, 1, 1½, 2, 3, 4) live in the menu in
/// `RecipePage.qml`; any ½ step between these bounds is also allowed via −/+.
const SCALE_MIN: f64 = 0.5;
const SCALE_MAX: f64 = 12.0;
const SCALE_STEP: f64 = 0.5;

/// The width the hero photo is requested at (matches the web's largest hero).
const HERO_WIDTH: u32 = 1280;

/// Clamps a scale to a whole ½ step between ½ and 12. A non-finite value is left at 1.
pub fn clamp_scale(value: f64) -> f64 {
    if !value.is_finite() {
        return 1.0;
    }
    let stepped = (value / SCALE_STEP).round() * SCALE_STEP;
    stepped.clamp(SCALE_MIN, SCALE_MAX)
}

/// The scale remembered for a recipe this session, defaulting to 1.
fn remembered_scale(id: i64) -> f64 {
    scale_store()
        .lock()
        .unwrap()
        .get(&id)
        .copied()
        .unwrap_or(1.0)
}

/// Remembers a recipe's scale for the rest of the app session (the web keeps this per tab).
fn remember_scale(id: i64, scale: f64) {
    scale_store().lock().unwrap().insert(id, scale);
}

fn scale_store() -> &'static StdMutex<HashMap<i64, f64>> {
    static STORE: OnceLock<StdMutex<HashMap<i64, f64>>> = OnceLock::new();
    STORE.get_or_init(|| StdMutex::new(HashMap::new()))
}

/// An ISO 8601 duration as "1h 10m"; anything else is kept as written. A duration under a
/// minute is dropped, matching the server's scraper.
pub fn display_time(value: Option<&str>) -> Option<String> {
    let value = value.map(str::trim).filter(|value| !value.is_empty())?;
    match iso_duration_minutes(value) {
        Some(minutes) if minutes < 1.0 => None,
        Some(minutes) => Some(format_minutes(minutes.round() as i64)),
        None => Some(value.to_string()),
    }
}

fn format_minutes(minutes: i64) -> String {
    let (hours, minutes) = (minutes / 60, minutes % 60);
    let mut parts = Vec::new();
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes}m"));
    }
    parts.join(" ")
}

/// The recipe's yield, adjusted by the scale. The web scales it the same way.
pub fn scaled_yield(recipe: &Recipe, scale: f64) -> Option<String> {
    let value = recipe
        .recipe_yield
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if scale == 1.0 {
        Some(value.to_string())
    } else {
        Some(scale_ingredient(value, scale))
    }
}

/// The `meta` row: the times that are present and the (scaled) yield, as JSON objects.
pub fn meta_json(recipe: &Recipe, scale: f64) -> String {
    let mut items = Vec::new();
    for (label, value) in [
        ("Prep", &recipe.prep_time),
        ("Cook", &recipe.cook_time),
        ("Extra", &recipe.freeze_time),
        ("Total", &recipe.total_time),
    ] {
        if let Some(value) = display_time(value.as_deref()) {
            items.push(json!({ "label": label, "value": value }));
        }
    }
    if let Some(value) = scaled_yield(recipe, scale) {
        items.push(json!({ "label": "Serves", "value": value }));
    }
    serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string())
}

/// The ingredient sections with each line scaled, as JSON.
pub fn ingredients_json(recipe: &Recipe, scale: f64) -> String {
    let sections: Vec<_> = recipe
        .ingredients
        .iter()
        .map(|section| {
            json!({
                "name": section.name,
                "items": section.items.iter().map(|item| scale_ingredient(item, scale)).collect::<Vec<_>>(),
            })
        })
        .collect();
    serde_json::to_string(&sections).unwrap_or_else(|_| "[]".to_string())
}

/// The instruction sections unchanged, as JSON.
pub fn instructions_json(recipe: &Recipe) -> String {
    serde_json::to_string(&recipe.instructions).unwrap_or_else(|_| "[]".to_string())
}

/// The nutrition table as JSON; an empty object when the recipe has none.
pub fn nutrition_json(recipe: &Recipe) -> String {
    let value = recipe
        .nutrition
        .clone()
        .filter(|value| value.is_object())
        .unwrap_or_else(|| json!({}));
    serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string())
}

/// Everything the QML page shows, already formatted. Pure: the QObject just copies this
/// into its properties, and tests assert on it directly.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecipeSnapshot {
    pub title: String,
    pub kicker: String,
    pub description: String,
    pub hero_url: String,
    pub notes: String,
    pub source_url: String,
    pub source_host: String,
    pub meta: String,
    pub ingredients_json: String,
    pub instructions_json: String,
    pub nutrition_json: String,
}

/// Builds the display snapshot for a recipe at a scale. `hero_url` is passed in so the
/// caller owns the client-base-specific part.
pub fn snapshot(recipe: &Recipe, hero_url: String, scale: f64) -> RecipeSnapshot {
    let source = source_url(recipe).map(str::to_string);
    RecipeSnapshot {
        title: recipe.title.clone(),
        kicker: format::kicker(
            recipe.recipe_category.as_deref(),
            recipe.recipe_cuisine.as_deref(),
        ),
        description: recipe.description.clone().unwrap_or_default(),
        hero_url,
        notes: recipe.notes.clone().unwrap_or_default(),
        source_host: format::host_of(source.as_deref()).unwrap_or_default(),
        source_url: source.unwrap_or_default(),
        meta: meta_json(recipe, scale),
        ingredients_json: ingredients_json(recipe, scale),
        instructions_json: instructions_json(recipe),
        nutrition_json: nutrition_json(recipe),
    }
}

/// A recipe loaded and ready to show.
#[derive(Debug)]
pub struct Loaded {
    pub recipe: Recipe,
    pub snapshot: RecipeSnapshot,
}

/// What a load ended as: the recipe and its snapshot, a signed-out signal, or a message.
#[derive(Debug)]
pub enum LoadOutcome {
    Loaded(Box<Loaded>),
    Unauthorized,
    Failed(String),
}

/// Fetches a recipe by id and builds its snapshot. Fire-and-forget logs the view.
pub async fn load_recipe(client: Option<Client>, id: i64, scale: f64) -> LoadOutcome {
    let Some(client) = client else {
        return LoadOutcome::Failed("You're not signed in to Crumb.".to_string());
    };
    match client.recipe(id).await {
        Ok(recipe) => {
            let hero_url = recipe
                .image
                .as_deref()
                .filter(|image| !image.is_empty())
                .map(|image| client.image_url(recipe.id, HERO_WIDTH, image))
                .unwrap_or_default();
            let snapshot = snapshot(&recipe, hero_url, scale);
            // The view log never gets in the way of showing the recipe.
            let viewed = client.clone();
            drop(crate::runtime::spawn(async move {
                let _ = viewed.viewed(id).await;
            }));
            LoadOutcome::Loaded(Box::new(Loaded { recipe, snapshot }))
        }
        Err(Error::Unauthorized) => LoadOutcome::Unauthorized,
        Err(err) => LoadOutcome::Failed(err.to_string()),
    }
}

/// The built-in recipe `--smoke-page recipe` renders, with a local hero (no network).
pub fn fixture_recipe() -> Recipe {
    Recipe {
        id: 1,
        url: Some("https://food.example/lemon-cake".to_string()),
        source: "url".to_string(),
        title: "Lemon Drizzle Cake".to_string(),
        description: Some("A bright, tender loaf with a sharp lemon soak.".to_string()),
        image: Some("qrc:/img/fixture.png".to_string()),
        author: Some("Crumb".to_string()),
        prep_time: Some("PT15M".to_string()),
        cook_time: Some("PT45M".to_string()),
        total_time: Some("PT1H".to_string()),
        freeze_time: None,
        recipe_yield: Some("8 slices".to_string()),
        recipe_category: Some("Dessert".to_string()),
        recipe_cuisine: Some("British".to_string()),
        ingredients: vec![
            crumb_core::model::Section {
                name: Some("Cake".to_string()),
                items: vec![
                    "225 g unsalted butter, softened".to_string(),
                    "225 g caster sugar".to_string(),
                    "4 large eggs".to_string(),
                    "2 lemons, zested".to_string(),
                ],
            },
            crumb_core::model::Section {
                name: Some("Drizzle".to_string()),
                items: vec![
                    "1 lemon, juiced".to_string(),
                    "85 g caster sugar".to_string(),
                ],
            },
        ],
        instructions: vec![crumb_core::model::Section {
            name: None,
            items: vec![
                "Heat the oven to 180°C and line a loaf tin.".to_string(),
                "Cream the butter and sugar, then beat in the eggs one at a time.".to_string(),
                "Fold in the flour and zest and bake for 45 minutes.".to_string(),
                "Spoon the lemon juice and sugar over the warm cake.".to_string(),
            ],
        }],
        nutrition: Some(json!({
            "calories": "320 kcal",
            "fatContent": "18 g",
            "proteinContent": "5 g",
        })),
        notes: Some("It gets better the next day.".to_string()),
        original_url: None,
        created_at: 0,
        updated_at: 0,
    }
}

/// The hero URL the fixture uses; a compiled-in image so the smoke run stays offline.
pub fn fixture_hero_url() -> String {
    "qrc:/img/fixture.png".to_string()
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
        #[qproperty(bool, loading)]
        #[qproperty(QString, error)]
        #[qproperty(QString, title)]
        #[qproperty(QString, kicker)]
        #[qproperty(QString, description)]
        #[qproperty(QString, hero_url, cxx_name = "heroUrl")]
        #[qproperty(QString, notes)]
        #[qproperty(QString, source_url, cxx_name = "sourceUrl")]
        #[qproperty(QString, source_host, cxx_name = "sourceHost")]
        #[qproperty(QString, meta)]
        #[qproperty(QString, ingredients_json, cxx_name = "ingredientsJson")]
        #[qproperty(QString, instructions_json, cxx_name = "instructionsJson")]
        #[qproperty(QString, nutrition_json, cxx_name = "nutritionJson")]
        #[qproperty(f64, scale)]
        #[qproperty(QString, toast)]
        type RecipeView = super::RecipeViewRust;

        /// Emitted when a later call was rejected with 401.
        #[qsignal]
        fn unauthorized(self: Pin<&mut RecipeView>);

        #[qinvokable]
        fn load(self: Pin<&mut RecipeView>, id: i64);

        #[qinvokable]
        fn retry(self: Pin<&mut RecipeView>);

        #[qinvokable]
        #[cxx_name = "setScaleValue"]
        fn set_scale_value(self: Pin<&mut RecipeView>, value: f64);

        #[qinvokable]
        fn mark_cooked(self: Pin<&mut RecipeView>);

        #[qinvokable]
        fn copy_text(self: Pin<&mut RecipeView>);

        #[qinvokable]
        fn open_source(self: Pin<&mut RecipeView>);
    }

    impl cxx_qt::Threading for RecipeView {}
}

/// The inner Rust struct behind the `RecipeView` QObject.
pub struct RecipeViewRust {
    core: Arc<Mutex<SessionCore>>,
    recipe: Option<Recipe>,
    /// The hero URL the current recipe was built with, kept for scale recomputes.
    image_src: String,
    recipe_id: i64,
    loading: bool,
    error: QString,
    title: QString,
    kicker: QString,
    description: QString,
    hero_url: QString,
    notes: QString,
    source_url: QString,
    source_host: QString,
    meta: QString,
    ingredients_json: QString,
    instructions_json: QString,
    nutrition_json: QString,
    scale: f64,
    toast: QString,
}

impl Default for RecipeViewRust {
    fn default() -> Self {
        Self {
            core: app_core(),
            recipe: None,
            image_src: String::new(),
            recipe_id: 0,
            loading: false,
            error: QString::default(),
            title: QString::default(),
            kicker: QString::default(),
            description: QString::default(),
            hero_url: QString::default(),
            notes: QString::default(),
            source_url: QString::default(),
            source_host: QString::default(),
            meta: QString::default(),
            ingredients_json: QString::default(),
            instructions_json: QString::default(),
            nutrition_json: QString::default(),
            scale: 1.0,
            toast: QString::default(),
        }
    }
}

impl qobject::RecipeView {
    pub fn load(mut self: Pin<&mut Self>, id: i64) {
        self.as_mut().rust_mut().recipe_id = id;
        self.as_mut().set_loading(true);
        self.as_mut().set_error(QString::default());
        self.as_mut().reset_display();

        if crate::smoke::page().as_deref() == Some("recipe") {
            let recipe = fixture_recipe();
            let hero = fixture_hero_url();
            let snapshot = snapshot(&recipe, hero.clone(), 1.0);
            self.as_mut().set_scale(1.0);
            self.as_mut().rust_mut().recipe = Some(recipe);
            self.as_mut().rust_mut().image_src = hero;
            self.as_mut().apply(&snapshot);
            self.as_mut().set_loading(false);
            return;
        }

        let scale = remembered_scale(id);
        self.as_mut().set_scale(scale);
        let core = self.core.clone();
        let qt_thread = self.as_mut().qt_thread();
        drop(crate::runtime::spawn(async move {
            let client = { core.lock().await.client() };
            let outcome = load_recipe(client, id, scale).await;
            let _ = qt_thread.queue(move |mut object| {
                object.as_mut().set_loading(false);
                match outcome {
                    LoadOutcome::Loaded(loaded) => {
                        let Loaded { recipe, snapshot } = *loaded;
                        object.as_mut().rust_mut().recipe = Some(recipe);
                        object.as_mut().rust_mut().image_src = snapshot.hero_url.clone();
                        object.as_mut().apply(&snapshot);
                    }
                    LoadOutcome::Unauthorized => object.as_mut().unauthorized(),
                    LoadOutcome::Failed(message) => {
                        object.as_mut().set_error(QString::from(&message));
                    }
                }
            });
        }));
    }

    pub fn retry(mut self: Pin<&mut Self>) {
        let id = self.recipe_id;
        self.as_mut().load(id);
    }

    pub fn set_scale_value(mut self: Pin<&mut Self>, value: f64) {
        let scale = clamp_scale(value);
        self.as_mut().set_scale(scale);
        remember_scale(self.recipe_id, scale);
        self.as_mut().recompute();
    }

    pub fn mark_cooked(mut self: Pin<&mut Self>) {
        let id = self.recipe_id;
        if crate::smoke::page().as_deref() == Some("recipe") {
            self.as_mut().set_toast(QString::from("Marked as cooked"));
            return;
        }
        let core = self.core.clone();
        let qt_thread = self.as_mut().qt_thread();
        drop(crate::runtime::spawn(async move {
            let client = { core.lock().await.client() };
            let result = match client {
                Some(client) => client.cooked(id).await,
                None => return,
            };
            let _ = qt_thread.queue(move |mut object| match result {
                Ok(()) => object.as_mut().set_toast(QString::from("Marked as cooked")),
                Err(Error::Unauthorized) => object.as_mut().unauthorized(),
                Err(err) => object
                    .as_mut()
                    .set_toast(QString::from(&format!("Couldn't mark as cooked: {err}"))),
            });
        }));
    }

    pub fn copy_text(mut self: Pin<&mut Self>) {
        let text = {
            let rust = self.rust();
            rust.recipe.as_ref().map(|recipe| {
                let link = rust.source_url.to_string();
                let link = (!link.is_empty()).then_some(link.as_str());
                crumb_core::format::recipe_to_text(recipe, link)
            })
        };
        let (text, toast) = match text {
            Some(text) => (text, "Copied"),
            None => (String::new(), "Nothing to copy"),
        };
        if !text.is_empty() {
            crate::native::set_clipboard_text(&QString::from(&text));
        }
        self.as_mut().set_toast(QString::from(toast));
    }

    pub fn open_source(self: Pin<&mut Self>) {
        let url = self.source_url.to_string();
        if is_valid_url(&url) {
            crate::native::open_url(&QString::from(&url));
        }
    }
}

impl qobject::RecipeView {
    /// Sets every display property from a snapshot.
    fn apply(mut self: Pin<&mut Self>, snapshot: &RecipeSnapshot) {
        self.as_mut().set_title(QString::from(&snapshot.title));
        self.as_mut().set_kicker(QString::from(&snapshot.kicker));
        self.as_mut()
            .set_description(QString::from(&snapshot.description));
        self.as_mut()
            .set_hero_url(QString::from(&snapshot.hero_url));
        self.as_mut().set_notes(QString::from(&snapshot.notes));
        self.as_mut()
            .set_source_url(QString::from(&snapshot.source_url));
        self.as_mut()
            .set_source_host(QString::from(&snapshot.source_host));
        self.as_mut().set_meta(QString::from(&snapshot.meta));
        self.as_mut()
            .set_ingredients_json(QString::from(&snapshot.ingredients_json));
        self.as_mut()
            .set_instructions_json(QString::from(&snapshot.instructions_json));
        self.as_mut()
            .set_nutrition_json(QString::from(&snapshot.nutrition_json));
    }

    /// Re-derives the scale-dependent parts (ingredients and the yield) in place.
    fn recompute(mut self: Pin<&mut Self>) {
        let scale = self.scale;
        let snapshot = {
            let rust = self.rust();
            rust.recipe
                .as_ref()
                .map(|recipe| snapshot(recipe, rust.image_src.clone(), scale))
        };
        if let Some(snapshot) = snapshot {
            self.as_mut().set_meta(QString::from(&snapshot.meta));
            self.as_mut()
                .set_ingredients_json(QString::from(&snapshot.ingredients_json));
        }
    }

    /// Clears the displayed recipe (keeps the scale and any toast).
    fn reset_display(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().recipe = None;
        self.as_mut().rust_mut().image_src = String::new();
        self.as_mut().set_title(QString::default());
        self.as_mut().set_kicker(QString::default());
        self.as_mut().set_description(QString::default());
        self.as_mut().set_hero_url(QString::default());
        self.as_mut().set_notes(QString::default());
        self.as_mut().set_source_url(QString::default());
        self.as_mut().set_source_host(QString::default());
        self.as_mut().set_meta(QString::default());
        self.as_mut().set_ingredients_json(QString::default());
        self.as_mut().set_instructions_json(QString::default());
        self.as_mut().set_nutrition_json(QString::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crumb_client::ImportInput;
    use crumb_core::ingredients::scale_ingredient;
    use crumb_core::model::Section;
    use serde_json::Value;

    use crumb::{AppState, app, browser::Browser, config::Config, db};

    fn sample() -> Recipe {
        Recipe {
            id: 7,
            url: Some("https://www.food.example/cake".to_string()),
            source: "url".to_string(),
            title: "Lemon Cake".to_string(),
            description: Some("Bright and light.".to_string()),
            image: Some("https://food.example/cake.jpg".to_string()),
            author: Some("A Baker".to_string()),
            prep_time: Some("PT10M".to_string()),
            cook_time: Some("P0Y0M0DT0H20M0.000S".to_string()),
            total_time: Some("PT30M".to_string()),
            freeze_time: Some("PT2H".to_string()),
            recipe_yield: Some("4".to_string()),
            recipe_category: Some("Dessert".to_string()),
            recipe_cuisine: Some("French".to_string()),
            ingredients: vec![
                Section {
                    name: Some("Cake".to_string()),
                    items: vec!["2 cups flour".to_string(), "1 cup sugar".to_string()],
                },
                Section {
                    name: None,
                    items: vec!["3 eggs".to_string()],
                },
            ],
            instructions: vec![Section {
                name: Some("Bake".to_string()),
                items: vec!["Mix.".to_string(), "Bake it.".to_string()],
            }],
            nutrition: Some(json!({"calories": "320 kcal", "proteinContent": "5 g"})),
            notes: Some("Don't overmix.".to_string()),
            original_url: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn ingredients_scale_exactly_like_the_core() {
        let recipe = sample();
        for scale in [1.0, 2.0, 0.5] {
            let parsed: Value = serde_json::from_str(&ingredients_json(&recipe, scale)).unwrap();
            let expected: Vec<Value> = recipe
                .ingredients
                .iter()
                .map(|section| {
                    json!({
                        "name": section.name,
                        "items": section
                            .items
                            .iter()
                            .map(|item| scale_ingredient(item, scale))
                            .collect::<Vec<_>>(),
                    })
                })
                .collect();
            assert_eq!(
                parsed,
                serde_json::to_value(expected).unwrap(),
                "scale {scale}"
            );
        }
    }

    #[test]
    fn meta_formats_iso_durations_and_a_scaled_yield() {
        let recipe = sample();
        let meta: Vec<Value> = serde_json::from_str(&meta_json(&recipe, 1.0)).unwrap();
        assert_eq!(
            meta,
            vec![
                json!({ "label": "Prep", "value": "10m" }),
                json!({ "label": "Cook", "value": "20m" }),
                json!({ "label": "Extra", "value": "2h" }),
                json!({ "label": "Total", "value": "30m" }),
                json!({ "label": "Serves", "value": "4" }),
            ]
        );

        let scaled: Vec<Value> = serde_json::from_str(&meta_json(&recipe, 2.0)).unwrap();
        assert!(scaled.contains(&json!({ "label": "Serves", "value": "8" })));

        // A duration under a minute is dropped; a non-duration is kept as written.
        assert_eq!(display_time(Some("PT30S")), None);
        assert_eq!(display_time(Some("20 mins")).as_deref(), Some("20 mins"));
        assert_eq!(display_time(None), None);
    }

    #[test]
    fn source_rules_refuse_scripts_and_share_links() {
        let mut recipe = sample();
        let snap = snapshot(&recipe, String::new(), 1.0);
        assert_eq!(snap.source_url, "https://www.food.example/cake");
        assert_eq!(snap.source_host, "food.example");

        recipe.url = Some("javascript:alert(1)".to_string());
        assert_eq!(snapshot(&recipe, String::new(), 1.0).source_url, "");

        // A share link must never be shown...
        recipe.url = Some("https://crumb.example/s/abcdefghijklmnopqrst".to_string());
        assert_eq!(snapshot(&recipe, String::new(), 1.0).source_url, "");

        // ...unless the recipe names its real source (saved from another Crumb's share).
        recipe.original_url = Some("https://www.bbc.example/cake".to_string());
        let snap = snapshot(&recipe, String::new(), 1.0);
        assert_eq!(snap.source_url, "https://www.bbc.example/cake");
        assert_eq!(snap.source_host, "bbc.example");
    }

    #[test]
    fn scale_clamps_and_steps_in_halves() {
        assert_eq!(clamp_scale(0.0), 0.5);
        assert_eq!(clamp_scale(0.1), 0.5);
        assert_eq!(clamp_scale(1.2), 1.0);
        assert_eq!(clamp_scale(1.3), 1.5);
        assert_eq!(clamp_scale(1.75), 2.0);
        assert_eq!(clamp_scale(4.0), 4.0);
        assert_eq!(clamp_scale(12.5), 12.0);
        assert_eq!(clamp_scale(100.0), 12.0);
        assert_eq!(clamp_scale(f64::NAN), 1.0);
    }

    /// A running Crumb server on a random local port, with a throwaway database.
    struct TestServer {
        origin: String,
        _dist: tempfile::TempDir,
    }

    impl TestServer {
        async fn start() -> Self {
            let dist = tempfile::tempdir().unwrap();
            for (rel, title) in [
                ("index.html", "home"),
                ("recipes/index.html", "recipes"),
                ("shell/recipe/index.html", "recipe"),
                ("add/index.html", "add"),
                ("login/index.html", "login"),
                ("404.html", "missing"),
            ] {
                let path = dist.path().join(rel);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(
                    &path,
                    format!(
                        "<!doctype html><title>{title}</title>{}",
                        crumb::web::MARKER
                    ),
                )
                .unwrap();
            }

            let config = Config {
                web_dist: dist.path().to_path_buf(),
                ..Config::default()
            };
            let state = AppState::new(db::open_in_memory().unwrap(), config, Browser::disabled());

            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let origin = format!("http://{}", listener.local_addr().unwrap());
            tokio::spawn(async move {
                axum::serve(listener, app(state)).await.unwrap();
            });

            Self {
                origin,
                _dist: dist,
            }
        }
    }

    #[tokio::test]
    async fn loads_a_posted_recipe_and_maps_a_missing_one_to_an_error() {
        let server = TestServer::start().await;
        let client = Client::new(&server.origin).unwrap();
        let imported = client
            .import(ImportInput::Text(
                "Toast\nIngredients\n1 slice bread\nInstructions\nToast the bread.".to_string(),
            ))
            .await
            .unwrap();
        let id = imported.recipe.id;

        match load_recipe(Some(client.clone()), id, 1.0).await {
            LoadOutcome::Loaded(loaded) => {
                assert_eq!(loaded.recipe.title, "Toast");
                assert_eq!(loaded.snapshot.title, "Toast");
                assert!(loaded.snapshot.ingredients_json.contains("slice bread"));
            }
            other => panic!("expected a loaded recipe, got {other:?}"),
        }

        match load_recipe(Some(client), 999_999, 1.0).await {
            LoadOutcome::Failed(message) => assert!(!message.is_empty(), "no error message"),
            other => panic!("expected a failure, got {other:?}"),
        }
    }
}
