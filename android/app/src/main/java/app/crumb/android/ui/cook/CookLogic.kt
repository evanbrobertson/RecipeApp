package app.crumb.android.ui.cook

import java.util.Locale
import kotlin.math.abs

// Pure arithmetic and groupings for cook mode. The sizes, the remembered step, the ingredient
// headings, the docked width and the typed timer length are pinned by CookLogicTest.

/** Step text size by length (spec §5.12): over 420 chars 21sp, over 240 24sp, else 28sp. */
fun stepTextSizeSp(length: Int): Int = when {
    length > 420 -> 21
    length > 240 -> 24
    else -> 28
}

/** The step to open on: a remembered step past the end (recipe edited since) starts over. */
fun cookStartIndex(saved: Int?, stepCount: Int): Int =
    if (saved != null && saved in 0 until stepCount) saved else 0

/**
 * A horizontal swipe on the step (spec §5.3): past [thresholdPx] and more sideways than up.
 * Returns 1 for next (swiped left), -1 for previous, 0 for not a step turn.
 */
fun swipeDirection(dx: Float, dy: Float, thresholdPx: Float): Int = when {
    abs(dx) <= thresholdPx || abs(dx) <= abs(dy) -> 0
    dx < 0 -> 1
    else -> -1
}

/** The width, in dp, from which cook mode can dock the ingredients beside the step (web: 768px). */
const val DOCK_MIN_WIDTH_DP = 600

/** Medium and expanded windows (tablets, unfolded foldables) have room for the ingredients pane. */
fun canDockIngredients(widthDp: Int): Boolean = widthDp >= DOCK_MIN_WIDTH_DP

/** The length a step's timer shows: the cook's own, else the step's first suggestion, else none. */
fun timerLength(chosen: Int?, suggested: Int?): Int? = chosen ?: suggested

/** A length as the minutes field starts out: 90 s is "1.5", 300 s is "5". */
fun minutesText(seconds: Int): String =
    String.format(Locale.ROOT, "%.2f", seconds / 60.0).trimEnd('0').trimEnd('.')

/** What the cook typed in the minutes field; null for anything but a number (a comma is a point). */
fun typedMinutes(text: String): Double? =
    text.trim().replace(',', '.').toDoubleOrNull()?.takeIf { it.isFinite() }

/** An ingredient line, with the section it sits under. */
data class IngredientItem(val raw: String, val section: String?)

/** One row of the ingredient list: a section heading, or a line at its index. */
sealed interface IngredientRow {
    data class Header(val text: String) : IngredientRow
    data class Line(val index: Int, val raw: String) : IngredientRow
}

/**
 * The ingredient list flattened (spec §5.8): a heading whenever a non-empty section starts,
 * matching the web's comparison against the previous line's section.
 */
fun ingredientRows(lines: List<IngredientItem>): List<IngredientRow> {
    val rows = mutableListOf<IngredientRow>()
    lines.forEachIndexed { i, line ->
        val section = line.section
        if (!section.isNullOrEmpty() && section != lines.getOrNull(i - 1)?.section) {
            rows += IngredientRow.Header(section)
        }
        rows += IngredientRow.Line(i, line.raw)
    }
    return rows
}
