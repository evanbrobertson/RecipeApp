package app.crumb.android.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.KSerializer
import kotlinx.serialization.builtins.ListSerializer
import java.io.File
import java.security.MessageDigest

/**
 * The last copy of everything the server sent, as JSON files, so the recipe box and any
 * recipe opened before still work without a connection. One folder per server; signing
 * out deletes it. The server stays the source of truth: this is only ever overwritten.
 */
class RecipeCache(private val root: File) {
    private var dir: File? = null

    fun useServer(server: String?) {
        dir = server?.let { File(root, sha(it)) }
    }

    fun clear() {
        dir?.deleteRecursively()
    }

    suspend fun recipes(): List<RecipeSummary>? = read("recipes", RecipeSummaries)
    suspend fun putRecipes(list: List<RecipeSummary>) = write("recipes", RecipeSummaries, list)

    suspend fun recipe(id: Long): Recipe? = read("recipe-$id", Recipe.serializer())
    suspend fun putRecipe(recipe: Recipe) = write("recipe-${recipe.id}", Recipe.serializer(), recipe)

    suspend fun cookbooks(): List<CookbookListItem>? = read("cookbooks", CookbookList)
    suspend fun putCookbooks(list: List<CookbookListItem>) = write("cookbooks", CookbookList, list)

    suspend fun cookbook(id: Long): Cookbook? = read("cookbook-$id", Cookbook.serializer())
    suspend fun putCookbook(book: Cookbook) = write("cookbook-${book.id}", Cookbook.serializer(), book)

    private suspend fun <T> read(name: String, serializer: KSerializer<T>): T? = withContext(Dispatchers.IO) {
        val file = dir?.let { File(it, "$name.json") }?.takeIf { it.exists() } ?: return@withContext null
        runCatching { CrumbJson.decodeFromString(serializer, file.readText()) }.getOrNull()
    }

    private suspend fun <T> write(name: String, serializer: KSerializer<T>, value: T) = withContext(Dispatchers.IO) {
        val folder = dir ?: return@withContext
        folder.mkdirs()
        val tmp = File(folder, "$name.json.tmp")
        tmp.writeText(CrumbJson.encodeToString(serializer, value))
        tmp.renameTo(File(folder, "$name.json"))
    }

    private companion object {
        val RecipeSummaries = ListSerializer(RecipeSummary.serializer())
        val CookbookList = ListSerializer(CookbookListItem.serializer())

        fun sha(text: String): String =
            MessageDigest.getInstance("SHA-256").digest(text.encodeToByteArray())
                .take(12).joinToString("") { "%02x".format(it) }
    }
}
