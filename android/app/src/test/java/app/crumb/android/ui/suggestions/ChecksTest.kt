package app.crumb.android.ui.suggestions

import app.crumb.android.data.ChecksStatus
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The Suggestions page's wording (web/src/lib/checks.ts), pinned to the web's cases. */
class ChecksTest {
    private fun status(
        eligible: Int = 0,
        checked: Int = 0,
        pending: Int = 0,
        tidied: Int = 0,
        toCheck: Int = 0,
        due: Int = 0,
        restored: Int = 0,
        edited: Int = 0,
    ) = ChecksStatus(
        eligible = eligible,
        checked = checked,
        pending = pending,
        tidied = tidied,
        toCheck = toCheck,
        due = due,
        restored = restored,
        edited = edited,
    )

    @Test
    fun runningCountsFailuresAsDoneWhenRunIsKnown() {
        assertEquals("Checking… 3 of 12 done", checksStatusText(status(eligible = 12, checked = 3, pending = 9)))
        assertEquals("Checking… 7 of 10 done", checksStatusText(status(eligible = 12, checked = 3, pending = 3), run = 10))
    }

    @Test
    fun idleSaysWhatIsDueAndWhy() {
        assertEquals(
            "5 recipes to check (2 restored, 1 edited since) · suggests fixes, tidies only stray symbols",
            checksStatusText(status(eligible = 10, checked = 5, due = 5, restored = 2, edited = 1)),
        )
        assertEquals(
            "1 recipe to check · suggests fixes, tidies only stray symbols",
            checksStatusText(status(eligible = 2, checked = 1, due = 1)),
        )
    }

    @Test
    fun idleSaysHowManyWereChecked() {
        assertEquals("3 of 4 checked", checksStatusText(status(eligible = 4, checked = 3)))
    }

    @Test
    fun doneStatesCountWhatWasTidiedAndIsWorthALook() {
        assertEquals("All 79 recipes checked", checksStatusText(status(eligible = 79, checked = 79)))
        assertEquals(
            "All 79 recipes checked · 2 things tidied · 3 recipes to look at",
            checksStatusText(status(eligible = 79, checked = 79, tidied = 2, toCheck = 3)),
        )
    }

    @Test
    fun summaryReadsBiggestFieldFirst() {
        assertEquals("2 steps · 1 ingredient", summary(mapOf("instructions" to 2, "ingredients" to 1)))
        assertEquals("1 step · 1 note", summary(mapOf("instructions" to 1, "notes" to 1)))
        assertEquals("3 things", summary(mapOf("weird" to 3)))
        assertEquals("", summary(emptyMap()))
    }

    @Test
    fun badgeLabelHidesZeroAndCapsAtNinetyNine() {
        assertNull(ReviewCount.label(0))
        assertEquals("1", ReviewCount.label(1))
        assertEquals("99", ReviewCount.label(99))
        assertEquals("99+", ReviewCount.label(100))
        assertEquals("99+", ReviewCount.label(1_000))
    }

    @Test
    fun pollBacksOffAndSpeedsUpWhenRecipesFinish() {
        assertEquals(1_500L, nextPollDelay(1_500L, moved = true, failed = false))
        assertEquals(2_250L, nextPollDelay(1_500L, moved = false, failed = false))
        assertEquals(8_000L, nextPollDelay(6_000L, moved = false, failed = false))
        assertEquals(10_000L, nextPollDelay(6_000L, moved = false, failed = true))
    }

    @Test
    fun progressCountsFinishedRecipes() {
        assertEquals(0f, checksProgress(run = 0, pending = 3), 0f)
        assertEquals(0.7f, checksProgress(run = 10, pending = 3), 0f)
        assertEquals(1f, checksProgress(run = 10, pending = 0), 0f)
    }
}
