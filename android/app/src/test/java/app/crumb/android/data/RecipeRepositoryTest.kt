package app.crumb.android.data

import org.junit.Assert.assertEquals
import org.junit.Test

class RecipeRepositoryTest {
    private val all = listOf(
        RecipeSummary(1, "Tomato soup", recipeCategory = "Soups"),
        RecipeSummary(2, "Roast chicken", recipeCuisine = "French"),
        RecipeSummary(3, "Chicken noodle soup"),
    )

    @Test
    fun offlineSearchNeedsEveryWord() {
        assertEquals(listOf(3L), RecipeRepository.filterOffline(all, "SOUP  chicken").map { it.id })
        assertEquals(listOf(2L), RecipeRepository.filterOffline(all, "french").map { it.id })
        assertEquals(listOf(1L, 3L), RecipeRepository.filterOffline(all, "soup").map { it.id })
    }
}
