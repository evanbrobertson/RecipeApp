package app.crumb.android.data

import kotlinx.coroutines.test.runTest
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import okhttp3.OkHttpClient
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test

class PreviewTest {
    private val server = MockWebServer()
    private val api = CrumbApi(OkHttpClient()) { Session(server.url("/"), "c") }

    @Before fun start() = server.start()
    @After fun stop() = server.close()

    private fun json(body: String, code: Int = 200) =
        MockResponse.Builder().code(code).addHeader("Content-Type", "application/json").body(body).build()

    @Test
    fun aSavedLink() {
        assertEquals(Preview.Saved(4, "Pie"), parsePreview("""{"status":"saved","id":4,"title":"Pie"}"""))
    }

    @Test
    fun aVideoOrSharedCookbookIsImportedInstead() {
        assertEquals(Preview.Import, parsePreview("""{"status":"import"}"""))
    }

    @Test
    fun aReadRecipeHasNoIdOrDates() {
        val ready = parsePreview(
            """{"status":"ready","recipe":{"url":"https://example.com/pie","source":"scraped","title":"Pie",
                "image":"https://example.com/pie.jpg","author":"Sam","totalTime":"PT1H","recipeYield":"8",
                "ingredients":[{"name":null,"items":["1 crust"]}],"instructions":[{"items":["Bake."]}],
                "notes":"Cool it","video":"https://youtu.be/abc",
                "videoEmbed":{"provider":"youtube","label":"YouTube","embedUrl":"https://www.youtube.com/embed/abc",
                "watchUrl":"https://youtu.be/abc","thumbnail":null,"vertical":false},"someday":1}}""",
        ) as Preview.Ready
        val r = ready.recipe
        assertEquals(0L, r.id)
        assertEquals("Pie", r.title)
        assertNull(r.createdAt)
        assertEquals("https://example.com/pie.jpg", r.image)
        assertEquals(listOf("1 crust"), r.ingredients.single().items)
        assertEquals("Cool it", r.notes)
        assertEquals("youtube", r.videoEmbed?.provider)
    }

    @Test
    fun anUnknownStatusIsAnError() {
        try {
            parsePreview("""{"status":"later"}""")
            fail()
        } catch (e: ApiException) {
            assertEquals("Couldn't read that recipe", e.message)
        }
    }

    @Test
    fun previewPostsTheLink() = runTest {
        server.enqueue(json("""{"status":"import"}"""))
        assertEquals(Preview.Import, api.preview("https://youtu.be/abc"))
        val request = server.takeRequest()
        assertEquals("/api/recipes/preview", request.url.encodedPath)
        assertEquals("""{"url":"https://youtu.be/abc"}""", request.body?.utf8())
    }

    @Test
    fun aSiteWhoseTermsForbidFetchingCarriesItsCode() = runTest {
        server.enqueue(
            json(
                """{"statusCode":422,"statusMessage":"Unprocessable","message":"Crumb doesn't fetch NYT Cooking","code":"site_terms","site":"NYT Cooking"}""",
                422,
            ),
        )
        try {
            api.preview("https://cooking.nytimes.com/recipes/1")
            fail()
        } catch (e: ApiException) {
            assertEquals(422, e.status)
            assertEquals(SITE_TERMS, e.code)
            assertEquals("NYT Cooking", e.site)
            assertEquals("Crumb doesn't fetch NYT Cooking", e.message)
        }
    }

    @Test
    fun anErrorWithoutACodeHasNone() = runTest {
        server.enqueue(json("""{"statusCode":400,"statusMessage":"Bad Request","message":"Please enter a valid URL"}""", 400))
        try {
            api.preview("nope")
            fail()
        } catch (e: ApiException) {
            assertNull(e.code)
            assertNull(e.site)
            assertEquals("Please enter a valid URL", e.message)
        }
    }

    @Test
    fun errorFieldSurvivesJunk() {
        assertNull(errorField("<html>bad gateway</html>", "code"))
        assertEquals(SITE_BLOCKED, errorField("""{"code":"site_blocked"}""", "code"))
    }

    @Test
    fun popularListsEightAtMost() = runTest {
        val items = (1..10).joinToString(",") { """{"url":"https://a.example/$it","host":"a.example","title":"Dish $it","households":3}""" }
        server.enqueue(json("""{"enabled":true,"optedOut":false,"minHouseholds":2,"items":[$items]}"""))
        val links = api.popular()
        assertEquals(8, links.size)
        assertEquals(PopularLink("https://a.example/1", "a.example", "Dish 1", 3), links.first())
        assertEquals("a.example · in 3 boxes", popularSubtitle(links.first()))
    }

    @Test
    fun popularIsEmptyWhenOffOrMissing() = runTest {
        server.enqueue(json("""{"enabled":false,"optedOut":false,"minHouseholds":2,"items":[{"url":"https://a.example/1","host":"a","title":"x","households":2}]}"""))
        assertTrue(api.popular().isEmpty())
        server.enqueue(json("""{"statusCode":404,"statusMessage":"Not Found","message":"Not found"}""", 404))
        assertTrue(api.popular().isEmpty())
    }

    @Test
    fun popularOptOutSetting() = runTest {
        server.enqueue(json("""{"enabled":true,"optedOut":true}"""))
        assertEquals(PopularSetting(enabled = true, optedOut = true), api.popularSetting())
        server.enqueue(json("""{"enabled":false,"optedOut":false}"""))
        assertNull(api.popularSetting())
        server.enqueue(json("""{"statusCode":404,"statusMessage":"Not Found","message":"Not found"}""", 404))
        assertNull(api.popularSetting())
        server.enqueue(json("""{"optedOut":false}"""))
        api.setPopularOptOut(false)
        server.takeRequest(); server.takeRequest(); server.takeRequest()
        val post = server.takeRequest()
        assertEquals("/api/popular/opt-out", post.url.encodedPath)
        assertEquals("""{"optedOut":false}""", post.body?.utf8())
    }
}
