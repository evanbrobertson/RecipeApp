package app.crumb.android.ui.theme

import org.junit.Assert.assertEquals
import org.junit.Test

class SunTimesTest {
    // 2026-06-21T12:00:00Z, the same instant the Rust sun module tests use
    private val midsummer = 1_782_043_200_000L
    private val london = SunLocation(51.5, -0.13)
    private val hour = 3_600_000.0

    @Test
    fun londonMidsummerRiseAndSet() {
        val times = SunClock.sunTimes(midsummer, london)
        val riseSet = times as SunTimes.RiseSet
        // About 03:43 and 20:21 UTC, as in crates/crumb-core/src/sun.rs
        val midnight = midsummer - 12.0 * hour
        assertEquals(3.73, (riseSet.rise - midnight) / hour, 0.1)
        assertEquals(20.35, (riseSet.set - midnight) / hour, 0.1)
    }

    @Test
    fun polarDayAndNight() {
        assertEquals(SunTimes.PolarDay, SunClock.sunTimes(midsummer, SunLocation(80.0, 15.0)))
        assertEquals(SunTimes.PolarNight, SunClock.sunTimes(midsummer, SunLocation(-80.0, 15.0)))
    }
}
