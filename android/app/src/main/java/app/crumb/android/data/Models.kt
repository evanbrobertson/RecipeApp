package app.crumb.android.data

import kotlinx.serialization.Serializable

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
