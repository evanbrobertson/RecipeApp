//! Bundled fonts: registered from the Qt resource system at start-up and exposed to QML.
//!
//! `build.rs` embeds the WOFF2 files from `web/src/assets/fonts`. Qt links FreeType with
//! Brotli, so `addApplicationFont` loads WOFF2 directly.

use core::pin::Pin;
use std::sync::OnceLock;

use cxx_qt_lib::{QFont, QGuiApplication, QString};

use crate::native;

/// Names used if a font did not register; Qt's font matching then falls back gracefully.
pub const SANS_FALLBACK: &str = "Nunito Sans";
pub const SERIF_FALLBACK: &str = "DM Serif Display";
pub const HAND_FALLBACK: &str = "Caveat";

/// The families that actually registered, if any.
#[derive(Debug, Clone, Default)]
pub struct Families {
    sans: Option<String>,
    serif: Option<String>,
    hand: Option<String>,
}

impl Families {
    pub fn sans(&self) -> &str {
        self.sans.as_deref().unwrap_or(SANS_FALLBACK)
    }

    pub fn serif(&self) -> &str {
        self.serif.as_deref().unwrap_or(SERIF_FALLBACK)
    }

    pub fn hand(&self) -> &str {
        self.hand.as_deref().unwrap_or(HAND_FALLBACK)
    }

    /// True when all three fonts registered.
    pub fn all_registered(&self) -> bool {
        self.sans.is_some() && self.serif.is_some() && self.hand.is_some()
    }
}

static FAMILIES: OnceLock<Families> = OnceLock::new();

/// The registered families (all empty until [`install`] runs).
pub fn families() -> &'static Families {
    FAMILIES.get_or_init(Families::default)
}

/// Registers the three bundled fonts and sets Nunito Sans as the application font.
/// Returns whether all three registered.
pub fn install(mut app: Pin<&mut QGuiApplication>) -> bool {
    let families = Families {
        sans: register(":/fonts/nunito-sans.woff2"),
        serif: register(":/fonts/dm-serif-display.woff2"),
        hand: register(":/fonts/caveat.woff2"),
    };

    let mut font = QFont::default();
    font.set_family(&QString::from(families.sans()));
    app.as_mut().set_application_font(&font);

    let ok = families.all_registered();
    let _ = FAMILIES.set(families);
    ok
}

/// Registers a bundled font file and returns the family name it registered under.
fn register(path: &str) -> Option<String> {
    let id = native::add_application_font(&QString::from(path));
    if id >= 0 {
        let family = native::application_font_family(id).to_string();
        if !family.is_empty() {
            return Some(family);
        }
    }
    None
}

/// Whether the desktop environment prefers dark, via `QStyleHints::colorScheme`.
pub fn preferred_dark() -> bool {
    native::prefers_dark()
}
