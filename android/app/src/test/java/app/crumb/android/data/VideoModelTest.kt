package app.crumb.android.data

import app.crumb.android.ui.edit.RecipeDraft
import app.crumb.android.ui.edit.photoFlag
import app.crumb.android.ui.edit.recipeDraft
import app.crumb.android.ui.edit.toFields
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class VideoModelTest {
    @Test fun decodesAVideoWithItsEmbed() {
        val json = """{"id":1,"title":"Soup","video":"https://youtu.be/abc","videoEmbed":{"provider":"youtube","label":"YouTube",
            "embedUrl":"https://www.youtube.com/embed/abc","watchUrl":"https://youtu.be/abc","thumbnail":"https://i.ytimg.com/vi/abc/hq.jpg","vertical":false}}"""
        val r = CrumbJson.decodeFromString(Recipe.serializer(), json)
        assertEquals("https://youtu.be/abc", r.video)
        assertEquals("YouTube", r.videoEmbed?.label)
        assertEquals("https://i.ytimg.com/vi/abc/hq.jpg", r.videoEmbed?.thumbnail)
        assertFalse(r.videoEmbed!!.vertical)
    }

    @Test fun decodesARecipeWithoutOne() {
        val r = CrumbJson.decodeFromString(Recipe.serializer(), """{"id":1,"title":"Soup"}""")
        assertNull(r.video)
        assertNull(r.videoEmbed)
        val unplayable = CrumbJson.decodeFromString(Recipe.serializer(), """{"id":1,"title":"Soup","video":"https://x.test/v","videoEmbed":null}""")
        assertNull(unplayable.videoEmbed)
    }

    @Test fun decodesATallEmbedWithoutAStill() {
        val e = CrumbJson.decodeFromString(
            VideoEmbed.serializer(),
            """{"provider":"tiktok","label":"TikTok","embedUrl":"https://www.tiktok.com/embed/v2/1","watchUrl":"https://www.tiktok.com/@a/video/1","thumbnail":null,"vertical":true}""",
        )
        assertNull(e.thumbnail)
        assertTrue(e.vertical)
    }

    @Test fun theEditorSendsTheVideoLink() {
        val draft = recipeDraft(Recipe(id = 1, title = "Soup", video = "https://youtu.be/abc"))
        assertEquals("https://youtu.be/abc", draft.video)
        assertEquals("https://youtu.be/abc", draft.toFields().video)
        assertNull(draft.copy(video = "  ").toFields().video)
    }

    @Test fun importReportsADroppedPhoto() {
        val r = CrumbJson.decodeFromString(ImportResult.serializer(), """{"id":3,"title":"Pie","isNew":true,"droppedPhoto":true}""")
        assertTrue(r.droppedPhoto)
        assertFalse(CrumbJson.decodeFromString(ImportResult.serializer(), """{"id":3,"title":"Pie","isNew":true}""").droppedPhoto)
    }

    @Test fun theEditorShowsThePhotoFlagOnlyWhileTheLinkIsStillThere() {
        val flag = Flag(9, "image", "https://x.test/a.jpg", "dead_image", "review")
        val other = Flag(10, "ingredients", "https://x.test/a.jpg", "junk", "review")
        assertEquals(flag, photoFlag(listOf(other, flag), "https://x.test/a.jpg"))
        assertNull(photoFlag(listOf(flag), "https://x.test/b.jpg"))
        assertNull(photoFlag(listOf(flag), RecipeDraft().image))
        assertNull(photoFlag(listOf(flag.copy(state = "dismissed")), "https://x.test/a.jpg"))
    }
}
