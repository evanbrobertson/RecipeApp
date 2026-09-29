package app.crumb.android.ui.recipe

import app.crumb.android.data.Flag
import app.crumb.android.data.Recipe
import app.crumb.android.data.RecipeChecks
import app.crumb.android.data.VideoEmbed
import app.crumb.android.data.VideoWin
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The video's placement arithmetic and the Wee Chef card's photo line (web RecipeVideo / WeeChefCard). */
class VideoLogicTest {
    private val phone = VideoScreen(width = 390f, height = 844f, top = 24f, bottom = 16f)
    private val youtube = VideoEmbed("youtube", "YouTube", "https://www.youtube.com/embed/abc", "https://youtu.be/abc", null, false)

    @Test fun landscapeFirstFloatsFullWidthBelowTheStatusBar() {
        val w = VideoWindow.firstPlace(tall = false, s = phone)
        assertEquals(374f, w.w, 0f)
        assertEquals(8f, w.x, 0f)
        assertEquals(32f, w.y, 0f)
    }

    @Test fun tallFirstFloatsTopRightAtAboutHalf() {
        val w = VideoWindow.firstPlace(tall = true, s = phone)
        assertEquals(176f, w.w, 0f) // 45% of 390, rounded
        assertEquals(390f - 176f - 8f, w.x, 0f)
        assertEquals(32f, w.y, 0f)
    }

    @Test fun wideScreensFloatBottomRight() {
        val tablet = VideoScreen(1000f, 700f)
        val w = VideoWindow.firstPlace(false, tablet)
        assertEquals(448f, w.w, 0f)
        assertEquals(1000f - 448f - 8f, w.x, 0f)
        assertEquals(700f - VideoWindow.height(448f, false) - 80f, w.y, 0f)
    }

    @Test fun clampsEightInsideTheScreenAndBelowTheBars() {
        val w = VideoWindow.fit(VideoWin(-50f, -50f, 300f), false, phone)
        assertEquals(8f, w.x, 0f)
        assertEquals(32f, w.y, 0f)
        val far = VideoWindow.fit(VideoWin(900f, 900f, 300f), false, phone)
        assertEquals(390f - 300f - 8f, far.x, 0f)
        assertEquals(844f - 16f - VideoWindow.height(300f, false) - 8f, far.y, 0f)
    }

    @Test fun widthStaysBetweenTheMinimumAndWhatFits() {
        assertEquals(260f, VideoWindow.fitWidth(100f, false, phone), 0f)
        assertEquals(150f, VideoWindow.fitWidth(100f, true, phone), 0f)
        assertEquals(374f, VideoWindow.fitWidth(900f, false, phone), 0f)
        // A short screen limits a tall video by height
        val landscapePhone = VideoScreen(800f, 360f)
        val most = VideoWindow.fitWidth(900f, true, landscapePhone)
        assertTrue(VideoWindow.height(most, true) <= 360f - 16f)
    }

    @Test fun heightFollowsThePictureShape() {
        assertEquals(360f + 40f, VideoWindow.height(640f, false), 0f)
        assertEquals(320f + 40f, VideoWindow.height(180f, true), 0f)
    }

    @Test fun dragMovesAndKeepsItOnScreen() {
        val from = VideoWin(50f, 100f, 260f)
        assertEquals(VideoWin(80f, 90f, 260f), VideoWindow.apply(VideoDrag.Move, from, 30f, -10f, false, phone))
        assertEquals(122f, VideoWindow.apply(VideoDrag.Move, from, 300f, 0f, false, phone).x, 0f)
        assertEquals(8f, VideoWindow.apply(VideoDrag.Move, from, -500f, 0f, false, phone).x, 0f)
    }

    @Test fun rightCornerGrowsKeepingTheTopLeft() {
        val from = VideoWin(20f, 100f, 260f)
        val grown = VideoWindow.apply(VideoDrag.Right, from, 60f, 0f, false, phone)
        assertEquals(320f, grown.w, 0f)
        assertEquals(20f, grown.x, 0f)
        // the bottom edge stays put, so the top moves up by the extra height
        assertEquals(100f + VideoWindow.height(260f, false) - VideoWindow.height(320f, false), grown.y, 0f)
    }

    @Test fun leftCornerGrowsKeepingTheRightEdge() {
        val from = VideoWin(100f, 100f, 260f)
        val grown = VideoWindow.apply(VideoDrag.Left, from, -20f, 0f, false, phone)
        assertEquals(280f, grown.w, 0f)
        assertEquals(80f, grown.x, 0f)
        assertEquals(from.x + from.w, grown.x + grown.w, 0f)
    }

    @Test fun resizeNeverGoesBelowTheMinimum() {
        val from = VideoWin(100f, 100f, 260f)
        assertEquals(260f, VideoWindow.apply(VideoDrag.Right, from, -200f, 0f, false, phone).w, 0f)
        val tall = VideoWin(100f, 100f, 176f)
        assertEquals(150f, VideoWindow.apply(VideoDrag.Right, tall, -100f, 0f, true, phone).w, 0f)
    }

