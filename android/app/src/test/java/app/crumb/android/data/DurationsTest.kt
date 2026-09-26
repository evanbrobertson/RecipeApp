package app.crumb.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class DurationsTest {
    @Test
    fun isoDurationsReadNaturally() {
        assertEquals("45 min", Durations.display("PT45M"))
        assertEquals("1 hr", Durations.display("PT1H"))
        assertEquals("1 hr 30 min", Durations.display("PT1H30M"))
        assertEquals("25 hr", Durations.display("P1DT1H"))
        assertEquals("1 min", Durations.display("PT20S"))
        assertEquals("2 hr", Durations.display("pt120m"))
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
