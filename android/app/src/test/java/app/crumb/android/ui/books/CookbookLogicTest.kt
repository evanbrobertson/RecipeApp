package app.crumb.android.ui.books

import org.junit.Assert.assertEquals
import org.junit.Test

/** The cookbook page's copy helpers, matching web/src/islands/CookbookPage.svelte. */
class CookbookLogicTest {
    @Test fun recipeCountIsPluralisedLikeTheWeb() {
        assertEquals("0 recipes", recipesLabel(0))
        assertEquals("1 recipe", recipesLabel(1))
        assertEquals("2 recipes", recipesLabel(2))
        assertEquals("12 recipes", recipesLabel(12))
    }

    @Test fun deleteDescriptionMentionsTheShareLinkOnlyWhenThereIsOne() {
        assertEquals("Recipes in it are kept.", deleteCookbookDescription(hasShare = false))
        assertEquals(
            "Recipes in it are kept. Its share link stops working.",
            deleteCookbookDescription(hasShare = true),
        )
    }

    @Test fun removeButtonNamesTheRecipe() {
        assertEquals("Remove Sunday roast from cookbook", removeRecipeLabel("Sunday roast"))
    }
}
