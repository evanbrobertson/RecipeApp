package app.crumb.android.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.selection.selectable
import androidx.compose.material3.Icon
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavDestination.Companion.hasRoute
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import androidx.navigation.toRoute
import app.crumb.android.ui.add.AddScreen
import app.crumb.android.ui.books.BookScreen
import app.crumb.android.ui.books.ShelfScreen
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.Message
import app.crumb.android.data.Incoming
import app.crumb.android.ui.components.ToastHost
import app.crumb.android.ui.components.ToastTone
import app.crumb.android.ui.components.Toaster
import app.crumb.android.timers.TimerDock
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.navigationBarsPadding
import app.crumb.android.ui.home.HomeScreen
import app.crumb.android.ui.suggestions.SuggestionsScreen
import com.composables.icons.lucide.BookOpenText
import com.composables.icons.lucide.Ellipsis
import com.composables.icons.lucide.House
import com.composables.icons.lucide.LibraryBig
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Sparkles
import app.crumb.android.ui.cook.CookScreen
import app.crumb.android.ui.more.MoreScreen
import app.crumb.android.ui.prep.PrepScreen
import app.crumb.android.ui.recipe.RecipeScreen
import app.crumb.android.ui.recipes.RecipesScreen
import app.crumb.android.ui.signin.SignInScreen
import app.crumb.android.ui.theme.Crumb
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable
import kotlin.reflect.KClass

@Serializable data object HomeRoute
@Serializable data class RecipesRoute(val query: String? = null)
@Serializable data object ShelfRoute
@Serializable data object SuggestionsRoute
@Serializable data object MoreRoute
@Serializable data class AddRoute(val text: String? = null)
@Serializable data class RecipeRoute(val id: Long, val fromRandom: Boolean = false)
@Serializable data class PrepRoute(val id: Long)
@Serializable data class EditRoute(val id: Long)
@Serializable data class NewRecipeRoute(val title: String? = null)
@Serializable data class ImportRoute(val links: String? = null)
@Serializable data object ConnectRoute
@Serializable data class CookRoute(val id: Long)
@Serializable data class BookRoute(val id: Long)

private data class Tab(val route: Any, val type: KClass<*>, val label: String, val icon: ImageVector)

/** The web's phone tab bar (Layout.astro): Suggestions is labelled "Review" there too. */
private val Tabs = listOf(
    Tab(HomeRoute, HomeRoute::class, "Home", Lucide.House),
    Tab(RecipesRoute(), RecipesRoute::class, "Recipes", Lucide.BookOpenText),
    Tab(ShelfRoute, ShelfRoute::class, "Shelf", Lucide.LibraryBig),
    Tab(SuggestionsRoute, SuggestionsRoute::class, "Review", Lucide.Sparkles),
    Tab(MoreRoute, MoreRoute::class, "More", Lucide.Ellipsis),
)

/**
 * The whole app: sign-in until there's a session, then five tabs. [shared] is what came
 * from Share → Crumb, waiting to be put into Add.
 */
@Composable
fun CrumbApp(shared: StateFlow<Incoming?>, onSharedUsed: () -> Unit) {
    val container = AppContainerProvider
    val loaded by container.session.isLoaded.collectAsStateWithLifecycle()
    val session by container.session.session.collectAsStateWithLifecycle()
    val colors = Crumb.colors

    Surface(Modifier.fillMaxSize(), color = colors.canvas, contentColor = colors.ink) {
        when {
            !loaded -> Unit
            session == null -> SignInScreen()
            else -> SignedIn(shared, onSharedUsed)
        }
        ToastHost()
    }
}

@Composable
private fun SignedIn(sharedIn: StateFlow<Incoming?>, onSharedUsed: () -> Unit) {
    val container = AppContainerProvider
    val nav = rememberNavController()
    val scope = rememberCoroutineScope()
    val incoming by sharedIn.collectAsStateWithLifecycle()
    val shared = incoming?.text
    // A 401 means the password changed or the session ran out; the cache stays for next time
    val signedOut: () -> Unit = { scope.launch { container.session.signOut() } }

    LaunchedEffect(incoming) {
        val got = incoming ?: return@LaunchedEffect
        if (got.text != null) {
            nav.navigate(AddRoute()) { launchSingleTop = true }
            return@LaunchedEffect
        }
        onSharedUsed()
        importShared(got, container.importer, nav)
    }

    val entry by nav.currentBackStackEntryAsState()
    val destination = entry?.destination
    val showTabs = destination?.hasRoute(CookRoute::class) != true

    val crumbNav = remember(nav) { CrumbNav(nav, container, scope, signedOut) }

    CompositionLocalProvider(LocalNav provides crumbNav) {
        Scaffold(
            containerColor = Crumb.colors.canvas,
            bottomBar = { if (showTabs) TabBar(nav) },
        ) { padding ->
            Box(Modifier.fillMaxSize()) {
            NavHost(nav, startDestination = HomeRoute, modifier = Modifier.padding(bottom = padding.calculateBottomPadding())) {
                composable<HomeRoute> { HomeScreen() }
                composable<RecipesRoute> { backStack -> RecipesScreen(initialQuery = backStack.toRoute<RecipesRoute>().query) }
                composable<ShelfRoute> {
                    ShelfScreen(onOpenRecipe = { crumbNav.recipe(it) }, onOpenCookbook = { crumbNav.cookbook(it) }, onSignedOut = signedOut)
                }
                composable<SuggestionsRoute> { SuggestionsScreen() }
                composable<MoreRoute> { MoreScreen() }
                composable<AddRoute> {
                    AddScreen(
                        shared = shared,
                        onSharedUsed = onSharedUsed,
                        onSaved = { recipeId, cookbookId ->
                            if (cookbookId != null) crumbNav.cookbook(cookbookId) else crumbNav.recipe(recipeId)
                        },
                    )
                }
                composable<RecipeRoute> { backStack ->
                    val route = backStack.toRoute<RecipeRoute>()
                    RecipeScreen(id = route.id, fromRandom = route.fromRandom, onSignedOut = signedOut)
                }
                composable<CookRoute> { backStack ->
                    CookScreen(backStack.toRoute<CookRoute>().id, onClose = { crumbNav.back() })
                }
                composable<PrepRoute> { PrepScreen(it.toRoute<PrepRoute>().id) }
                composable<EditRoute> { Message("Edit recipe", "Coming together.") }
                composable<NewRecipeRoute> { Message("New recipe", "Coming together.") }
                composable<ImportRoute> { Message("Import recipes", "Coming together.") }
                composable<ConnectRoute> { Message("Connect to Claude", "Coming together.") }
                composable<BookRoute> { backStack ->
                    BookScreen(
                        id = backStack.toRoute<BookRoute>().id,
                        onBack = { crumbNav.back() },
                        onOpenRecipe = { crumbNav.recipe(it) },
                        onSignedOut = signedOut,
                    )
                }
            }
            // Above the tab bar, or above cook mode's step controls (web data-timer-dock)
            TimerDock(
                Modifier
                    .align(Alignment.BottomCenter)
                    .padding(bottom = if (showTabs) padding.calculateBottomPadding() + 12.dp else 0.dp)
                    .then(if (showTabs) Modifier else Modifier.navigationBarsPadding().padding(bottom = 92.dp)),
            )
            }
        }
    }
}

