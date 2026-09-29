package app.crumb.android.ui.trash

import app.crumb.android.data.CrumbJson
import app.crumb.android.data.Restored
import app.crumb.android.data.Trashed
import kotlinx.serialization.builtins.ListSerializer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.time.Instant

/** The trash's wording and the JSON it reads (web/src/islands/TrashPage.svelte). */
class TrashLogicTest {
    private val now = Instant.parse("2026-09-01T12:00:00Z").toEpochMilli()

    @Test fun decodesTheTrashList() {
        val list = CrumbJson.decodeFromString(
            ListSerializer(Trashed.serializer()),
            """[{"id":4,"title":"Soup","url":null,"image":"https://x.test/a.jpg","deletedAt":"2026-09-01T10:00:00.000Z","purgeAt":"2026-10-01T10:00:00.000Z","extra":1}]""",
        )
        assertEquals(Trashed(4, "Soup", null, "https://x.test/a.jpg", "2026-09-01T10:00:00.000Z", "2026-10-01T10:00:00.000Z"), list.single())
    }

    @Test fun aRestoreThatFoundItsLinkAgainIsNotNew() {
        val back = CrumbJson.decodeFromString(Restored.serializer(), """{"id":4,"title":"Soup","isNew":true,"ingredients":[]}""")
        assertEquals(Restored(4, "Soup", true), back)
        val again = CrumbJson.decodeFromString(Restored.serializer(), """{"id":9,"title":"Soup","isNew":false}""")
        assertEquals("It's already in your box" to "Its link was saved again since", restoredToast(again, "Soup"))
        assertEquals("Put back" to "Soup", restoredToast(back, "Soup"))
    }

    @Test fun countsDaysToTheDayItGoes() {
        assertEquals("12 days left", daysLeftLabel("2026-09-13T12:00:00.000Z", now))
        // Part of a day counts as a whole one, as on the web
        assertEquals("2 days left", daysLeftLabel("2026-09-02T12:00:01.000Z", now))
        assertEquals("Goes for good today", daysLeftLabel("2026-09-02T12:00:00.000Z", now))
        assertEquals("Goes for good today", daysLeftLabel("2026-08-30T00:00:00.000Z", now))
        assertNull(daysLeftLabel("soon", now))
    }

    @Test fun deleteWordingSaysTheyGoToTheTrash() {
        assertEquals("Moved 1 recipe to the trash", movedToTrashTitle(1))
        assertEquals("Moved 3 recipes to the trash", movedToTrashTitle(3))
        assertEquals("1 recipe will go to the trash, where you can put it back for 30 days.", deleteManyDescription(1))
        assertEquals("2 recipes will go to the trash, where you can put them back for 30 days.", deleteManyDescription(2))
        assertEquals("Put back", putBackTitle(1))
        assertEquals("Put back 2 recipes", putBackTitle(2))
        assertEquals("1 recipe will be deleted for good. This can't be undone.", emptyTrashDescription(1))
    }

    @Test
    fun actionsGoBelowTheTitleWhenTheRowIsNarrow() {
        assertEquals(false, actionsBeside(350f, 1f))
        assertEquals(false, actionsBeside(450f, 1.3f))
        assertEquals(true, actionsBeside(600f, 1f))
        assertEquals(true, actionsBeside(720f, 1.3f))
    }
}
