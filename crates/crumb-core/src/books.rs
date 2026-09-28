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

/// A book lying flat (`bookSize`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookSize {
    /// Px: thicker books hold more recipes, never thinner than a comfortable tap.
    pub thickness: u32,
    /// A fraction of the tower's length, a little different from its neighbours'.
    pub length: f64,
    /// Px the spine needs for the whole title, so a short book can stretch to fit.
    pub title: u32,
}

/// JavaScript's `Math.round`: halves go up, also for negatives.
fn js_round(n: f64) -> f64 {
    (n + 0.5).floor()
}

pub fn book_size(book: &ShelfBook) -> BookSize {
    let thickness = js_round(50f64.min(38.0 + book.recipe_count as f64 * 0.8)) as u32;
    let length = 0.84 + seeded(book.id, 0) * 0.16;
    let bands = if spine_band(book).is_some() {
        52.0
    } else {
        28.0
    };
    // `name.length` counts UTF-16 units in the web
    let title = js_round(book.name.encode_utf16().count() as f64 * 7.6 + bands) as u32;
    BookSize {
        thickness,
        length,
        title,
    }
}

/// How untidily a book sits in its stack (`bookLean`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BookLean {
    /// Degrees, one decimal.
    pub tilt: f64,
    /// Px sideways.
    pub nudge: i32,
}

/// Books at the foot of a tower sit flatter, so the stack looks like it rests on the plank.
pub fn book_lean(book: &ShelfBook, at_foot: bool) -> BookLean {
    let range = if at_foot { 0.6 } else { 2.5 };
    let tilt = (seeded(book.id, 7) * 2.0 - 1.0) * range;
    let nudge = (seeded(book.id, 11) * 2.0 - 1.0) * 7.0;
    BookLean {
        tilt: js_round(tilt * 10.0) / 10.0,
        nudge: js_round(nudge) as i32,
    }
}

/// The spine's two bands' colour, or None (`spineBands`): about half the books get them.
pub fn spine_band(book: &ShelfBook) -> Option<&'static str> {
    let r = seeded(book.id, 3);
    if r < 0.45 {
        return None;
    }
    let bands = book_look(book.color.as_deref()).bands;
    Some(if r < 0.75 { bands[0] } else { bands[1] })
}

/// Splits books into `towers` stacks of roughly equal height, keeping their order
/// (`stackBooks`). Each tower lists indexes into `books`, top to bottom.
pub fn stack_books(books: &[ShelfBook], towers: usize) -> Vec<Vec<usize>> {
    let n = towers.min(books.len()).max(1);
    let thickness: Vec<f64> = books
        .iter()
        .map(|b| f64::from(book_size(b).thickness))
        .collect();
    let total: f64 = thickness.iter().sum();
    let mut out: Vec<Vec<usize>> = vec![Vec::new()];
    let mut height = 0.0;
    for (i, t) in thickness.iter().enumerate() {
        let left = books.len() - i;
        let towers_left = n - out.len();
        let current = out.last().expect("one tower");
        // Start the next tower once this one reaches its share, but leave a book for every tower
        if !current.is_empty()
            && towers_left > 0
            && (height >= total * out.len() as f64 / n as f64 || left <= towers_left)
        {
            out.push(Vec::new());
        }
        out.last_mut().expect("one tower").push(i);
        height += t;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book(id: i64, name: &str, count: i64) -> ShelfBook {
        ShelfBook {
            id,
            name: name.into(),
            color: None,
            recipe_count: count,
        }
    }

    #[test]
    fn colours_fall_back_to_tile() {
        assert_eq!(book_color(Some("Forest")), "forest");
        assert_eq!(book_color(Some("tomato")), "clay");
        assert_eq!(book_color(Some("purple")), "tile");
        assert_eq!(book_color(None), "tile");
        assert_eq!(book_look(Some("cream")).edge, Some(FOREST));
    }

    #[test]
    fn thickness_grows_with_recipes_up_to_a_cap() {
        assert_eq!(book_size(&book(1, "A", 0)).thickness, 38);
        assert_eq!(book_size(&book(1, "A", 10)).thickness, 46);
        assert_eq!(book_size(&book(1, "A", 100)).thickness, 50);
    }

    #[test]
    fn every_tower_gets_a_book() {
        let books: Vec<_> = (1..=5).map(|i| book(i, "Book", i * 3)).collect();
        let towers = stack_books(&books, 3);
        assert_eq!(towers.len(), 3);
        assert!(towers.iter().all(|t| !t.is_empty()));
        let flat: Vec<usize> = towers.concat();
        assert_eq!(flat, vec![0, 1, 2, 3, 4]);
        assert_eq!(stack_books(&books[..1], 3), vec![vec![0]]);
        assert_eq!(stack_books(&[], 2), vec![Vec::<usize>::new()]);
    }
}
