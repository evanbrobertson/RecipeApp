//! The recipe page's wording: nutrition labels, Wee Chef's note (`WeeChefCard.svelte`) and the
//! toast when a check someone asked for finishes (`RecipePage.svelte`).

use serde_json::Value;

use crate::checks::plural;

const NUTRITION_LABELS: [(&str, &str); 12] = [
    ("calories", "Calories"),
    ("fatContent", "Fat"),
    ("saturatedFatContent", "Saturated fat"),
    ("unsaturatedFatContent", "Unsaturated fat"),
    ("transFatContent", "Trans fat"),
    ("carbohydrateContent", "Carbs"),
    ("sugarContent", "Sugar"),
    ("fiberContent", "Fiber"),
    ("proteinContent", "Protein"),
    ("cholesterolContent", "Cholesterol"),
    ("sodiumContent", "Sodium"),
    ("servingSize", "Serving size"),
];

/// A nutrition key as a label: "Carbs" for `carbohydrateContent`, else the key without a
/// trailing "Content".
pub fn nutrition_label(key: &str) -> String {
    NUTRITION_LABELS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, label)| (*label).to_string())
        .unwrap_or_else(|| key.strip_suffix("Content").unwrap_or(key).to_string())
}

/// "Wee Chef tidied 2 things".
pub fn tidied_title(fixed: u32) -> String {
    format!("Wee Chef tidied {}", plural(fixed, "thing"))
}

/// "3 lines might need a look".
pub fn look_title(review: u32) -> String {
    format!("{} might need a look", plural(review, "line"))
}

/// How many things one fixed flag counts for: a tidy counts each small thing it cleaned up.
pub fn fix_weight(detail: &Value) -> u32 {
    if detail.get("fix").and_then(Value::as_str) == Some("tidy") {
        detail
            .get("count")
            .and_then(Value::as_u64)
            .map_or(1, |n| n as u32)
    } else {
        1
    }
}

/// The toast when a check someone asked for is done: `(title, description)`. `fixed` counts
/// only this check's fixes (by [`fix_weight`]); `review` leaves out the photo's flag.
pub fn check_done_toast(status: &str, fixed: u32, review: u32) -> (String, Option<String>) {
    if status == "failed" {
        return ("Wee Chef couldn't check this recipe".into(), None);
    }
    match (fixed, review) {
        (0, 0) => ("Wee Chef found nothing to change".into(), None),
        (0, r) => (look_title(r), None),
        (f, 0) => (tidied_title(f), None),
        (f, r) => (tidied_title(f), Some(look_title(r))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn nutrition_labels() {
        assert_eq!(nutrition_label("carbohydrateContent"), "Carbs");
        assert_eq!(nutrition_label("potassiumContent"), "potassium");
        assert_eq!(nutrition_label("other"), "other");
    }

    #[test]
    fn check_toasts() {
        assert_eq!(fix_weight(&json!({"fix": "tidy", "count": 4})), 4);
        assert_eq!(fix_weight(&json!({"fix": "tidy"})), 1);
        assert_eq!(fix_weight(&json!({"fix": "unit"})), 1);
        assert_eq!(
            check_done_toast("done", 2, 1),
            (
                "Wee Chef tidied 2 things".into(),
                Some("1 line might need a look".into())
            )
        );
        assert_eq!(
            check_done_toast("done", 0, 0).0,
            "Wee Chef found nothing to change"
        );
        assert_eq!(
            check_done_toast("failed", 3, 3).0,
            "Wee Chef couldn't check this recipe"
        );
    }
}