    @Test fun aSavedWindowFromAnotherScreenIsBroughtBack() {
        val w = VideoWindow.fit(VideoWin(Float.NaN, Float.NaN, Float.NaN), false, phone)
        assertEquals(260f, w.w, 0f)
        assertTrue(w.x >= 8f && w.y >= 32f)
    }

    @Test fun theBarDropsBackToVideoBeforeItOverflows() {
        // 4 buttons of 40dp plus a 40dp grip fit in 200dp; the narrow portrait window is 150-176dp
        assertFalse(VideoWindow.showBack(176f))
        assertFalse(VideoWindow.showBack(150f))
        assertTrue(VideoWindow.showBack(260f))
        assertFalse(VideoWindow.showBackLabel(260f))
        assertTrue(VideoWindow.showBackLabel(300f))
    }

    @Test fun floatsOnceAQuarterOfTheSlotIsLeftAboveTheScreen() {
        assertFalse(slotAway(top = 50f, height = 200f, viewport = 800f)) // in view
        assertFalse(slotAway(top = -100f, height = 200f, viewport = 800f)) // half visible
        assertTrue(slotAway(top = -160f, height = 200f, viewport = 800f)) // a fifth visible
        assertTrue(slotAway(top = -300f, height = 200f, viewport = 800f))
        assertFalse(slotAway(top = 900f, height = 200f, viewport = 800f)) // not reached yet
    }

    @Test fun floatsAtTheEndOfAShortPageOnceTheSlotStartsToLeave() {
        assertFalse(slotAway(top = -100f, height = 200f, viewport = 800f)) // room left to scroll
        assertTrue(slotAway(top = -100f, height = 200f, viewport = 800f, atEnd = true))
        assertFalse(slotAway(top = 0f, height = 200f, viewport = 800f, atEnd = true)) // wholly in view
        assertFalse(slotAway(top = 50f, height = 200f, viewport = 800f, atEnd = true))
    }

    @Test fun autoplayOnlyWhereTheSiteAllowsIt() {
        assertEquals("https://www.youtube.com/embed/abc?autoplay=1", autoplayUrl(youtube))
        assertEquals("https://x.test/e?a=1&autoplay=1", autoplayUrl(youtube.copy(embedUrl = "https://x.test/e?a=1")))
        assertEquals("https://www.tiktok.com/embed/v2/1", autoplayUrl(youtube.copy(provider = "tiktok", embedUrl = "https://www.tiktok.com/embed/v2/1")))
    }

    @Test fun playerPageOnlyLoadsWebAddresses() {
        val html = playerHtml(youtube.copy(embedUrl = "https://a.test/e?x=1&y=\"2\""))!!
        assertTrue(html.contains("<iframe src=\"https://a.test/e?x=1&amp;y=&quot;2&quot;&amp;autoplay=1\""))
        assertNull(playerHtml(youtube.copy(embedUrl = "javascript:alert(1)")))
        assertNull(playerHtml(youtube.copy(embedUrl = "file:///etc/passwd")))
    }

    @Test fun watchLinkNamesTheSite() {
        assertEquals("Watch on YouTube", watchLabel(youtube, "youtu.be"))
        assertEquals("cdn.example.com", watchLabel(youtube.copy(provider = "file", label = "Video"), "cdn.example.com"))
        assertEquals("Watch", watchLabel(null, null))
    }

    @Test fun offlineCopyWithoutAnEmbedFallsBackToCore() {
        val recipe = Recipe(id = 1, title = "Soup", video = "https://www.youtube.com/watch?v=dQw4w9WgXcQ")
        val embed = videoEmbedFor(recipe)
        assertNotNull(embed)
        assertEquals("youtube", embed!!.provider)
        assertNull(videoEmbedFor(Recipe(id = 2, title = "Toast")))
        // The server's answer wins
        assertEquals(youtube, videoEmbedFor(recipe.copy(videoEmbed = youtube)))
    }

    @Test fun photoFlagsAreNotLinesToLookAt() {
        fun f(id: Long, field: String, state: String = "review") = Flag(id, field, "x", "dead_image", state)
        val checks = RecipeChecks("done", flags = listOf(f(1, "ingredients"), f(2, "image"), f(3, "instructions", "fixed")))
        assertEquals(listOf(1L), reviewFlags(checks).map { it.id })
        assertEquals(2L, photoFlag(checks)!!.id)
        assertNull(photoFlag(RecipeChecks("done", flags = listOf(f(2, "image", "dismissed")))))
        // Only a photo flag: the card still has something to say
        assertTrue(reviewFlags(RecipeChecks("done", flags = listOf(f(2, "image")))).isEmpty())
    }
}
