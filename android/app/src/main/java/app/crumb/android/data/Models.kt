package app.crumb.android.data

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject

// Shapes of the Crumb server's JSON (src/model.rs). Fields the app doesn't use yet are
// skipped by the decoder, so the server can add fields freely.

@Serializable
data class Section(
    val name: String? = null,
    val items: List<String> = emptyList(),
)

@Serializable
data class Recipe(
    val id: Long,
    val title: String,
    val url: String? = null,
    val source: String = "manual",
    val description: String? = null,
    val image: String? = null,
    val author: String? = null,
    val prepTime: String? = null,
    val cookTime: String? = null,
    val totalTime: String? = null,
    val recipeYield: String? = null,
    val recipeCategory: String? = null,
    val recipeCuisine: String? = null,
    val ingredients: List<Section> = emptyList(),
    val instructions: List<Section> = emptyList(),
    val notes: String? = null,
    val createdAt: String? = null,
    val updatedAt: String? = null,
    val freezeTime: String? = null,
    /** Nutrition values may be strings or numbers, so the object is kept raw. */
    val nutrition: JsonObject? = null,
    /** The original source of a recipe saved from another Crumb's share. */
    val originalUrl: String? = null,
)

@Serializable
data class RecipeSummary(
    val id: Long,
    val title: String,
    val image: String? = null,
    val totalTime: String? = null,
    val recipeYield: String? = null,
    val recipeCategory: String? = null,
    val recipeCuisine: String? = null,
    val source: String = "manual",
    val createdAt: String? = null,
)

@Serializable
data class CookbookListItem(
    val id: Long,
    val name: String,
    val description: String? = null,
    val color: String? = null,
    val recipeCount: Long = 0,
    val createdAt: String? = null,
)

@Serializable
data class Cookbook(
    val id: Long,
    val name: String,
    val description: String? = null,
    val color: String? = null,
    val recipes: List<RecipeSummary> = emptyList(),
)

@Serializable
data class ImportedCookbook(
    val id: Long,
    val name: String,
    val added: Int = 0,
    val duplicates: Int = 0,
    val skipped: Int = 0,
)

/** `POST /api/recipes/import`: the recipe (or, for a shared cookbook link, the cookbook). */
@Serializable
data class ImportResult(
    val id: Long,
    val title: String,
    val isNew: Boolean,
    val cookbook: ImportedCookbook? = null,
)

/** Every server error: `{statusCode, statusMessage, message}`. */
@Serializable
data class ApiErrorBody(
    val statusCode: Int? = null,
    val statusMessage: String? = null,
    val message: String? = null,
)

/**
 * `POST /api/recipes` and `PATCH /api/recipes/{id}`: every settable field, camelCase.
 * Nutrition values are strings; lists default to empty.
 */
@Serializable
data class RecipeFields(
    val title: String,
    val description: String? = null,
    val url: String? = null,
    val image: String? = null,
    val author: String? = null,
    val prepTime: String? = null,
    val cookTime: String? = null,
    val totalTime: String? = null,
    val freezeTime: String? = null,
    val recipeYield: String? = null,
    val recipeCategory: String? = null,
    val recipeCuisine: String? = null,
    val ingredients: List<Section> = emptyList(),
    val instructions: List<Section> = emptyList(),
    val nutrition: Map<String, String>? = null,
    val notes: String? = null,
)

/** `POST /api/recipes/{id}/cooked`: the cook stats plus the new event (`null` if deduped). */
@Serializable
data class Cooked(
    val count: Int = 0,
    val lastCookedAt: String? = null,
    val eventId: Long? = null,
)

/** A downloaded file: `{fileName, mimeType, bytes}`. */
data class Download(
    val fileName: String,
    val mimeType: String,
    val bytes: ByteArray,
)

/** One file for `POST /api/import/files`. */
data class UploadFile(
    val name: String,
    val mimeType: String,
    val bytes: ByteArray,
)

