//! Turns float quantities some sites put in their structured data ("0.33333334326744 cup
//! olive oil", "1.5 teaspoons salt") back into the fractions the cook would write ("⅓ cup
//! olive oil", "1 ½ teaspoons salt").
//!
//! Only the leading quantity (or range) of an ingredient line is touched, and only when
//! every decimal in it is plainly a fraction: float noise with three or more places that
//! sits on a halves, thirds, quarters, sixths or eighths fraction ("0.33333334",
//! "0.667"), or an exact eighth ("0.5", "2.25", "0.375"). Two-place amounts ("0.33",
//! "1.12"), metric amounts ("12.5 g", "0.67 l") and percentages ("2.5 % fat") are real
//! measurements and left as written, as is anything else ("0.4 kg", "2.2 lbs").
//! The output uses the same Unicode glyphs and "1 ½" spacing that the recipe page's
//! scaler prints (`formatQuantity` in `web/src/lib/ingredients.ts`), which parses them back.

use crate::model::Section;
use regex::Regex;
use std::sync::LazyLock;

/// How close a decimal of three or more places must be to a fraction. Float noise
/// (0.33333334) and a three-place rounding (0.333, 0.667, 0.167) are inside it; a real
/// measurement like 0.120 or 0.35 is not.
const TOLERANCE: f64 = 0.002;

/// Exact binary fractions, written out: the only decimals under three places rewritten.
const EXACT: [&str; 7] = ["5", "25", "75", "125", "375", "625", "875"];

const FRACTIONS: [(f64, &str); 11] = [
    (1.0 / 8.0, "⅛"),
    (1.0 / 6.0, "⅙"),
    (1.0 / 4.0, "¼"),
    (1.0 / 3.0, "⅓"),
    (3.0 / 8.0, "⅜"),
    (1.0 / 2.0, "½"),
    (5.0 / 8.0, "⅝"),
    (2.0 / 3.0, "⅔"),
    (3.0 / 4.0, "¾"),
    (5.0 / 6.0, "⅚"),
    (7.0 / 8.0, "⅞"),
];

/// A metric unit or a percent sign right after the quantity: the amount is a measurement.
static MEASURED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^\s*(?:%|(?:g|kg|mg|ml|l|cl|dl|grams?|kilograms?|millilit(?:re|er)s?|lit(?:re|er)s?)\b)",
    )
    .unwrap()
});

static LEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(\s*(?:(?:about|approx\.?|~)\s*)?)(\d+(?:\.\d+)?|\.\d+)(?:(\s*(?:-|–|to)\s*)(\d+(?:\.\d+)?|\.\d+))?",
    )
    .unwrap()
});

/// "0.5" → "½", "2.25" → "2 ¼", "0.33333334" → "⅓"; `None` for a whole number or a
/// decimal that isn't plainly a fraction (see the module docs).
fn as_fraction(number: &str) -> Option<String> {
    let (_, places) = number.split_once('.')?;
    let exact = EXACT.contains(&places.trim_end_matches('0'));
    if !exact && places.len() < 3 {
        return None;
    }
    let value: f64 = number.parse().ok()?;
    let whole = value.trunc();
    let part = value - whole;
    let tolerance = if exact { 1e-9 } else { TOLERANCE };
    let (_, glyph) = FRACTIONS
        .iter()
        .find(|(f, _)| (part - f).abs() <= tolerance)?;
    Some(if whole == 0.0 {
        (*glyph).to_string()
    } else {
        format!("{whole} {glyph}")
    })
}

/// A whole number stays as it is; a decimal must become a fraction.
fn part(number: &str) -> Option<String> {
    if number.contains('.') {
        as_fraction(number)
    } else {
        Some(number.to_string())
    }
}

/// Rewrites the leading quantity of one ingredient line (see the module docs). Returns the
/// line unchanged when there's nothing to rewrite.
pub fn fractionize(line: &str) -> String {
    let Some(m) = LEADING.captures(line) else {
        return line.to_string();
    };
    let end = m.get(0).map_or(0, |g| g.end());
    // "1.5% milk", "0.5.1", "1.5/2": not a plain quantity
    if line[end..].starts_with(|c: char| c == '%' || c == '.' || c == '/' || c.is_ascii_digit()) {
        return line.to_string();
    }
    // "12.5 g", "0.67 l", "2.5 % fat": a measurement, not a cup fraction
    if MEASURED.is_match(&line[end..]) {
        return line.to_string();
    }
    let first = &m[2];
    let second = m.get(4).map(|g| g.as_str());
    if !first.contains('.') && !second.is_some_and(|s| s.contains('.')) {
        return line.to_string();
    }
    let Some(a) = part(first) else {
        return line.to_string();
    };
    let quantity = match second {
        Some(second) => match part(second) {
            Some(b) => format!("{a}{}{b}", &m[3]),
            None => return line.to_string(),
        },
        None => a,
    };
    format!("{}{quantity}{}", &m[1], &line[end..])
}

