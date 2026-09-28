package app.crumb.android.ui.more

import app.crumb.android.data.SharedLink
import app.crumb.android.ui.theme.SunLocation
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.Instant
import java.time.ZoneId
import java.util.Locale

class MoreLogicTest {
    private val utc = ZoneId.of("UTC")
    private val us = Locale.US

    @Test
    fun clockIsTwelveHourAndLowercase() {
        val ms = Instant.parse("2026-06-21T20:21:00Z").toEpochMilli()
        assertEquals("8:21 pm", formatSunTime(ms, utc, us))
    }

    @Test
    fun sunLineReadsAsTheWebDoes() {
        val now = Instant.parse("2026-06-21T12:00:00Z").toEpochMilli()
        val line = sunLine(now, SunLocation(51.5, -0.13), utc, us)
        assertTrue(line, Regex("""Dark from \d{1,2}:\d{2} [ap]m until \d{1,2}:\d{2} [ap]m""").matches(line))
    }

    @Test
    fun polarDaysSaySo() {
        val now = Instant.parse("2026-06-21T12:00:00Z").toEpochMilli()
        assertEquals("No sunset here today, so it stays light", sunLine(now, SunLocation(80.0, 15.0), utc, us))
        assertEquals("No sunset here today, so it stays dark", sunLine(now, SunLocation(-80.0, 15.0), utc, us))
    }

    @Test
    fun shareMetaNamesTheKindAndOpening() {
        val opened = SharedLink(
            kind = "recipe",
            id = 1,
            title = "Soup",
            url = "https://crumb.example/s/abc",
            createdAt = "2026-09-26T16:26:43.000Z",
            lastOpenedAt = "2026-09-27T09:00:00.000Z",
        )
        assertEquals("Recipe · shared 26 Sep 2026 · last opened 27 Sep 2026", shareMeta(opened, utc, us))

        val never = opened.copy(kind = "cookbook", lastOpenedAt = null)
        assertEquals("Cookbook · shared 26 Sep 2026 · not opened yet", shareMeta(never, utc, us))
    }
}
