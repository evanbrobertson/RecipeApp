package app.crumb.android.data

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long

/** What `POST /api/recipes/preview` says about a link (src/preview.rs `api`). */
sealed interface Preview {
    /** Already in the box. */
    data class Saved(val id: Long, val title: String) : Preview

    /** A cooking video or another Crumb's shared cookbook: import it as usual, there is nothing to read first. */
    data object Import : Preview

    /** The recipe as an import would keep it. It has no id or dates yet, so [Recipe.id] is 0. */
    data class Ready(val recipe: Recipe) : Preview
}

/** The `code` on the 422 for a site whose terms forbid fetching it. */
const val SITE_TERMS = "site_terms"

/** The `code` on the error for a site that turned the server away as a bot. */
const val SITE_BLOCKED = "site_blocked"

fun parsePreview(body: String): Preview {
    val obj = CrumbJson.parseToJsonElement(body).jsonObject
    return when (obj["status"]?.jsonPrimitive?.contentOrNull) {
        "saved" -> Preview.Saved(obj.getValue("id").jsonPrimitive.long, obj["title"]?.jsonPrimitive?.contentOrNull.orEmpty())
        "import" -> Preview.Import
        "ready" -> {
            val recipe = obj.getValue("recipe").jsonObject
            Preview.Ready(CrumbJson.decodeFromJsonElement(Recipe.serializer(), JsonObject(recipe + ("id" to JsonPrimitive(0)))))
        }
        else -> throw ApiException(500, "Couldn't read that recipe")
    }
}

/** One of Popular's links: a recipe page several other households saved. */
@Serializable
data class PopularLink(
    val url: String,
    val host: String = "",
    val title: String = "",
    /** How many households saved it. */
    val households: Int = 0,
)

/** `GET /api/popular` (src/api.rs `popular`). */
@Serializable
data class Popular(
    val enabled: Boolean = false,
    val optedOut: Boolean = false,
    val items: List<PopularLink> = emptyList(),
) {
    /** What the Add page lists: nothing unless Popular is on, and at most [SHOWN] links (web `slice(0, 8)`). */
    fun links(): List<PopularLink> = if (enabled) items.take(SHOWN) else emptyList()

    companion object {
        const val SHOWN = 8
    }
}

/** `GET /api/popular/opt-out`: whether Popular exists here, and whether this household is left out of it. */
@Serializable
data class PopularSetting(val enabled: Boolean = false, val optedOut: Boolean = false)

/** The subtitle under a Popular link: "seriouseats.com · in 3 boxes". */
fun popularSubtitle(link: PopularLink): String = "${link.host} · in ${link.households} boxes"
