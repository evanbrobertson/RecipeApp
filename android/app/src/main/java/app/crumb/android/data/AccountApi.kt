package app.crumb.android.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.put
import okhttp3.HttpUrl
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import java.net.URLEncoder

/**
 * Accounts and households, whichever way this Crumb signs people in: its own accounts
 * (`AUTH_MODE=accounts`, `api/auth/...` answered by the Rust server) or the hosted edition's
 * Better Auth (the same paths, proxied). A port of crumb-client's `accounts.rs` and the web's
 * `lib/account.ts`; Google, Apple and passkeys need a browser, so signing in with them isn't
 * here (linked ones can be listed and unlinked). Ids are strings in either mode.
 *
 * A 401 from these is often "wrong password", not a lapsed session: only the screens that
 * load things treat it as being signed out.
 */
class AccountApi(private val api: CrumbApi) {
    // ─── Before signing in: [server] is what was typed, there's no session yet ───

    /**
     * `GET /api/auth/status`: how this server signs people in. Any error answer reads as one
     * password, as the web does; only an unreachable server throws.
     */
    suspend fun status(server: HttpUrl? = null): AuthStatus = try {
        val request = Request.Builder().url(server?.resolve("api/auth/status") ?: api.url("api/auth/status")).build()
        api.decode(api.send(request, withSession = server == null), AuthStatus.serializer())
    } catch (e: ApiException) {
        AuthStatus()
    } catch (e: kotlinx.serialization.SerializationException) {
        AuthStatus()
    }

    /** Email and password sign-in; returns the session cookie to keep. A wrong one is a 401. */
    suspend fun signIn(server: HttpUrl, mode: AuthMode, email: String, password: String): String {
        val path = if (mode == AuthMode.Hosted) "api/auth/sign-in/email" else "api/auth/login"
        val body = buildJsonObject {
            put("email", email.trim())
            put("password", password)
        }
        return cookieOf(open(server, path, mode, body))
    }

    /** Accounts: makes the first account, which owns the recipe box already here. */
    suspend fun setUp(server: HttpUrl, name: String, email: String, password: String, appPassword: String?): String {
        val body = buildJsonObject {
            put("name", name.trim())
            put("email", email.trim())
            put("password", password)
            if (!appPassword.isNullOrEmpty()) put("appPassword", appPassword)
        }
        return cookieOf(open(server, "api/auth/setup", AuthMode.Accounts, body))
    }

    /**
     * Makes an account, when sign-up is open. Hosted ones may have to confirm the email
     * first ([SignedUp.verify]); otherwise the session cookie is ready.
     */
    suspend fun signUp(server: HttpUrl, mode: AuthMode, name: String, email: String, password: String): SignedUp {
        val body = buildJsonObject {
            put("name", name.trim())
            put("email", email.trim())
            put("password", password)
            if (mode == AuthMode.Hosted) put("callbackURL", "/")
        }
        val response = open(server, if (mode == AuthMode.Hosted) "api/auth/sign-up/email" else "api/auth/signup", mode, body)
        val cookie = response.use { res ->
            val text = res.body.string()
            val token = runCatching { CrumbJson.parseToJsonElement(text).jsonObject["token"] }.getOrNull()
            val cookie = SessionCookies.fromSetCookie(res.headers("Set-Cookie"))
            if (mode == AuthMode.Hosted && (token == null || token is JsonNull)) return SignedUp(verify = true, cookie = null)
            cookie
        }
        return SignedUp(verify = false, cookie = cookie ?: signIn(server, mode, email, password))
    }

    /** Hosted: emails a link to choose a new password. */
    suspend fun requestReset(server: HttpUrl, email: String) {
        val body = buildJsonObject {
            put("email", email.trim())
            put("redirectTo", "/reset-password")
        }
        open(server, "api/auth/request-password-reset", AuthMode.Hosted, body).close()
    }

    private suspend fun open(server: HttpUrl, path: String, mode: AuthMode, body: JsonObject): Response {
        val request = Request.Builder()
            .url(server.resolve(path)!!)
            .post(body.toString().toRequestBody(JSON))
            .apply { if (mode == AuthMode.Hosted) header("Origin", server.origin()) }
            .build()
        return api.send(request, withSession = false)
    }

    private fun cookieOf(response: Response): String = response.use { res ->
        SessionCookies.fromSetCookie(res.headers("Set-Cookie"))
            ?: throw ApiException(res.code, "The server didn't start a session. Try again.")
    }

    // ─── Signed in ───

    private fun hosted(mode: AuthMode) = mode == AuthMode.Hosted

    private fun get(path: String): Request = Request.Builder().url(api.url(path)).build()

    private suspend fun json(request: Request): JsonElement = withContext(Dispatchers.Default) {
        api.send(request).use { CrumbJson.parseToJsonElement(it.body.string().ifBlank { "null" }) }
    }

    private suspend fun call(request: Request) {
        api.send(request).close()
    }

    private fun post(path: String, body: JsonObject = buildJsonObject {}): Request =
        Request.Builder().url(api.url(path)).post(body.toString().toRequestBody(JSON)).build()

    private fun patch(path: String, body: JsonObject): Request =
        Request.Builder().url(api.url(path)).patch(body.toString().toRequestBody(JSON)).build()

    private fun delete(path: String): Request = Request.Builder().url(api.url(path)).delete().build()

    private fun segment(id: String) = URLEncoder.encode(id, "UTF-8").replace("+", "%20")

    suspend fun signOut(mode: AuthMode) {
        call(if (hosted(mode)) post("api/auth/sign-out") else post("api/auth/logout"))
    }

