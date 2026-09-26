package app.crumb.android.ui.recipe

import androidx.compose.runtime.Composable
import app.crumb.android.data.Loaded
import app.crumb.android.data.Recipe
import app.crumb.android.data.RecipeRepository
import app.crumb.android.ui.LoadingViewModel
import app.crumb.android.ui.crumbViewModel

/**
 * The one recipe cook mode and mise en place read: just the recipe, through the repository's
 * offline cache. The full page uses [RecipePageViewModel].
 */
class RecipeViewModel(private val repo: RecipeRepository, private val id: Long) : LoadingViewModel<Recipe>() {
    init {
        load()
    }

    override suspend fun fetch(): Loaded<Recipe> = repo.recipe(id)
}

@Composable
fun recipeViewModel(id: Long): RecipeViewModel = crumbViewModel(key = "recipe-$id") { RecipeViewModel(it.recipes, id) }
