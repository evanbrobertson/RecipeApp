//! Heuristic parser that turns pasted recipe text (from a website, an email, notes,
//! a cookbook scan...) into structured fields. No network or LLM required.

use regex::Regex;
use serde_json::{Map, Value};
use std::sync::LazyLock;

use crate::model::{RecipeFields, Section, normalize_sections};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Preamble,
    Ingredients,
    Instructions,
    Notes,
    Nutrition,
}

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("valid regex")
}

static HEADINGS: LazyLock<Vec<(Mode, Regex)>> = LazyLock::new(|| {
    vec![
        (
            Mode::Ingredients,
            re(
                r"(?i)^(?:ingredients?(?: list)?|what you(?:'|’)?ll need|you(?:'|’)?ll need|you will need|shopping list)$",
            ),
        ),
        (
            Mode::Instructions,
            re(
                r"(?i)^(?:instructions?|directions?|method|steps|preparation|prep(?:aration)? steps|how to make(?: it)?|to make)$",
            ),
        ),
        (
            Mode::Notes,
            re(
                r"(?i)^(?:notes?|recipe notes?|tips?(?: (?:&|and) (?:notes|tricks))?|cook(?:'|’)?s notes?)$",
            ),
        ),
        (
            Mode::Nutrition,
            re(r"(?i)^(?:nutrition(?:al)?(?: (?:facts|info(?:rmation)?))?(?: per serving)?)$"),
        ),
    ]
});

#[derive(Clone, Copy)]
enum Meta {
    Prep,
    Cook,
    Total,
    Freeze,
    Yield,
    Cuisine,
    Category,
    Author,
}

impl Meta {
    fn is_time(self) -> bool {
        matches!(self, Meta::Prep | Meta::Cook | Meta::Total | Meta::Freeze)
    }

    fn slot(self, r: &mut RecipeFields) -> &mut Option<String> {
        match self {
            Meta::Prep => &mut r.prep_time,
            Meta::Cook => &mut r.cook_time,
            Meta::Total => &mut r.total_time,
            Meta::Freeze => &mut r.freeze_time,
            Meta::Yield => &mut r.recipe_yield,
            Meta::Cuisine => &mut r.recipe_cuisine,
            Meta::Category => &mut r.recipe_category,
            Meta::Author => &mut r.author,
        }
    }
}

// Time labels need a separator or the word "time" so prose like "Cook the pasta..." isn't matched
const SEP: &str = r"(?:\s+time\s*[:\-–]?|\s*[:\-–])\s*(.+)$";

static META: LazyLock<Vec<(Meta, Regex)>> = LazyLock::new(|| {
    vec![
        (Meta::Prep, re(&format!("(?i)^prep(?:aration)?{SEP}"))),
        (Meta::Cook, re(&format!("(?i)^cook(?:ing)?{SEP}"))),
        (Meta::Total, re(&format!("(?i)^(?:total|ready in){SEP}"))),
        (
            Meta::Freeze,
            re(&format!(
                "(?i)^(?:additional|chill(?:ing)?|rest(?:ing)?|inactive|freeze|freezing){SEP}"
            )),
        ),
        (
            Meta::Yield,
            re(r"(?i)^(?:serves|servings?|yield|makes)\s*[:\-–]?\s*(.+)$"),
        ),
        (Meta::Cuisine, re(r"(?i)^cuisine\s*[:\-–]\s*(.+)$")),
        (
            Meta::Category,
            re(r"(?i)^(?:course|category)\s*[:\-–]\s*(.+)$"),
        ),
        (
            Meta::Author,
            re(r"(?i)^(?:author|recipe by|by)\s*[:\-–]\s*(.+)$"),
        ),
    ]
});

