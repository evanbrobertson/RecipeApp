package app.crumb.android.timers

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class KitchenTimerTest {
    @Test fun clockMatchesTheWeb() {
        assertEquals("0:00", formatClock(0))
        assertEquals("0:09", formatClock(9))
        assertEquals("10:00", formatClock(600))
        assertEquals("1:00:00", formatClock(3600))
        assertEquals("1:05:07", formatClock(3907))
        assertEquals("0:00", formatClock(-5))
    }

    @Test fun remainingRoundsUpUntilItRings() {
        val t = KitchenTimer(id = 1, label = "Step 2: 10 minutes", total = 600, endsAt = 1_000_000)
        assertEquals(600, t.remainingSeconds(1_000_000 - 600_000))
        // 0.4s left still reads 0:01, then it's done
        assertEquals(1, t.remainingSeconds(1_000_000 - 400))
        assertFalse(t.done(1_000_000 - 1))
        assertTrue(t.done(1_000_000))
        assertEquals(0, t.remainingSeconds(1_000_005))
    }
}
