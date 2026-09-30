package app.crumb.android.data

import app.crumb.core.InviteLink
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull

// Links into the app (crumb-core's app_link.rs): the browser handing back a Google or Apple
// sign-in, and invites. Parsing is the shared core's; what's here is what the app keeps between
// the browser and the phone, and which screen each situation leads to.

/** What the browser or a pasted link asks the app to do. */
sealed interface AppLink {
    /** `app.crumb://signed-in?code=…`; [code] is null when the link carries none the core accepts. */
    data class SignedIn(val code: String?) : AppLink

    /** An invite, from `https://host/invite#token` or `app.crumb://invite?server=…&token=…`. */
    data class Invite(val invite: InviteLink) : AppLink

    /** An `app.crumb://invite` link the core doesn't accept. */
    data object BadInvite : AppLink

    companion object {
        /** The link a VIEW intent carries, or null for anything that isn't Crumb's. */
        fun parse(link: String?): AppLink? {
            val text = link?.trim().orEmpty()
            val host = text.substringAfter("${app.crumb.core.appScheme()}://", "").substringBefore('?').substringBefore('/')
            return when (host) {
                "signed-in" -> SignedIn(app.crumb.core.signedInCode(text))
                "invite" -> app.crumb.core.inviteLink(text)?.let(::Invite) ?: BadInvite
                else -> null
            }
        }
    }
}

/** A Google or Apple sign-in that has been sent to the browser: where to open, and the secret to redeem it with. */
data class AppSignInStart(val url: String, val verifier: String)

/** What the browser sign-in kept while the browser is up: the server it began at, and the PKCE verifier. */
data class PendingSignIn(val server: HttpUrl, val verifier: String) {
    /** One line for the encrypted store; the verifier is URL-safe, so it can't contain a newline. */
    fun encode(): String = "$server\n$verifier"

    companion object {
        fun decode(text: String): PendingSignIn? {
            val (server, verifier) = text.split('\n', limit = 2).takeIf { it.size == 2 } ?: return null
            val url = server.toHttpUrlOrNull() ?: return null
            return if (verifier.isEmpty()) null else PendingSignIn(url, verifier)
        }
    }
}

/** What an invite is for: "{invitedBy} invited you to {household}". */
data class InvitePreview(val household: String, val invitedBy: String?, val email: String? = null) {
    val headline: String
        get() = if (invitedBy.isNullOrBlank()) "You're invited to $household" else "$invitedBy invited you to $household"
}

/** Words for the invite and browser sign-in flows. */
object InviteText {
    const val NOT_AN_INVITE = "That isn't a Crumb invite link"
    const val GONE = "This invite has expired or was already used. Ask for a new one."
    const val SIGN_IN_EXPIRED = "That sign-in expired. Try again."
    const val OFFLINE = "Couldn't reach your Crumb server. Check your connection."

    /** "Sign out of {current} to join?" when the invite is for another server. */
    fun otherServer(invite: String, current: String) = "This invite is for $invite. Sign out of $current to join?"

    /** What to tell someone when redeeming a browser sign-in's code failed. */
    fun redeemFailure(e: Exception): String = when {
        e is OfflineException -> OFFLINE
        e is ApiException && (e.status == 410 || e.status == 404) -> SIGN_IN_EXPIRED
        else -> e.message ?: SIGN_IN_EXPIRED
    }
}

/** `host` or `host:port`, how the app names a server to people. */
fun HttpUrl.label(): String = if (port == HttpUrl.defaultPort(scheme)) host else "$host:$port"

fun sameServer(a: HttpUrl, b: HttpUrl): Boolean = a.scheme == b.scheme && a.host == b.host && a.port == b.port

/** Which screen an invite leads to, given who's signed in. */
sealed interface InviteStep {
    /** Nothing is waiting. */
    data object None : InviteStep

