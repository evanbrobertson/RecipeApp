package app.crumb.android.ui

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.AddCircleOutline
import androidx.compose.material.icons.outlined.CollectionsBookmark
import androidx.compose.material.icons.outlined.Menu
import androidx.compose.material.icons.automirrored.outlined.MenuBook
import androidx.compose.material.icons.outlined.Timer
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavDestination.Companion.hasRoute
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import androidx.navigation.toRoute
import app.crumb.android.ui.add.AddScreen
import app.crumb.android.ui.books.BookScreen
import app.crumb.android.ui.books.BooksScreen
import app.crumb.android.ui.cook.CookScreen
import app.crumb.android.ui.more.MoreScreen
import app.crumb.android.ui.recipe.RecipeScreen
import app.crumb.android.ui.recipes.RecipesScreen
import app.crumb.android.ui.signin.SignInScreen
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.timers.TimersScreen
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable
import kotlin.reflect.KClass

@Serializable data object RecipesRoute
@Serializable data object BooksRoute
@Serializable data object AddRoute
@Serializable data object TimersRoute
@Serializable data object MoreRoute
@Serializable data class RecipeRoute(val id: Long)
@Serializable data class CookRoute(val id: Long)
@Serializable data class BookRoute(val id: Long)

private data class Tab(val route: Any, val type: KClass<*>, val label: String, val icon: ImageVector)

private val Tabs = listOf(
    Tab(RecipesRoute, RecipesRoute::class, "Recipes", Icons.AutoMirrored.Outlined.MenuBook),
    Tab(BooksRoute, BooksRoute::class, "Books", Icons.Outlined.CollectionsBookmark),
    Tab(AddRoute, AddRoute::class, "Add", Icons.Outlined.AddCircleOutline),
    Tab(TimersRoute, TimersRoute::class, "Timers", Icons.Outlined.Timer),
    Tab(MoreRoute, MoreRoute::class, "More", Icons.Outlined.Menu),
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
        if (shared != null) nav.switchTab(AddRoute)
    }

    val entry by nav.currentBackStackEntryAsState()
    val destination = entry?.destination
    val showTabs = destination?.hasRoute(CookRoute::class) != true

    Scaffold(
        containerColor = Crumb.colors.canvas,
        bottomBar = { if (showTabs) TabBar(nav, destination) },
    ) { padding ->
        NavHost(nav, startDestination = RecipesRoute, modifier = Modifier.padding(bottom = padding.calculateBottomPadding())) {
            composable<RecipesRoute> {
                RecipesScreen(onOpen = { nav.navigate(RecipeRoute(it)) }, onSignedOut = signedOut)
            }
            composable<BooksRoute> {
                BooksScreen(onOpen = { nav.navigate(BookRoute(it)) }, onSignedOut = signedOut)
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
            composable<TimersRoute> { TimersScreen() }
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

@Composable
private fun TabBar(nav: NavHostController, destination: androidx.navigation.NavDestination?) {
    val colors = Crumb.colors
    // A recipe or cookbook keeps the tab it was opened from lit
    var lastTab by rememberSaveable { mutableStateOf(Tabs.first().label) }
    Tabs.firstOrNull { tab -> destination?.hierarchy?.any { it.hasRoute(tab.type) } == true }
        ?.let { lastTab = it.label }
    NavigationBar(containerColor = colors.nav, modifier = Modifier.testTag("tabs")) {
        Tabs.forEach { tab ->
            val selected = tab.label == lastTab
            NavigationBarItem(
                selected = selected,
                onClick = { nav.switchTab(tab.route) },
                icon = { Icon(tab.icon, contentDescription = null) },
                label = { Text(tab.label) },
                colors = NavigationBarItemDefaults.colors(
                    selectedIconColor = colors.onButter,
                    selectedTextColor = colors.butter,
                    indicatorColor = colors.butter,
                    unselectedIconColor = colors.navInk,
                    unselectedTextColor = colors.navInk,
                ),
                modifier = Modifier.testTag("tab-${tab.label.lowercase()}"),
            )
        }
    }
}