/// [`fractionize`] over every ingredient line; returns how many lines changed.
pub fn fractionize_sections(sections: &mut [Section]) -> usize {
    let mut changes = 0;
    for item in sections.iter_mut().flat_map(|s| s.items.iter_mut()) {
        let next = fractionize(item);
        if next != *item {
            *item = next;
            changes += 1;
        }
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_float_quantities_as_fractions() {
        for (from, to) in [
            ("0.33333334326744 cup olive oil", "⅓ cup olive oil"),
            ("0.3333333 cup sugar", "⅓ cup sugar"),
            ("0.6666667 cup milk", "⅔ cup milk"),
            ("0.667 cup milk", "⅔ cup milk"),
            ("0.50 cup water", "½ cup water"),
            ("0.5 cup water", "½ cup water"),
            (".5 cup water", "½ cup water"),
            ("0.25 teaspoon salt", "¼ teaspoon salt"),
            ("0.75 cup flour", "¾ cup flour"),
            ("0.125 teaspoon cayenne", "⅛ teaspoon cayenne"),
            ("0.375 cup oats", "⅜ cup oats"),
            ("0.625 cup oats", "⅝ cup oats"),
            ("0.875 cup oats", "⅞ cup oats"),
            ("0.16666667 cup honey", "⅙ cup honey"),
            ("0.8333333 cup honey", "⅚ cup honey"),
            ("1.5 teaspoons salt", "1 ½ teaspoons salt"),
            ("0.33333334 cup flour", "⅓ cup flour"),
            ("0.5 lb butter", "½ lb butter"),
            ("1.5 large eggs", "1 ½ large eggs"),
            ("2.25 cups stock", "2 ¼ cups stock"),
            ("1.3333334 cups rice", "1 ⅓ cups rice"),
            (
                "1.5 pounds chicken, cut into 0.5-inch cubes",
                "1 ½ pounds chicken, cut into 0.5-inch cubes",
            ),
            ("about 0.5 cup parsley", "about ½ cup parsley"),
        ] {
            assert_eq!(fractionize(from), to, "{from}");
        }
    }

    #[test]
    fn rewrites_ranges() {
        assert_eq!(fractionize("0.5-0.75 cup cream"), "½-¾ cup cream");
        assert_eq!(fractionize("0.5 to 0.75 cup cream"), "½ to ¾ cup cream");
        assert_eq!(fractionize("1–1.5 cups broth"), "1–1 ½ cups broth");
        assert_eq!(
            fractionize("0.5 - 1 teaspoon chili"),
            "½ - 1 teaspoon chili"
        );
    }

    #[test]
    fn leaves_other_quantities_alone() {
        for line in [
            "0.4 kg potatoes",
            "2.2 lbs beef",
            "0.35 kg flour",
            "1.2 litres stock",
            "0.1 g saffron",
            "2 cups flour",
            "1/3 cup olive oil",
            "⅓ cup olive oil",
            "1 ½ teaspoons salt",
            "8 potatoes, cut into 1/2-inch cubes",
            "cooking spray",
            "1.5% milk",
            "2.5 % fat",
            "1.12 kg beef",
            "0.13 g yeast",
            "0.67 l stock",
            "12.5 g butter",
            "12.5g butter",
            "0.5 L milk",
            "1.5 kilograms potatoes",
            "0.75 litres water",
            "0.5 millilitre vanilla",
            "2.25 grams salt",
            "0.66 cup milk",
            "0.33 cup sugar",
            "1.12 cups flour",
            "0.120 cup oil",
            "0.5-0.4 cup cream",
            "0.5-1.2 kg beef",
            "",
        ] {
            assert_eq!(fractionize(line), line, "{line}");
        }
    }

    #[test]
    fn counts_changed_lines() {
        let mut sections = vec![Section {
            name: None,
            items: vec![
                "0.33333334326744 cup olive oil".into(),
                "2 tablespoons garlic powder".into(),
                "1.5 teaspoons salt".into(),
            ],
        }];
        assert_eq!(fractionize_sections(&mut sections), 2);
        assert_eq!(
            sections[0].items,
            [
                "⅓ cup olive oil",
                "2 tablespoons garlic powder",
                "1 ½ teaspoons salt"
            ]
        );
        assert_eq!(fractionize_sections(&mut sections), 0);
    }
}
