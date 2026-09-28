package app.crumb.android.ui.more

import app.crumb.android.data.SharedLink
import app.crumb.android.ui.theme.SunClock
import app.crumb.android.ui.theme.SunLocation
import app.crumb.android.ui.theme.SunTimes
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

// The More page's wording that isn't already shared Rust logic (web/src/islands/MoreSettings.svelte).

private val Clock = DateTimeFormatter.ofPattern("h:mm a")
private val Day = DateTimeFormatter.ofPattern("d MMM yyyy")

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
 * "Recipe shared 26 Sep 2026 · last opened 26 Sep 2026" (web `shareMeta`), or "… · not
 * opened yet" for a link nobody has used.
 */
fun shareMeta(link: SharedLink, zone: ZoneId = ZoneId.systemDefault(), locale: Locale = Locale.getDefault()): String {
    val kind = if (link.kind == "cookbook") "Cookbook" else "Recipe"
    val parts = mutableListOf(kind, "shared ${formatDay(link.createdAt, zone, locale)}")
    parts += if (link.lastOpenedAt != null) "last opened ${formatDay(link.lastOpenedAt, zone, locale)}" else "not opened yet"
    return parts.joinToString(" · ")
}

private fun formatDay(iso: String?, zone: ZoneId, locale: Locale): String {
    if (iso == null) return ""
    return runCatching {
        Instant.parse(iso).atZone(zone).format(Day.withLocale(locale))
    }.getOrDefault("")
}