/** One recipe saved by an import file. */
@Serializable
data class CreatedRecipe(
    val id: Long,
    val title: String,
)

/** `POST /api/import/files`: what one uploaded file gave. */
@Serializable
data class ImportSummary(
    val file: String,
    val created: List<CreatedRecipe> = emptyList(),
    val duplicates: Int = 0,
    val skipped: Int = 0,
    val error: String? = null,
)

/** One "Try next" card: a recipe and why Wee Chef or the algorithm picked it. */
@Serializable
data class Suggestion(
    val recipe: RecipeSummary,
    val reason: String,
    val reasonKind: String,
    val ai: Boolean? = null,
)

/** A dish Wee Chef thinks the cook would like that isn't in their box. */
@Serializable
data class Idea(
    val title: String,
    val why: String,
    val searchUrl: String,
)

/** `GET /api/suggestions`: the cards, the AI status (`ready`/`pending`/`off`) and an idea. */
@Serializable
data class Suggestions(
    val items: List<Suggestion> = emptyList(),
    val ai: String = "off",
    val idea: Idea? = null,
)

/** One line Wee Chef flagged: what it saw and what was done about it. */
@Serializable
data class Flag(
    val id: Long,
    val field: String,
    val itemText: String? = null,
    val kind: String,
    val state: String,
    /** The fix's data (`{p, fix, ...}`), kept raw as the server sends an object. */
    val detail: JsonElement? = null,
)

/** `GET`/`POST /api/recipes/{id}/checks`: `null` when the recipe was never checked. */
@Serializable
data class RecipeChecks(
    val status: String,
    val canUndo: Boolean = false,
    val flags: List<Flag> = emptyList(),
)

/** `POST /api/recipes/{id}/checks/undo`: the restored recipe and its new check. */
@Serializable
data class UndoResult(
    val recipe: Recipe,
    val checks: RecipeChecks? = null,
)

/** `GET`/`POST /api/checks`: Wee Chef's import check progress. */
@Serializable
data class ChecksStatus(
    val enabled: Boolean = false,
    val eligible: Int = 0,
    val checked: Int = 0,
    val pending: Int = 0,
    val failed: Int = 0,
    val tidied: Int = 0,
    val toCheck: Int = 0,
    val due: Int = 0,
    val restored: Int = 0,
    val edited: Int = 0,
    /** Only on `POST /api/checks`: how many "Check all" queued. */
    val queued: Int = 0,
)

/** One recipe on the Suggestions page with how many flags it has per field. */
@Serializable
data class ReviewRecipe(
    val id: Long,
    val title: String,
    val image: String? = null,
    val count: Int = 0,
    val fields: Map<String, Int> = emptyMap(),
)

/** Which kind of thing a share link shows. `path` is the API path segment. */
enum class ShareKind(val path: String) {
    Recipe("recipes"),
    Cookbook("cookbooks"),
}

/** `POST`/`PATCH`/`DELETE /api/{recipes|cookbooks}/{id}/share`. */
@Serializable
data class Share(
    val token: String,
    val url: String,
    val includeNotes: Boolean = true,
    val createdAt: String? = null,
)

/** `GET /api/shares`: one live share link, newest first. */
@Serializable
data class SharedLink(
    val kind: String,
    val id: Long,
    val title: String,
    val url: String,
    val includeNotes: Boolean = false,
    val createdAt: String? = null,
    val lastOpenedAt: String? = null,
)

/** `GET /api/connector`: what this server has configured. */
@Serializable
data class ConnectorInfo(
    val mcpUrl: String = "",
    val authEnabled: Boolean = false,
    val weeChef: Boolean = false,
    val claudeParsing: Boolean = false,
    val aiProvider: String? = null,
    val vision: Boolean = false,
    val browserScraping: Boolean = false,
    val weeChefChecks: Boolean = false,
)
