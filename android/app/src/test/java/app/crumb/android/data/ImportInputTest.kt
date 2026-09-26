package app.crumb.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ImportInputTest {
    @Test
    fun aBareLinkIsALink() {
        assertEquals(ImportInput.Link("https://example.com/soup"), ImportInput.classify("  https://example.com/soup \n"))
    }

    @Test
    fun aBrowserShareWithATitleIsStillALink() {
        assertEquals(
            ImportInput.Link("https://www.bbcgoodfood.com/recipes/easy-pancakes"),
            ImportInput.classify("Easy pancakes https://www.bbcgoodfood.com/recipes/easy-pancakes"),
        )
        assertEquals(ImportInput.Link("https://example.com/a"), ImportInput.classify("Look (https://example.com/a)."))
    }

    @Test
    fun aPastedRecipeIsText() {
        val recipe = "Pancakes\n\n100g flour\n2 eggs\n300ml milk\n\nWhisk everything and fry."
        assertEquals(ImportInput.Text(recipe), ImportInput.classify(recipe))
    }

    @Test
    fun longTextWithALinkIsText() {
        val long = "Source: https://example.com/x\n" + "Stir the pot. ".repeat(60)
        assertTrue(ImportInput.classify(long) is ImportInput.Text)
    }

    @Test
    fun twoLinksAreText() {
        assertTrue(ImportInput.classify("https://a.com/1 https://b.com/2") is ImportInput.Text)
    }

    @Test
    fun blankIsNothing() {
        assertNull(ImportInput.classify("   "))
    }
}