    /** Signed out: sign in (or up) at the invite's server first; the invite stays waiting. */
    data class SignInFirst(val server: HttpUrl) : InviteStep

    /** Signed in to the invite's server: the join dialog. */
    data class Join(val invite: InviteLink) : InviteStep

    /** Signed in somewhere else: sign out of it first. */
    data class OtherServer(val invite: String, val current: String) : InviteStep
}

fun inviteStep(session: Session?, invite: InviteLink?): InviteStep {
    val server = invite?.server?.toHttpUrlOrNull() ?: return InviteStep.None
    return when {
        session == null -> InviteStep.SignInFirst(server)
        sameServer(session.server, server) -> InviteStep.Join(invite)
        else -> InviteStep.OtherServer(server.label(), session.server.label())
    }
}

/** What [AppLinks] keeps across the browser (and the process being killed while it's up). */
interface PendingStore {
    suspend fun signIn(server: HttpUrl, cookie: String?)
    suspend fun savePendingSignIn(pending: PendingSignIn?)
    suspend fun loadPendingSignIn(): PendingSignIn?
    suspend fun savePendingInvite(invite: InviteLink?)
    suspend fun loadPendingInvite(): InviteLink?
}

/**
 * The browser's ways back into the app. A Google or Apple sign-in is started with
 * [beginSignIn] (which keeps the verifier) and finished by [handle] when the browser ends at
 * `app.crumb://signed-in`. An invite waits in [invite] until it's joined or dismissed, however
 * many sign-ins it takes to get there; [notice] is what went wrong, for whichever screen is up.
 */
class AppLinks(
    private val store: PendingStore,
    private val accounts: AccountApi,
    private val scope: CoroutineScope,
) {
    private val _invite = MutableStateFlow<InviteLink?>(null)
    val invite: StateFlow<InviteLink?> = _invite.asStateFlow()

    private val _notice = MutableStateFlow<String?>(null)
    val notice: StateFlow<String?> = _notice.asStateFlow()

    /** Brings back an invite that was waiting when the app was last closed. */
    suspend fun load() {
        // A link that came in before this finished is newer than what was saved
        runCatching { store.loadPendingInvite() }.getOrNull()?.let { _invite.compareAndSet(null, it) }
    }

    /** Asks the server to start [provider]'s sign-in; the caller opens the returned url. */
    suspend fun beginSignIn(server: HttpUrl, provider: Provider): String {
        val started = accounts.startAppSignIn(server, provider)
        store.savePendingSignIn(PendingSignIn(server, started.verifier))
        return started.url
    }

    fun handle(link: AppLink): Job = scope.launch {
        when (link) {
            is AppLink.SignedIn -> redeem(link.code)
            is AppLink.Invite -> waitFor(link.invite)
            AppLink.BadInvite -> _notice.value = InviteText.NOT_AN_INVITE
        }
    }

    /** A pasted invite link: whether it was one. */
    fun offer(text: String): Boolean {
        val found = app.crumb.core.inviteLink(text) ?: return false
        scope.launch { waitFor(found) }
        return true
    }

    fun dismissInvite() {
        _invite.value = null
        scope.launch { store.savePendingInvite(null) }
    }

    fun clearNotice() {
        _notice.value = null
    }

    fun notify(text: String) {
        _notice.value = text
    }

    private suspend fun waitFor(invite: InviteLink) {
        _invite.value = invite
        store.savePendingInvite(invite)
    }

    private suspend fun redeem(code: String?) {
        // Kept once: a second link, or a replayed one, finds nothing and says so
        val pending = store.loadPendingSignIn()
        store.savePendingSignIn(null)
        if (code == null || pending == null) {
            _notice.value = InviteText.SIGN_IN_EXPIRED
            return
        }
        try {
            val cookie = accounts.finishAppSignIn(pending.server, code, pending.verifier)
            store.signIn(pending.server, cookie)
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            _notice.value = InviteText.redeemFailure(e)
        }
    }
}
