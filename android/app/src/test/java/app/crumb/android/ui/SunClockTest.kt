package app.crumb.android.ui

import app.crumb.android.ui.theme.SunClock
import app.crumb.android.ui.theme.SunLocation
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.Instant
import java.time.ZoneId

class SunClockTest {
    private val london = SunLocation(51.5074, -0.1278)
    private fun at(iso: String) = Instant.parse(iso).toEpochMilli()

    @Test fun londonMidsummerMatchesPublishedTimes() {
        // Published: sunrise 04:43, sunset 21:21 BST (03:43 / 20:21 UTC)
        val noon = SunClock.sun(at("2026-06-21T12:00:00Z"), london)
        assertEquals(at("2026-06-21T20:21:00Z").toDouble(), noon.next.toDouble(), 120_000.0)
        val night = SunClock.sun(at("2026-06-21T01:00:00Z"), london)
        assertEquals(at("2026-06-21T03:43:00Z").toDouble(), night.next.toDouble(), 120_000.0)
    }

    @Test fun darkAfterSunsetUntilTomorrowsSunrise() {
        val evening = SunClock.sun(at("2026-06-21T22:00:00Z"), london)
        assertTrue(evening.dark)
        assertEquals(at("2026-06-22T03:43:00Z").toDouble(), evening.next.toDouble(), 180_000.0)
        assertFalse(SunClock.sun(at("2026-06-21T12:00:00Z"), london).dark)
    }

    @Test fun polarNightStaysDark() {
        assertTrue(SunClock.sun(at("2026-12-21T12:00:00Z"), SunLocation(78.2, 15.6)).dark)
    }

    @Test fun estimatesFromTheTimeZoneLikeTheWeb() {
        // Standard offsets only: New York is UTC-5 → longitude -75, and north (40°)
        assertEquals(SunLocation(40.0, -75.0), SunClock.estimate(ZoneId.of("America/New_York"), 2026))
        assertEquals(SunLocation(50.0, 0.0), SunClock.estimate(ZoneId.of("Europe/London"), 2026))
        assertEquals(SunLocation(-34.0, 150.0), SunClock.estimate(ZoneId.of("Australia/Sydney"), 2026))
    }
}
