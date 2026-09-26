package app.crumb.android.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import kotlinx.serialization.KSerializer
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import okhttp3.Call
import okhttp3.Callback
import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import java.io.IOException
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/** A request that reached the server and was refused. [message] is written for people. */
class ApiException(val status: Int, override val message: String) : Exception(message) {
    val isSignedOut get() = status == 401
}

/** The request never got an answer: no network, wrong address, server down. */
class OfflineException(cause: Throwable) : IOException(cause.message, cause)

val CrumbJson = Json {
    ignoreUnknownKeys = true
    explicitNulls = false
    coerceInputValues = true
}

/**
 * The Crumb server's REST API (src/api.rs). Signed-in requests carry the `crumb_session`
 * cookie from the current [session]; a 401 means the session expired or the password changed.
 */
class CrumbApi(
    private val client: OkHttpClient,
    private val session: () -> Session?,
) {
    suspend fun health(server: HttpUrl) {
        send(Request.Builder().url(server.resolve("api/health")!!).build(), withSession = false)
            .close()
    }

    /** Signs in and returns the session cookie value (`issued.signature`). */
    suspend fun login(server: HttpUrl, password: String): String? {
        val body = buildJsonObject { put("password", password) }.toString()
        val request = Request.Builder()
            .url(server.resolve("api/auth/login")!!)
            .post(body.toRequestBody(JSON))
            .build()
        return send(request, withSession = false).use { response ->
            response.headers("Set-Cookie")
                .firstNotNullOfOrNull { sessionCookie(it) }
        }
    }

    suspend fun logout() {
        runCatching { post("api/auth/logout", "{}").close() }
    }

    suspend fun recipes(query: String? = null): List<RecipeSummary> {
        val url = url("api/recipes").newBuilder().apply {
            query?.trim()?.takeIf { it.isNotEmpty() }?.let { addQueryParameter("q", it.take(200)) }
        }.build()
        return get(url, ListSerializer(RecipeSummary.serializer()))
    }

    suspend fun recipe(id: Long): Recipe = get(url("api/recipes/$id"), Recipe.serializer())

    suspend fun cookbooks(): List<CookbookListItem> =
        get(url("api/cookbooks"), ListSerializer(CookbookListItem.serializer()))

    suspend fun cookbook(id: Long): Cookbook = get(url("api/cookbooks/$id"), Cookbook.serializer())

    suspend fun createCookbook(name: String, description: String?, color: String): CookbookListItem {
        val body = buildJsonObject {
            put("name", name)
            put("description", description.orEmpty())
            put("color", color)
        }.toString()
        return decode(post("api/cookbooks", body), CookbookListItem.serializer())
    }

    suspend fun importUrl(link: String): ImportResult =
        decode(post("api/recipes/import", buildJsonObject { put("url", link) }.toString()), ImportResult.serializer())

    suspend fun importText(text: String): ImportResult =
        decode(post("api/recipes/import", buildJsonObject { put("text", text) }.toString()), ImportResult.serializer())

    suspend fun markCooked(id: Long) {
        post("api/recipes/$id/cooked", "{}").close()
    }

    /** `/img/{id}/{width}?v={key}`: the server's resized WebP of a recipe photo. */
    fun photoUrl(recipeId: Long, image: String?, width: Int): String? {
        if (image.isNullOrBlank()) return null
        val server = session()?.server ?: return null
        return server.newBuilder()
            .addPathSegments("img/$recipeId/${Photos.snapWidth(width)}")
            .addQueryParameter("v", Photos.imageKey(image))
            .build()
            .toString()
    }

    private fun url(path: String): HttpUrl {
        val server = session()?.server ?: throw ApiException(401, "Not signed in")
        return server.resolve(path)!!
    }

    private suspend fun <T> get(url: HttpUrl, serializer: KSerializer<T>): T =
        decode(send(Request.Builder().url(url).build()), serializer)

    private suspend fun post(path: String, json: String): Response =
        send(Request.Builder().url(url(path)).post(json.toRequestBody(JSON)).build())

    private suspend fun <T> decode(response: Response, serializer: KSerializer<T>): T =
        withContext(Dispatchers.Default) {
            response.use { CrumbJson.decodeFromString(serializer, it.body.string()) }
        }

    private suspend fun send(request: Request, withSession: Boolean = true): Response {
        val authed = if (withSession) {
            val cookie = session()?.cookie
            request.newBuilder().apply {
                if (cookie != null) header("Cookie", "$COOKIE=$cookie")
            }.build()
        } else request
        val response = try {
            client.newCall(authed).await()
        } catch (e: IOException) {
            throw OfflineException(e)
        }
        if (!response.isSuccessful) {
            val text = response.use { runCatching { it.body.string() }.getOrDefault("") }
            val parsed = runCatching { CrumbJson.decodeFromString(ApiErrorBody.serializer(), text) }.getOrNull()
            throw ApiException(response.code, errorMessage(response.code, parsed))
        }
        return response
    }

    companion object {
        const val COOKIE = "crumb_session"
        private val JSON = "application/json".toMediaType()

        /** The value of a `crumb_session=…` Set-Cookie header, unless it clears the cookie. */
        fun sessionCookie(header: String): String? {
            val pair = header.substringBefore(';').trim()
            if (!pair.startsWith("$COOKIE=")) return null
            return pair.removePrefix("$COOKIE=").takeIf { it.isNotEmpty() }
        }

        /** Server messages are field-prefixed ("password: Password is required"). */
        fun errorMessage(status: Int, body: ApiErrorBody?): String {
            val raw = body?.message ?: body?.statusMessage
            val cleaned = raw?.substringAfter(": ", raw)?.takeIf { it.isNotBlank() }
            return cleaned ?: when (status) {
                401 -> "Please sign in again."
                404 -> "That isn't in your recipe box any more."
                in 500..599 -> "Your Crumb server had a problem. Try again in a moment."
                else -> "Something went wrong ($status)."
            }
        }

        /**
         * What someone typed as their server ("crumb.example.com", "http://192.168.1.5:3000")
         * as a base URL ending in "/", or null if it isn't a web address. Defaults to HTTPS.
         */
        fun serverUrl(input: String): HttpUrl? {
            val trimmed = input.trim().trimEnd('/')
            if (trimmed.isEmpty() || trimmed.any { it.isWhitespace() }) return null
            val withScheme = if ("://" in trimmed) trimmed else "https://$trimmed"
            val url = withScheme.toHttpUrlOrNull() ?: return null
            if (!url.host.contains('.') && url.host != "localhost") return null
            val path = url.encodedPath.trimEnd('/') + "/"
            return url.newBuilder().encodedPath(path).query(null).fragment(null).build()
        }
    }
}

private suspend fun Call.await(): Response = suspendCancellableCoroutine { cont ->
    enqueue(object : Callback {
        override fun onResponse(call: Call, response: Response) = cont.resume(response) { _, r, _ -> r.close() }
        override fun onFailure(call: Call, e: IOException) {
            if (!cont.isCancelled) cont.resumeWithException(e)
        }
    })
    cont.invokeOnCancellation { runCatching { cancel() } }
}
