//! The trash's wording: how long a deleted recipe has left, and what deleting, emptying and
//! putting back say. Ported from `web/src/islands/TrashPage.svelte`, `RecipePage.svelte` and
//! `RecipesPage.svelte`.

const DAY_MS: i64 = 86_400_000;

fn recipes(n: u32) -> String {
    format!("{n} recipe{}", if n == 1 { "" } else { "s" })
}

/// "12 days left", counted to the day it goes; "Goes for good today" on its last day.
/// `purge_at_ms` and `now_ms` are Unix milliseconds.
pub fn days_left(purge_at_ms: i64, now_ms: i64) -> String {
    let days = (purge_at_ms - now_ms)
        .max(0)
        .unsigned_abs()
        .div_ceil(DAY_MS as u64);
    if days <= 1 {
        "Goes for good today".into()
    } else {
        format!("{days} days left")
    }
}

/// The single delete's confirmation.
pub const DELETE_ONE: &str = "It goes to the trash, where you can put it back for 30 days.";

/// The bulk delete's confirmation: "3 recipes will go to the trash, …".
pub fn delete_many(n: u32) -> String {
    let them = if n == 1 { "it" } else { "them" };
    format!(
        "{} will go to the trash, where you can put {them} back for 30 days.",
        recipes(n)
    )
}

/// The toast after a bulk delete: "Moved 3 recipes to the trash".
pub fn moved_to_trash(n: u32) -> String {
    format!("Moved {} to the trash", recipes(n))
}

/// The toast after Undo: "Put back", or "Put back 3 recipes".
pub fn put_back(n: u32) -> String {
    if n == 1 {
        "Put back".into()
    } else {
        format!("Put back {}", recipes(n))
    }
}

/// "Empty the trash?": "3 recipes will be deleted for good. This can't be undone."
pub fn empty_trash(n: u32) -> String {
    format!(
        "{} will be deleted for good. This can't be undone.",
        recipes(n)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_down_to_the_day_it_goes() {
        let now = 1_000 * DAY_MS;
        assert_eq!(days_left(now + 12 * DAY_MS, now), "12 days left");
        assert_eq!(days_left(now + 11 * DAY_MS + 1, now), "12 days left");
        assert_eq!(days_left(now + 2 * DAY_MS, now), "2 days left");
        assert_eq!(days_left(now + DAY_MS, now), "Goes for good today");
        assert_eq!(days_left(now + 5, now), "Goes for good today");
        assert_eq!(days_left(now - DAY_MS, now), "Goes for good today");
    }

    #[test]
    fn words_deleting_and_putting_back() {
        assert_eq!(
            delete_many(1),
            "1 recipe will go to the trash, where you can put it back for 30 days."
        );
        assert_eq!(
            delete_many(3),
            "3 recipes will go to the trash, where you can put them back for 30 days."
        );
        assert_eq!(moved_to_trash(1), "Moved 1 recipe to the trash");
        assert_eq!(moved_to_trash(2), "Moved 2 recipes to the trash");
        assert_eq!(put_back(1), "Put back");
        assert_eq!(put_back(4), "Put back 4 recipes");
        assert_eq!(
            empty_trash(2),
            "2 recipes will be deleted for good. This can't be undone."
        );
    }
}
