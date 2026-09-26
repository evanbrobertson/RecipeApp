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
import androidx.navigation.NavBackStackEntry
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
import app.crumb.android.ui.components.ToastHost
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
import app.crumb.android.ui.recipe.RecipeScreen
import app.crumb.android.ui.recipes.RecipesScreen
import app.crumb.android.ui.signin.SignInScreen
import app.crumb.android.ui.theme.Crumb
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable
import kotlin.reflect.KClass

@Serializable data object HomeRoute
@Serializable data object RecipesRoute
@Serializable data object ShelfRoute
@Serializable data object SuggestionsRoute
@Serializable data object MoreRoute
@Serializable data object AddRoute
@Serializable data class RecipeRoute(val id: Long)
@Serializable data class CookRoute(val id: Long)
@Serializable data class BookRoute(val id: Long)

private data class Tab(val route: Any, val type: KClass<*>, val label: String, val icon: ImageVector)

/** The web's phone tab bar (Layout.astro): Suggestions is labelled "Review" there too. */
private val Tabs = listOf(
    Tab(HomeRoute, HomeRoute::class, "Home", Lucide.House),
    Tab(RecipesRoute, RecipesRoute::class, "Recipes", Lucide.BookOpenText),
    Tab(ShelfRoute, ShelfRoute::class, "Shelf", Lucide.LibraryBig),
    Tab(SuggestionsRoute, SuggestionsRoute::class, "Review", Lucide.Sparkles),
    Tab(MoreRoute, MoreRoute::class, "More", Lucide.Ellipsis),
)

/**
 * The whole app: sign-in until there's a session, then five tabs. [sharedText] is text
 * from Share → Crumb, waiting to be put into Add.
 */
@Composable
fun CrumbApp(sharedText: StateFlow<String?>, onSharedUsed: () -> Unit) {
    val container = AppContainerProvider
    val loaded by container.session.isLoaded.collectAsStateWithLifecycle()
    val session by container.session.session.collectAsStateWithLifecycle()
    val colors = Crumb.colors

    Surface(Modifier.fillMaxSize(), color = colors.canvas, contentColor = colors.ink) {
        when {
            !loaded -> Unit
            session == null -> SignInScreen()
            else -> SignedIn(sharedText, onSharedUsed)
        }
        ToastHost()
    }
}

@Composable
private fun SignedIn(sharedText: StateFlow<String?>, onSharedUsed: () -> Unit) {
    val container = AppContainerProvider
    val nav = rememberNavController()
    val scope = rememberCoroutineScope()
    val shared by sharedText.collectAsStateWithLifecycle()
    // A 401 means the password changed or the session ran out; the cache stays for next time
    val signedOut: () -> Unit = { scope.launch { container.session.signOut() } }

    LaunchedEffect(shared) {
        if (shared != null) nav.navigate(AddRoute) { launchSingleTop = true }
    }

    val entry by nav.currentBackStackEntryAsState()
    val destination = entry?.destination
    val showTabs = destination?.hasRoute(CookRoute::class) != true

    Scaffold(
        containerColor = Crumb.colors.canvas,
        bottomBar = { if (showTabs) TabBar(nav) },
    ) { padding ->
        NavHost(nav, startDestination = HomeRoute, modifier = Modifier.padding(bottom = padding.calculateBottomPadding())) {
            composable<HomeRoute> { HomeScreen() }
            composable<RecipesRoute> {
                RecipesScreen(onOpen = { nav.navigate(RecipeRoute(it)) }, onSignedOut = signedOut)
            }
            composable<SuggestionsRoute> { SuggestionsScreen() }
            composable<ShelfRoute> {
                ShelfScreen(
                    onOpenRecipe = { nav.navigate(RecipeRoute(it)) },
                    onOpenCookbook = { nav.navigate(BookRoute(it)) },
                    onSignedOut = signedOut,
                )
            }
            composable<AddRoute> {
                AddScreen(
                    shared = shared,
                    onSharedUsed = onSharedUsed,
                    onSaved = { recipeId, cookbookId ->
                        nav.navigate(if (cookbookId != null) BookRoute(cookbookId) else RecipeRoute(recipeId))
                    },
                )
            }
            composable<MoreRoute> { MoreScreen() }
            composable<RecipeRoute> { backStack ->
                val id = backStack.toRoute<RecipeRoute>().id
                RecipeScreen(
                    id = id,
                    onBack = { nav.popBackStack() },
                    onCook = { nav.navigate(CookRoute(id)) },
                    onSignedOut = signedOut,
                )
            }
            composable<CookRoute> { backStack ->
                CookScreen(backStack.toRoute<CookRoute>().id, onClose = { nav.popBackStack() })
            }
            composable<BookRoute> { backStack ->
                BookScreen(
                    id = backStack.toRoute<BookRoute>().id,
                    onBack = { nav.popBackStack() },
                    onOpenRecipe = { nav.navigate(RecipeRoute(it)) },
                    onSignedOut = signedOut,
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
 * The tab that owns what's on screen: the topmost tab root in the back stack. A recipe
 * opened from Recipes keeps Recipes lit, and so does coming back to it from More (the tab's
 * saved stack is restored with the recipe on top, so the destination alone isn't enough).
 */
private fun selectedTab(stack: List<NavBackStackEntry>): Tab? =
    stack.asReversed().firstNotNullOfOrNull { entry ->
        Tabs.firstOrNull { tab -> entry.destination.hasRoute(tab.type) }
    }

@Composable
private fun TabBar(nav: NavHostController) {
    val colors = Crumb.colors
    val stack by nav.currentBackStack.collectAsStateWithLifecycle()
    val selected = selectedTab(stack)
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
