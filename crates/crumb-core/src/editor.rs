//! The recipe editor's form as text: ingredients and steps as one textarea per section (one
//! item per line), nutrition as "key: value" lines, and Wee Chef's one-tap fixes for lines it
//! flagged. Ported from `web/src/components/RecipeEditor.svelte`.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::categories;
use crate::model::{Recipe, RecipeFields, Section};

/// One section as the editor holds it: a name and one item per line.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftSection {
    pub name: String,
    pub text: String,
}

/// The whole form as text, before [`RecipeDraft::to_fields`] turns it into a save.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

impl Default for RecipeDraft {
    /// A blank form: one empty section each for ingredients and steps.
    fn default() -> Self {
        Self::new("")
    }
}

/// Which of the two section lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SectionKind {
    Ingredients,
    Instructions,
}

impl SectionKind {
    /// The flag `field` for this list.
    pub fn field(self) -> &'static str {
        match self {
            Self::Ingredients => "ingredients",
            Self::Instructions => "instructions",
        }
    }
}

/// The editor's time, yield and label fields, in the web's order: `(key, label, placeholder)`.
pub const META_FIELDS: [(&str, &str, &str); 8] = [
    ("prepTime", "Prep time", "15m"),
    ("cookTime", "Cook time", "30m"),
    ("freezeTime", "Extra time", "Chill 1h"),
    ("totalTime", "Total time", "1h 45m"),
    ("recipeYield", "Yield", "4 servings"),
    ("recipeCategory", "Category", ""),
    ("recipeCuisine", "Cuisine", "Italian"),
    ("author", "Author", ""),
];

static ITEM_MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:[-*•]|\d+[.)])\s+").unwrap());
static HEADING_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[:.\s]+$").unwrap());

/// Sections as drafts, or one blank section when there are none.
pub fn draft_sections(sections: &[Section]) -> Vec<DraftSection> {
    if sections.is_empty() {
        return vec![DraftSection::default()];
    }
    sections
        .iter()
        .map(|s| DraftSection {
            name: s.name.clone().unwrap_or_default(),
            text: s.items.join("\n"),
        })
        .collect()
}

/// Drafts as sections: bullets and numbering gone, blank lines dropped, empty sections gone.
pub fn section_list(sections: &[DraftSection]) -> Vec<Section> {
    sections
        .iter()
        .filter_map(|s| {
            let items: Vec<String> = s
                .text
                .split('\n')
                .map(|line| ITEM_MARKER.replace(line, "").trim().to_string())
                .filter(|line| !line.is_empty())
                .collect();
            (!items.is_empty()).then(|| Section {
                name: nullable(&s.name),
                items,
            })
        })
        .collect()
}

/// A text field's value, or None when it's blank, so the server clears it.
pub fn nullable(value: &str) -> Option<String> {
    let v = value.trim();
    (!v.is_empty()).then(|| v.to_string())
}

