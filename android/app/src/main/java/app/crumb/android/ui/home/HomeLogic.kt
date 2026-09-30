package app.crumb.android.ui.home

import app.crumb.android.data.RecipeSummary
import java.time.Instant
import java.time.ZoneId

// Home's day and source wording comes from crumb-core (`freshMeta`, `viewedLine`, `dayLabel`),
// so the phone says it exactly as the web does. These only supply the clock and the offset.

/** The viewer's UTC offset at [nowMs] in minutes, east positive, as core's day labels take it. */
fun offsetMinutes(nowMs: Long = System.currentTimeMillis(), zone: ZoneId = ZoneId.systemDefault()): Int =
    zone.rules.getOffset(Instant.ofEpochMilli(nowMs)).totalSeconds / 60

/** "Yesterday · from a link" under a new recipe, or null when it has no creation time. */
fun freshMeta(recipe: RecipeSummary, nowMs: Long = System.currentTimeMillis(), zone: ZoneId = ZoneId.systemDefault()): String? {
    val at = recipe.createdAt?.let { runCatching { Instant.parse(it).toEpochMilli() }.getOrNull() } ?: return null
    return app.crumb.core.freshMeta(at, recipe.source, nowMs, offsetMinutes(nowMs, zone))
}

/** "Viewed yesterday" under a recent recipe. */
fun viewedLine(at: Long?, nowMs: Long = System.currentTimeMillis(), zone: ZoneId = ZoneId.systemDefault()): String =
    app.crumb.core.viewedLine(at, nowMs, offsetMinutes(nowMs, zone))
