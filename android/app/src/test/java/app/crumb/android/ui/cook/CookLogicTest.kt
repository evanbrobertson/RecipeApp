package app.crumb.android.ui.cook

import app.crumb.core.IngredientLine
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
            IngredientLine("2 eggs", null),
            IngredientLine("100g flour", "For the batter"),
            IngredientLine("1 tsp salt", "For the batter"),
            IngredientLine("1 tbsp oil", "To fry"),
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
        val lines = listOf(IngredientLine("2 eggs", ""), IngredientLine("1 tsp salt", null))
        assertEquals(
            listOf(IngredientRow.Line(0, "2 eggs"), IngredientRow.Line(1, "1 tsp salt")),
            ingredientRows(lines),
        )
    }
}
