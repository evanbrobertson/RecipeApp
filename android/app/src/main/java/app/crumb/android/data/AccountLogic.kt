package app.crumb.android.data

import kotlinx.serialization.Serializable
import okhttp3.HttpUrl

// Sign-in modes and the pure logic around them (web/src/lib/account.ts, crumb-client's
// accounts.rs): what the server's auth status says, which cookie a mode uses, how forms are
// checked. Nothing here touches the network or Android.

/** How this Crumb signs people in (`AUTH_MODE`). */
enum class AuthMode {
    /** One app password. */
    Password,

    /** Accounts the Rust server keeps. */
    Accounts,

    /** Better Auth, in the hosted edition. */
    Hosted,
    ;

    companion object {
        /** Anything unknown reads as one password, as the web does. */
        fun parse(raw: String?): AuthMode = when (raw) {
            "accounts" -> Accounts
            "hosted" -> Hosted
            else -> Password
        }
    }
}

@Serializable
data class StatusUser(val name: String = "", val email: String = "")

@Serializable
data class StatusHousehold(val name: String = "", val role: String = "")

/** `GET /api/auth/status`: how this Crumb signs people in, and who's signed in. */
@Serializable
data class AuthStatus(
    val mode: String = "password",
    val signedIn: Boolean = false,
    /** Accounts: nobody has an account yet, so the first one is made by the setup form. */
    val setupNeeded: Boolean = false,
    /** The setup asks for the old app password too. */
    val setupNeedsAppPassword: Boolean = false,
    val signupOpen: Boolean = false,
    /** One password: whether it's set (so there's a sign out). */
    val passwordRequired: Boolean = false,
    /** Google and Apple, when the server has their keys. */
    val providers: List<String> = emptyList(),
    val user: StatusUser? = null,
    val household: StatusHousehold? = null,
) {
    val authMode: AuthMode get() = AuthMode.parse(mode)
    val hasAccounts: Boolean get() = authMode != AuthMode.Password

    /** An account is signed in (never true with one password). */
    val isAccountSignedIn: Boolean get() = hasAccounts && user != null
}

/** Which form the sign-in screen shows. */
enum class SignInForm { Password, SignIn, SignUp, SetUp, Forgot }

/** The forms a server in [status]'s mode offers to someone signed out; the first is the default. */
fun signInForms(status: AuthStatus): List<SignInForm> = when {
    !status.hasAccounts -> listOf(SignInForm.Password)
    status.setupNeeded -> listOf(SignInForm.SetUp)
    else -> buildList {
        add(SignInForm.SignIn)
        if (status.signupOpen) add(SignInForm.SignUp)
        // Only Better Auth can email a reset link
        if (status.authMode == AuthMode.Hosted) add(SignInForm.Forgot)
    }
}

/** The pieces of the Account page, in the order the web shows them. */
enum class AccountPart { Account, SignInMethods, Household, ConnectedApps, Data, SignOut }

/**
 * What More → Account shows for [status] (web AccountPage.svelte): with one password only
 * the connected apps and, if there is a password, signing out; with accounts, everything.
 */
fun accountParts(status: AuthStatus): List<AccountPart> = buildList {
    if (status.isAccountSignedIn) {
        add(AccountPart.Account)
        add(AccountPart.SignInMethods)
        add(AccountPart.Household)
    }
    add(AccountPart.ConnectedApps)
    if (status.isAccountSignedIn) add(AccountPart.Data)
    if (status.isAccountSignedIn || status.passwordRequired) add(AccountPart.SignOut)
}

/** The account row on More: "{name}" and where it leads (web MoreSettings.svelte). */
fun accountRow(status: AuthStatus?, authEnabled: Boolean): Pair<String, String> = when {
    status != null && status.isAccountSignedIn ->
        (status.user?.name.orEmpty().ifBlank { "Account" }) to
            "${status.household?.name?.ifBlank { null } ?: "Household"}, sign-in and connected apps"
    authEnabled -> "Account" to "Connected apps and signing out"
    else -> "Account" to "Connected apps"
}

/** Someone in a household. */
data class Member(val id: String, val name: String, val email: String, val role: String, val you: Boolean)

/** An invite that hasn't been used yet. Hosted ones go to one address; self-hosted ones are links. */
data class PendingInvite(val id: String, val email: String?, val createdBy: String?, val expiresAt: Long)

data class HouseholdRef(val id: String, val name: String)

/** The signed-in household: who's in it and what's waiting. */
data class Household(
    val id: String,
    val name: String,
    val role: String,
    val members: List<Member>,
    val invites: List<PendingInvite>,
    /** Every household the person is in, this one included. */
    val households: List<HouseholdRef>,
) {
    val isOwner: Boolean get() = role == "owner"
    val others: List<HouseholdRef> get() = households.filter { it.id != id }
}

/** A signed-in browser or app. */
data class Device(val id: String, val userAgent: String?, val lastSeenAt: Long, val current: Boolean)

enum class Provider(val id: String, val label: String) {
    Google("google", "Google"),
    Apple("apple", "Apple"),
    ;

    companion object {
        fun of(id: String?): Provider? = entries.firstOrNull { it.id == id }
    }
}

data class LinkedProvider(val provider: Provider, val email: String?, val createdAt: Long)

/** How the account signs in. */
data class SignInMethods(val password: Boolean, val linked: List<LinkedProvider>) {
    /** Unlinking the last way in isn't allowed. */
    val ways: Int get() = (if (password) 1 else 0) + linked.size
    val canUnlink: Boolean get() = ways > 1
}

