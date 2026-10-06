package app.crumb.android.ui.cook

import org.junit.Assert.assertEquals
import org.junit.Test

class CookLogicTest {
    @Test
    fun stepTextSizeFollowsLength() {
        assertEquals(28, stepTextSizeSp(0))
        assertEquals(28, stepTextSizeSp(240))
        assertEquals(24, stepTextSizeSp(241))
        assertEquals(24, stepTextSizeSp(420))
        assertEquals(21, stepTextSizeSp(421))
        assertEquals(21, stepTextSizeSp(5_000))
    }

    @Test
    fun aRememberedStepPastTheEndStartsOver() {
        assertEquals(0, cookStartIndex(null, 5))
        assertEquals(0, cookStartIndex(5, 5))
        assertEquals(0, cookStartIndex(9, 5))
        assertEquals(2, cookStartIndex(2, 5))
        assertEquals(0, cookStartIndex(0, 0))
    }

    @Test
    fun swipeNeedsDistanceAndMostlySideways() {
        assertEquals(1, swipeDirection(-100f, 10f, 60f))
        assertEquals(-1, swipeDirection(100f, 10f, 60f))
        assertEquals(0, swipeDirection(-60f, 0f, 60f))
        assertEquals(0, swipeDirection(100f, 120f, 60f))
        assertEquals(0, swipeDirection(10f, 2f, 60f))
    }

    @Test
    fun ingredientRowsAddSectionHeadings() {
        val lines = listOf(
            IngredientItem("2 eggs", null),
            IngredientItem("100g flour", "For the batter"),
            IngredientItem("1 tsp salt", "For the batter"),
            IngredientItem("1 tbsp oil", "To fry"),
        )
        assertEquals(
            listOf(
                IngredientRow.Line(0, "2 eggs"),
                IngredientRow.Header("For the batter"),
                IngredientRow.Line(1, "100g flour"),
                IngredientRow.Line(2, "1 tsp salt"),
                IngredientRow.Header("To fry"),
                IngredientRow.Line(3, "1 tbsp oil"),
            ),
            ingredientRows(lines),
        )
    }

    @Test
    fun emptySectionsAreNotHeadings() {
        val lines = listOf(IngredientItem("2 eggs", ""), IngredientItem("1 tsp salt", null))
        assertEquals(
            listOf(IngredientRow.Line(0, "2 eggs"), IngredientRow.Line(1, "1 tsp salt")),
            ingredientRows(lines),
        )
    }

    @Test
    fun ingredientsDockFromMediumWidths() {
        assertEquals(false, canDockIngredients(411))
        assertEquals(false, canDockIngredients(599))
        assertEquals(true, canDockIngredients(600))
        assertEquals(true, canDockIngredients(1280))
    }

    @Test
    fun aStepTimerShowsTheCooksLengthThenTheSuggestion() {
        assertEquals(600, timerLength(600, 1500))
        assertEquals(1500, timerLength(null, 1500))
        assertEquals(null, timerLength(null, null))
    }

    @Test
    fun theMinutesFieldStartsAsPlainMinutes() {
        assertEquals("5", minutesText(300))
        assertEquals("1.5", minutesText(90))
        assertEquals("0.25", minutesText(15))
        assertEquals("100", minutesText(6_000))
        assertEquals("1.33", minutesText(80))
    }

    @Test
    fun typedMinutesAreANumberOrNothing() {
        assertEquals(12.0, typedMinutes("12")!!, 0.0)
        assertEquals(1.5, typedMinutes(" 1,5 ")!!, 0.0)
        assertEquals(0.5, typedMinutes(".5")!!, 0.0)
        assertEquals(null, typedMinutes(""))
        assertEquals(null, typedMinutes("abc"))
        assertEquals(null, typedMinutes("NaN"))
    }
}
