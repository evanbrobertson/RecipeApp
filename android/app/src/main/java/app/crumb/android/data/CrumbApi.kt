package app.crumb.android.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.nullable
import kotlinx.serialization.builtins.serializer
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.add
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import okhttp3.Call
import okhttp3.Callback
import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.MultipartBody
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import java.io.IOException
import java.net.URLDecoder
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
 * For recipe create/update: the server reads a missing field as "leave it" and null as
 * "clear it", so the editor's empty fields must go as explicit nulls (never "", which the
 * server keeps, or rejects for links).
 */
private val FieldsJson = Json(CrumbJson) {
    explicitNulls = true
    encodeDefaults = true
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

    suspend fun recipes(query: String? = null, limit: Int? = null): List<RecipeSummary> {
        val url = url("api/recipes").newBuilder().apply {
            query?.trim()?.takeIf { it.isNotEmpty() }?.let { addQueryParameter("q", it.take(200)) }
            limit?.let { addQueryParameter("limit", it.toString()) }
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

    /** `POST /api/recipes/import/photos`: 1-6 page photos read by Wee Chef. */
    suspend fun importPhotos(photos: List<ByteArray>, hint: String? = null): ImportResult {
        val body = multipart {
            photos.forEach { addFormDataPart("photo", "photo.jpg", it.toRequestBody(JPEG)) }
            hint?.let { addFormDataPart("text", it) }
        }
        return decode(
            send(Request.Builder().url(url("api/recipes/import/photos")).post(body).build()),
            ImportResult.serializer(),
        )
    }

    /** `POST /api/import/files`: one `file` part per file (Paprika, JSON, HTML, PDF, text, zip). */
    suspend fun importFiles(files: List<UploadFile>): List<ImportSummary> {
        val body = multipart {
            files.forEach { addFormDataPart("file", it.name, it.bytes.toRequestBody(mediaType(it.mimeType))) }
        }
        return decode(
            send(Request.Builder().url(url("api/import/files")).post(body).build()),
            ListSerializer(ImportSummary.serializer()),
        )
    }

    /** `POST /api/recipes`: save the editor's fields. */
    suspend fun createRecipe(fields: RecipeFields): Recipe =
        decode(post("api/recipes", FieldsJson.encodeToString(RecipeFields.serializer(), fields)), Recipe.serializer())

    /** `PATCH /api/recipes/{id}`: save the editor's fields over an existing recipe. */
    suspend fun updateRecipe(id: Long, fields: RecipeFields): Recipe =
        decode(patch("api/recipes/$id", FieldsJson.encodeToString(RecipeFields.serializer(), fields)), Recipe.serializer())

    /** `DELETE /api/recipes/{id}`. */
    suspend fun deleteRecipe(id: Long) {
        delete("api/recipes/$id").close()
    }

    /** `POST /api/recipes/bulk-delete`: the number of recipes actually deleted. */
    suspend fun deleteRecipes(ids: List<Long>): Int =
        decode(post("api/recipes/bulk-delete", idsBody("ids", ids)), Deleted.serializer()).deleted

    /** `GET /api/recipes/random`: a random recipe, or null when the box is empty (404). */
    suspend fun randomRecipe(exclude: List<Long> = emptyList(), current: Long? = null): RecipeSummary? {
        val url = url("api/recipes/random").newBuilder().apply {
            if (exclude.isNotEmpty()) addQueryParameter("exclude", exclude.joinToString(","))
            current?.let { addQueryParameter("current", it.toString()) }
        }.build()
        return try {
            get(url, RecipeSummary.serializer())
        } catch (e: ApiException) {
            if (e.status == 404) null else throw e
        }
    }

    /** `GET /api/recipes/{id}/cookbooks`: the cookbook ids holding the recipe. */
    suspend fun recipeCookbooks(id: Long): List<Long> =
        get(url("api/recipes/$id/cookbooks"), ListSerializer(Long.serializer()))

    /** `POST /api/recipes/{id}/viewed` (204). */
    suspend fun logViewed(id: Long) {
        post("api/recipes/$id/viewed", "{}").close()
    }

    /** `POST /api/recipes/{id}/cooked`: log a cook; `eventId` is null when deduped. */
    suspend fun markCooked(id: Long): Cooked =
        decode(post("api/recipes/$id/cooked", "{}"), Cooked.serializer())

    /** `GET /api/recipes/{id}/cooked`: how often and when it was last cooked. */
    suspend fun cookStats(id: Long): Cooked = get(url("api/recipes/$id/cooked"), Cooked.serializer())

    /** `DELETE /api/recipes/{id}/cooked?event={eventId}`: undo one logged cook; returns the stats. */
    suspend fun undoCooked(id: Long, eventId: Long): Cooked =
        decode(delete("api/recipes/$id/cooked?event=$eventId"), Cooked.serializer())

    /** `GET /api/recipes/{id}/export?format=json|md`: the recipe as a download. */
    suspend fun exportRecipe(id: Long, format: String): Download =
        download(send(Request.Builder().url(url("api/recipes/$id/export?format=$format")).build()), "recipe.$format")

    /** `GET /api/cookbooks/{id}/export`: the cookbook as a backup-format download. */
    suspend fun exportCookbook(id: Long): Download =
        download(send(Request.Builder().url(url("api/cookbooks/$id/export")).build()), "cookbook.json")

    /** `GET /api/export`: the whole box as a backup-format download. */
    suspend fun exportAll(): Download =
        download(send(Request.Builder().url(url("api/export")).build()), "crumb.json")

    /** `PATCH /api/cookbooks/{id}`: rename or recolour a cookbook. */
    suspend fun updateCookbook(id: Long, name: String, description: String?, color: String): Cookbook {
        val body = buildJsonObject {
            put("name", name)
            put("description", description.orEmpty())
            put("color", color)
        }.toString()
        return decode(patch("api/cookbooks/$id", body), Cookbook.serializer())
    }

    /** `DELETE /api/cookbooks/{id}`. */
    suspend fun deleteCookbook(id: Long) {
        delete("api/cookbooks/$id").close()
    }

    /** `POST /api/cookbooks/{id}/recipes`: the number of recipes added. */
    suspend fun addToCookbook(bookId: Long, recipeIds: List<Long>): Int =
        decode(post("api/cookbooks/$bookId/recipes", idsBody("recipeIds", recipeIds)), Added.serializer()).added

    /** `DELETE /api/cookbooks/{id}/recipes/{recipeId}`. */
    suspend fun removeFromCookbook(bookId: Long, recipeId: Long) {
        delete("api/cookbooks/$bookId/recipes/$recipeId").close()
    }

    /** `GET /api/suggestions`: Try next's cards, the AI status and (some days) an idea. */
    suspend fun suggestions(limit: Int = 4, seed: Int? = null, exclude: List<Long> = emptyList()): Suggestions {
        val url = url("api/suggestions").newBuilder().apply {
            addQueryParameter("limit", limit.toString())
            seed?.let { addQueryParameter("seed", it.toString()) }
            if (exclude.isNotEmpty()) addQueryParameter("exclude", exclude.joinToString(","))
        }.build()
        return get(url, Suggestions.serializer())
    }

    /** `GET /api/recipes/{id}/checks`: null when Wee Chef never checked the recipe. */
    suspend fun recipeChecks(id: Long): RecipeChecks? =
        get(url("api/recipes/$id/checks"), RecipeChecks.serializer().nullable)

    /** `POST /api/recipes/{id}/checks`: re-check now; returns the pending check. */
    suspend fun checkRecipe(id: Long): RecipeChecks? =
        decode(post("api/recipes/$id/checks", "{}"), RecipeChecks.serializer().nullable)

    /** `POST /api/recipes/{id}/checks/undo`: put back what Wee Chef changed. */
    suspend fun undoChecks(id: Long): UndoResult =
        decode(post("api/recipes/$id/checks/undo", "{}"), UndoResult.serializer())

    /** `POST /api/recipes/{id}/flags/{flag}/dismiss`: "Keep as is". */
    suspend fun dismissFlag(recipeId: Long, flagId: Long): RecipeChecks? =
        decode(post("api/recipes/$recipeId/flags/$flagId/dismiss", "{}"), RecipeChecks.serializer().nullable)

    /** `GET /api/checks`: Wee Chef's import check progress. */
    suspend fun checksStatus(): ChecksStatus = get(url("api/checks"), ChecksStatus.serializer())

    /** `POST /api/checks`: "Check all"; the status carries `queued`. */
    suspend fun checkAll(): ChecksStatus = decode(post("api/checks", "{}"), ChecksStatus.serializer())

    /** `GET /api/checks/review`: the recipes with suggestions waiting. */
    suspend fun reviewList(): List<ReviewRecipe> =
        get(url("api/checks/review"), ReviewList.serializer()).recipes

    /** `POST /api/{recipes|cookbooks}/{id}/share`: make (or return) the share link. */
    suspend fun createShare(kind: ShareKind, id: Long): Share =
        decode(post("api/${kind.path}/$id/share", "{}"), Share.serializer())

    /** `PATCH /api/{recipes|cookbooks}/{id}/share`: show or hide the cook's notes. */
    suspend fun setShareNotes(kind: ShareKind, id: Long, includeNotes: Boolean): Share {
        val body = buildJsonObject { put("includeNotes", includeNotes) }.toString()
        return decode(patch("api/${kind.path}/$id/share", body), Share.serializer())
    }

    /** `DELETE /api/{recipes|cookbooks}/{id}/share`: stop sharing for good. */
    suspend fun stopSharing(kind: ShareKind, id: Long) {
        delete("api/${kind.path}/$id/share").close()
    }

    /** `GET /api/shares`: every live share link, newest first. */
    suspend fun shares(): List<SharedLink> =
        get(url("api/shares"), ListSerializer(SharedLink.serializer()))

    /** `GET /api/connector`: what this server has (MCP URL, keys, scraping). */
    suspend fun connector(): ConnectorInfo = get(url("api/connector"), ConnectorInfo.serializer())

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

    private suspend fun patch(path: String, json: String): Response =
        send(Request.Builder().url(url(path)).patch(json.toRequestBody(JSON)).build())

    private suspend fun delete(path: String): Response =
        send(Request.Builder().url(url(path)).delete().build())

    private fun idsBody(key: String, ids: List<Long>): String =
        buildJsonObject { put(key, buildJsonArray { ids.forEach { add(it) } }) }.toString()

    private fun multipart(build: MultipartBody.Builder.() -> Unit): RequestBody =
        MultipartBody.Builder().setType(MultipartBody.FORM).apply(build).build()

    private fun mediaType(raw: String): okhttp3.MediaType =
        runCatching { raw.toMediaType() }.getOrDefault(OCTET)

    /** A file response: its bytes plus the server's file name and MIME type. */
    private suspend fun download(response: Response, fallbackName: String): Download =
        withContext(Dispatchers.IO) {
            response.use { res ->
                Download(
                    fileName = fileName(res.header("Content-Disposition"), fallbackName),
                    mimeType = res.header("Content-Type")?.substringBefore(';')?.trim().orEmpty(),
                    bytes = res.body.bytes(),
                )
            }
        }

    /** `attachment; filename="pie.json"; filename*=UTF-8''…` as just the file name. */
    private fun fileName(header: String?, fallback: String): String {
        if (header == null) return fallback
        val encoded = Regex("""filename\*=UTF-8''([^;]+)""").find(header)?.groupValues?.get(1)
        if (encoded != null) {
            return runCatching { URLDecoder.decode(encoded, "UTF-8") }.getOrDefault(fallback)
        }
        val plain = Regex("""filename="?([^";]+)"?""").find(header)?.groupValues?.get(1)?.trim('"')
        return plain?.takeIf { it.isNotEmpty() } ?: fallback
    }

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
        private val JPEG = "image/jpeg".toMediaType()
        private val OCTET = "application/octet-stream".toMediaType()

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

/** The `{deleted}` count from `POST /api/recipes/bulk-delete`. */
@Serializable
private data class Deleted(val deleted: Int = 0)

/** The `{added}` count from `POST /api/cookbooks/{id}/recipes`. */
@Serializable
private data class Added(val added: Int = 0)

/** `GET /api/checks/review`'s `{recipes: [...]}` wrapper. */
@Serializable
private data class ReviewList(val recipes: List<ReviewRecipe> = emptyList())
