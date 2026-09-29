package app.crumb.android.ui.home

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AddDetectTest {
    // The rules live in crumb-core; these prove the wiring and the mode mapping
    @Test
    fun detectionComesFromCore() {
        assertEquals(Detected(AddMode.Auto, ""), detect("   "))
        assertEquals(Detected(AddMode.Link, "seriouseats.com"), detect("https://www.seriouseats.com/pie"))
        assertEquals(Detected(AddMode.Link, "2 links"), detect("https://a.com/x\nhttps://b.com/y"))
        assertEquals(Detected(AddMode.Text, "2 ingredients"), detect("Pancakes\n2 eggs\n1 cup flour\nMix and fry"))
        assertEquals(Detected(AddMode.Scratch, "New recipe"), detect("Lemon drizzle cake"))
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

    @Test fun oneSharedLinkOpensItsPreview() {
        assertEquals("https://example.com/pie", sharedLink("https://example.com/pie"))
        assertEquals("https://example.com/pie", sharedLink("  https://example.com/pie\n"))
        assertEquals("https://example.com/pie", sharedLink("Best pie https://example.com/pie"))
    }

    @Test fun textAndSeveralLinksAreNotOneLink() {
        assertNull(sharedLink("2 cups flour\n1 cup sugar\nMix and bake"))
        assertNull(sharedLink("https://example.com/a https://example.com/b"))
        assertNull(sharedLink("https://example.com/a\nhttps://example.com/b"))
        assertNull(sharedLink(""))
    }
}
