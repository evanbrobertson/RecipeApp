package app.crumb.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class DurationsTest {
    @Test
    fun isoDurationsReadNaturally() {
        assertEquals("45m", Durations.display("PT45M"))
        assertEquals("1h", Durations.display("PT1H"))
        assertEquals("1h 30m", Durations.display("PT1H30M"))
        assertEquals("25h", Durations.display("P1DT1H"))
        assertEquals("1m", Durations.display("PT20S"))
        assertEquals("2h", Durations.display("pt120m"))
        assertEquals("10m", Durations.display("P0Y0M0DT0H10M0.000S"))
    }

    @Test
    fun otherTextIsShownAsWritten() {
        assertEquals("about 40 minutes", Durations.display(" about 40 minutes "))
        assertEquals("30m", Durations.display("30m"))
    }

    @Test
    fun emptyAndZeroAreHidden() {
        assertNull(Durations.display(null))
        assertNull(Durations.display("  "))
        assertNull(Durations.display("PT0M"))
    }
}