    /** Hosted: `get-session` as (session id, user id), or null when signed out. */
    private suspend fun currentSession(): Pair<String?, String?>? {
        val root = json(get("api/auth/get-session")) as? JsonObject ?: return null
        val session = (root["session"] as? JsonObject)?.get("id")?.text()
        val user = (root["user"] as? JsonObject)?.get("id")?.text()
        return session to user
    }

    private fun JsonElement.text(): String? = (this as? JsonPrimitive)?.contentOrNull

    suspend fun devices(mode: AuthMode): List<Device> =
        if (!hosted(mode)) {
            AccountParse.accountsDevices(json(get("api/auth/sessions")))
        } else {
            val rows = json(get("api/auth/list-sessions"))
            AccountParse.hostedDevices(rows, currentSession()?.first)
        }

    suspend fun signOutDevice(mode: AuthMode, id: String) {
        if (hosted(mode)) call(post("api/auth/revoke-session", buildJsonObject { put("token", id) }))
        else call(delete("api/auth/sessions/${segment(id)}"))
    }

    suspend fun signOutOthers(mode: AuthMode) {
        if (hosted(mode)) call(post("api/auth/revoke-other-sessions")) else call(post("api/auth/sessions/revoke-others"))
    }

    suspend fun household(mode: AuthMode): Household {
        if (!hosted(mode)) return AccountParse.accountsHousehold(json(get("api/auth/household")))
        val org = json(get("api/auth/organization/get-full-organization"))
        val orgs = json(get("api/auth/organization/list"))
        val me = currentSession()?.second
        return AccountParse.hostedHousehold(org, orgs, me, System.currentTimeMillis() / 1000)
    }

    suspend fun rename(mode: AuthMode, name: String) {
        if (hosted(mode)) {
            call(post("api/auth/organization/update", buildJsonObject { put("data", buildJsonObject { put("name", name) }) }))
        } else {
            call(patch("api/auth/household", buildJsonObject { put("name", name) }))
        }
    }

    /**
     * A new invite: a link to hand over (hosted also emails it to [email]; self-hosted ones
     * are links anyone can use once).
     */
    suspend fun invite(mode: AuthMode, email: String?): String {
        if (!hosted(mode)) {
            val made = json(post("api/auth/invites")).jsonObject
            return made["url"]?.text().orEmpty()
        }
        val made = json(
            post(
                "api/auth/organization/invite-member",
                buildJsonObject {
                    put("email", email.orEmpty())
                    put("role", "member")
                },
            ),
        ).jsonObject
        val base = api.url("").toString().trimEnd('/')
        return "$base/invite#${made["id"]?.text().orEmpty()}"
    }

    suspend fun cancelInvite(mode: AuthMode, id: String) {
        if (hosted(mode)) call(post("api/auth/organization/cancel-invitation", buildJsonObject { put("invitationId", id) }))
        else call(delete("api/auth/invites/${segment(id)}"))
    }

    suspend fun removeMember(mode: AuthMode, id: String) {
        if (hosted(mode)) call(post("api/auth/organization/remove-member", buildJsonObject { put("memberIdOrEmail", id) }))
        else call(delete("api/auth/members/${segment(id)}"))
    }

    suspend fun leave(mode: AuthMode, id: String) {
        if (hosted(mode)) call(post("api/auth/organization/leave", buildJsonObject { put("organizationId", id) }))
        else call(post("api/auth/household/leave"))
    }

    suspend fun switchTo(mode: AuthMode, id: String) {
        if (hosted(mode)) {
            call(post("api/auth/organization/set-active", buildJsonObject { put("organizationId", id) }))
        } else {
            val number = id.toLongOrNull() ?: throw ApiException(400, "That isn't a household")
            call(post("api/auth/household/switch", buildJsonObject { put("id", number) }))
        }
    }

    suspend fun signInMethods(mode: AuthMode): SignInMethods =
        if (hosted(mode)) AccountParse.hostedMethods(json(get("api/auth/list-accounts")))
        else AccountParse.accountsMethods(json(get("api/auth/identities")))

    suspend fun unlink(mode: AuthMode, provider: Provider) {
        if (!hosted(mode)) return call(delete("api/auth/identities/${provider.id}"))
        val rows = json(get("api/auth/list-accounts")) as? kotlinx.serialization.json.JsonArray ?: return
        val row = rows.map { it.jsonObject }.firstOrNull { it["providerId"]?.text() == provider.id } ?: return
        call(post("api/auth/unlink-account", buildJsonObject { put("accountId", row["id"]?.text().orEmpty()) }))
    }

    /** `GET /api/connections`: the apps connected to this account. */
    suspend fun connectedApps(): List<ConnectedApp> =
        api.decode(api.send(get("api/connections")), ListSerializer(ConnectedApp.serializer()))

    /** `DELETE /api/connections/{id}`: the app has to be approved again to reconnect. */
    suspend fun disconnect(id: String) = call(delete("api/connections/${segment(id)}"))

    /** `POST /api/account/email`: changes the email with the password, or with none, a recent sign-in. */
    suspend fun changeEmail(email: String, password: String): EmailChange {
        val body = buildJsonObject {
            put("email", email.trim())
            put("password", password)
        }
        return api.decode(api.send(post("api/account/email", body)), EmailChange.serializer())
    }

    /** `POST /api/account/delete`: the password, or with none, the email typed out. Signs out. */
    suspend fun deleteAccount(password: String?, confirm: String?) {
        val body = buildJsonObject {
            put("password", password)
            put("confirm", confirm)
        }
        call(post("api/account/delete", body))
    }

    private companion object {
        val JSON = "application/json".toMediaType()
    }
}
