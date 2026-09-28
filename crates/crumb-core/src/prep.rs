//! Mise en place as the prep page lays it out: which group each ingredient goes in, how
//! much to measure at a scale, what to get out of the cupboard, and a colour for what's in
//! the bowl. Ported from `web/src/islands/PrepPage.svelte` and `web/src/lib/ingredientColor.ts`;
//! [`crate::ingredients::mise_en_place`] decides each ingredient's vessel.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use crate::ingredients::{MiseItem, Vessel, format_quantity, mise_en_place, plural_unit};

/// One of the prep page's three groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PrepGroupKind {
    /// Chop & prep: everything bound for the board.
    Chop,
    /// Measure into bowls, biggest first.
    Measure,
    /// Keep within reach: seasoning and extras.
    Reach,
}

impl PrepGroupKind {
    pub fn title(self) -> &'static str {
        match self {
            Self::Chop => "Chop & prep",
            Self::Measure => "Measure into bowls",
            Self::Reach => "Keep within reach",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Chop => "Wash, peel and cut these first",
            Self::Measure => "Biggest bowls first, then the little ones",
            Self::Reach => "Seasoning and extras to have out on the counter",
        }
    }
}

/// A group and its items. `key` is the item's index in [`mise_en_place`]'s list, which is
/// what the page remembers as ready.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PrepGroup {
    pub kind: PrepGroupKind,
    pub items: Vec<(usize, MiseItem)>,
}

const SIZE_ORDER: [Vessel; 5] = [
    Vessel::Large,
    Vessel::Medium,
    Vessel::Small,
    Vessel::Ramekin,
    Vessel::Pinch,
];

fn size_rank(v: Vessel) -> Option<usize> {
    SIZE_ORDER.iter().position(|s| *s == v)
}

/// The prep page's groups for a recipe's ingredient lines, leaving out empty ones.
pub fn prep_groups(lines: &[&str]) -> Vec<PrepGroup> {
    let items: Vec<(usize, MiseItem)> = mise_en_place(lines).into_iter().enumerate().collect();
    let pick = |f: &dyn Fn(Vessel) -> bool| -> Vec<(usize, MiseItem)> {
        items.iter().filter(|(_, i)| f(i.vessel)).cloned().collect()
    };
    let mut measure = pick(&|v| size_rank(v).is_some());
    // A stable sort, as `Array.prototype.sort` is
    measure.sort_by_key(|(_, i)| size_rank(i.vessel));
    [
        PrepGroup {
            kind: PrepGroupKind::Chop,
            items: pick(&|v| v == Vessel::Board),
        },
        PrepGroup {
            kind: PrepGroupKind::Measure,
            items: measure,
        },
        PrepGroup {
            kind: PrepGroupKind::Reach,
            items: pick(&|v| v == Vessel::Jar),
        },
    ]
    .into_iter()
    .filter(|g| !g.items.is_empty())
    .collect()
}

/// "Large bowl", "Pinch bowl", "Board", "On hand".
pub fn vessel_label(v: Vessel) -> &'static str {
    match v {
        Vessel::Large => "Large bowl",
        Vessel::Medium => "Medium bowl",
        Vessel::Small => "Small bowl",
        Vessel::Ramekin => "Ramekin",
        Vessel::Pinch => "Pinch bowl",
        Vessel::Board => "Board",
        Vessel::Jar => "On hand",
    }
}

/// How much to measure at `scale`: "1½–2 cups", "a pinch", or "" when there's no amount.
pub fn prep_amount(item: &MiseItem, scale: f64) -> String {
    let Some(quantity) = item.quantity else {
        return item
            .unit
            .as_ref()
            .map(|u| format!("a {u}"))
            .unwrap_or_default();
    };
    let q = format_quantity(quantity * scale);
    let max = item
        .quantity_max
        .map(|m| format!("–{}", format_quantity(m * scale)))
        .unwrap_or_default();
    let n = item.quantity_max.unwrap_or(quantity) * scale;
    let unit = item
        .unit
        .as_ref()
        .map(|u| format!(" {}", plural_unit(u, n)))
        .unwrap_or_default();
    format!("{q}{max}{unit}")
}

/// What to get out, in first-seen order: each vessel and how many, one board however much
/// there is to chop, and nothing for jars (they're already on the counter).
pub fn vessel_counts(items: &[MiseItem]) -> Vec<(Vessel, usize)> {
    let mut counts: Vec<(Vessel, usize)> = Vec::new();
    for item in items {
        match counts.iter_mut().find(|(v, _)| *v == item.vessel) {
            Some((_, n)) => *n += 1,
            None => counts.push((item.vessel, 1)),
        }
    }
    counts
        .into_iter()
        .filter(|(v, _)| *v != Vessel::Jar)
        .map(|(v, n)| (v, if v == Vessel::Board { 1 } else { n }))
        .collect()
}

