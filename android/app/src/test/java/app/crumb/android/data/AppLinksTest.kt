package app.crumb.android.data

import app.crumb.core.InviteLink
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

class AppLinksTest {
    private val server = MockWebServer()
    private val crumb = CrumbApi(OkHttpClient()) { null }
    private val accounts = AccountApi(crumb)
    private val store = FakeStore()
    private val links = AppLinks(store, accounts, CoroutineScope(SupervisorJob() + Dispatchers.Unconfined))

    @Before fun start() = server.start()
    @After fun stop() = server.close()

    private class FakeStore : PendingStore {
        var signedIn: Pair<String, String?>? = null
        var signIn: PendingSignIn? = null
        var invite: InviteLink? = null
        override suspend fun signIn(server: okhttp3.HttpUrl, cookie: String?) { signedIn = server.toString() to cookie }
        override suspend fun savePendingSignIn(pending: PendingSignIn?) { signIn = pending }
        override suspend fun loadPendingSignIn() = signIn
        override suspend fun savePendingInvite(invite: InviteLink?) { this.invite = invite }
        override suspend fun loadPendingInvite() = invite
    }

    private fun json(body: String, code: Int = 200) =
        MockResponse.Builder().code(code).addHeader("Content-Type", "application/json").body(body).build()

    private fun session(url: String) = Session(url.toHttpUrl(), "1.a")

    // ─── Links ───

    @Test
    fun aSignedInLinkCarriesItsCode() {
        assertEquals(AppLink.SignedIn("abc_DEF-1"), AppLink.parse("app.crumb://signed-in?code=abc_DEF-1"))
        // Not a code the core accepts: still a sign-in link, which then reads as expired
        assertEquals(AppLink.SignedIn(null), AppLink.parse("app.crumb://signed-in"))
        assertEquals(AppLink.SignedIn(null), AppLink.parse("app.crumb://signed-in?code=a%20b"))
    }

    @Test
    fun invitesComeFromBothLinkShapesAndOnlyTheCoreDecides() {
        val app = AppLink.parse("app.crumb://invite?server=https%3A%2F%2Fcrumb.example.com%2F&token=tok_1")
        assertEquals(AppLink.Invite(InviteLink("https://crumb.example.com/", "tok_1")), app)
        assertEquals(app, AppLink.parse("  app.crumb://invite?server=https%3A%2F%2Fcrumb.example.com%2F&token=tok_1 "))
        assertEquals(AppLink.BadInvite, AppLink.parse("app.crumb://invite?server=https%3A%2F%2Fcrumb.example.com%2F"))
        assertEquals(AppLink.BadInvite, AppLink.parse("app.crumb://invite"))
    }

    @Test
    fun otherLinksAreNotOurs() {
        assertNull(AppLink.parse(null))
        assertNull(AppLink.parse("https://crumb.example.com/invite#tok"))
        assertNull(AppLink.parse("app.crumb://elsewhere"))
    }

    @Test
    fun aPendingSignInSurvivesTheStore() {
        val pending = PendingSignIn("http://localhost:3200/".toHttpUrl(), "ver-ifier_1")
        assertEquals(pending, PendingSignIn.decode(pending.encode()))
        assertNull(PendingSignIn.decode("nonsense"))
        assertNull(PendingSignIn.decode("http://localhost:3200/\n"))
        assertNull(PendingSignIn.decode("not a url\nverifier"))
    }

    // ─── Which screen an invite leads to ───

    private val invite = InviteLink("https://crumb.example.com/", "tok_1")

    @Test
    fun nothingWaitingLeadsNowhere() {
        assertEquals(InviteStep.None, inviteStep(null, null))
        assertEquals(InviteStep.None, inviteStep(session("https://crumb.example.com/"), null))
    }

    @Test
    fun signedOutSignsInAtTheInvitesServerFirst() {
        assertEquals(InviteStep.SignInFirst("https://crumb.example.com/".toHttpUrl()), inviteStep(null, invite))
    }

    @Test
    fun signedInToTheInvitesServerJoins() {
        assertEquals(InviteStep.Join(invite), inviteStep(session("https://crumb.example.com/"), invite))
    }

    @Test
    fun signedInElsewhereAsksToSignOutFirst() {
        val step = inviteStep(session("http://192.168.1.5:3000/"), invite) as InviteStep.OtherServer
        assertEquals("crumb.example.com", step.invite)
        assertEquals("192.168.1.5:3000", step.current)
        assertEquals(
            "This invite is for crumb.example.com. Sign out of 192.168.1.5:3000 to join?",
            InviteText.otherServer(step.invite, step.current),
        )
    }

    @Test
    fun aDifferentPortOrSchemeIsADifferentServer() {
        assertTrue(inviteStep(session("http://crumb.example.com/"), invite) is InviteStep.OtherServer)
        assertTrue(inviteStep(session("https://crumb.example.com:8443/"), invite) is InviteStep.OtherServer)
    }

    @Test
    fun anInviteOpensASignUpEvenWhereSignUpIsClosedButOnlyInAccounts() {
        val closed = AuthStatus(mode = "accounts", signupOpen = false)
        assertEquals(listOf(SignInForm.SignIn), signInForms(closed))
        assertEquals(listOf(SignInForm.SignIn, SignInForm.SignUp), signInForms(closed, invited = true))
        val hosted = AuthStatus(mode = "hosted", signupOpen = false)
        assertEquals(listOf(SignInForm.SignIn, SignInForm.Forgot), signInForms(hosted, invited = true))
        // Never for a first account's setup or a single password
        assertEquals(listOf(SignInForm.SetUp), signInForms(AuthStatus(mode = "accounts", setupNeeded = true), invited = true))
        assertEquals(listOf(SignInForm.Password), signInForms(AuthStatus(), invited = true))
    }

