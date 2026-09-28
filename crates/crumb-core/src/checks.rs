//! Wording for Wee Chef's import checks (`web/src/lib/checks.ts`), so every app says what
//! Wee Chef did and why a line needs a look in the same words as the web.

/// A line quoted in a sentence, shortened with "…" when longer than `max` characters.
pub fn quote(text: &str, max: usize) -> String {
    let t = text.trim();
    if t.chars().count() > max {
        let cut: String = t.chars().take(max.saturating_sub(1)).collect();
        format!("“{}…”", cut.trim_end())
    } else {
        format!("“{t}”")
    }
}

/// What Wee Chef did to a line on import, e.g. Made “Sauce:” a section heading.
/// `fix`, `category` and `was` come from the flag's `detail` object.
pub fn fix_text(
    fix: Option<&str>,
    item_text: &str,
    category: Option<&str>,
    was: Option<&str>,
) -> String {
    let q = |t: &str| quote(t, 48);
    match fix {
        Some("heading") => format!("Made {} a section heading", q(item_text)),
        Some("removed") => format!("Removed {}", q(item_text)),
        Some("notes") => format!("Moved a tip to the notes: {}", q(item_text)),
        Some("joined") => format!("Joined a step that was split in two: {}", q(item_text)),
        Some("tidy") => {
            "Cleaned up stray checkboxes, web codes, repeated lines, quantities or times".into()
        }
        Some("category") => match (category.filter(|c| !c.is_empty()), was) {
            (Some(c), Some(w)) if !w.is_empty() => {
                format!("Changed the category from {} to {c}", q(w))
            }
            (Some(c), _) => format!("Set the category to {c}"),
            (None, w) => format!(
                "Cleared the category {}: it isn't one of Crumb's",
                q(w.unwrap_or(""))
            ),
        },
        _ => format!("Tidied {}", q(item_text)),
    }
}

/// Why a flagged line might need a look: “Sauce:” {looks like a section heading}.
pub fn review_text(kind: &str) -> &'static str {
    match kind {
        "heading" => "looks like a section heading",
        "junk" => "doesn't look like part of the recipe",
        "fragment" => "looks like part of the step next to it",
        "not_instruction" => "reads like a tip rather than a step",
        "merged" => "might be two ingredients on one line",
        "step" => "looks like a step, not an ingredient",
        "ingredient" => "looks like an ingredient, not a step",
        _ => "might need a look",
    }
}

fn plural(n: u32, one: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {one}s")
    }
}

/// `GET /api/checks` as the numbers `checks_status_text` reads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChecksCounts {
    pub eligible: u32,
    pub checked: u32,
    pub pending: u32,
    pub tidied: u32,
    pub to_check: u32,
    pub due: u32,
    pub restored: u32,
    pub edited: u32,
}

/// One line on where Check all stands: "5 recipes to check (2 restored) · …",
/// "Checking… 3 of 12 done", "All 79 recipes checked". `run` is how many this Check all
/// queued, so progress counts failures as done too.
pub fn checks_status_text(c: ChecksCounts, run: Option<u32>) -> String {
    if c.pending > 0 {
        if let Some(run) = run.filter(|r| *r >= c.pending && *r > 0) {
            return format!("Checking… {} of {run} done", run - c.pending);
        }
        return format!("Checking… {} of {} done", c.checked, c.eligible);
    }
    let mut parts = Vec::new();
    if c.due > 0 {
        let mut why = Vec::new();
        if c.restored > 0 {
            why.push(format!("{} restored", c.restored));
        }
        if c.edited > 0 {
            why.push(format!("{} edited since", c.edited));
        }
        let why = if why.is_empty() {
            String::new()
        } else {
            format!(" ({})", why.join(", "))
        };
        parts.push(format!("{} to check{why}", plural(c.due, "recipe")));
        parts.push("suggests fixes, tidies only stray symbols".into());
    } else if c.checked < c.eligible {
        parts.push(format!("{} of {} checked", c.checked, c.eligible));
    } else {
        parts.push(format!("All {} checked", plural(c.checked, "recipe")));
    }
    if c.tidied > 0 {
        parts.push(format!("{} tidied", plural(c.tidied, "thing")));
    }
    if c.to_check > 0 {
        parts.push(format!("{} to look at", plural(c.to_check, "recipe")));
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_shorten_long_lines() {
        assert_eq!(quote(" Sauce: ", 48), "“Sauce:”");
        assert_eq!(quote("abcdefghij", 5), "“abcd…”");
    }

    #[test]
    fn fixes_read_like_the_web() {
        assert_eq!(
            fix_text(Some("heading"), "Sauce:", None, None),
            "Made “Sauce:” a section heading"
        );
        assert_eq!(
            fix_text(Some("category"), "", Some("Dessert"), Some("Pudding")),
            "Changed the category from “Pudding” to Dessert"
        );
        assert_eq!(
            fix_text(Some("category"), "", Some("Dessert"), None),
            "Set the category to Dessert"
        );
        assert_eq!(
            fix_text(Some("category"), "", None, Some("Stuff")),
            "Cleared the category “Stuff”: it isn't one of Crumb's"
        );
        assert_eq!(fix_text(None, "x", None, None), "Tidied “x”");
        assert_eq!(
            review_text("merged"),
            "might be two ingredients on one line"
        );
        assert_eq!(review_text("new"), "might need a look");
    }

    #[test]
    fn status_lines() {
        let running = ChecksCounts {
            eligible: 12,
            checked: 3,
            pending: 9,
            ..Default::default()
        };
        assert_eq!(checks_status_text(running, None), "Checking… 3 of 12 done");
        assert_eq!(
            checks_status_text(running, Some(10)),
            "Checking… 1 of 10 done"
        );
        let due = ChecksCounts {
            eligible: 10,
            checked: 5,
            due: 5,
            restored: 2,
            tidied: 1,
            ..Default::default()
        };
        assert_eq!(
            checks_status_text(due, None),
            "5 recipes to check (2 restored) · suggests fixes, tidies only stray symbols · 1 thing tidied"
        );
        let done = ChecksCounts {
            eligible: 79,
            checked: 79,
            to_check: 1,
            ..Default::default()
        };
        assert_eq!(
            checks_status_text(done, None),
            "All 79 recipes checked · 1 recipe to look at"
        );
        let failed = ChecksCounts {
            eligible: 4,
            checked: 3,
            ..Default::default()
        };
        assert_eq!(checks_status_text(failed, None), "3 of 4 checked");
    }
}
