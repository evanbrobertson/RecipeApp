package app.crumb.android.data

import org.junit.Assert.assertEquals
import org.junit.Test

class PhotosTest {
    // Expected keys come from imageKey() in web/src/lib/img.ts, so the URLs match the server's
    @Test
    fun imageKeyMatchesTheWebApp() {
        assertEquals("efcb9837", Photos.imageKey("https://example.com/pie.jpg"))
        assertEquals("811c9dc5", Photos.imageKey(""))
        assertEquals("b9764a8c", Photos.imageKey("data:image/png;base64,iVBORw0KGgo="))
        assertEquals("4322a7bb", Photos.imageKey("crème brûlée.jpg"))
    }

    @Test
    fun widthsSnapUpToWhatTheServerMakes() {
        assertEquals(160, Photos.snapWidth(1))
        assertEquals(320, Photos.snapWidth(161))
        assertEquals(768, Photos.snapWidth(600))
        assertEquals(1200, Photos.snapWidth(5000))
    }
}