    // ─── Words ───

    @Test
    fun redeemFailuresReadAsAnExpiredSignInUnlessTheyAreSomethingElse() {
        assertEquals("That sign-in expired. Try again.", InviteText.redeemFailure(ApiException(410, "That sign-in has expired. Try again.")))
        assertEquals("That sign-in expired. Try again.", InviteText.redeemFailure(ApiException(404, "Not found")))
        assertEquals("Slow down", InviteText.redeemFailure(ApiException(429, "Slow down")))
        assertEquals(InviteText.OFFLINE, InviteText.redeemFailure(OfflineException(java.io.IOException("down"))))
    }

    @Test
    fun anInviteWithoutAnInviterStillSaysWhatItIsFor() {
        assertEquals("Evan invited you to Home", InvitePreview("Home", "Evan").headline)
        assertEquals("You're invited to Home", InvitePreview("Home", null).headline)
        assertEquals("You're invited to Home", InvitePreview("Home", " ").headline)
    }

    // ─── The round trip ───

    @Test
    fun aBrowserSignInIsRedeemedWithTheVerifierKeptWhenItStarted() = runTest {
        server.enqueue(json("""{"url":"http://localhost/app/sign-in?id=x"}"""))
        val url = links.beginSignIn(server.url("/"), Provider.Google)
        assertEquals("http://localhost/app/sign-in?id=x", url)
        val kept = store.signIn!!
        assertEquals(server.url("/"), kept.server)
        server.takeRequest()

        server.enqueue(
            MockResponse.Builder().addHeader("Set-Cookie", "crumb_session=9.sig; Path=/").body("{}").build(),
        )
        links.handle(AppLink.parse("app.crumb://signed-in?code=c_1")!!).join()
        val request = server.takeRequest()
        assertEquals("/api/auth/app/redeem", request.url.encodedPath)
        assertTrue(request.body!!.utf8().contains(""""verifier":"${kept.verifier}""""))
        assertEquals(server.url("/").toString() to "9.sig", store.signedIn)
        // The verifier is used up, and there's nothing to say
        assertNull(store.signIn)
        assertNull(links.notice.value)
    }

    @Test
    fun aLinkWithNoPendingSignInIsExpired() = runTest {
        links.handle(AppLink.SignedIn("c_1")).join()
        assertEquals(InviteText.SIGN_IN_EXPIRED, links.notice.value)
        assertNull(store.signedIn)
        assertEquals(0, server.requestCount)
    }

    @Test
    fun aLinkWithoutACodeIsExpiredAndSpendsTheVerifier() = runTest {
        store.signIn = PendingSignIn(server.url("/"), "v")
        links.handle(AppLink.SignedIn(null)).join()
        assertEquals(InviteText.SIGN_IN_EXPIRED, links.notice.value)
        assertNull(store.signIn)
    }

    @Test
    fun aUsedCodeIs410AndLeavesTheFormAsItWas() = runTest {
        store.signIn = PendingSignIn(server.url("/"), "v")
        server.enqueue(json("""{"statusCode":410,"message":"That sign-in has expired. Try again."}""", 410))
        links.handle(AppLink.SignedIn("c_1")).join()
        assertEquals(InviteText.SIGN_IN_EXPIRED, links.notice.value)
        assertNull(store.signedIn)
        assertNull(store.signIn)
    }

    @Test
    fun anUnreachableServerSaysSo() = runTest {
        store.signIn = PendingSignIn(server.url("/"), "v")
        server.close()
        links.handle(AppLink.SignedIn("c_1")).join()
        assertEquals(InviteText.OFFLINE, links.notice.value)
    }

    @Test
    fun anInviteWaitsUntilDismissedAndSurvivesRestarts() = runTest {
        links.handle(AppLink.Invite(invite)).join()
        assertEquals(invite, links.invite.value)
        assertEquals(invite, store.invite)

        // A new process starts with what was kept
        val restarted = AppLinks(store, accounts, CoroutineScope(SupervisorJob() + Dispatchers.Unconfined))
        restarted.load()
        assertEquals(invite, restarted.invite.value)

        links.dismissInvite()
        assertNull(links.invite.value)
        assertNull(store.invite)
    }

    @Test
    fun aLinkThatArrivedBeforeLoadingIsNotReplacedByAnOlderOne() = runTest {
        store.invite = InviteLink("https://old.example.com/", "old")
        links.handle(AppLink.Invite(invite)).join()
        links.load()
        assertEquals(invite, links.invite.value)
    }

    @Test
    fun pastedTextIsAnInviteOnlyIfTheCoreSaysSo() = runTest {
        assertFalse(links.offer("just some words"))
        assertFalse(links.offer("https://crumb.example.com/recipes/1"))
        assertNull(links.invite.value)
        assertTrue(links.offer("https://crumb.example.com/invite#tok_1"))
        assertEquals(InviteLink("https://crumb.example.com/", "tok_1"), links.invite.value)
    }

    @Test
    fun aBadInviteLinkSaysItIsntOne() = runTest {
        links.handle(AppLink.BadInvite).join()
        assertEquals("That isn't a Crumb invite link", links.notice.value)
        links.clearNotice()
        assertNull(links.notice.value)
    }
}
