package app.crumb.android.ui.more

import app.crumb.android.data.SharedLink
import app.crumb.android.ui.theme.SunClock
import app.crumb.android.ui.theme.SunLocation
import app.crumb.android.ui.theme.SunTimes
import app.crumb.core.dateLabel
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

// The More page's wording that isn't already shared Rust logic (web/src/islands/MoreSettings.svelte).

private val Clock = DateTimeFormatter.ofPattern("h:mm a")

/** "7:12 pm" — the web's `toLocaleTimeString([], {hour:"numeric", minute:"2-digit"})`. */
fun formatSunTime(ms: Long, zone: ZoneId = ZoneId.systemDefault(), locale: Locale = Locale.getDefault()): String =
    Instant.ofEpochMilli(ms).atZone(zone).format(Clock.withLocale(locale)).lowercase(locale)

/**
 * The sub-panel's first line: "Dark from 7:12 pm until 6:48 am", or, on a polar day or
 * night, "No sunset here today, so it stays light/dark".
 */
fun sunLine(now: Long, loc: SunLocation, zone: ZoneId = ZoneId.systemDefault(), locale: Locale = Locale.getDefault()): String =
    when (val times = SunClock.sunTimes(now, loc)) {
        is SunTimes.RiseSet -> "Dark from ${formatSunTime(times.set, zone, locale)} until ${formatSunTime(times.rise, zone, locale)}"
        SunTimes.PolarDay -> "No sunset here today, so it stays light"
        SunTimes.PolarNight -> "No sunset here today, so it stays dark"
    }

/**
 * "Recipe · shared Sep 26, 2026 · last opened Sep 27, 2026" (web `shareMeta`), or "… · not
 * opened yet" for a link nobody has used.
 */
fun shareMeta(link: SharedLink, zone: ZoneId = ZoneId.systemDefault()): String {
    val kind = if (link.kind == "cookbook") "Cookbook" else "Recipe"
    val parts = mutableListOf(kind, "shared ${formatDay(link.createdAt, zone)}")
    parts += if (link.lastOpenedAt != null) "last opened ${formatDay(link.lastOpenedAt, zone)}" else "not opened yet"
    return parts.joinToString(" · ")
}

/** A date as crumb-core prints one ("Sep 26, 2026"), in the viewer's zone. */
private fun formatDay(iso: String?, zone: ZoneId): String {
    if (iso == null) return ""
    return runCatching {
        val at = Instant.parse(iso)
        dateLabel(at.toEpochMilli(), zone.rules.getOffset(at).totalSeconds / 60)
    }.getOrDefault("")
}
