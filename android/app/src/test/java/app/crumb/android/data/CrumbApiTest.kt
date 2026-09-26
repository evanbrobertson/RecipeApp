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

    @Test
    fun createRecipePostsTheFields() = runTest {
        session = Session(server.url("/crumb/"), "c")
        server.enqueue(
            json(
                """{"id":10,"title":"Soup","source":"manual","url":null,"description":null,"image":null,
                   "author":null,"prepTime":null,"cookTime":null,"totalTime":"PT30M","freezeTime":null,
                   "recipeYield":"4","recipeCategory":"Soups","recipeCuisine":null,
                   "ingredients":[{"name":null,"items":["stock"]}],"instructions":[{"name":null,"items":["Simmer."]}],
                   "nutrition":{"calories":"120"},"notes":null,"originalUrl":null,
                   "createdAt":"2026-09-26T16:26:43.000Z","updatedAt":"2026-09-26T16:26:43.000Z","isNew":true}""",
            ),
        )
        val recipe = api.createRecipe(
            RecipeFields(title = "Soup", totalTime = "PT30M", ingredients = listOf(Section(items = listOf("stock")))),
        )
        assertEquals("Soup", recipe.title)
        assertEquals("120", recipe.nutrition?.get("calories")?.toString()?.trim('"'))
        val request = server.takeRequest()
        assertEquals("POST", request.method)
        assertEquals("/crumb/api/recipes", request.url.encodedPath)
        assertEquals("""{"title":"Soup","totalTime":"PT30M","ingredients":[{"items":["stock"]}]}""", request.body?.utf8())
    }

    @Test
    fun deleteRecipesSendsIdsAndReadsTheCount() = runTest {
        session = Session(server.url("/"), "c")
        server.enqueue(json("""{"deleted":2}"""))
        assertEquals(2, api.deleteRecipes(listOf(1, 2, 3)))
        val request = server.takeRequest()
        assertEquals("POST", request.method)
        assertEquals("/api/recipes/bulk-delete", request.url.encodedPath)
        assertEquals("""{"ids":[1,2,3]}""", request.body?.utf8())
    }

    @Test
    fun addToCookbookSendsRecipeIds() = runTest {
        session = Session(server.url("/"), "c")
        server.enqueue(json("""{"added":2}"""))
        assertEquals(2, api.addToCookbook(3, listOf(4, 5)))
        val request = server.takeRequest()
        assertEquals("/api/cookbooks/3/recipes", request.url.encodedPath)
        assertEquals("""{"recipeIds":[4,5]}""", request.body?.utf8())
    }

    @Test
    fun markCookedReadsTheEventId() = runTest {
        session = Session(server.url("/"), "c")
        server.enqueue(json("""{"count":3,"lastCookedAt":"2026-09-26T16:26:43.000Z","eventId":7}"""))
        val cooked = api.markCooked(5)
        assertEquals(3, cooked.count)
        assertEquals(7L, cooked.eventId)
        assertEquals("/api/recipes/5/cooked", server.takeRequest().url.encodedPath)
    }

    @Test
    fun markCookedCanBeDedupedWithNoEvent() = runTest {
        session = Session(server.url("/"), "c")
        server.enqueue(json("""{"count":1,"lastCookedAt":null,"eventId":null}"""))
        val cooked = api.markCooked(5)
        assertEquals(1, cooked.count)
        assertNull(cooked.eventId)
        assertNull(cooked.lastCookedAt)
    }

    @Test
    fun undoCookedSendsTheEventQuery() = runTest {
        session = Session(server.url("/"), "c")
        server.enqueue(json("""{"count":2,"lastCookedAt":null}"""))
        api.undoCooked(5, 9)
        val request = server.takeRequest()
        assertEquals("DELETE", request.method)
        assertEquals("/api/recipes/5/cooked", request.url.encodedPath)
        assertEquals("9", request.url.queryParameter("event"))
    }

    @Test
    fun randomRecipeReturnsNullOn404() = runTest {
        session = Session(server.url("/"), "c")
        server.enqueue(json("""{"statusCode":404,"statusMessage":"Not Found","message":"No recipes saved yet"}""", 404))
        assertNull(api.randomRecipe(exclude = listOf(2), current = 5))
        val request = server.takeRequest()
        assertEquals("2", request.url.queryParameter("exclude"))
        assertEquals("5", request.url.queryParameter("current"))
    }

    @Test
    fun randomRecipeDecodesASummary() = runTest {
        session = Session(server.url("/"), "c")
        server.enqueue(json("""{"id":4,"title":"Pie","source":"url","createdAt":"2026-09-26T16:26:43.000Z"}"""))
        assertEquals(4L, api.randomRecipe(listOf(1))?.id)
    }

    @Test
    fun suggestionsDecodeWithAndWithoutAnIdea() = runTest {
        session = Session(server.url("/"), "c")
        server.enqueue(
            json(
                """{"items":[{"recipe":{"id":1,"title":"Soup","source":"manual"},"reason":"Quick and warm",
                   "reasonKind":"quick","ai":false}],"ai":"ready",
                   "idea":{"title":"Ramen","why":"You love soup","searchUrl":"https://www.google.com/search?q=Ramen+recipe"}}""",
            ),
        )
        val withIdea = api.suggestions(limit = 3, seed = 7, exclude = listOf(9, 10))
        assertEquals(1, withIdea.items.size)
        assertEquals("quick", withIdea.items.single().reasonKind)
        assertEquals("Ramen", withIdea.idea?.title)
        assertEquals("ready", withIdea.ai)
        val request = server.takeRequest()
        assertEquals("/api/suggestions", request.url.encodedPath)
        assertEquals("3", request.url.queryParameter("limit"))
        assertEquals("7", request.url.queryParameter("seed"))
        assertEquals("9,10", request.url.queryParameter("exclude"))

        server.enqueue(json("""{"items":[],"ai":"off","idea":null}"""))
        val noIdea = api.suggestions()
        assertTrue(noIdea.items.isEmpty())
        assertNull(noIdea.idea)
    }

    @Test
    fun recipeChecksDecodeNullWhenNeverChecked() = runTest {
        session = Session(server.url("/"), "c")
        server.enqueue(json("null"))
        assertNull(api.recipeChecks(3))
        assertEquals("/api/recipes/3/checks", server.takeRequest().url.encodedPath)

        server.enqueue(
            json(
                """{"status":"done","canUndo":true,"flags":[{"id":8,"field":"ingredients","itemText":"flour",
                   "kind":"duplicate","state":"review","detail":{"p":0.8,"fix":"removed"}}]}""",
            ),
        )
        val checks = api.recipeChecks(3)
        assertEquals("done", checks?.status)
        assertTrue(checks?.canUndo == true)
        assertEquals("flour", checks?.flags?.single()?.itemText)
        assertTrue(checks?.flags?.single()?.detail?.toString()?.contains("0.8") == true)
    }

    @Test
    fun checksStatusDecodesEveryCount() = runTest {
        session = Session(server.url("/"), "c")
        server.enqueue(
            json(
                """{"enabled":true,"eligible":20,"checked":12,"pending":1,"failed":2,"tidied":5,
                   "toCheck":3,"due":4,"restored":1,"edited":1,"queued":4}""",
            ),
        )
        val status = api.checksStatus()
        assertTrue(status.enabled)
        assertEquals(20, status.eligible)
        assertEquals(12, status.checked)
        assertEquals(4, status.due)
        assertEquals(1, status.restored)
        assertEquals("GET", server.takeRequest().method)

        server.enqueue(
            json(
                """{"enabled":true,"eligible":20,"checked":12,"pending":2,"failed":2,"tidied":5,
                   "toCheck":3,"due":4,"restored":1,"edited":1,"queued":4}""",
            ),
        )
        val all = api.checkAll()
        assertEquals(4, all.queued)
        assertEquals("POST", server.takeRequest().method)
    }

    @Test
    fun reviewListDecodesTheWrapper() = runTest {
        session = Session(server.url("/"), "c")
        server.enqueue(
            json(
                """{"recipes":[{"id":1,"title":"Pie","image":null,"count":3,
                   "fields":{"instructions":2,"ingredients":1}}]}""",
            ),
        )
        val list = api.reviewList()
        assertEquals(1, list.size)
        assertEquals("Pie", list.single().title)
        assertEquals(3, list.single().count)
        assertEquals(2, list.single().fields["instructions"])
    }

    @Test
    fun sharesAreCreatedAndStoppedByKind() = runTest {
        session = Session(server.url("/crumb/"), "c")
        server.enqueue(
            json(
                """{"token":"abcDEF","url":"https://crumb.example.com/s/abcDEF","includeNotes":true,
                   "createdAt":"2026-09-26T16:26:43.000Z"}""",
            ),
        )
        val share = api.createShare(ShareKind.Recipe, 4)
        assertEquals("abcDEF", share.token)
        assertTrue(share.includeNotes)
        val create = server.takeRequest()
        assertEquals("POST", create.method)
        assertEquals("/crumb/api/recipes/4/share", create.url.encodedPath)

        server.enqueue(json("""{"ok":true}"""))
        api.stopSharing(ShareKind.Cookbook, 4)
        val stop = server.takeRequest()
        assertEquals("DELETE", stop.method)
        assertEquals("/crumb/api/cookbooks/4/share", stop.url.encodedPath)
    }

    @Test
    fun connectorDecodesTheServerFeatures() = runTest {
        session = Session(server.url("/"), "c")
        server.enqueue(
            json(
                """{"mcpUrl":"https://crumb.example.com/mcp","authEnabled":true,"weeChef":true,
                   "claudeParsing":true,"aiProvider":"deepseek","vision":true,
                   "browserScraping":false,"weeChefChecks":true}""",
            ),
        )
        val info = api.connector()
        assertEquals("https://crumb.example.com/mcp", info.mcpUrl)
        assertEquals("deepseek", info.aiProvider)
        assertTrue(info.weeChef)
        assertFalse(info.browserScraping)
    }

    @Test
    fun importPhotosSendsOnePartPerPhotoAndTheHint() = runTest {
        session = Session(server.url("/"), "c")
        server.enqueue(json("""{"id":11,"title":"Pancakes","isNew":true}"""))
        val result = api.importPhotos(
            listOf("page one".encodeToByteArray(), "page two".encodeToByteArray()),
            hint = "brunch",
        )
        assertEquals(11L, result.id)
        val body = server.takeRequest().body?.utf8().orEmpty()
        assertEquals(2, body.split("name=\"photo\"").size - 1)
        assertEquals(1, body.split("name=\"text\"").size - 1)
        assertTrue(body.contains("brunch"))
    }

    @Test
    fun importFilesSendsOnePartPerFile() = runTest {
        session = Session(server.url("/"), "c")
        server.enqueue(
            json(
                """[{"file":"a.json","created":[{"id":1,"title":"Pie"}],"duplicates":0,"skipped":0},
                   {"file":"b.pdf","created":[],"duplicates":0,"skipped":1,"error":"Couldn't read this file"}]""",
            ),
        )
        val results = api.importFiles(
            listOf(
                UploadFile("a.json", "application/json", "{}".encodeToByteArray()),
                UploadFile("b.pdf", "application/pdf", "PDF".encodeToByteArray()),
            ),
        )
        assertEquals(2, results.size)
        assertEquals("Couldn't read this file", results[1].error)
        val body = server.takeRequest().body?.utf8().orEmpty()
        assertEquals(2, body.split("name=\"file\"").size - 1)
        assertTrue(body.contains("filename=\"a.json\""))
        assertTrue(body.contains("filename=\"b.pdf\""))
    }

    @Test
    fun exportRecipeTakesTheNameFromContentDisposition() = runTest {
        session = Session(server.url("/"), "c")
        server.enqueue(
            MockResponse.Builder()
                .code(200)
                .addHeader("Content-Type", "application/json; charset=utf-8")
                .addHeader("Content-Disposition", "attachment; filename=\"pie.json\"")
                .body("""{"title":"Pie"}""")
                .build(),
        )
        val download = api.exportRecipe(3, "json")
        assertEquals("pie.json", download.fileName)
        assertEquals("application/json", download.mimeType)
        assertEquals("""{"title":"Pie"}""", download.bytes.decodeToString())
        val request = server.takeRequest()
        assertEquals("/api/recipes/3/export", request.url.encodedPath)
        assertEquals("json", request.url.queryParameter("format"))
    }
}