/// "2 × small bowls" for the Get out line.
pub fn vessel_count_label(v: Vessel, n: usize) -> String {
    let s = if n > 1 { "s" } else { "" };
    format!("{n} × {}{s}", vessel_label(v).to_lowercase())
}

static COLOR_RULES: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (
            r"flour|sugar|salt|milk|cream|yogh?urt|rice|coconut|mayo|egg white|powder|oats?|cornstarch|breadcrumb",
            "#faf5e8",
        ),
        (
            r"butter|egg|yolk|cheese|parmesan|cheddar|corn|mustard|honey|lemon|pineapple|polenta|turmeric",
            "#eed27a",
        ),
        (
            r"oil|vinegar|stock|broth|wine|syrup|maple|soy|beer|juice",
            "#d2a24c",
        ),
        // JavaScript's `pepper(?!corn)`, without look-ahead: "pepper" not followed by "corn"
        (
            r"tomato|chil+i|paprika|pepper(?:$|[^c]|c(?:$|[^o]|o(?:$|[^r]|r(?:$|[^n]))))|strawberr|raspberr|beet|red",
            "#b8543a",
        ),
        (
            r"basil|parsley|cilantro|coriander|herb|spinach|kale|pea|lime|mint|dill|chive|scallion|spring onion|green|avocado|pesto|zucchini|cucumber|leek|celery|broccoli",
            "#6a9a72",
        ),
        (
            r"chocolate|cocoa|coffee|espresso|cinnamon|nutmeg|clove|molasses|brown sugar|beef|mince|mushroom|cumin|soy",
            "#6e4a36",
        ),
        (
            r"carrot|orange|pumpkin|squash|sweet potato|apricot|peach|salmon",
            "#d9854f",
        ),
        (
            r"onion|garlic|shallot|ginger|potato|apple|pear|nut|almond|walnut|pecan|cashew|chicken|pork|tofu|bread|pasta|noodle",
            "#eadcb6",
        ),
        (
            r"blueberr|blackberr|plum|grape|cabbage|eggplant|aubergine",
            "#6c4b6b",
        ),
        (r"water|ice", "#c7dedc"),
    ]
    .into_iter()
    .map(|(re, hex)| (Regex::new(&format!("(?i){re}")).unwrap(), hex))
    .collect()
});

/// A plausible colour for what's in the bowl, from the ingredient's name. Purely for fun.
pub fn ingredient_color(name: &str) -> &'static str {
    COLOR_RULES
        .iter()
        .find(|(re, _)| re.is_match(name))
        .map(|(_, hex)| *hex)
        .unwrap_or("#d8bf9a")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_sort_bowls_biggest_first() {
        let lines = [
            "1 tsp salt",
            "2 onions, diced",
            "500 g flour",
            "black pepper, to taste",
            "250 ml milk",
        ];
        let groups = prep_groups(&lines);
        let kinds: Vec<_> = groups.iter().map(|g| g.kind).collect();
        assert!(kinds.contains(&PrepGroupKind::Chop));
        let measure = groups
            .iter()
            .find(|g| g.kind == PrepGroupKind::Measure)
            .unwrap();
        let ranks: Vec<_> = measure
            .items
            .iter()
            .map(|(_, i)| size_rank(i.vessel).unwrap())
            .collect();
        assert!(ranks.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn amounts_scale() {
        let items = mise_en_place(&["1-2 cups flour", "a pinch salt", "3 eggs"]);
        assert_eq!(prep_amount(&items[0], 1.0), "1–2 cups");
        assert_eq!(prep_amount(&items[0], 0.5), "½–1 cup");
        assert_eq!(prep_amount(&items[2], 2.0), "6");
    }

    #[test]
    fn one_board_and_no_jars() {
        let items = mise_en_place(&["2 onions, diced", "1 carrot, chopped", "salt, to taste"]);
        let counts = vessel_counts(&items);
        assert!(counts.contains(&(Vessel::Board, 1)));
        assert!(counts.iter().all(|(v, _)| *v != Vessel::Jar));
        assert_eq!(vessel_count_label(Vessel::Small, 2), "2 × small bowls");
        assert_eq!(vessel_count_label(Vessel::Board, 1), "1 × board");
    }

    #[test]
    fn bowl_colours() {
        assert_eq!(ingredient_color("Plain Flour"), "#faf5e8");
        assert_eq!(ingredient_color("red pepper"), "#b8543a");
        // "pepper" before "corn" isn't red, but "corn" itself is butter-yellow
        assert_eq!(ingredient_color("peppercorns"), "#eed27a");
        assert_eq!(ingredient_color("zzz"), "#d8bf9a");
    }
}
