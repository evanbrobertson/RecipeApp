package app.crumb.android.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.navigation.NavDestination.Companion.hasRoute
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import androidx.navigation.toRoute
import app.crumb.android.ui.add.AddScreen
import app.crumb.android.ui.books.CookbookScreen
import app.crumb.android.ui.books.ShelfScreen
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.connect.ConnectScreen
import app.crumb.android.ui.importer.ImportScreen
import app.crumb.android.data.Incoming
import app.crumb.android.data.JobEnd
import app.crumb.android.ui.components.Toast
import app.crumb.android.ui.components.ToastAction
import app.crumb.android.ui.components.ToastHost
import app.crumb.android.ui.components.ToastTone
import app.crumb.android.ui.components.Toaster
import app.crumb.android.timers.TimerDock
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.navigationBarsPadding
import app.crumb.android.ui.home.HomeScreen
import app.crumb.android.ui.home.sharedLink
import app.crumb.android.ui.home.showOutcome
import app.crumb.android.ui.suggestions.ReviewCount
import app.crumb.android.ui.suggestions.SuggestionsScreen
import com.composables.icons.lucide.BookOpenText
import com.composables.icons.lucide.Ellipsis
import com.composables.icons.lucide.House
import com.composables.icons.lucide.LibraryBig
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Sparkles
import app.crumb.android.ui.cook.CookScreen
import app.crumb.android.ui.edit.EditScreen
import app.crumb.android.ui.edit.NewRecipeScreen
import app.crumb.android.ui.more.MoreScreen
import app.crumb.android.ui.prep.PrepScreen
import app.crumb.android.ui.preview.PreviewScreen
import app.crumb.android.ui.recipe.RecipeScreen
import app.crumb.android.ui.recipes.RecipesScreen
import app.crumb.android.ui.account.AccountScreen
import app.crumb.android.ui.account.ConnectionsScreen
import app.crumb.android.ui.signin.SignInScreen
import app.crumb.android.ui.trash.TrashScreen
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.NunitoSans
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable
import kotlin.reflect.KClass

