package app.crumb.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.ZoneOffset

class AccountLogicTest {
    private fun status(json: String) = CrumbJson.decodeFromString(AuthStatus.serializer(), json)

    @Test
    fun theStatusSaysWhichModeTheServerRuns() {
        assertEquals(AuthMode.Password, status("""{"mode":"password","passwordRequired":true,"signedIn":false}""").authMode)
        assertEquals(AuthMode.Accounts, status("""{"mode":"accounts","signupOpen":true}""").authMode)
        assertEquals(AuthMode.Hosted, status("""{"mode":"hosted","providers":["google"]}""").authMode)
        // Anything unrecognised (an older server) is one password, as on the web
        assertEquals(AuthMode.Password, status("""{"mode":"saml"}""").authMode)
        assertEquals(AuthMode.Password, status("{}").authMode)
    }

    @Test
    fun signedInNeedsAnAccountModeAndAUser() {
        assertFalse(status("""{"mode":"password","signedIn":true}""").isAccountSignedIn)
        assertFalse(status("""{"mode":"accounts"}""").isAccountSignedIn)
        val s = status("""{"mode":"hosted","user":{"name":"Sam","email":"s@x.io"},"household":{"id":4,"name":"Home","role":"owner"}}""")
        assertTrue(s.isAccountSignedIn)
        assertEquals("Home", s.household?.name)
    }

    @Test
    fun eachModeOffersItsOwnForms() {
        assertEquals(listOf(SignInForm.Password), signInForms(status("""{"mode":"password"}""")))
        assertEquals(listOf(SignInForm.SetUp), signInForms(status("""{"mode":"accounts","setupNeeded":true,"signupOpen":true}""")))
        assertEquals(listOf(SignInForm.SignIn), signInForms(status("""{"mode":"accounts"}""")))
        assertEquals(
            listOf(SignInForm.SignIn, SignInForm.SignUp),
            signInForms(status("""{"mode":"accounts","signupOpen":true}""")),
        )
        // Only Better Auth can email a reset link
        assertEquals(
            listOf(SignInForm.SignIn, SignInForm.SignUp, SignInForm.Forgot),
            signInForms(status("""{"mode":"hosted","signupOpen":true}""")),
        )
        assertEquals(listOf(SignInForm.SignIn, SignInForm.Forgot), signInForms(status("""{"mode":"hosted"}""")))
    }

    @Test
    fun accountPartsDependOnTheMode() {
        val password = status("""{"mode":"password","passwordRequired":true}""")
        assertEquals(listOf(AccountPart.ConnectedApps, AccountPart.SignOut), accountParts(password))
        // No password set: nothing to sign out of
        assertEquals(listOf(AccountPart.ConnectedApps), accountParts(status("""{"mode":"password"}""")))
        val accounts = status("""{"mode":"accounts","user":{"name":"Sam","email":"s@x.io"}}""")
        assertEquals(
            listOf(
                AccountPart.Account, AccountPart.SignInMethods, AccountPart.Household,
                AccountPart.ConnectedApps, AccountPart.Data, AccountPart.SignOut,
            ),
            accountParts(accounts),
        )
        assertEquals(accountParts(accounts), accountParts(status("""{"mode":"hosted","user":{"name":"Sam","email":"s@x.io"}}""")))
    }

    @Test
    fun theMoreRowNamesTheAccountOrWhatItHolds() {
        val signedIn = status("""{"mode":"accounts","user":{"name":"Sam","email":"s@x.io"},"household":{"name":"Robertsons","role":"owner"}}""")
        assertEquals("Sam" to "Robertsons, sign-in and connected apps", accountRow(signedIn, true))
        assertEquals("Account" to "Connected apps and signing out", accountRow(status("""{"mode":"password"}"""), true))
        assertEquals("Account" to "Connected apps", accountRow(null, false))
    }

    @Test
    fun theSessionCookieIsWhicheverTheModeUses() {
        assertEquals("123.abc", SessionCookies.fromSetCookie(listOf("crumb_session=123.abc; Path=/; HttpOnly")))
        assertEquals(
            "crumb.session_token=tok.sig%3D",
            SessionCookies.fromSetCookie(listOf("crumb.session_data=x; Path=/", "crumb.session_token=tok.sig%3D; Path=/; HttpOnly")),
        )
        assertEquals(
            "__Secure-crumb.session_token=tok.sig",
            SessionCookies.fromSetCookie(listOf("__Secure-crumb.session_token=tok.sig; Secure; HttpOnly")),
        )
        // A cleared cookie, or someone else's, isn't a session
        assertNull(SessionCookies.fromSetCookie(listOf("crumb_session=; Max-Age=0", "crumb.session_token=; Max-Age=0", "other=1")))
        assertNull(SessionCookies.fromSetCookie(listOf("crumb_sessionx=1", "xcrumb.session_token=2")))
    }

