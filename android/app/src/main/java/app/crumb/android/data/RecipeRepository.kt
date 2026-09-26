package app.crumb.android.data

/** A value and whether it came from the phone because the server couldn't be reached. */
data class Loaded<T>(val value: T, val offline: Boolean = false)

/**
 * Recipes and cookbooks: from the server when it answers, otherwise the last copy on the
 * phone. Pure recipe logic (scaling, parsing, ranking) belongs to crumb-core, not here.
 */
class RecipeRepository(
    private val api: CrumbApi,
    private val cache: RecipeCache,
) {
    suspend fun recipes(query: String? = null): Loaded<List<RecipeSummary>> {
        val searching = !query.isNullOrBlank()
        return try {
            val list = api.recipes(query)
            if (!searching) cache.putRecipes(list)
            Loaded(list)
        } catch (e: OfflineException) {
            val all = cache.recipes() ?: throw e
            Loaded(if (searching) filterOffline(all, query) else all, offline = true)
        }
    }

    suspend fun recipe(id: Long): Loaded<Recipe> = cached(
        fetch = { api.recipe(id).also { cache.putRecipe(it) } },
        fallback = { cache.recipe(id) },
    )

    suspend fun cookbooks(): Loaded<List<CookbookListItem>> = cached(
        fetch = { api.cookbooks().also { cache.putCookbooks(it) } },
        fallback = { cache.cookbooks() },
    )

    suspend fun cookbook(id: Long): Loaded<Cookbook> = cached(
        fetch = { api.cookbook(id).also { cache.putCookbook(it) } },
        fallback = { cache.cookbook(id) },
    )

    suspend fun importLink(url: String) = api.importUrl(url)
    suspend fun importText(text: String) = api.importText(text)
    suspend fun markCooked(id: Long): Cooked = api.markCooked(id)

    fun photoUrl(recipeId: Long, image: String?, width: Int) = api.photoUrl(recipeId, image, width)

    private suspend fun <T> cached(fetch: suspend () -> T, fallback: suspend () -> T?): Loaded<T> =
        try {
            Loaded(fetch())
        } catch (e: OfflineException) {
            Loaded(fallback() ?: throw e, offline = true)
        }

    companion object {
        /** Offline search: every word in the title, category or cuisine, ignoring case. */
        fun filterOffline(all: List<RecipeSummary>, query: String): List<RecipeSummary> {
            val words = query.lowercase().split(Regex("\\s+")).filter { it.isNotEmpty() }
            return all.filter { r ->
                val haystack = listOfNotNull(r.title, r.recipeCategory, r.recipeCuisine)
                    .joinToString(" ").lowercase()
                words.all { it in haystack }
            }
        }
    }
}
