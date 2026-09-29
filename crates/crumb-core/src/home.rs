//! Home's wording: the greeting on the tile header, and the day and source lines under
//! new and recently viewed recipes. Ported from `web/src/pages/index.astro` and
//! `web/src/islands/HomeFeed.svelte`, in English (the web formats dates in en-US).
//!
//! Times are Unix milliseconds; `offset_minutes` is the viewer's UTC offset (east
//! positive), so "today" starts at their local midnight.

use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveTime, TimeZone, Utc};

/// The handwritten greeting for a local hour (0–23).
pub fn greeting(hour: u32) -> &'static str {
    match hour {
        0..5 => "midnight snack?",
        5..11 => "what's for breakfast?",
        11..15 => "what's for lunch?",
        15..21 => "what's cooking tonight?",
        _ => "something sweet?",
    }
}

fn local(ms: i64, offset_minutes: i32) -> Option<DateTime<FixedOffset>> {
    let zone = FixedOffset::east_opt(offset_minutes * 60)?;
    Some(Utc.timestamp_millis_opt(ms).single()?.with_timezone(&zone))
}

const WEEKDAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// "Today", "Yesterday", a weekday within the last week, else "Sep 20" (`dayLabel`).
pub fn day_label(ms: i64, now_ms: i64, offset_minutes: i32) -> String {
    let (Some(at), Some(now)) = (local(ms, offset_minutes), local(now_ms, offset_minutes)) else {
        return String::new();
    };
    let midnight = now
        .with_time(NaiveTime::MIN)
        .single()
        .unwrap_or(now)
        .timestamp_millis();
    const DAY: i64 = Duration::days(1).num_milliseconds();
    if ms >= midnight {
        "Today".into()
    } else if ms >= midnight - DAY {
        "Yesterday".into()
    } else if ms >= midnight - 6 * DAY {
        WEEKDAYS[at.weekday().num_days_from_monday() as usize].into()
    } else {
        format!("{} {}", MONTHS[at.month0() as usize], at.day())
    }
}

/// A full date as the web prints one ("Mar 11, 2025": `toLocaleDateString` with day, short
/// month and year, in en-US), for shared links, connected apps, devices and invites.
pub fn date_label(ms: i64, offset_minutes: i32) -> String {
    match local(ms, offset_minutes) {
        Some(at) => format!(
            "{} {}, {}",
            MONTHS[at.month0() as usize],
            at.day(),
            at.year()
        ),
        None => String::new(),
    }
}

/// How long a recipe has left in the trash, counted up to the day it goes: "12 days left",
/// or "Goes for good today" on its last day (`left` on the web's Trash page).
pub fn trash_left(purge_ms: i64, now_ms: i64) -> String {
    const DAY: i64 = 86_400_000;
    let days = ((purge_ms - now_ms).max(0) + DAY - 1) / DAY;
    if days <= 1 {
        "Goes for good today".into()
    } else {
        format!("{days} days left")
    }
}

/// How a recipe got into the box, as Fresh in the box says it ("from a link").
pub fn source_label(source: &str) -> Option<&'static str> {
    Some(match source {
        "url" => "from a link",
        "text" => "from pasted text",
        "claude" => "via Claude",
        "manual" => "written by you",
        "import" => "imported",
        "video" => "from a video",
        _ => return None,
    })
}

/// "Today · from a link" under a new recipe (`freshMeta`).
pub fn fresh_meta(created_ms: i64, source: &str, now_ms: i64, offset_minutes: i32) -> String {
    let when = day_label(created_ms, now_ms, offset_minutes);
    match source_label(source) {
        Some(how) => format!("{when} · {how}"),
        None => when,
    }
}

/// "Viewed yesterday" under a recently viewed recipe, or "Viewed recently" when the
/// time wasn't kept (`viewedLine`).
pub fn viewed_line(viewed_ms: Option<i64>, now_ms: i64, offset_minutes: i32) -> String {
    match viewed_ms {
        Some(at) if at > 0 => format!(
            "Viewed {}",
            day_label(at, now_ms, offset_minutes).to_lowercase()
        ),
        _ => "Viewed recently".into(),
    }
}

/// Where a cook left off, as "Pick up where you left off" shows it: `(step, of)`, both
/// counted from 1. None when not started, or on the last step (cook mode stays there after
/// Finish), so the row opens the recipe instead of cook mode (`progress`).
pub fn cook_progress(step_index: Option<i64>, steps: Option<i64>) -> Option<(i64, i64)> {
    let (step, of) = (step_index?, steps?);
    (step > 0 && step < of - 1).then_some((step + 1, of))
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-09-28 12:00 UTC, a Monday
    const NOW: i64 = 1_790_596_800_000;
    const HOUR: i64 = 3_600_000;

    #[test]
    fn greetings_follow_the_clock() {
        assert_eq!(greeting(4), "midnight snack?");
        assert_eq!(greeting(5), "what's for breakfast?");
        assert_eq!(greeting(12), "what's for lunch?");
        assert_eq!(greeting(18), "what's cooking tonight?");
        assert_eq!(greeting(22), "something sweet?");
    }

    #[test]
    fn days_count_from_local_midnight() {
        assert_eq!(day_label(NOW - HOUR, NOW, 0), "Today");
        assert_eq!(day_label(NOW - 13 * HOUR, NOW, 0), "Yesterday");
        // 09:00 UTC is still yesterday ten hours west, where it's 02:00
        assert_eq!(day_label(NOW - 3 * HOUR, NOW, -10 * 60), "Yesterday");
        assert_eq!(day_label(NOW - 3 * 24 * HOUR, NOW, 0), "Friday");
        assert_eq!(day_label(NOW - 8 * 24 * HOUR, NOW, 0), "Sep 20");
    }

    #[test]
    fn full_dates_are_local() {
        assert_eq!(date_label(NOW, 0), "Sep 28, 2026");
        // 12:00 UTC is already tomorrow fourteen hours east
        assert_eq!(date_label(NOW, 14 * 60), "Sep 29, 2026");
        assert_eq!(date_label(0, 0), "Jan 1, 1970");
    }

    #[test]
    fn the_trash_counts_up_to_the_day_it_goes() {
        let day = 24 * HOUR;
        assert_eq!(trash_left(NOW + 30 * day, NOW), "30 days left");
        assert_eq!(trash_left(NOW + 2 * day - HOUR, NOW), "2 days left");
        assert_eq!(trash_left(NOW + day + HOUR, NOW), "2 days left");
        assert_eq!(trash_left(NOW + day, NOW), "Goes for good today");
        assert_eq!(trash_left(NOW - HOUR, NOW), "Goes for good today");
    }

    #[test]
    fn lines_under_recipes() {
        assert_eq!(fresh_meta(NOW, "url", NOW, 0), "Today · from a link");
        assert_eq!(fresh_meta(NOW, "other", NOW, 0), "Today");
        assert_eq!(viewed_line(Some(NOW), NOW, 0), "Viewed today");
        assert_eq!(viewed_line(None, NOW, 0), "Viewed recently");
        assert_eq!(cook_progress(Some(2), Some(8)), Some((3, 8)));
        assert_eq!(cook_progress(Some(0), Some(8)), None);
        assert_eq!(cook_progress(Some(7), Some(8)), None);
        assert_eq!(cook_progress(None, Some(8)), None);
    }
}