    @Test
    fun savedCookiesBecomeTheRightRequestHeader() {
        assertEquals("crumb_session=123.abc", SessionCookies.header("123.abc"))
        // A bare value may itself contain "="
        assertEquals("crumb_session=a=b", SessionCookies.header("a=b"))
        assertEquals("crumb.session_token=t.s", SessionCookies.header("crumb.session_token=t.s"))
        assertEquals("__Secure-crumb.session_token=t", SessionCookies.header("__Secure-crumb.session_token=t"))
        assertTrue(SessionCookies.isHosted("__Secure-crumb.session_token=t"))
        assertFalse(SessionCookies.isHosted("123.abc"))
        assertFalse(SessionCookies.isHosted(null))
    }

    @Test
    fun hostedRequestsNameTheirOrigin() {
        assertEquals("https://crumb.example.com", okhttp3.HttpUrl.Builder().scheme("https").host("crumb.example.com").build().origin())
        assertEquals("http://192.168.1.5:3000", okhttp3.HttpUrl.Builder().scheme("http").host("192.168.1.5").port(3000).build().origin())
    }

    @Test
    fun devicesAreNamedAsTheWebDoes() {
        assertEquals("Unknown device", deviceName(null))
        assertEquals("Firefox on Linux", deviceName("Mozilla/5.0 (X11; Linux x86_64; rv:1) Gecko/20100101 Firefox/130.0"))
        assertEquals("Edge on Windows", deviceName("Mozilla/5.0 (Windows NT 10.0) Chrome/120 Safari/537 Edg/120"))
        assertEquals("Safari on iOS", deviceName("Mozilla/5.0 (iPhone; CPU iPhone OS 17 like Mac OS X) Safari/604"))
        assertEquals("Browser", deviceName("curl/8.0"))
        // This app's own user agent
        assertEquals("Crumb app on Android", deviceName(appUserAgent("1.2.3", "14", "Pixel 8 (test)")))
        assertEquals("Crumb/1.2.3 (Android 14; Pixel 8 test)", appUserAgent("1.2.3", "14", "Pixel 8 (test)"))
    }

    @Test
    fun emailsAreChecked() {
        assertTrue(AccountForms.looksLikeEmail(" sam@example.com "))
        assertFalse(AccountForms.looksLikeEmail("sam@example"))
        assertFalse(AccountForms.looksLikeEmail("sam@@example.com"))
        assertFalse(AccountForms.looksLikeEmail("@example.com"))
        assertFalse(AccountForms.looksLikeEmail("s am@example.com"))
    }

    @Test
    fun formsAreCheckedBeforeAskingTheServer() {
        assertNull(AccountForms.signIn("sam@example.com", "x"))
        assertEquals("Enter your password.", AccountForms.signIn("sam@example.com", ""))
        assertNull(AccountForms.signUp("Sam", "sam@example.com", "12345678"))
        assertEquals("Enter your name.", AccountForms.signUp(" ", "sam@example.com", "12345678"))
        assertEquals("Use at least 8 characters for the password.", AccountForms.signUp("Sam", "sam@example.com", "1234567"))
        assertEquals(
            "Enter the app password this box used before accounts.",
            AccountForms.setUp("Sam", "sam@example.com", "12345678", true, ""),
        )
        assertNull(AccountForms.setUp("Sam", "sam@example.com", "12345678", false, ""))
        assertNull(AccountForms.forgot("sam@example.com"))
        assertEquals("Enter the email you signed up with.", AccountForms.forgot("nope"))
    }

    @Test
    fun changingEmailAsksTwiceAndForThePasswordIfThereIsOne() {
        assertEquals("The two addresses don't match", AccountForms.changeEmail("a@x.io", "b@x.io", "pw", true))
        assertNull(AccountForms.changeEmail("A@x.io", "a@X.io", "pw", true))
        assertEquals("Enter your password.", AccountForms.changeEmail("a@x.io", "a@x.io", "", true))
        assertNull(AccountForms.changeEmail("a@x.io", "a@x.io", "", false))
    }

    @Test
    fun deletingAsksForThePasswordOrTheTypedEmail() {
        assertEquals("Enter your password.", AccountForms.deleteAccount(true, "", "", "a@x.io"))
        assertNull(AccountForms.deleteAccount(true, "pw", "", "a@x.io"))
        assertEquals("Type a@x.io to confirm.", AccountForms.deleteAccount(false, "", "b@x.io", "a@x.io"))
        assertNull(AccountForms.deleteAccount(false, "", " A@x.io ", "a@x.io"))
    }

    @Test
    fun unlinkingTheLastWayInIsNotOffered() {
        val google = LinkedProvider(Provider.Google, null, 0)
        assertFalse(SignInMethods(password = false, linked = listOf(google)).canUnlink)
        assertTrue(SignInMethods(password = true, linked = listOf(google)).canUnlink)
    }

    @Test
    fun daysReadLikeTheWebs() {
        assertEquals("Sep 28, 2026", dayLabel(1_790_553_600, ZoneOffset.UTC))
        assertEquals("Jan 1, 1970", dayLabel(0, ZoneOffset.UTC))
    }
}
