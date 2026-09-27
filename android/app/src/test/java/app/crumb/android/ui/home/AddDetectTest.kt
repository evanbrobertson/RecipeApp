package app.crumb.android.ui.home

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AddDetectTest {
    @Test
    fun blankIsAuto() {
        assertEquals(Detected(AddMode.Auto, ""), detect("   "))
    }

    @Test
    fun linksShowTheSiteOrACount() {
        assertEquals(Detected(AddMode.Link, "seriouseats.com"), detect("https://www.seriouseats.com/pie"))
        assertEquals(Detected(AddMode.Link, "2 links"), detect("https://a.com/x\nhttps://b.com/y"))
        // A browser share: the page title and its link on one line
        assertEquals(Detected(AddMode.Link, "bbc.co.uk"), detect("Best lasagna https://bbc.co.uk/food/lasagna"))
    }

    @Test
    fun recipeTextCountsIngredientLines() {
        assertEquals(Detected(AddMode.Text, "2 ingredients"), detect("Pancakes\n2 eggs\n1 cup flour\nMix and fry"))
        assertEquals(Detected(AddMode.Text, "3 lines"), detect("Soup\nMake it\nEat it"))
        assertEquals(Detected(AddMode.Text, "2 lines"), detect("Grandma's stew\nSimmer it for hours"))
    }

    @Test
    fun aShortNameStartsFromScratch() {
        assertEquals(Detected(AddMode.Scratch, "New recipe"), detect("Lemon drizzle cake"))
        assertEquals(AddMode.Text, detect("a".repeat(81)).mode)
    }

    @Test
    fun addNeedsEnoughToGoOn() {
        assertTrue(canAdd(AddMode.Link, "https://a.com/x", 0, false))
        assertFalse(canAdd(AddMode.Link, "just words", 0, false))
        assertFalse(canAdd(AddMode.Text, "too short", 0, false))
        assertTrue(canAdd(AddMode.Text, "two eggs and flour", 0, false))
        assertFalse(canAdd(AddMode.Photo, "", 2, false))
        assertTrue(canAdd(AddMode.Photo, "", 2, true))
        assertTrue(canAdd(AddMode.Claude, "", 0, false))
    }

    @Test
    fun theButtonSaysWhatItWillDo() {
        assertEquals("Import all", addLabel(AddMode.Link, "https://a.com https://b.com"))
        assertEquals("Add", addLabel(AddMode.Link, "https://a.com"))
        assertEquals("Open", addLabel(AddMode.Apps, ""))
    }
}
