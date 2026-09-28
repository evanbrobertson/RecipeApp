package app.crumb.android.ui.home

import app.crumb.android.data.RecipeSummary
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle
import java.util.Locale

// Home's day and source wording, ported from HomeFeed.svelte. HomeLogicTest pins it.

/** "Today", "Yesterday", a weekday within the week, else "Sep 20" (web `dayLabel`). */
fun dayLabel(ms: Long, nowMs: Long = System.currentTimeMillis(), zone: ZoneId = ZoneId.systemDefault()): String {
    val day = Instant.ofEpochMilli(ms).atZone(zone).toLocalDate()
    val today = Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate()
    return when {
        !day.isBefore(today) -> "Today"
        day == today.minusDays(1) -> "Yesterday"
        !day.isBefore(today.minusDays(6)) -> day.dayOfWeek.getDisplayName(TextStyle.FULL, Locale.getDefault())
        else -> day.format(DateTimeFormatter.ofPattern("MMM d", Locale.getDefault()))
    }
}

private val Source = mapOf(
    "url" to "from a link",
    "text" to "from pasted text",
    "claude" to "via Claude",
    "manual" to "written by you",
    "import" to "imported",
)

/** "Yesterday · from a link" under a new recipe (web `freshMeta`). */
fun freshMeta(recipe: RecipeSummary, nowMs: Long = System.currentTimeMillis(), zone: ZoneId = ZoneId.systemDefault()): String? {
    val at = recipe.createdAt?.let { runCatching { Instant.parse(it).toEpochMilli() }.getOrNull() } ?: return null
    val how = Source[recipe.source]
    val day = dayLabel(at, nowMs, zone)
    return if (how != null) "$day · $how" else day
}

/** "Viewed yesterday" under a recent recipe (web `viewedLine`). */
fun viewedLine(at: Long?, nowMs: Long = System.currentTimeMillis(), zone: ZoneId = ZoneId.systemDefault()): String =
    at?.let { "Viewed ${dayLabel(it, nowMs, zone).lowercase()}" } ?: "Viewed recently"
