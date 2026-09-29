package app.crumb.android.data

import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
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

    @Test
    fun startingABrowserSignInSendsAPkceChallengeAndKeepsTheVerifier() = runTest {
        server.enqueue(json("""{"url":"http://localhost:3200/app/sign-in?id=abc"}"""))
        val started = api.startAppSignIn(server.url("/"), Provider.Google)
        assertEquals("http://localhost:3200/app/sign-in?id=abc", started.url)
        val request = server.takeRequest()
        assertEquals("POST", request.method)
        assertEquals("/api/auth/app/start", request.url.encodedPath)
        assertNull(request.headers["Cookie"])
        val body = CrumbJson.parseToJsonElement(request.body!!.utf8()).jsonObject
        assertEquals("google", body["provider"]!!.jsonPrimitive.content)
        // The server gets the challenge; only the app keeps the verifier that makes it
        assertEquals(43, started.verifier.length)
        assertEquals(app.crumb.core.appSignIn().challenge.length, body["challenge"]!!.jsonPrimitive.content.length)
        assertFalse(request.body!!.utf8().contains(started.verifier))
    }

    @Test
    fun aProviderTheServerDoesntOfferIsA404() = runTest {
        server.enqueue(json("""{"statusCode":404,"message":"That sign-in isn't set up here"}""", 404))
        try {
            api.startAppSignIn(server.url("/"), Provider.Apple)
            fail()
        } catch (e: ApiException) {
            assertEquals(404, e.status)
            assertEquals("That sign-in isn't set up here", e.message)
        }
    }

    @Test
    fun finishingABrowserSignInTradesTheCodeAndVerifierForTheSessionCookie() = runTest {
        server.enqueue(withCookie("""{"ok":true}""", "crumb_session=9.sig; Path=/; HttpOnly"))
        assertEquals("9.sig", api.finishAppSignIn(server.url("/"), "code_1", "verifier_1"))
        val request = server.takeRequest()
        assertEquals("/api/auth/app/redeem", request.url.encodedPath)
        assertEquals("""{"code":"code_1","verifier":"verifier_1"}""", request.body!!.utf8())
        assertNull(request.headers["Cookie"])

        // Hosted sign-ins come back as Better Auth's cookie, kept the same way as its password one
        server.enqueue(withCookie("""{"ok":true}""", "crumb.session_token=t.s; Path=/"))
        assertEquals("crumb.session_token=t.s", api.finishAppSignIn(server.url("/"), "c", "v"))
    }

    @Test
    fun anExpiredOrUsedCodeIsA410() = runTest {
        server.enqueue(json("""{"statusCode":410,"message":"That sign-in has expired. Try again."}""", 410))
        try {
            api.finishAppSignIn(server.url("/"), "c", "v")
            fail()
        } catch (e: ApiException) {
            assertEquals(410, e.status)
            assertEquals(InviteText.SIGN_IN_EXPIRED, InviteText.redeemFailure(e))
        }
    }

    @Test
    fun anAccountsInvitePreviewNeedsNoSession() = runTest {
        session = Session(server.url("/"), "1.a")
        server.enqueue(json("""{"householdName":"The Robertsons","invitedBy":"Evan"}"""))
        val preview = api.previewInvite(AuthMode.Accounts, "tok_1")
        assertEquals("Evan invited you to The Robertsons", preview.headline)
        val request = server.takeRequest()
        assertEquals("/api/auth/invite/preview", request.url.encodedPath)
        assertEquals("""{"token":"tok_1"}""", request.body!!.utf8())
    }

    @Test
    fun aHostedInvitePreviewReadsBetterAuthsInvitation() = runTest {
        session = Session(server.url("/"), "crumb.session_token=t.s")
        server.enqueue(json("""{"organizationName":"Home","inviterEmail":"al@x.io","email":"me@x.io"}"""))
        val preview = api.previewInvite(AuthMode.Hosted, "inv1")
        assertEquals("Home", preview.household)
        assertEquals("me@x.io", preview.email)
        val request = server.takeRequest()
        assertEquals("/api/auth/organization/get-invitation", request.url.encodedPath)
        assertEquals("inv1", request.url.queryParameter("id"))
    }

    @Test
    fun anInviteThatIsGoneIsA410WithTheServersWords() = runTest {
        session = Session(server.url("/"), "1.a")
        server.enqueue(json("""{"statusCode":410,"message":"This invite has expired or was already used. Ask for a new one."}""", 410))
        try {
            api.previewInvite(AuthMode.Accounts, "old")
            fail()
        } catch (e: ApiException) {
            assertEquals(410, e.status)
            assertEquals(InviteText.GONE, e.message)
        }
        // Hosted answers a missing invitation with anything from a 400 to a 404
        session = Session(server.url("/"), "crumb.session_token=t.s")
        server.enqueue(json("""{"message":"Invitation not found"}""", 400))
        try {
            api.previewInvite(AuthMode.Hosted, "old")
            fail()
        } catch (e: ApiException) {
            assertEquals(InviteText.GONE, e.message)
        }
    }

    @Test
    fun joiningSignedInPostsTheTokenAndHostedSwitchesToTheNewHousehold() = runTest {
        session = Session(server.url("/"), "1.a")
        server.enqueue(json("""{"ok":true}"""))
        api.acceptInvite(AuthMode.Accounts, "tok_1")
        val accounts = server.takeRequest()
        assertEquals("/api/auth/invite/accept", accounts.url.encodedPath)
        assertEquals("""{"token":"tok_1"}""", accounts.body!!.utf8())
        assertEquals("crumb_session=1.a", accounts.headers["Cookie"])

        session = Session(server.url("/"), "crumb.session_token=t.s")
        server.enqueue(json("""{"member":{"organizationId":"org_9"}}"""))
        server.enqueue(json("""{"ok":true}"""))
        api.acceptInvite(AuthMode.Hosted, "inv1")
        assertEquals("/api/auth/organization/accept-invitation", server.takeRequest().url.encodedPath)
        val switch = server.takeRequest()
        assertEquals("/api/auth/organization/set-active", switch.url.encodedPath)
        assertTrue(switch.body!!.utf8().contains("org_9"))
    }

    @Test
    fun anAccountsInviteCanMakeTheAccountAsItJoins() = runTest {
        server.enqueue(withCookie("""{"ok":true}""", "crumb_session=4.z"))
        val cookie = api.acceptInviteWithAccount(server.url("/"), "tok_1", " Sam ", "sam@example.com", "hunter22")
        assertEquals("4.z", cookie)
        val request = server.takeRequest()
        assertEquals("/api/auth/invite/accept", request.url.encodedPath)
        assertEquals("""{"token":"tok_1","name":"Sam","email":"sam@example.com","password":"hunter22"}""", request.body!!.utf8())
        assertNull(request.headers["Cookie"])
    }
}
