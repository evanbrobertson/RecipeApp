//! ISO 8601 durations, as recipe sites publish them.

use std::sync::LazyLock;

use regex::Regex;

/// ISO 8601 durations in full: "PT1H10M", "P1DT2H", and the long form some sites emit,
/// "P0Y0M0DT0H10M0.000S". Any unit may have decimals; weeks count as 7 days.
static DURATION: LazyLock<Regex> = LazyLock::new(|| {
    let n = r"(\d+(?:[.,]\d+)?)";
    Regex::new(&format!(
        r"(?i)^P(?:{n}Y)?(?:{n}M)?(?:{n}W)?(?:{n}D)?(?:T(?:{n}H)?(?:{n}M)?(?:{n}S)?)?$"
    ))
    .unwrap()
});

/// Minutes in an ISO 8601 duration, or None if it isn't one. Years and months have no
/// fixed length and never describe cooking, so a duration using them is not parsed.
pub fn iso_duration_minutes(iso: &str) -> Option<f64> {
    let iso = iso.trim();
    // "P" or "PT" alone would otherwise match as zero
    if iso.len() < 3 {
        return None;
    }
    let c = DURATION.captures(iso)?;
    let n = |i: usize| {
        c.get(i)
            .and_then(|m| m.as_str().replace(',', ".").parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    if n(1) != 0.0 || n(2) != 0.0 {
        return None;
    }
    Some(n(3) * 10_080.0 + n(4) * 1440.0 + n(5) * 60.0 + n(6) + n(7) / 60.0)
}