private fun NavHostController.switchTab(route: Any) {
    navigate(route) {
        popUpTo(graph.findStartDestination().id) { saveState = true }
        launchSingleTop = true
        restoreState = true
    }
}

/**
 * The tab that owns what's on screen. A tab root owns itself; any other screen belongs to
 * the tab it was opened from, remembered per back-stack entry. So a recipe opened from Recipes
 * keeps Recipes lit, also after visiting More and coming back (the tab's saved stack comes
 * back with the recipe on top, and its entry keeps its id).
 */
@Composable
private fun rememberSelectedTab(nav: NavHostController): Tab? {
    val entry by nav.currentBackStackEntryAsState()
    val owners = remember { mutableMapOf<String, Tab>() }
    val current = entry ?: return null
    val root = Tabs.firstOrNull { current.destination.hasRoute(it.type) }
    return root ?: owners.getOrPut(current.id) {
        val from = nav.previousBackStackEntry
        from?.let { prev -> Tabs.firstOrNull { prev.destination.hasRoute(it.type) } ?: owners[prev.id] }
            ?: Tabs.first()
    }
}

@Composable
private fun TabBar(nav: NavHostController) {
    val colors = Crumb.colors
    val selected = rememberSelectedTab(nav)
    Row(
        Modifier
            .fillMaxWidth()
            .background(colors.nav)
            .windowInsetsPadding(WindowInsets.navigationBars)
            .padding(start = 8.dp, end = 8.dp, top = 8.dp, bottom = 6.dp)
            .testTag("tabs"),
    ) {
        Tabs.forEach { tab ->
            val active = tab == selected
            val tint = if (active) colors.butter else colors.navInk
            Column(
                Modifier
                    .weight(1f)
                    .height(52.dp)
                    .clip(ControlShape)
                    .selectable(active, role = Role.Tab) { nav.switchTab(tab.route) }
                    .testTag("tab-${tab.label.lowercase()}"),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                Icon(tab.icon, contentDescription = null, tint = tint, modifier = Modifier.size(22.dp))
                Spacer(Modifier.height(4.dp))
                Text(
                    tab.label,
                    color = tint,
                    fontSize = 13.sp,
                    fontWeight = if (active) FontWeight.Bold else FontWeight.SemiBold,
                    maxLines = 1,
                )
            }
        }
    }
}

/** Photos or files shared into Crumb: saved straight away, with the web's toasts. */
private suspend fun importShared(incoming: Incoming, importer: app.crumb.android.data.Importer, nav: NavHostController) {
    try {
        if (incoming.photos.isNotEmpty()) {
            val pages = incoming.photos.take(app.crumb.android.data.PhotoImport.MAX_PHOTOS)
            if (incoming.photos.size > pages.size) Toaster.show("Up to ${pages.size} photos", "They should all be one recipe.")
            Toaster.show("Reading ${pages.size} photo${if (pages.size == 1) "" else "s"}…")
            val saved = importer.photos(pages)
            if (saved.onDevice) {
                Toaster.show("Check the amounts", "Photos are read on this device, so a few numbers or words may be off.")
                nav.navigate(EditRoute(saved.id))
            } else {
                if (!saved.isNew) Toaster.show("Already in your recipes")
                nav.navigate(RecipeRoute(saved.id))
            }
        }
        if (incoming.files.isNotEmpty()) {
            val result = importer.files(incoming.files)
            val created = result.created
            result.results.firstOrNull { it.error != null }?.let {
                Toaster.show("Couldn't read that file", it.error, ToastTone.Error)
            }
            when {
                created.size == 1 -> nav.navigate(RecipeRoute(created.first().id))
                created.size > 1 -> {
                    Toaster.show("Imported ${created.size} recipes", tone = ToastTone.Success)
                    nav.navigate(RecipesRoute())
                }
                result.results.all { it.error == null } -> Toaster.show("Already in your recipes")
            }
        }
    } catch (e: Exception) {
        val what = if (incoming.photos.isNotEmpty()) "Couldn't read that photo" else "Couldn't read that file"
        Toaster.show(what, e.friendlyMessage(), ToastTone.Error)
    }
}
