//! The `--smoke` support shim: a thin Rust binding to `src/smoke.cpp`, plus a tiny QML
//! singleton so a page can report its own failure and know which smoke scenario it is in.
//!
//! Three scenarios ride on `--smoke`:
//!
//! * the login page check: with no configured server (state "setup"), QML verifies the
//!   page's `session` is bound and its submit button enables once a URL is typed. This is
//!   the regression guard for the page binding its own `session` property to itself.
//! * `--smoke-page recipe` (or `CRUMB_SMOKE_PAGE=recipe`): loads `RecipePage` with a
//!   built-in fixture, no network, so the detail page's QML is exercised headless.
//! * `--smoke-page shelf`: the shelf with fixture books and the open book; it hovers and opens
//!   a book with real pointer events. With `--shot <dir>` it runs longer instead, saving frames
//!   of the hover, the pot riding and the open and close flights into `<dir>`.

use std::sync::atomic::{AtomicBool, Ordering};

use core::pin::Pin;

use crumb_core::model::{Recipe, Section};
use serde_json::json;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

/// Whether this run is a smoke check: `--smoke`, or a named `--smoke-page`.
pub fn enabled() -> bool {
    std::env::args().any(|arg| arg == "--smoke") || page().is_some()
}

/// The page named by `--smoke-page <name>` (or `CRUMB_SMOKE_PAGE`), if any.
pub fn page() -> Option<String> {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == "--smoke-page" {
            return args.next();
        }
        if let Some(value) = arg.strip_prefix("--smoke-page=") {
            return Some(value.to_string());
        }
    }
    std::env::var("CRUMB_SMOKE_PAGE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// The value after `--<name>` (or `--<name>=value`).
fn arg(name: &str) -> Option<String> {
    let flag = format!("--{name}");
    let prefix = format!("--{name}=");
    let mut args = std::env::args();
    while let Some(a) = args.next() {
        if a == flag {
            return args.next();
        }
        if let Some(value) = a.strip_prefix(&prefix) {
            return Some(value.to_string());
        }
    }
    None
}

/// `--shot <file.png>`: open `--route` (default `home`), wait for its calls to settle, save a
/// screenshot and quit. For comparing each screen with the web.
pub fn shot() -> Option<String> {
    arg("shot")
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
            Section {
                name: Some("Cake".to_string()),
                items: vec![
                    "225 g unsalted butter, softened".to_string(),
                    "225 g caster sugar".to_string(),
                    "4 large eggs".to_string(),
                    "2 lemons, zested".to_string(),
                ],
            },
            Section {
                name: Some("Drizzle".to_string()),
                items: vec![
                    "1 lemon, juiced".to_string(),
                    "85 g caster sugar".to_string(),
                ],
            },
        ],
        instructions: vec![Section {
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
        video: None,
        video_embed: None,
        original_url: None,
        created_at: 0,
        updated_at: 0,
    }
}

/// Records a failed smoke assertion on the Rust side, alongside the Qt message handler.
static FAILED: AtomicBool = AtomicBool::new(false);

/// Whether any Qt warning was seen or any [`fail`] assertion fired.
pub fn failed() -> bool {
    FAILED.load(Ordering::SeqCst) || qobject::native_failed()
}

/// Fails the smoke run, printing why.
pub fn fail(reason: &str) {
    eprintln!("crumb-desktop: smoke check failed: {reason}");
    FAILED.store(true, Ordering::SeqCst);
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("smoke.h");

        #[rust_name = "install_handler"]
        fn crumbSmokeInstallHandler();

        #[rust_name = "native_failed"]
        fn crumbSmokeFailed() -> bool;

        #[rust_name = "native_pointer"]
        fn crumbSmokePointer(x: f64, y: f64, kind: i32);

        #[rust_name = "native_grab"]
        fn crumbSmokeGrab(path: &QString) -> bool;
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(bool, enabled)]
        #[qproperty(QString, page)]
        /// `--route <name[:id]>`: the page to open first.
        #[qproperty(QString, route)]
        /// `--shot <file.png>`: where to save a screenshot of it.
        #[qproperty(QString, shot)]
        /// `--size <width>x<height>` for the window.
        #[qproperty(QString, size)]
        type Smoke = super::SmokeRust;

        /// Fails the smoke run with a reason; a no-op outside a smoke run.
        #[qinvokable]
        fn fail(self: Pin<&mut Smoke>, reason: QString);

        /// Moves (0), presses (1) or releases (2) the mouse at window coordinates, so a
        /// scenario can hover and click; a no-op outside a smoke run.
        #[qinvokable]
        fn pointer(self: &Smoke, x: f64, y: f64, kind: i32);

        /// Saves the whole window (popups included) to `path`; false outside a smoke run.
        #[qinvokable]
        fn grab(self: &Smoke, path: QString) -> bool;
    }
}

/// The inner Rust struct behind the `Smoke` singleton.
pub struct SmokeRust {
    enabled: bool,
    page: QString,
    route: QString,
    shot: QString,
    size: QString,
}

impl Default for SmokeRust {
    fn default() -> Self {
        Self {
            enabled: enabled(),
            page: QString::from(&page().unwrap_or_default()),
            route: QString::from(&arg("route").unwrap_or_default()),
            shot: QString::from(&shot().unwrap_or_default()),
            size: QString::from(&arg("size").unwrap_or_default()),
        }
    }
}

impl qobject::Smoke {
    pub fn fail(self: Pin<&mut Self>, reason: QString) {
        if self.rust().enabled {
            crate::smoke::fail(&reason.to_string());
        }
    }
}

impl qobject::Smoke {
    pub fn pointer(&self, x: f64, y: f64, kind: i32) {
        if self.rust().enabled {
            qobject::native_pointer(x, y, kind);
        }
    }

    pub fn grab(&self, path: QString) -> bool {
        self.rust().enabled && qobject::native_grab(&path)
    }
}

pub use qobject::install_handler;
