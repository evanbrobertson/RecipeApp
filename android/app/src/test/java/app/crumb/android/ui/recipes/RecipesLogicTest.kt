package app.crumb.android.ui.recipes

import app.crumb.android.data.RecipeSummary
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The recipes list's pure logic (web/src/islands/RecipesPage.svelte, RecipeCard.svelte). */
class RecipesLogicTest {
    private fun recipe(id: Long, category: String? = null, cuisine: String? = null, totalTime: String? = null) =
        RecipeSummary(id = id, title = "Recipe $id", recipeCategory = category, recipeCuisine = cuisine, totalTime = totalTime)

    @Test fun countsRecipesInWords() {
        assertEquals("1 recipe", recipeCountLabel(1))
        assertEquals("3 recipes", recipeCountLabel(3))
        assertEquals("2 recipes will be permanently deleted.", deleteDescription(2))
        assertEquals("Deleted 1 recipe", deletedTitle(1))
        assertEquals("Added 4 to Weeknight dinners", addedTitle(4, "Weeknight dinners"))
    }

    @Test fun searchPlaceholderFollowsTheQuery() {
        assertEquals("Search 12 recipes", searchPlaceholder(12, hasQuery = false))
        assertEquals("Search by name or ingredient…", searchPlaceholder(12, hasQuery = true))
        assertEquals("Search by name or ingredient…", searchPlaceholder(0, hasQuery = false))
    }

    @Test fun noMatchesCopyUsesCurlyQuotes() {
        assertEquals("No recipes found for “soup”.", noMatchesDescription("soup"))
        assertEquals("Try another category.", noMatchesDescription(""))
    }

    @Test fun cardSubtitleJoinsKickerAndTime() {
        assertEquals("Breakfast · British · 1h 30m", cardSubtitle(recipe(1, "Breakfast", "British", "PT1H30M")))
        assertEquals("30 min", cardSubtitle(recipe(1, null, null, "30 min")))
        assertEquals("", cardSubtitle(recipe(1)))
    }

    @Test fun categoriesComeBackInFixedOrder() {
        val order = listOf("Breakfast", "Main", "Side", "Soup", "Salad", "Baking", "Dessert", "Snack", "Sauce", "Drink", "Other")
        val recipes = listOf(recipe(1, "Dessert"), recipe(2, "Breakfast"), recipe(3, null), recipe(4, "Breakfast"))
        assertEquals(listOf("Breakfast", "Dessert"), categoriesIn(recipes, order))
        assertEquals(emptyList<String>(), categoriesIn(listOf(recipe(1, null)), order))
    }

    @Test fun selectionTogglesAndSelectsAll() {
        assertEquals(setOf(1L), toggleSelection(emptySet(), 1))
        assertEquals(emptySet<Long>(), toggleSelection(setOf(1L), 1))
        val visible = listOf(1L, 2L, 3L)
        assertFalse(allSelected(visible, setOf(1L, 2L)))
        assertTrue(allSelected(visible, setOf(1L, 2L, 3L)))
        assertEquals(setOf(1L, 2L, 3L), toggleAllSelection(visible, emptySet()))
        assertEquals(emptySet<Long>(), toggleAllSelection(visible, setOf(1L, 2L, 3L)))
    }

    @Test fun searchGateKeepsOnlyTheNewestRequest() {
        val gate = SearchGate()
        val first = gate.begin()
        assertTrue(gate.isCurrent(first))
        val second = gate.begin()
        assertFalse(gate.isCurrent(first))
        assertTrue(gate.isCurrent(second))
    }
}
