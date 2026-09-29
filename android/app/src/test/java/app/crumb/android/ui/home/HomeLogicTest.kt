package app.crumb.android.ui.home

import app.crumb.android.data.RecipeSummary
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.time.Instant
import java.time.ZoneId
import java.util.Locale

class HomeLogicTest {
    private val zone = ZoneId.of("America/Toronto")
    // Saturday 26 September 2026, mid-afternoon in Toronto
    private val now = Instant.parse("2026-09-26T19:00:00Z").toEpochMilli()
    private fun at(iso: String) = Instant.parse(iso).toEpochMilli()

    init {
        Locale.setDefault(Locale.US)
    }

    @Test
    fun freshMetaSaysWhenAndHow() {
        val r = RecipeSummary(id = 1, title = "Soup", source = "url", createdAt = "2026-09-26T12:00:00Z")
        assertEquals("Today · from a link", freshMeta(r, now, zone))
        assertEquals("Today", freshMeta(r.copy(source = "mystery"), now, zone))
        assertNull(freshMeta(r.copy(createdAt = null), now, zone))
    }

    @Test
    fun viewedLineIsLowercase() {
        assertEquals("Viewed yesterday", viewedLine(at("2026-09-25T15:00:00Z"), now, zone))
        assertEquals("Viewed recently", viewedLine(null, now, zone))
    }

    @Test
    fun viewedDaysUseTheViewersOffset() {
        // 1am UTC on the 26th is still the 25th in Toronto, so it was yesterday
        assertEquals("Viewed yesterday", viewedLine(at("2026-09-26T01:00:00Z"), now, zone))
        assertEquals("Viewed monday", viewedLine(at("2026-09-21T15:00:00Z"), now, zone))
    }
}
