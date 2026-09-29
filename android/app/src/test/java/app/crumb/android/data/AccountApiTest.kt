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

class AccountApiTest {
    private val server = MockWebServer()
    private var session: Session? = null
    private val crumb = CrumbApi(OkHttpClient()) { session }
    private val api = AccountApi(crumb)

    @Before fun start() = server.start()
    @After fun stop() = server.close()

    private fun json(body: String, code: Int = 200) =
        MockResponse.Builder().code(code).addHeader("Content-Type", "application/json").body(body).build()

    private fun withCookie(body: String, cookie: String) =
        MockResponse.Builder().addHeader("Content-Type", "application/json").addHeader("Set-Cookie", cookie).body(body).build()

    @Test
    fun statusIsReadWithoutASession() = runTest {
        server.enqueue(json("""{"mode":"accounts","signupOpen":true,"providers":["google"]}"""))
        val status = api.status(server.url("/"))
        assertEquals(AuthMode.Accounts, status.authMode)
        assertTrue(status.signupOpen)
        val request = server.takeRequest()
        assertEquals("/api/auth/status", request.url.encodedPath)
        assertNull(request.headers["Cookie"])
    }

    @Test
    fun aServerThatDoesntKnowStatusReadsAsOnePassword() = runTest {
        server.enqueue(json("""{"message":"Not found"}""", 404))
        assertEquals(AuthMode.Password, api.status(server.url("/")).authMode)
        server.enqueue(json("not json"))
        assertEquals(AuthMode.Password, api.status(server.url("/")).authMode)
    }

    @Test
    fun accountsSignInPostsEmailAndKeepsTheSessionCookie() = runTest {
        server.enqueue(withCookie("""{"ok":true}""", "crumb_session=9.sig; Path=/; HttpOnly"))
        val cookie = api.signIn(server.url("/"), AuthMode.Accounts, " sam@example.com ", "hunter22")
        assertEquals("9.sig", cookie)
        val request = server.takeRequest()
        assertEquals("/api/auth/login", request.url.encodedPath)
        assertEquals("""{"email":"sam@example.com","password":"hunter22"}""", request.body?.utf8())
        assertNull(request.headers["Origin"])
    }

    @Test
    fun hostedSignInUsesBetterAuthAndNamesItsOrigin() = runTest {
        server.enqueue(withCookie("""{"token":"t"}""", "crumb.session_token=t.s; Path=/; HttpOnly"))
        val cookie = api.signIn(server.url("/"), AuthMode.Hosted, "sam@example.com", "hunter22")
        assertEquals("crumb.session_token=t.s", cookie)
        val request = server.takeRequest()
        assertEquals("/api/auth/sign-in/email", request.url.encodedPath)
        assertEquals("http://${server.hostName}:${server.port}", request.headers["Origin"])
    }

    @Test
    fun aWrongPasswordIsA401WithTheServersWords() = runTest {
        server.enqueue(json("""{"statusCode":401,"statusMessage":"Unauthorized","message":"Incorrect email or password"}""", 401))
        try {
            api.signIn(server.url("/"), AuthMode.Accounts, "sam@example.com", "nope")
            fail()
        } catch (e: ApiException) {
            assertEquals(401, e.status)
            assertEquals("Incorrect email or password", e.message)
        }
    }

    @Test
    fun aSignInWithoutACookieIsAnError() = runTest {
        server.enqueue(json("""{"ok":true}"""))
        try {
            api.signIn(server.url("/"), AuthMode.Accounts, "sam@example.com", "hunter22")
            fail()
        } catch (e: ApiException) {
            assertTrue(e.message.contains("session"))
        }
    }

