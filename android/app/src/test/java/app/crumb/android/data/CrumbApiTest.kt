package app.crumb.android.data

import kotlinx.coroutines.test.runTest
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import okhttp3.OkHttpClient
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test

class CrumbApiTest {
    private val server = MockWebServer()
    private var session: Session? = null
    private val api = CrumbApi(OkHttpClient()) { session }

    @Before fun start() = server.start()
    @After fun stop() = server.close()

    private fun json(body: String, code: Int = 200) =
        MockResponse.Builder().code(code).addHeader("Content-Type", "application/json").body(body).build()

    @Test
    fun loginReturnsTheSessionCookie() = runTest {
        server.enqueue(
            MockResponse.Builder()
                .body("""{"ok":true}""")
                .addHeader("Set-Cookie", "crumb_session=123.abc; Path=/; Max-Age=7776000; HttpOnly; SameSite=Lax")
                .build(),
        )
        assertEquals("123.abc", api.login(server.url("/"), "hunter2"))
        val request = server.takeRequest()
        assertEquals("/api/auth/login", request.url.encodedPath)
        assertEquals("""{"password":"hunter2"}""", request.body?.utf8())
    }

    @Test
    fun aServerWithoutAPasswordGivesNoCookie() = runTest {
        server.enqueue(json("""{"ok":true}"""))
        assertNull(api.login(server.url("/"), "x"))
    }

    @Test
    fun wrongPasswordIsA401WithTheServersMessage() = runTest {
        server.enqueue(json("""{"statusCode":401,"statusMessage":"Unauthorized","message":"Incorrect password"}""", 401))
        try {
            api.login(server.url("/"), "nope")
            fail()
        } catch (e: ApiException) {
            assertTrue(e.isSignedOut)
            assertEquals("Incorrect password", e.message)
        }
    }

    @Test
    fun signedInRequestsSendTheCookieAndDecodeRecipes() = runTest {
        session = Session(server.url("/crumb/"), "123.abc")
        server.enqueue(
            json(
                """[{"id":1,"title":"Soup","image":null,"totalTime":"PT45M","recipeYield":"4","recipeCategory":"Soups",
                   "recipeCuisine":null,"source":"manual","createdAt":"2026-09-26T16:26:43.000Z","somethingNew":true}]""",
            ),
        )
        val list = api.recipes(query = "  soup ")
        assertEquals(listOf(RecipeSummary(1, "Soup", totalTime = "PT45M", recipeYield = "4", recipeCategory = "Soups", createdAt = "2026-09-26T16:26:43.000Z")), list)
        val request = server.takeRequest()
        assertEquals("/crumb/api/recipes", request.url.encodedPath)
        assertEquals("soup", request.url.queryParameter("q"))
        assertEquals("crumb_session=123.abc", request.headers["Cookie"])
    }

    @Test
    fun recipeSectionsAndNullsDecode() = runTest {
        session = Session(server.url("/"), null)
        server.enqueue(
            json(
                """{"id":2,"url":null,"source":"url","title":"Pie","description":null,"image":"https://x/p.jpg",
                   "ingredients":[{"name":null,"items":["flour"]},{"name":"Filling","items":["apples"]}],
                   "instructions":[{"name":null,"items":["Bake."]}],"nutrition":{"calories":"300"},"notes":null}""",
            ),
        )
        val recipe = api.recipe(2)
        assertEquals("Filling", recipe.ingredients[1].name)
        assertEquals(listOf("Bake."), recipe.instructions.single().items)
        assertNull(server.takeRequest().headers["Cookie"])
    }

    @Test
    fun importTellsNewFromExisting() = runTest {
        session = Session(server.url("/"), "c")
        server.enqueue(json("""{"id":9,"title":"Pie","isNew":false}"""))
        val result = api.importUrl("https://example.com/pie")
        assertFalse(result.isNew)
        assertEquals("""{"url":"https://example.com/pie"}""", server.takeRequest().body?.utf8())
    }

    @Test
    fun unreachableServerIsOffline() = runTest {
        val dead = server.url("/")
        server.close()
        try {
            api.health(dead)
            fail()
        } catch (_: OfflineException) {
        }
    }

    @Test
    fun photoUrlsMatchTheServerRoute() {
        session = Session(server.url("/"), "c")
        val url = api.photoUrl(5, "https://example.com/pie.jpg", 600)!!
        assertTrue(url.endsWith("/img/5/768?v=efcb9837"))
        assertNull(api.photoUrl(5, null, 600))
    }

    @Test
    fun serverAddressesAreForgiving() {
        assertEquals("https://crumb.example.com/", CrumbApi.serverUrl("crumb.example.com")?.toString())
        assertEquals("https://crumb.example.com/", CrumbApi.serverUrl(" https://crumb.example.com/ ")?.toString())
        assertEquals("http://192.168.1.5:3000/", CrumbApi.serverUrl("http://192.168.1.5:3000")?.toString())
        assertEquals("https://example.com/crumb/", CrumbApi.serverUrl("example.com/crumb?x=1")?.toString())
        assertEquals("http://localhost:3100/", CrumbApi.serverUrl("http://localhost:3100")?.toString())
        assertNull(CrumbApi.serverUrl("not a url"))
        assertNull(CrumbApi.serverUrl("crumb"))
        assertNull(CrumbApi.serverUrl(""))
    }

    @Test
    fun errorMessagesDropTheFieldPrefix() {
        assertEquals("Please enter a valid URL", CrumbApi.errorMessage(400, ApiErrorBody(400, "Bad Request", "Please enter a valid URL")))
        assertEquals("Password is required", CrumbApi.errorMessage(400, ApiErrorBody(message = "password: Password is required")))
        assertEquals("Please sign in again.", CrumbApi.errorMessage(401, null))
    }

    @Test
    fun onlyTheCrumbSessionCookieCounts() {
        assertEquals("1.a", CrumbApi.sessionCookie("crumb_session=1.a; Path=/"))
        assertNull(CrumbApi.sessionCookie("crumb_session=; Max-Age=0"))
        assertNull(CrumbApi.sessionCookie("crumb_tz=America/Toronto"))
    }
}
