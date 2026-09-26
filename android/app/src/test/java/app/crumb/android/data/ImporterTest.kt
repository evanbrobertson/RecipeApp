package app.crumb.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ImporterTest {
    @Test fun findsEveryDistinctLinkInAnyText() {
        val text = """
            Try these: https://www.bbcgoodfood.com/recipes/easy-pancakes, and
            (https://example.com/soup). Again: https://www.bbcgoodfood.com/recipes/easy-pancakes
            not a link: www.nope.com
        """.trimIndent()
        assertEquals(
            listOf("https://www.bbcgoodfood.com/recipes/easy-pancakes", "https://example.com/soup"),
            Importer.links(text),
        )
    }

    @Test fun refusesFilesTheServerCantRead() {
        assertTrue(Importer.unreadable("video/mp4", "clip.mp4"))
        assertTrue(Importer.unreadable("application/octet-stream", "Nan's recipes.docx"))
        assertFalse(Importer.unreadable("application/pdf", "lasagne.pdf"))
        assertFalse(Importer.unreadable(null, "export.paprikarecipes"))
    }

    @Test fun photosByTypeOrExtension() {
        assertTrue(Incoming.isPhoto("image/jpeg", null))
        assertTrue(Incoming.isPhoto(null, "IMG_2031.HEIC"))
        assertFalse(Incoming.isPhoto("application/pdf", "card.pdf"))
    }
}
