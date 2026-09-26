package app.crumb.android.ui

import androidx.compose.runtime.staticCompositionLocalOf
import androidx.navigation.NavHostController
import app.crumb.android.AppContainer
import app.crumb.android.ui.components.Toaster
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

/**
 * Where a screen can go. Screens read it from [LocalNav] instead of taking a lambda per
 * destination. Every web page is a route here; the web's full page loads become pushes.
 */
class CrumbNav(
    private val nav: NavHostController,
    private val container: AppContainer,
    private val scope: CoroutineScope,
    /** A 401: the password changed or the session ran out. */
    val signedOut: () -> Unit,
) {
    fun back() {
        nav.popBackStack()
    }

    fun recipe(id: Long, fromRandom: Boolean = false) = nav.navigate(RecipeRoute(id, fromRandom))
    fun cook(id: Long) = nav.navigate(CookRoute(id))
    fun prep(id: Long) = nav.navigate(PrepRoute(id))
    fun edit(id: Long) = nav.navigate(EditRoute(id))
    fun newRecipe(title: String? = null) = nav.navigate(NewRecipeRoute(title))
    fun cookbook(id: Long) = nav.navigate(BookRoute(id))
    fun add(text: String? = null) = nav.navigate(AddRoute(text)) { launchSingleTop = true }
    fun import(links: String? = null) = nav.navigate(ImportRoute(links))
    fun connect() = nav.navigate(ConnectRoute)
    fun recipes(query: String? = null) = nav.navigate(RecipesRoute(query)) { launchSingleTop = true }
    fun shelf() = nav.navigate(ShelfRoute) { launchSingleTop = true }
    fun suggestions() = nav.navigate(SuggestionsRoute) { launchSingleTop = true }

    /** Replace the current screen (after saving a new recipe, deleting one, …). */
    fun replaceWithRecipe(id: Long) {
        nav.popBackStack()
        recipe(id)
    }

    /**
     * Surprise me: one recipe at random, skipping this session's picks and anything opened
     * recently (the server also skips what was cooked lately). [current] is avoided when
     * there's any alternative.
     */
    fun surprise(current: Long? = null) {
        scope.launch {
            try {
                val pick = container.api.randomRecipe(container.local.randomExclusions(), current)
                if (pick == null) {
                    Toaster.show("Nothing to pick from yet", "Add a recipe or two first.")
                } else {
                    container.local.rememberRandom(pick.id)
                    recipe(pick.id, fromRandom = true)
                }
            } catch (e: Exception) {
                Toaster.show("Couldn't pick one", e.friendlyMessage(), app.crumb.android.ui.components.ToastTone.Error)
            }
        }
    }
}

val LocalNav = staticCompositionLocalOf<CrumbNav> { error("No CrumbNav") }