@Serializable
data class ConnectedHousehold(val id: Long = 0, val name: String? = null)

/** An app connected to Crumb (Claude, mostly): one per app and household. */
@Serializable
data class ConnectedApp(
    val id: String,
    val name: String,
    val household: ConnectedHousehold? = null,
    val connectedAt: Long = 0,
)

/** `POST /api/account/email`: the new address, and whether it must be confirmed first. */
@Serializable
data class EmailChange(val email: String, val pending: Boolean = false)

/** A made account: hosted ones may have to confirm the email before signing in. */
data class SignedUp(val verify: Boolean, val cookie: String?)

/** Session cookies: `crumb_session` (password and accounts) and Better Auth's (hosted). */
object SessionCookies {
    const val CREDENTIAL = "crumb_session"
    private const val HOSTED = "crumb.session_token"
    private const val SECURE = "__Secure-"

    private fun isHostedName(name: String) = name == HOSTED || (name.startsWith(SECURE) && name.removePrefix(SECURE) == HOSTED)

    /**
     * What to save from a response's `Set-Cookie` headers: the bare `crumb_session` value, or
     * `name=value` for the hosted edition's cookie (its name differs over HTTPS). Cleared
     * cookies (empty values) don't count. Saved the same way crumb-client keeps them.
     */
    // TODO(core): client::session_cookie for the hosted cookie (crumb-client keeps it private)
    fun fromSetCookie(headers: List<String>): String? = headers.firstNotNullOfOrNull { header ->
        val pair = header.substringBefore(';').trim()
        val name = pair.substringBefore('=', "")
        val value = pair.substringAfter('=', "")
        when {
            value.isEmpty() -> null
            name == CREDENTIAL -> value
            isHostedName(name) -> "$name=$value"
            else -> null
        }
    }

    /** A saved cookie as the `Cookie` request header. */
    fun header(saved: String): String {
        val name = saved.substringBefore('=')
        return if (isHostedName(name)) saved else "$CREDENTIAL=$saved"
    }

    /** Whether [saved] is Better Auth's cookie, so requests need the hosted edition's paths. */
    fun isHosted(saved: String?): Boolean = saved != null && isHostedName(saved.substringBefore('='))
}

/** `scheme://host[:port]`, the `Origin` Better Auth wants on a cookie's request. */
fun HttpUrl.origin(): String = buildString {
    append(scheme).append("://").append(host)
    if (port != HttpUrl.defaultPort(scheme)) append(':').append(port)
}

/** A session's user agent as "Firefox on Linux", worded by the shared core. */
fun deviceName(agent: String?): String = app.crumb.core.deviceName(agent?.takeIf { it.isNotEmpty() })

/** The user agent the app announces, so the server's device list can name this phone. */
fun appUserAgent(version: String, release: String, model: String): String =
    "Crumb/$version (Android $release; ${model.replace(Regex("[^A-Za-z0-9 ._-]"), "")})"

/** Form checks, worded as the web's are (LoginForm.svelte, AccountPage.svelte). */
object AccountForms {
    const val MIN_PASSWORD = 8

    fun looksLikeEmail(raw: String): Boolean {
        val email = raw.trim()
        val at = email.indexOf('@')
        return at > 0 && at < email.length - 1 && email.indexOf('@', at + 1) < 0 &&
            email.substringAfter('@').contains('.') && !email.any { it.isWhitespace() }
    }

    fun signIn(email: String, password: String): String? = when {
        !looksLikeEmail(email) -> "Enter the email you signed up with."
        password.isEmpty() -> "Enter your password."
        else -> null
    }

    fun signUp(name: String, email: String, password: String): String? = when {
        name.isBlank() -> "Enter your name."
        !looksLikeEmail(email) -> "That doesn't look like an email address."
        password.length < MIN_PASSWORD -> "Use at least $MIN_PASSWORD characters for the password."
        else -> null
    }

    fun setUp(name: String, email: String, password: String, needsAppPassword: Boolean, appPassword: String): String? =
        signUp(name, email, password)
            ?: if (needsAppPassword && appPassword.isEmpty()) "Enter the app password this box used before accounts." else null

    fun forgot(email: String): String? = if (looksLikeEmail(email)) null else "Enter the email you signed up with."

    /** Typed twice, because a typo here is the very thing this is for. */
    fun changeEmail(email: String, again: String, password: String, needsPassword: Boolean): String? = when {
        !looksLikeEmail(email) -> "That doesn't look like an email address."
        !email.trim().equals(again.trim(), ignoreCase = true) -> "The two addresses don't match"
        needsPassword && password.isEmpty() -> "Enter your password."
        else -> null
    }

    /** Deleting asks for the password, or with none, the email typed out. */
    fun deleteAccount(hasPassword: Boolean, password: String, typed: String, email: String): String? = when {
        hasPassword && password.isEmpty() -> "Enter your password."
        !hasPassword && !typed.trim().equals(email, ignoreCase = true) -> "Type $email to confirm."
        else -> null
    }
}

/** "Mar 11, 2025", as the web's `toLocaleDateString` prints it for devices, apps and invites. */
fun dayLabel(seconds: Long, zone: java.time.ZoneId = java.time.ZoneId.systemDefault()): String {
    val ms = seconds * 1000
    val offset = zone.rules.getOffset(java.time.Instant.ofEpochMilli(ms)).totalSeconds / 60
    return app.crumb.core.dateLabel(ms, offset)
}