    @Test
    fun signUpAccountsKeepsTheCookieHostedMayNeedAConfirmation() = runTest {
        server.enqueue(withCookie("""{"ok":true}""", "crumb_session=1.a"))
        val made = api.signUp(server.url("/"), AuthMode.Accounts, "Sam", "sam@example.com", "hunter22")
        assertFalse(made.verify)
        assertEquals("1.a", made.cookie)
        assertEquals("/api/auth/signup", server.takeRequest().url.encodedPath)

        server.enqueue(json("""{"token":null,"user":{"id":"u"}}"""))
        val verify = api.signUp(server.url("/"), AuthMode.Hosted, "Sam", "sam@example.com", "hunter22")
        assertTrue(verify.verify)
        assertNull(verify.cookie)
        val request = server.takeRequest()
        assertEquals("/api/auth/sign-up/email", request.url.encodedPath)
        assertTrue(request.body!!.utf8().contains(""""callbackURL":"/""""))

        server.enqueue(withCookie("""{"token":"tok"}""", "crumb.session_token=tok.s"))
        val direct = api.signUp(server.url("/"), AuthMode.Hosted, "Sam", "sam@example.com", "hunter22")
        assertFalse(direct.verify)
        assertEquals("crumb.session_token=tok.s", direct.cookie)
    }

    @Test
    fun setUpSendsTheAppPasswordOnlyWhenGiven() = runTest {
        server.enqueue(withCookie("""{"ok":true}""", "crumb_session=1.a"))
        api.setUp(server.url("/"), "Sam", "sam@example.com", "hunter22", "old")
        assertTrue(server.takeRequest().body!!.utf8().contains(""""appPassword":"old""""))
        server.enqueue(withCookie("""{"ok":true}""", "crumb_session=1.a"))
        api.setUp(server.url("/"), "Sam", "sam@example.com", "hunter22", "")
        assertFalse(server.takeRequest().body!!.utf8().contains("appPassword"))
    }

    @Test
    fun signedInCallsCarryTheHostedCookieAndOrigin() = runTest {
        session = Session(server.url("/"), "crumb.session_token=t.s")
        // hosted reads list-accounts, which answers a list
        server.enqueue(json("""[{"id":"a1","providerId":"credential","createdAt":"2026-01-01T00:00:00.000Z"}]"""))
        assertTrue(api.signInMethods(AuthMode.Hosted).password)
        val request = server.takeRequest()
        assertEquals("/api/auth/list-accounts", request.url.encodedPath)
        assertEquals("crumb.session_token=t.s", request.headers["Cookie"])
        assertEquals("http://${server.hostName}:${server.port}", request.headers["Origin"])
    }

    @Test
    fun hostedSignOutUsesBetterAuthsPath() = runTest {
        session = Session(server.url("/"), "crumb.session_token=t.s")
        server.enqueue(json("""{"success":true}"""))
        crumb.logout()
        assertEquals("/api/auth/sign-out", server.takeRequest().url.encodedPath)

        session = Session(server.url("/"), "1.a")
        server.enqueue(json("""{"ok":true}"""))
        crumb.logout()
        val request = server.takeRequest()
        assertEquals("/api/auth/logout", request.url.encodedPath)
        assertEquals("crumb_session=1.a", request.headers["Cookie"])
    }

    @Test
    fun connectedAppsListAndDisconnect() = runTest {
        session = Session(server.url("/"), "1.a")
        server.enqueue(json("""[{"id":"abc/1","name":"Claude","household":{"id":1,"name":"Home"},"connectedAt":1700000000}]"""))
        val apps = api.connectedApps()
        assertEquals("Claude", apps.single().name)
        assertEquals("Home", apps.single().household?.name)
        server.takeRequest()
        server.enqueue(MockResponse.Builder().code(204).build())
        api.disconnect("abc/1")
        val request = server.takeRequest()
        assertEquals("DELETE", request.method)
        assertEquals("/api/connections/abc%2F1", request.url.encodedPath)
    }

    @Test
    fun changingEmailReportsWhetherItIsPending() = runTest {
        session = Session(server.url("/"), "1.a")
        server.enqueue(json("""{"email":"new@x.io","pending":true}"""))
        val made = api.changeEmail(" new@x.io ", "pw")
        assertTrue(made.pending)
        assertEquals("""{"email":"new@x.io","password":"pw"}""", server.takeRequest().body?.utf8())
    }

    @Test
    fun aHostedInviteIsALinkToTheInvitePage() = runTest {
        session = Session(server.url("/crumb/"), "crumb.session_token=t.s")
        server.enqueue(json("""{"id":"inv1"}"""))
        val link = api.invite(AuthMode.Hosted, "al@x.io")
        assertEquals("http://${server.hostName}:${server.port}/crumb/invite#inv1", link)
    }
}
