package app.crumb.android.ui.cook

import app.crumb.core.IngredientLine
import kotlin.math.abs

// Pure arithmetic and groupings for cook mode. The sizes, the remembered step and the
// ingredient-sheet headings are pinned by CookLogicTest.

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

/** One row of the ingredients sheet: a section heading, or a line at its index. */
sealed interface IngredientRow {
    data class Header(val text: String) : IngredientRow
    data class Line(val index: Int, val raw: String) : IngredientRow
}

/**
 * The ingredients sheet flattened (spec §5.8): a heading whenever a non-empty section starts,
 * matching the web's comparison against the previous line's section.
 */
fun ingredientRows(lines: List<IngredientLine>): List<IngredientRow> {
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
