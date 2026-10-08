//! How cookbooks look on the shelf: cover colours, and the size, lean and stacking of each
//! book, the same on every load and every app. Ported from `web/src/lib/books.ts`; the
//! parity fixtures in `tests/fixtures/app.json` hold the web's own answers.

use serde::{Deserialize, Serialize};

use crate::model::book_color as current_book_color;

/// One cover colour's look. Hex colours, the same in light and dark mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookLook {
    /// Cover cloth.
    pub cloth: &'static str,
    /// Darker cloth for the hidden faces, for depth.
    pub shade: &'static str,
    /// Title text on the cloth.
    pub foil: &'static str,
    /// A thin inset edge for a cover that would vanish against a light page (cream only),
    /// drawn at [`EDGE_ALPHA`].
    pub edge: Option<&'static str>,
    /// Contrasting colours for the optional spine bands.
    pub bands: [&'static str; 2],
}

/// The opacity of a cover's [`BookLook::edge`] (`rgb(28 43 34 / 0.22)` in the web).
pub const EDGE_ALPHA: f64 = 0.22;

const FOREST: &str = "#1C2B22";
const TILE: &str = "#2F6B4F";
const SAGE: &str = "#A9C4AE";
const BUTTER: &str = "#F3DA8B";
const CLAY: &str = "#A55A40";
const CREAM: &str = "#F8F3E6";

/// Any stored colour name as one of the six (`bookColor`): legacy names map to the
/// nearest current one, anything else is `tile`. Case doesn't matter.
pub fn book_color(color: Option<&str>) -> &'static str {
    let name = color.unwrap_or_default().to_lowercase();
    current_book_color(&name).unwrap_or("tile")
}

/// The look of a cover colour (`bookPalette`), after [`book_color`].
pub fn book_look(color: Option<&str>) -> BookLook {
    match book_color(color) {
        "forest" => BookLook {
            cloth: FOREST,
            shade: "#111a15",
            foil: BUTTER,
            edge: None,
            bands: [BUTTER, SAGE],
        },
        "sage" => BookLook {
            cloth: SAGE,
            shade: "#8aa890",
            foil: FOREST,
            edge: None,
            bands: [FOREST, TILE],
        },
        "butter" => BookLook {
            cloth: BUTTER,
            shade: "#d9bd67",
            foil: FOREST,
            edge: None,
            bands: [FOREST, CLAY],
        },
        "clay" => BookLook {
            cloth: CLAY,
            shade: "#7e412d",
            foil: "#FFF8EA",
            edge: None,
            bands: [BUTTER, CREAM],
        },
        "cream" => BookLook {
            cloth: CREAM,
            shade: "#ddd5c1",
            foil: FOREST,
            edge: Some(FOREST),
            bands: [TILE, CLAY],
        },
        _ => BookLook {
            cloth: TILE,
            shade: "#224f3a",
            foil: "#FFFDF8",
            edge: None,
            bands: [BUTTER, CREAM],
        },
    }
}

/// A pseudo-random number in [0, 1) from an id (`seeded`), so a book keeps its size.
pub fn seeded(id: i64, salt: i64) -> f64 {
    let x = ((id as f64) * 9301.0 + (salt as f64) * 49297.0).sin() * 233280.0;
    x - x.floor()
}

/// What the shelf needs to know about a book.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfBook {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub recipe_count: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_fall_back_to_tile() {
        assert_eq!(book_color(Some("Forest")), "forest");
        assert_eq!(book_color(Some("tomato")), "clay");
        assert_eq!(book_color(Some("purple")), "tile");
        assert_eq!(book_color(None), "tile");
        assert_eq!(book_look(Some("cream")).edge, Some(FOREST));
    }
}