@Serializable data object HomeRoute
@Serializable data class RecipesRoute(val query: String? = null)
@Serializable data object ShelfRoute
@Serializable data object SuggestionsRoute
@Serializable data object MoreRoute
@Serializable data class AddRoute(val text: String? = null, val textMode: Boolean = false)
@Serializable data class PreviewRoute(val url: String)
@Serializable data class RecipeRoute(val id: Long, val fromRandom: Boolean = false)
@Serializable data class PrepRoute(val id: Long)
@Serializable data class EditRoute(val id: Long)
@Serializable data class NewRecipeRoute(val title: String? = null)
@Serializable data class ImportRoute(val links: String? = null)
@Serializable data object ConnectRoute
@Serializable data object AccountRoute
@Serializable data object ConnectionsRoute
@Serializable data object TrashRoute
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

    val crumbNav = remember(nav) { CrumbNav(nav, container, scope, signedOut) }

    LaunchedEffect(incoming) {
        val got = incoming ?: return@LaunchedEffect
        if (got.text != null) {
            // One link goes to its preview; anything else to the Add box
            val link = sharedLink(got.text)
            if (link != null) {
                onSharedUsed()
                crumbNav.preview(link)
            } else {
                nav.navigate(AddRoute()) { launchSingleTop = true }
            }
            return@LaunchedEffect
        }
        onSharedUsed()
        importShared(got, container.importer, crumbNav)
    }

    // A video that lands while the Add box isn't on screen: a toast with a way to open it
    LaunchedEffect(Unit) {
        container.videoJobs.background.collect { end ->
            when (end) {
                is JobEnd.Saved -> Toaster.show(
                    Toast(
                        if (end.result.isNew) "Saved from the video" else "Already in your recipes",
                        end.result.title,
                        ToastTone.Success,
                        ToastAction("Open") { crumbNav.recipe(end.result.id) },
                    ),
                )
                is JobEnd.Failed -> Toaster.show("Couldn't read that video", end.message, ToastTone.Error)
            }
        }
    }
    // Back from another app or the lock screen: ask after a video the server was still reading
    LifecycleResumeEffect(Unit) {
        container.videoJobs.resume()
        onPauseOrDispose {}
    }

    val entry by nav.currentBackStackEntryAsState()
    val destination = entry?.destination
    val showTabs = destination?.hasRoute(CookRoute::class) != true


    CompositionLocalProvider(LocalNav provides crumbNav) {
        Scaffold(
            containerColor = Crumb.colors.canvas,
            bottomBar = { if (showTabs) TabBar(nav) },
        ) { padding ->
            Box(Modifier.fillMaxSize()) {
            // The tab bar's height is already taken off the screen here, so the keyboard's padding
            // (`imePadding` in a screen) must only count what rises above it, not the tab bar twice
            val tabBar = PaddingValues(bottom = padding.calculateBottomPadding())
            NavHost(nav, startDestination = HomeRoute, modifier = Modifier.padding(tabBar).consumeWindowInsets(tabBar)) {
                composable<HomeRoute> { HomeScreen() }
                composable<RecipesRoute> { backStack -> RecipesScreen(initialQuery = backStack.toRoute<RecipesRoute>().query) }
                composable<ShelfRoute> {
                    ShelfScreen(onOpenRecipe = { crumbNav.recipe(it) }, onOpenCookbook = { crumbNav.cookbook(it) }, onSignedOut = signedOut)
                }
                composable<SuggestionsRoute> { SuggestionsScreen() }
                composable<MoreRoute> { MoreScreen() }
                composable<AddRoute> { backStack ->
                    val route = backStack.toRoute<AddRoute>()
                    AddScreen(shared = shared ?: route.text, onSharedUsed = onSharedUsed, textMode = route.textMode)
                }
                composable<PreviewRoute> { backStack -> PreviewScreen(backStack.toRoute<PreviewRoute>().url) }
                composable<RecipeRoute> { backStack ->
                    val route = backStack.toRoute<RecipeRoute>()
                    RecipeScreen(id = route.id, fromRandom = route.fromRandom, onSignedOut = signedOut)
                }
                composable<CookRoute> { backStack ->
                    CookScreen(backStack.toRoute<CookRoute>().id, onClose = { crumbNav.back() })
                }
                composable<PrepRoute> { PrepScreen(it.toRoute<PrepRoute>().id) }
                composable<EditRoute> { backStack -> EditScreen(backStack.toRoute<EditRoute>().id) }
                composable<NewRecipeRoute> { backStack -> NewRecipeScreen(backStack.toRoute<NewRecipeRoute>().title) }
                composable<ImportRoute> { backStack -> ImportScreen(backStack.toRoute<ImportRoute>().links) }
                composable<ConnectRoute> { ConnectScreen() }
                composable<AccountRoute> { AccountScreen() }
                composable<ConnectionsRoute> { ConnectionsScreen() }
                composable<TrashRoute> { TrashScreen() }
                composable<BookRoute> { backStack ->
                    CookbookScreen(
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
    val container = AppContainerProvider
    val scope = rememberCoroutineScope()
    val reviewCount by ReviewCount.count.collectAsStateWithLifecycle()
    // The web sets the badge from the page's Server-Timing hint on every load; here it's
    // refreshed when the app starts and whenever it comes back to the front.
    LifecycleResumeEffect(Unit) {
        scope.launch { ReviewCount.refresh(container.api) }
        onPauseOrDispose {}
    }
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
            val badge = if (tab.route == SuggestionsRoute) ReviewCount.label(reviewCount) else null
            Column(
                Modifier
                    .weight(1f)
                    .heightIn(min = 52.dp)
                    .clip(ControlShape)
                    .selectable(active, role = Role.Tab) { nav.switchTab(tab.route) }
                    .testTag("tab-${tab.label.lowercase()}"),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                // Fixed height, so a badge never pushes its tab's label below the others'
                Box(Modifier.height(22.dp)) {
                    Icon(tab.icon, contentDescription = null, tint = tint, modifier = Modifier.size(22.dp))
                    if (badge != null) {
                        ReviewBadge(badge, Modifier.align(Alignment.TopEnd).offset(x = 9.dp, y = (-4).dp))
                    }
                }
                Spacer(Modifier.height(4.dp))
                // Large font scales grow the labels only so far, or they outgrow their fifth of the bar
                val scale = LocalDensity.current.fontScale
                Text(
                    tab.label,
                    color = tint,
                    fontSize = (13f / scale * minOf(scale, 1.15f)).sp,
                    fontWeight = if (active) FontWeight.Bold else FontWeight.SemiBold,
                    maxLines = 1,
                    softWrap = false,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

/** The web's `.review-badge`: a butter pill at the icon's top-right, "99+" past 99. */
@Composable
private fun ReviewBadge(text: String, modifier: Modifier = Modifier) {
    val c = Crumb.colors
    Box(
        modifier
            .defaultMinSize(minWidth = 22.dp, minHeight = 22.dp)
            .clip(RoundedCornerShape(50))
            .background(c.butter)
            .padding(horizontal = 6.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text,
            color = c.onButter,
            fontFamily = NunitoSans,
            fontSize = 13.sp,
            lineHeight = 13.sp,
            fontWeight = FontWeight.ExtraBold,
            maxLines = 1,
        )
    }
}

/** Photos or files shared into Crumb: saved straight away, with the web's toasts. */
private suspend fun importShared(incoming: Incoming, importer: app.crumb.android.data.Importer, nav: CrumbNav) {
    try {
        if (incoming.photos.isNotEmpty()) {
            val pages = incoming.photos.take(app.crumb.android.data.PhotoImport.MAX_PHOTOS)
            if (incoming.photos.size > pages.size) Toaster.show("Up to ${pages.size} photos", "They should all be one recipe.")
            Toaster.show("Reading ${pages.size} photo${if (pages.size == 1) "" else "s"}…")
            showOutcome(importer.photos(pages), nav)
        }
        if (incoming.files.isNotEmpty()) showOutcome(importer.files(incoming.files), nav)
    } catch (e: Exception) {
        val what = if (incoming.photos.isNotEmpty()) "Couldn't read that photo" else "Couldn't read that file"
        Toaster.show(what, e.friendlyMessage(), ToastTone.Error)
    }
}