static BULLET: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)^\s*(?:[-*•·▢☐□◦‣⁃–—]|\[\s?[x ]?\s?\])\s*"));
// "-" only counts as step numbering when whitespace follows ("2 - Mix"), not "2-3 cloves"
static NUMBERED: LazyLock<Regex> = LazyLock::new(|| {
    re(r"(?i)^\s*(?:step\s*)?(\d{1,2})\s*(?:[.):]|-\s)\s*|^\s*step\s*(\d{1,2})\s*$")
});
static QUANTITY: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?i)^(?:\d|[¼½¾⅓⅔⅛⅜⅝⅞]|a |an |one |two |three |four |five |six |half |pinch|dash|handful|splash|some |few )",
    )
});
static UNITS: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?i)\b(?:cups?|c\.|tbsps?|tablespoons?|tsps?|teaspoons?|grams?|g|kg|ml|l|litres?|liters?|oz|ounces?|lbs?|pounds?|pinch|cloves?|cans?|sticks?|slices?|bunch|sprigs?|packages?|pkg)\b",
    )
});
static COOKING_VERB: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?i)^(?:add|bake|beat|blend|boil|bring|combine|cook|cool|cover|cut|drain|fold|fry|grease|heat|let|line|melt|mix|place|pour|preheat|reduce|remove|roast|serve|simmer|slice|sprinkle|stir|strain|toss|transfer|whisk|season|chop|dice|garnish|allow|repeat|set|spread|top|turn|wash|rinse|put|make|using|in a|in the|once|meanwhile|when|while|after|then|finally)\b",
    )
});
static URL_LINE: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^https?://\S+$"));
static LEADING_HASHES: LazyLock<Regex> = LazyLock::new(|| re(r"^#+\s*"));
static HASH_HEADING: LazyLock<Regex> = LazyLock::new(|| re(r"^#+\s"));
static TRAILING_COLON: LazyLock<Regex> = LazyLock::new(|| re(r"[:：]\s*$"));
static BOLD: LazyLock<Regex> = LazyLock::new(|| re(r"^\*\*(.+)\*\*$"));
static FOR_THE: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^for (?:the )?[a-z][a-z\s&,'-]*$"));
static NUTRITION_LINE: LazyLock<Regex> =
    LazyLock::new(|| re(r"^([A-Za-z][A-Za-z\s]+?)\s*[:\-–]\s*(.+)$"));
static WS: LazyLock<Regex> = LazyLock::new(|| re(r"\s+"));

fn clean_line(line: &str) -> String {
    WS.replace_all(&line.replace('\u{a0}', " "), " ")
        .trim()
        .to_string()
}

fn heading_mode(line: &str) -> Option<Mode> {
    let t = LEADING_HASHES.replace(line, "");
    let t = TRAILING_COLON.replace(&t, "");
    let t = BOLD.replace(&t, "$1");
    let t = t.trim();
    HEADINGS
        .iter()
        .find(|(_, r)| r.is_match(t))
        .map(|(m, _)| *m)
}

/// A short line ending with ":" (or a "For the ..." line) that names a sub-section.
fn sub_heading(line: &str) -> Option<String> {
    let t = LEADING_HASHES.replace(line, "");
    let t = BOLD.replace(&t, "$1");
    let text = t.trim();
    if text.chars().count() > 60 {
        return None;
    }
    if (text.ends_with(':') || text.ends_with('：')) && !text.chars().any(|c| c.is_ascii_digit()) {
        return Some(text.trim_end_matches([':', '：']).trim().to_string());
    }
    if FOR_THE.is_match(text) && !QUANTITY.is_match(text) {
        return Some(text.to_string());
    }
    if HASH_HEADING.is_match(line) {
        return Some(text.to_string());
    }
    None
}

fn word_count(s: &str) -> usize {
    s.split(' ').count()
}

fn looks_like_ingredient(line: &str) -> bool {
    if line.chars().count() > 120 || COOKING_VERB.is_match(line) || line.ends_with('.') {
        return false;
    }
    QUANTITY.is_match(line) || (UNITS.is_match(line) && word_count(line) <= 10)
}

fn looks_like_instruction(line: &str) -> bool {
    line.chars().count() > 60
        || COOKING_VERB.is_match(line)
        || line.ends_with('.')
        || line.ends_with('!')
}

fn push_item(sections: &mut Vec<Section>, item: String) {
    if sections.is_empty() {
        sections.push(Section::default());
    }
    sections.last_mut().unwrap().items.push(item);
}

fn start_section(sections: &mut Vec<Section>, name: String) {
    match sections.last_mut() {
        Some(last) if last.items.is_empty() => last.name = Some(name),
        _ => sections.push(Section {
            name: Some(name),
            items: vec![],
        }),
    }
}

pub struct ParsedText {
    pub recipe: RecipeFields,
    /// True when the text had recognisable Ingredients/Instructions headings.
    pub structured: bool,
}

fn truncate(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

pub fn parse_recipe_text(input: &str) -> ParsedText {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<String> = normalized.split('\n').map(clean_line).collect();

    let mut recipe = RecipeFields::default();
    let mut preamble: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut nutrition = Map::new();
    let mut mode = Mode::Preamble;
    let mut structured = false;
    let mut last_was_numbered = false;

    for raw in &lines {
        if raw.is_empty() {
            last_was_numbered = false;
            continue;
        }

        // URL on its own line → remember as the source link
        if URL_LINE.is_match(raw) {
            if recipe.url.is_none() {
                recipe.url = Some(raw.clone());
            }
            continue;
        }

        if let Some(h) = heading_mode(raw) {
            mode = h;
            structured = true;
            last_was_numbered = false;
            continue;
        }

        // Metadata lines ("Prep time: 10 mins") appear before the ingredient/step lists
        if mode == Mode::Preamble
            && raw.chars().count() < 80
            && let Some((key, r)) = META.iter().find(|(_, r)| r.is_match(raw))
        {
            let value = r
                .captures(raw)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string());
            let plausible = value.as_deref().is_some_and(|v| {
                !v.is_empty() && (!key.is_time() || v.chars().any(|c| c.is_ascii_digit()))
            });
            let slot = key.slot(&mut recipe);
            if plausible && slot.as_deref().is_none_or(str::is_empty) {
                *slot = value;
                continue;
            }
        }

        let bulletless = BULLET.replace(raw, "").trim().to_string();
        // Numbering only means "step" in the instructions; in ingredients "2-3 cloves" is a quantity
        let numbered =
            matches!(mode, Mode::Instructions | Mode::Preamble) && NUMBERED.is_match(raw);
        let text = if numbered {
            NUMBERED.replace(raw, "").trim().to_string()
        } else {
            bulletless
        };

        match mode {
            Mode::Preamble => {
                if recipe.title.is_empty() {
                    recipe.title = LEADING_HASHES.replace(&text, "").into_owned();
                } else {
                    preamble.push(text);
                }
            }
            Mode::Ingredients => match sub_heading(raw) {
                Some(sub) => start_section(&mut recipe.ingredients, sub),
                None => push_item(&mut recipe.ingredients, text),
            },
            Mode::Instructions => {
                if numbered && text.is_empty() {
                    // "Step 3" on its own line: the next line starts a new step
                    last_was_numbered = false;
                    continue;
                }
                if !numbered && let Some(sub) = sub_heading(raw) {
                    start_section(&mut recipe.instructions, sub);
                    last_was_numbered = false;
                    continue;
                }
                // A wrapped continuation of a numbered step
                if !numbered
                    && last_was_numbered
                    && !BULLET.is_match(raw)
                    && let Some(current) = recipe.instructions.last_mut()
                    && let Some(last) = current.items.last_mut()
                {
                    last.push(' ');
                    last.push_str(&text);
                    continue;
                }
                push_item(&mut recipe.instructions, text);
                last_was_numbered = numbered;
            }
            Mode::Notes => notes.push(text),
            Mode::Nutrition => match NUTRITION_LINE.captures(&text) {
                Some(c) => {
                    nutrition.insert(
                        c[1].trim().to_lowercase(),
                        Value::String(c[2].trim().to_string()),
                    );
                }
                None => notes.push(text),
            },
        }
    }

    // No headings found: classify lines by shape
    if !structured {
        for line in std::mem::take(&mut preamble) {
            let t = BULLET.replace(&line, "");
            let text = NUMBERED.replace(&t, "").trim().to_string();
            let in_ingredients = !recipe.ingredients.is_empty() && recipe.instructions.is_empty();
            let short_item = word_count(&text) <= 8 && !looks_like_instruction(&text);
            if recipe.instructions.is_empty()
                && (looks_like_ingredient(&text) || (in_ingredients && short_item))
            {
                push_item(&mut recipe.ingredients, text);
            } else if !recipe.ingredients.is_empty() {
                push_item(&mut recipe.instructions, text);
            } else {
                preamble.push(line);
            }
        }
    }

    recipe.title = if recipe.title.is_empty() {
        "Untitled recipe".into()
    } else {
        truncate(&recipe.title, 300)
    };
    recipe.description = Some(truncate(&preamble.join(" "), 1000)).filter(|d| !d.is_empty());
    recipe.notes = Some(notes.join("\n")).filter(|n| !n.is_empty());
    recipe.nutrition = if nutrition.is_empty() {
        None
    } else {
        Some(nutrition)
    };
    recipe.ingredients = normalize_sections(recipe.ingredients);
    recipe.instructions = normalize_sections(recipe.instructions);

    ParsedText { recipe, structured }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_structured_text() {
        let text = "Grandma's Chili\nA cozy weeknight pot.\nPrep time: 15 mins\nCook: 1 hour\nServes 6\n\n\
            https://chili.test/recipe\n\nIngredients\nFor the chili:\n- 1 lb beef\n- 2-3 cloves garlic\n\
            Toppings:\n• sour cream\n\nInstructions\n1. Brown the beef in a large pot over\nmedium heat.\n\
            2) Add garlic and simmer.\nStep 3\nServe hot!\n\nNotes\nFreezes well.\n\nNutrition\nCalories: 450\nProtein - 30g";
        let p = parse_recipe_text(text);
        assert!(p.structured);
        let r = p.recipe;
        assert_eq!(r.title, "Grandma's Chili");
        assert_eq!(r.description.as_deref(), Some("A cozy weeknight pot."));
        assert_eq!(r.prep_time.as_deref(), Some("15 mins"));
        assert_eq!(r.cook_time.as_deref(), Some("1 hour"));
        assert_eq!(r.recipe_yield.as_deref(), Some("6"));
        assert_eq!(r.url.as_deref(), Some("https://chili.test/recipe"));
        assert_eq!(r.ingredients.len(), 2);
        assert_eq!(r.ingredients[0].name.as_deref(), Some("For the chili"));
        assert_eq!(
            r.ingredients[0].items,
            vec!["1 lb beef", "2-3 cloves garlic"]
        );
        assert_eq!(r.ingredients[1].items, vec!["sour cream"]);
        assert_eq!(
            r.instructions[0].items,
            vec![
                "Brown the beef in a large pot over medium heat.",
                "Add garlic and simmer.",
                "Serve hot!"
            ]
        );
        assert_eq!(r.notes.as_deref(), Some("Freezes well."));
        let n = r.nutrition.unwrap();
        assert_eq!(n["calories"], "450");
        assert_eq!(n["protein"], "30g");
    }

    #[test]
    fn classifies_unstructured_text() {
        let text = "Quick Pancakes\n1 cup flour\n1 egg\n1 cup milk\nWhisk everything together until smooth.\nFry in a hot pan.";
        let p = parse_recipe_text(text);
        assert!(!p.structured);
        assert_eq!(p.recipe.title, "Quick Pancakes");
        assert_eq!(
            p.recipe.ingredients[0].items,
            vec!["1 cup flour", "1 egg", "1 cup milk"]
        );
        assert_eq!(p.recipe.instructions[0].items.len(), 2);
    }

    #[test]
    fn prose_about_cooking_is_not_a_time() {
        let p = parse_recipe_text("Pasta\nCook the pasta until tender\nIngredients\npasta");
        assert!(p.recipe.cook_time.is_none());
        assert_eq!(
            p.recipe.description.as_deref(),
            Some("Cook the pasta until tender")
        );
    }
}
