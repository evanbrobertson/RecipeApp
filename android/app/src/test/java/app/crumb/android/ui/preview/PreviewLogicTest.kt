package app.crumb.android.ui.preview

import app.crumb.android.data.ApiException
import app.crumb.android.data.OfflineException
import app.crumb.android.data.Preview
import app.crumb.android.data.Recipe
import app.crumb.android.data.SITE_BLOCKED
import app.crumb.android.data.SITE_TERMS
import java.io.IOException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class PreviewLogicTest {
    @Test
    fun eachStatusHasItsState() {
        assertEquals(PreviewState.Saved(3, "Pie"), stateOf(Preview.Saved(3, "Pie")))
        assertEquals(PreviewState.Import, stateOf(Preview.Import))
        val recipe = Recipe(id = 0, title = "Pie")
        assertEquals(PreviewState.Ready(recipe), stateOf(Preview.Ready(recipe)))
    }

    @Test
    fun aFailureKeepsTheServersReason() {
        val terms = failedState(ApiException(422, "Crumb doesn't fetch that site", SITE_TERMS, "NYT Cooking"), "Crumb doesn't fetch that site")
        assertEquals(SITE_TERMS, terms.code)
        assertFalse(canTryAgain(terms.code))
        val blocked = failedState(ApiException(422, "Blocked", SITE_BLOCKED), "Blocked")
        assertTrue(canTryAgain(blocked.code))
        assertTrue(failedState(ApiException(401, "Signed out"), "Signed out").signedOut)
        val offline = failedState(OfflineException(IOException("no route")), "Can't reach your Crumb server.")
        assertNull(offline.code)
        assertFalse(offline.signedOut)
        assertTrue(isOffline(OfflineException(IOException())))
    }

    @Test
    fun hostWording() {
        assertEquals("From seriouseats.com", fromHost("https://www.seriouseats.com/pie"))
        assertEquals("From the site", fromHost(null))
        assertEquals("Reading seriouseats.com…", readingHost("https://www.seriouseats.com/pie"))
    }
}
