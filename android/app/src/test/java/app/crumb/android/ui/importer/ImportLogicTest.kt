package app.crumb.android.ui.importer

import org.junit.Assert.assertEquals
import org.junit.Test

class ImportLogicTest {
    @Test
    fun linksFoundTextCountsAndFallsBackToTheHint() {
        assertEquals("One per line, or any text with links in it", linksFoundText(0))
        assertEquals("1 link found", linksFoundText(1))
        assertEquals("4 links found", linksFoundText(4))
    }

    @Test
    fun fileResultSkipsEmptyCounts() {
        assertEquals("2 added, 1 already saved, 3 skipped", fileResultText(2, 1, 3))
        assertEquals("1 added", fileResultText(1, 0, 0))
        assertEquals("0 added, 5 skipped", fileResultText(0, 0, 5))
    }

    @Test
    fun photoNameCountsPages() {
        assertEquals("photo.jpg", photoJobName(1, "photo.jpg"))
        assertEquals("3 photos", photoJobName(3, "photo.jpg"))
    }

    @Test
    fun tooManyPhotosMatchesTheWeb() {
        assertEquals("Up to 6 photos at a time, all pages of one recipe", tooManyPhotosText(6))
    }
}
