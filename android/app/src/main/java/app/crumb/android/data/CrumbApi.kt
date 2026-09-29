package app.crumb.android.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.nullable
import kotlinx.serialization.builtins.serializer
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
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
import java.net.URLEncoder
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/** A request that reached the server and was refused. [message] is written for people. */
class ApiException(
    val status: Int,
    override val message: String,
    /** The server's machine-readable reason, when it gives one (`site_terms`, `site_blocked`). */
    val code: String? = null,
    /** With `site_terms`: the listed site's name. */
    val site: String? = null,
) : Exception(message) {
    val isSignedOut get() = status == 401
}

/** A video import's job is gone from the server (it was restarted, or the job aged out). */
class JobGoneException : Exception("That video import is no longer running")

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
        val hosted = SessionCookies.isHosted(session()?.cookie)
        runCatching { post(if (hosted) "api/auth/sign-out" else "api/auth/logout", "{}").close() }
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

    /**
     * `POST /api/recipes/import` for a link. A cooking video answers 202 with a job instead of a
     * recipe, which is polled every [POLL_MS] until it's saved; [progress] gets crumb-core's
     * short lines ("Queued (2nd)…"). A full queue is a 429 [ApiException]; a job that fails is one
     * with the job's own status and message. A dropped connection between polls doesn't lose the
     * job (it keeps running on the server), so a few misses are waited out.
     */
    suspend fun importUrl(link: String, progress: (String) -> Unit = {}): ImportResult =
        when (val started = startImport(link)) {
            is ImportStart.Done -> started.result
            is ImportStart.Queued -> watchJob(started.job.jobId!!, started.job, progress)
        }

    /** `POST /api/recipes/import` for a link, without waiting for a cooking video's job. */
    suspend fun startImport(link: String): ImportStart {
        val body = withContext(Dispatchers.Default) {
            post("api/recipes/import", buildJsonObject { put("url", link) }.toString()).use { it.body.string() }
        }
        val started = CrumbJson.decodeFromString(ImportJob.serializer(), body)
        return if (started.jobId != null) ImportStart.Queued(started)
        else ImportStart.Done(CrumbJson.decodeFromString(ImportResult.serializer(), body))
    }

    /** `GET /api/import/jobs/{id}`; a job the server no longer has is a [JobGoneException]. */
    suspend fun jobStatus(jobId: String): ImportJob =
        try {
            get(url("api/import/jobs/${URLEncoder.encode(jobId, "UTF-8")}"), ImportJob.serializer())
        } catch (e: ApiException) {
            if (e.status == 404) throw JobGoneException() else throw e
        }

    /**
     * Polls a video's job until it is saved (or fails), from its [first] answer or, for a job
     * picked up again after the app was away, from the server's. Gives up with the
     * [OfflineException] after [MAX_POLL_MISSES] polls in a row that can't connect; the job itself
     * keeps running on the server.
     */
    suspend fun watchJob(jobId: String, first: ImportJob? = null, progress: (String) -> Unit = {}): ImportResult {
        var job = first
        var missed = 0
        while (true) {
            val current = job
            if (current != null) {
                when (current.status) {
                    "done" -> {
                        val r = current.recipe ?: throw ApiException(422, "Couldn't read that video")
                        return ImportResult(r.id, r.title, r.isNew, fromVideo = true)
                    }
                    "failed" -> throw ApiException(current.statusCode ?: 422, current.message ?: "Couldn't read that video")
                }
                progress(app.crumb.core.jobProgress(current.status, current.position?.toUInt()))
            }
            if (current != null || missed > 0) delay(POLL_MS)
            job = try {
                jobStatus(jobId).also { missed = 0 }
            } catch (e: OfflineException) {
                if (++missed > MAX_POLL_MISSES) throw e
                current
            }
        }
    }

    /** `POST /api/recipes/preview`: a link read and shown before it's saved. */
    suspend fun preview(link: String): Preview {
        val body = withContext(Dispatchers.Default) {
            post("api/recipes/preview", buildJsonObject { put("url", link) }.toString()).use { it.body.string() }
        }
        return parsePreview(body)
    }

    /**
     * `GET /api/popular`: links several other households saved that this one hasn't. Empty when
     * the server has no Popular (one household, password mode, `POPULAR=off`).
     */
    suspend fun popular(): List<PopularLink> =
        try {
            get(url("api/popular"), Popular.serializer()).links()
        } catch (e: ApiException) {
            if (e.status == 404) emptyList() else throw e
        }

    /** `GET /api/popular/opt-out`: null when the server has no Popular. */
    suspend fun popularSetting(): PopularSetting? =
        try {
            get(url("api/popular/opt-out"), PopularSetting.serializer()).takeIf { it.enabled }
        } catch (e: ApiException) {
            if (e.status == 404) null else throw e
        }

    /** `POST /api/popular/opt-out`: whether this household's saved links stay out of Popular. */
    suspend fun setPopularOptOut(optedOut: Boolean) {
        post("api/popular/opt-out", buildJsonObject { put("optedOut", optedOut) }.toString()).close()
    }

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

    /** `GET /api/account/export`: the account, its devices and every household's recipes. */
    suspend fun exportAccount(): Download =
        download(send(Request.Builder().url(url("api/account/export")).build()), "crumb-account.json")

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

    /** `GET /api/staples`: the ingredients most of the box's recipes use. */
    suspend fun staples(): Staples = get(url("api/staples"), Staples.serializer())

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

    /** The live share link for one recipe or cookbook, if it has one (from `GET /api/shares`). */
    suspend fun existingShare(kind: ShareKind, id: Long): Share? {
        val type = if (kind == ShareKind.Recipe) "recipe" else "cookbook"
        val link = shares().firstOrNull { it.kind == type && it.id == id } ?: return null
        return Share(link.url.substringAfterLast('/'), link.url, link.includeNotes, link.createdAt)
    }

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

    internal fun url(path: String): HttpUrl {
        val server = session()?.server ?: throw ApiException(401, "Not signed in")
        return server.resolve(path)!!
    }

    private suspend fun <T> get(url: HttpUrl, serializer: KSerializer<T>): T =
        decode(send(Request.Builder().url(url).build()), serializer)

    internal suspend fun post(path: String, json: String): Response =
        send(Request.Builder().url(url(path)).post(json.toRequestBody(JSON)).build())

    internal suspend fun patch(path: String, json: String): Response =
        send(Request.Builder().url(url(path)).patch(json.toRequestBody(JSON)).build())

    internal suspend fun delete(path: String): Response =
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

    internal suspend fun <T> decode(response: Response, serializer: KSerializer<T>): T =
        withContext(Dispatchers.Default) {
            response.use { CrumbJson.decodeFromString(serializer, it.body.string()) }
        }

    internal suspend fun send(request: Request, withSession: Boolean = true): Response {
        val authed = if (withSession) {
            val cookie = session()?.cookie
            request.newBuilder().apply {
                if (cookie != null) header("Cookie", SessionCookies.header(cookie))
                // Better Auth refuses a cookie's request that names no origin, as a browser's always does
                if (SessionCookies.isHosted(cookie)) header("Origin", request.url.origin())
            }.build()
        } else request
        val response = try {
            client.newCall(authed).await()
        } catch (e: IOException) {
            throw OfflineException(e)
        }
        if (!response.isSuccessful) {
            val text = response.use { runCatching { it.body.string() }.getOrDefault("") }
            throw ApiException(response.code, errorMessage(response.code, text), errorField(text, "code"), errorField(text, "site"))
        }
        return response
    }

    companion object {
        const val COOKIE = "crumb_session"

        /** How often a video's job is asked after (web `POLL_MS`). */
        const val POLL_MS = 2000L

        /** Polls in a row that may fail to connect (about three minutes) before the wait is given up. */
        const val MAX_POLL_MISSES = 90
        private val JSON = "application/json".toMediaType()
        private val JPEG = "image/jpeg".toMediaType()
        private val OCTET = "application/octet-stream".toMediaType()

        /** The value of a `crumb_session=…` Set-Cookie header, unless it clears the cookie. */
        fun sessionCookie(header: String): String? = app.crumb.core.sessionCookie(header)

        /** The server's `{message}` without its "field: " prefix, or a default for [status]. */
        fun errorMessage(status: Int, body: String): String = app.crumb.core.errorMessage(status.toUShort(), body)

        /**
         * What someone typed as their server ("crumb.example.com", "http://192.168.1.5:3000")
         * as a base URL ending in "/", or null if it isn't a web address. Defaults to HTTPS.
         */
        fun serverUrl(input: String): HttpUrl? = app.crumb.core.serverUrl(input)?.toHttpUrlOrNull()
    }
}

/** A string field of an error body (`{message, code, site}`), if there is one. */
internal fun errorField(body: String, name: String): String? =
    runCatching { CrumbJson.parseToJsonElement(body).jsonObject[name]?.jsonPrimitive?.contentOrNull }.getOrNull()

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
