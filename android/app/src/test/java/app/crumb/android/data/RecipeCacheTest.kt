package app.crumb.android.data

import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class RecipeCacheTest {
    @get:Rule val tmp = TemporaryFolder()

    private val soup = Recipe(id = 7, title = "Soup", ingredients = listOf(Section(items = listOf("water"))))

    @Test
    fun keepsRecipesPerServer() = runTest {
        val cache = RecipeCache(tmp.root)
        cache.useServer("https://one.example/")
        cache.putRecipe(soup)
        assertEquals(soup, cache.recipe(7))

        cache.useServer("https://two.example/")
        assertNull(cache.recipe(7))

        cache.useServer("https://one.example/")
        assertEquals(soup, cache.recipe(7))
    }

    @Test
    fun clearForgetsTheServer() = runTest {
        val cache = RecipeCache(tmp.root)
        cache.useServer("https://one.example/")
        cache.putRecipes(listOf(RecipeSummary(id = 1, title = "Soup")))
        cache.clear()
        assertNull(cache.recipes())
    }

    @Test
    fun signedOutReadsNothingAndWritesNothing() = runTest {
        val cache = RecipeCache(tmp.root)
        cache.putRecipe(soup)
        assertNull(cache.recipe(7))
        assertEquals(0, tmp.root.listFiles()!!.size)
    }
}