/// Nutrition as "key: value" lines, in the recipe's order.
pub fn nutrition_text(nutrition: Option<&Value>) -> String {
    let Some(Value::Object(map)) = nutrition else {
        return String::new();
    };
    map.iter()
        .map(|(k, v)| match v {
            Value::String(s) => format!("{k}: {s}"),
            Value::Null => format!("{k}: null"),
            other => format!("{k}: {other}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// "calories: 320" lines as a nutrition map: split at the first colon, and lines without a
/// key and a value are left out. A repeated key keeps its first place and its last value.
pub fn parse_nutrition(text: &str) -> Map<String, Value> {
    let mut map = Map::new();
    for line in text.split('\n') {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        if !key.is_empty() && !value.is_empty() {
            map.insert(key.to_string(), Value::String(value.to_string()));
        }
    }
    map
}

impl RecipeDraft {
    /// A new recipe's form, with the title "From scratch" was given.
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            description: String::new(),
            author: String::new(),
            prep_time: String::new(),
            cook_time: String::new(),
            freeze_time: String::new(),
            total_time: String::new(),
            recipe_yield: String::new(),
            recipe_category: String::new(),
            recipe_cuisine: String::new(),
            url: String::new(),
            image: String::new(),
            video: String::new(),
            notes: String::new(),
            nutrition: String::new(),
            ingredients: vec![DraftSection::default()],
            instructions: vec![DraftSection::default()],
        }
    }

    /// A saved recipe as a form.
    pub fn from_recipe(r: &Recipe) -> Self {
        let s = |v: &Option<String>| v.clone().unwrap_or_default();
        Self {
            title: r.title.clone(),
            description: s(&r.description),
            author: s(&r.author),
            prep_time: s(&r.prep_time),
            cook_time: s(&r.cook_time),
            freeze_time: s(&r.freeze_time),
            total_time: s(&r.total_time),
            recipe_yield: s(&r.recipe_yield),
            recipe_category: s(&r.recipe_category),
            recipe_cuisine: s(&r.recipe_cuisine),
            url: s(&r.url),
            image: s(&r.image),
            video: s(&r.video),
            notes: s(&r.notes),
            nutrition: nutrition_text(r.nutrition.as_ref()),
            ingredients: draft_sections(&r.ingredients),
            instructions: draft_sections(&r.instructions),
        }
    }

    /// Why the form can't be saved yet, as the editor says it.
    pub fn problem(&self) -> Option<&'static str> {
        self.title
            .trim()
            .is_empty()
            .then_some("Give the recipe a title.")
    }

    /// The form as a save. Blank fields go as None, never "".
    pub fn to_fields(&self) -> RecipeFields {
        let nutrition = parse_nutrition(&self.nutrition);
        RecipeFields {
            title: self.title.trim().to_string(),
            description: nullable(&self.description),
            url: nullable(&self.url),
            image: nullable(&self.image),
            author: nullable(&self.author),
            prep_time: nullable(&self.prep_time),
            cook_time: nullable(&self.cook_time),
            total_time: nullable(&self.total_time),
            freeze_time: nullable(&self.freeze_time),
            recipe_yield: nullable(&self.recipe_yield),
            recipe_category: nullable(&self.recipe_category),
            recipe_cuisine: nullable(&self.recipe_cuisine),
            ingredients: section_list(&self.ingredients),
            instructions: section_list(&self.instructions),
            nutrition: (!nutrition.is_empty()).then_some(nutrition),
            notes: nullable(&self.notes),
            video: nullable(&self.video),
        }
    }

    /// The form's recipe as the create/update JSON body (camelCase, as the API takes it).
    pub fn to_json(&self) -> Value {
        let f = self.to_fields();
        let sections = |list: &[Section]| {
            Value::Array(
                list.iter()
                    .map(|s| serde_json::json!({ "name": s.name, "items": s.items }))
                    .collect(),
            )
        };
        serde_json::json!({
            "title": f.title,
            "description": f.description,
            "author": f.author,
            "prepTime": f.prep_time,
            "cookTime": f.cook_time,
            "freezeTime": f.freeze_time,
            "totalTime": f.total_time,
            "recipeYield": f.recipe_yield,
            "recipeCategory": f.recipe_category,
            "recipeCuisine": f.recipe_cuisine,
            "url": f.url,
            "image": f.image,
            "video": f.video,
            "notes": f.notes,
            "nutrition": f.nutrition.map(Value::Object),
            "ingredients": sections(&f.ingredients),
            "instructions": sections(&f.instructions),
        })
    }

    fn sections(&self, kind: SectionKind) -> &Vec<DraftSection> {
        match kind {
            SectionKind::Ingredients => &self.ingredients,
            SectionKind::Instructions => &self.instructions,
        }
    }

    fn sections_mut(&mut self, kind: SectionKind) -> &mut Vec<DraftSection> {
        match kind {
            SectionKind::Ingredients => &mut self.ingredients,
            SectionKind::Instructions => &mut self.instructions,
        }
    }
}

/// A category from before the fixed list: it stays selected, marked old, until changed.
pub fn legacy_category(category: &str) -> Option<String> {
    (!category.is_empty() && !categories::LIST.contains(&category)).then(|| category.to_string())
}

/// Whether a review flag belongs under this section: it's for this list and the section
/// still has its line. Editing the line away clears the hint; the server resolves it on save.
pub fn flag_is_here(
    kind: SectionKind,
    section: &DraftSection,
    field: &str,
    state: &str,
    item_text: &str,
) -> bool {
    field == kind.field()
        && state == "review"
        && section.text.split('\n').any(|l| l.trim() == item_text)
}

/// Whether a review flag on the photo still applies: the draft still has that link.
pub fn photo_flag_is_here(draft: &RecipeDraft, field: &str, state: &str, item_text: &str) -> bool {
    field == "image" && state == "review" && item_text == draft.image
}

/// A one-tap fix: the button's label and the form it leaves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EditorFix {
    pub label: &'static str,
    pub draft: RecipeDraft,
}

/// The fix offered for a flagged line (`kind` is the flag's kind, `item_text` its line) in
/// section `index` of the list, or None when the line is gone or nothing fits (`fixFor`).
pub fn fix_for(
    draft: &RecipeDraft,
    list: SectionKind,
    index: usize,
    kind: &str,
    item_text: &str,
) -> Option<EditorFix> {
    let section = draft.sections(list).get(index)?.clone();
    let all: Vec<&str> = section.text.split('\n').collect();
    let at = all.iter().position(|l| l.trim() == item_text)?;
    let without = |drop: usize| -> String {
        all.iter()
            .enumerate()
            .filter(|(j, _)| *j != drop)
            .map(|(_, l)| *l)
            .collect::<Vec<_>>()
            .join("\n")
    };
    let mut next = draft.clone();
    let label = match kind {
        "heading" => {
            // A heading needs lines under it, or the empty section would vanish on save
            if !all[at + 1..].iter().any(|l| !l.trim().is_empty()) {
                return None;
            }
            let name = HEADING_END.replace(item_text, "").into_owned();
            let before = all[..at].join("\n").trim().to_string();
            let after = all[at + 1..].join("\n");
            let sections = next.sections_mut(list);
            if before.is_empty() && section.name.trim().is_empty() {
                sections[index] = DraftSection { name, text: after };
            } else {
                sections[index].text = before;
                sections.insert(index + 1, DraftSection { name, text: after });
            }
            "Make it a heading"
        }
        "junk" => {
            next.sections_mut(list)[index].text = without(at);
            "Remove it"
        }
        "not_instruction" if list == SectionKind::Instructions => {
            next.sections_mut(list)[index].text = without(at);
            next.notes = [draft.notes.trim(), item_text]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
            "Move to notes"
        }
        "fragment" if at > 0 => {
            let mut lines: Vec<String> = all.iter().map(|l| l.to_string()).collect();
            lines[at - 1] = format!("{} {}", lines[at - 1].trim_end(), lines[at].trim());
            lines.remove(at);
            next.sections_mut(list)[index].text = lines.join("\n");
            "Join with the step above"
        }
        _ => return None,
    };
    Some(EditorFix { label, draft: next })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_steps(text: &str) -> RecipeDraft {
        let mut d = RecipeDraft::new("Soup");
        d.instructions = vec![DraftSection {
            name: String::new(),
            text: text.into(),
        }];
        d
    }

    #[test]
    fn sections_lose_markers_and_blanks() {
        let drafts = vec![
            DraftSection {
                name: " Sauce ".into(),
                text: "- 1 onion\n\n2) 2 cloves garlic\n• salt".into(),
            },
            DraftSection {
                name: "Empty".into(),
                text: "  \n".into(),
            },
        ];
        let out = section_list(&drafts);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name.as_deref(), Some("Sauce"));
        assert_eq!(out[0].items, vec!["1 onion", "2 cloves garlic", "salt"]);
        // "2 eggs" has an amount, not a marker
        let eggs = section_list(&[DraftSection {
            name: String::new(),
            text: "2 eggs".into(),
        }]);
        assert_eq!(eggs[0].items, vec!["2 eggs"]);
        assert_eq!(draft_sections(&[]), vec![DraftSection::default()]);
    }

    #[test]
    fn nutrition_round_trips() {
        let map = parse_nutrition("calories: 320\nnope\nfatContent: 12 g\nurl: http://x");
        assert_eq!(map.len(), 3);
        assert_eq!(map["url"], "http://x");
        let text = nutrition_text(Some(&Value::Object(map)));
        assert_eq!(text, "calories: 320\nfatContent: 12 g\nurl: http://x");
        assert_eq!(
            nutrition_text(Some(&serde_json::json!({"calories": 320}))),
            "calories: 320"
        );
    }

    #[test]
    fn blank_fields_save_as_none() {
        let mut d = RecipeDraft::new("  Pie ");
        d.notes = "  ".into();
        let f = d.to_fields();
        assert_eq!(f.title, "Pie");
        assert_eq!(f.notes, None);
        assert_eq!(f.nutrition, None);
        assert!(f.ingredients.is_empty());
        assert_eq!(
            RecipeDraft::new(" ").problem(),
            Some("Give the recipe a title.")
        );
        assert_eq!(d.to_json()["prepTime"], Value::Null);
    }

    #[test]
    fn heading_fix_names_the_section_or_splits_it() {
        let d = with_steps("For the sauce:\nStir.\nSimmer.");
        let fix = fix_for(
            &d,
            SectionKind::Instructions,
            0,
            "heading",
            "For the sauce:",
        )
        .unwrap();
        assert_eq!(fix.label, "Make it a heading");
        assert_eq!(
            fix.draft.instructions,
            vec![DraftSection {
                name: "For the sauce".into(),
                text: "Stir.\nSimmer.".into()
            }]
        );
        let d = with_steps("Boil.\nFor the sauce:\nStir.");
        let fix = fix_for(
            &d,
            SectionKind::Instructions,
            0,
            "heading",
            "For the sauce:",
        )
        .unwrap();
        assert_eq!(fix.draft.instructions.len(), 2);
        assert_eq!(fix.draft.instructions[0].text, "Boil.");
        assert_eq!(fix.draft.instructions[1].name, "For the sauce");
        // Nothing under it
        let d = with_steps("Boil.\nServe:");
        assert!(fix_for(&d, SectionKind::Instructions, 0, "heading", "Serve:").is_none());
    }

    #[test]
    fn other_fixes() {
        let mut d = with_steps("Boil.\nJump to recipe\nand stir.");
        d.notes = "Old note".into();
        let junk = fix_for(&d, SectionKind::Instructions, 0, "junk", "Jump to recipe").unwrap();
        assert_eq!(junk.draft.instructions[0].text, "Boil.\nand stir.");
        let moved = fix_for(
            &d,
            SectionKind::Instructions,
            0,
            "not_instruction",
            "Jump to recipe",
        )
        .unwrap();
        assert_eq!(moved.draft.notes, "Old note\n\nJump to recipe");
        let joined = fix_for(&d, SectionKind::Instructions, 0, "fragment", "and stir.").unwrap();
        assert_eq!(
            joined.draft.instructions[0].text,
            "Boil.\nJump to recipe and stir."
        );
        assert!(fix_for(&d, SectionKind::Instructions, 0, "fragment", "Boil.").is_none());
        assert!(fix_for(&d, SectionKind::Instructions, 0, "junk", "gone").is_none());
        assert!(fix_for(&d, SectionKind::Instructions, 3, "junk", "Boil.").is_none());
        let mut ing = RecipeDraft::new("x");
        ing.ingredients[0].text = "salt".into();
        assert!(fix_for(&ing, SectionKind::Ingredients, 0, "not_instruction", "salt").is_none());
    }

    #[test]
    fn flags_follow_their_line() {
        let s = DraftSection {
            name: String::new(),
            text: " Jump \nBoil.".into(),
        };
        assert!(flag_is_here(
            SectionKind::Instructions,
            &s,
            "instructions",
            "review",
            "Jump"
        ));
        assert!(!flag_is_here(
            SectionKind::Instructions,
            &s,
            "instructions",
            "fixed",
            "Jump"
        ));
        assert!(!flag_is_here(
            SectionKind::Ingredients,
            &s,
            "instructions",
            "review",
            "Jump"
        ));
        assert_eq!(legacy_category("Weird"), Some("Weird".into()));
        assert_eq!(legacy_category(""), None);
    }
}
