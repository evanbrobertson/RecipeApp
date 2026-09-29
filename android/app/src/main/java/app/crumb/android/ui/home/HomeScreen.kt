package app.crumb.android.ui.home

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import app.crumb.android.data.ApiException
import app.crumb.android.data.CookbookListItem
import app.crumb.android.AppContainer
import app.crumb.android.data.RecipeSession
import app.crumb.android.data.RecipeSummary
import app.crumb.android.data.Suggestions
import app.crumb.android.data.Viewed
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.LocalNav
import app.crumb.android.ui.books.Bookshelf
import app.crumb.android.ui.books.OpenBook
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.CardShape
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.ListCard
import app.crumb.android.ui.components.ListRow
import app.crumb.android.ui.components.OfflineNote
import app.crumb.android.ui.components.ProgressBar
import app.crumb.android.ui.components.RecipePhoto
import app.crumb.android.ui.components.SectionHeader
import app.crumb.android.ui.components.Skeleton
import app.crumb.android.ui.components.ToastTone
import app.crumb.android.ui.components.Toaster
import app.crumb.android.ui.components.tileSurface
import app.crumb.android.ui.crumbViewModel
import app.crumb.android.ui.friendlyMessage
import app.crumb.android.ui.recipes.RecipeCard
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.NunitoSans
import com.composables.icons.lucide.ChefHat
import com.composables.icons.lucide.ChevronRight
import com.composables.icons.lucide.ExternalLink
import com.composables.icons.lucide.LayoutGrid
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Shuffle
import com.composables.icons.lucide.Sparkles
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import app.crumb.core.cookProgress
import app.crumb.core.greeting
import java.time.LocalTime

/** Everything Home shows below the header, loaded together. */
data class HomeState(
    val loading: Boolean = true,
    val offline: Boolean = false,
    val recipes: List<RecipeSummary> = emptyList(),
    val cookbooks: List<CookbookListItem> = emptyList(),
    val suggestions: Suggestions? = null,
    val signedOut: Boolean = false,
)

class HomeViewModel(private val container: AppContainer) : androidx.lifecycle.ViewModel() {
    private val repo = container.recipes
    private val _state = MutableStateFlow(HomeState())
    val state = _state.asStateFlow()

    /** Try next's Shuffle: today's seed and what's been shown (the web's crumb:suggest). */
    private var seed = 0
    private var shown = emptyList<Long>()

    init {
        load()
    }

    fun load() {
        viewModelScope.launch {
            val recipes = runCatching { repo.recipes() }
            val books = runCatching { repo.cookbooks() }
            val signedOut = listOf(recipes.exceptionOrNull(), books.exceptionOrNull())
                .any { (it as? ApiException)?.isSignedOut == true }
            _state.update {
                it.copy(
                    loading = false,
                    recipes = recipes.getOrNull()?.value ?: it.recipes,
                    cookbooks = books.getOrNull()?.value ?: it.cookbooks,
                    offline = recipes.getOrNull()?.offline == true || books.getOrNull()?.offline == true,
                    signedOut = signedOut,
                )
            }
        }
    }

    /** "What should I cook next?" loads when first opened. */
    fun loadSuggestions() {
        if (_state.value.suggestions != null) return
        viewModelScope.launch {
            runCatching { container.api.suggestions(limit = TryNextCount, seed = seed.takeIf { it > 0 }, exclude = shown) }
                .onSuccess { s -> _state.update { it.copy(suggestions = s) } }
        }
    }

    fun shuffle(onDone: () -> Unit) {
        val current = _state.value.suggestions?.items.orEmpty().map { it.recipe.id }
        viewModelScope.launch {
            try {
                val seen = (shown + current).distinct()
                var next = container.api.suggestions(TryNextCount, seed + 1, seen)
                var kept = seen
                // Seen everything: start again from the whole box
                if (next.items.size < 2) {
                    next = container.api.suggestions(TryNextCount, seed + 1, current)
                    kept = emptyList()
                }
                if (next.items.size < 2) {
                    Toaster.show("That's everything for now", "Cook something and check back.")
                } else {
                    seed += 1
                    shown = kept
                    _state.update { it.copy(suggestions = next) }
                }
            } catch (e: Exception) {
                Toaster.show("Couldn't shuffle", e.friendlyMessage(), ToastTone.Error)
            } finally {
                onDone()
            }
        }
    }
}

private const val TryNextCount = 4

/**
 * Home (web/src/pages/index.astro + islands/HomeFeed.svelte): the tile header with the
 * greeting and the Search/Add box, then pick up where you left off, can't decide, the shelf
 * and the newest recipes. [shared] (Share → Crumb text) goes straight into Add.
 */
@Composable
fun HomeScreen(shared: String? = null, onSharedUsed: () -> Unit = {}) {
    val vm = crumbViewModel { HomeViewModel(it) }
    val state by vm.state.collectAsStateWithLifecycle()
    val container = AppContainerProvider
    val nav = LocalNav.current
    val c = Crumb.colors
    val recent by container.local.recentlyViewed.collectAsStateWithLifecycle()
    val nextOpen by container.local.tryNextOpen.collectAsStateWithLifecycle()
    var opened by remember { mutableStateOf<CookbookListItem?>(null) }
    val scope = rememberCoroutineScope()

    LaunchedEffect(state.signedOut) {
        if (state.signedOut) nav.signedOut()
    }
    // Back on Home after adding or cooking: the newest recipes and pick-up rows are current
    var shown by remember { mutableStateOf(false) }
    LifecycleResumeEffect(Unit) {
        if (shown) vm.load() else shown = true
        onPauseOrDispose {}
    }
    LaunchedEffect(nextOpen) {
        if (nextOpen) vm.loadSuggestions()
    }

    Box(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
            Column(
                Modifier
                    .fillMaxWidth()
                    .tileSurface(c)
                    .statusBarsPadding()
                    .padding(start = 20.dp, end = 20.dp, top = 22.dp, bottom = 24.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                Text("Crumb", style = CrumbText.pageTitle, color = c.onTile)
                Text(
                    remember { greeting(LocalTime.now().hour.toUInt()) },
                    style = CrumbText.hand.copy(fontSize = 38.sp, lineHeight = 38.sp),
                    color = c.onTile,
                    modifier = Modifier.padding(bottom = 6.dp),
                )
                TopBox(recipeCount = state.recipes.size.takeIf { !state.loading }, shared = shared, onSharedUsed = onSharedUsed)
            }

            Column(
                Modifier.fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 24.dp, bottom = 32.dp),
                verticalArrangement = Arrangement.spacedBy(28.dp),
            ) {
                if (state.offline) OfflineNote()

                val pickUp = recent.take(3)
                if (pickUp.isNotEmpty()) {
                    Column {
                        SectionHeader("Pick up where you left off", Modifier.padding(bottom = 14.dp))
                        ListCard(rows = pickUp.map { v -> { PickUpRow(v) } })
                    }
                }

                Column {
                    SectionHeader("Can't decide?", Modifier.padding(bottom = 14.dp))
                    val turn by animateFloatAsState(if (nextOpen) 90f else 0f, label = "chevron")
                    ListCard(
                        rows = listOf(
                            {
                                ListRow(
                                    "Surprise me",
                                    subtitle = "One recipe, picked at random",
                                    icon = Lucide.Shuffle,
                                    onClick = { nav.surprise() },
                                )
                            },
                            {
                                ListRow(
                                    "What should I cook next?",
                                    subtitle = "Four ideas to choose from",
                                    icon = Lucide.LayoutGrid,
                                    onClick = { container.local.setTryNextOpen(!nextOpen) },
                                    chevron = false,
                                    trailing = {
                                        Icon(Lucide.ChevronRight, contentDescription = null, tint = c.inkMuted, modifier = Modifier.size(20.dp).rotate(turn))
                                    },
                                )
                            },
                        ),
                    )
                }

                if (nextOpen) {
                    TryNext(state.suggestions, total = state.recipes.size, onShuffle = { done -> vm.shuffle(done) })
                }

                if (state.cookbooks.isNotEmpty()) {
                    Column {
                        SectionHeader("The shelf", Modifier.padding(bottom = 14.dp), action = "See all", onAction = { nav.shelf() })
                        // The plank runs to the card's edges and sits on its bottom
                        Box(Modifier.fillMaxWidth().clip(CardShape).background(c.tint).padding(top = 20.dp)) {
                            Bookshelf(
                                books = state.cookbooks.take(14),
                                single = true,
                                pulledId = opened?.id,
                                onOpen = { opened = it },
                            )
                        }
                    }
                }

                if (state.loading || state.recipes.isNotEmpty()) {
                    Column {
                        SectionHeader("Fresh in the box", Modifier.padding(bottom = 14.dp), action = "See all", onAction = { nav.recipes() })
                        CardGrid(if (state.loading) null else state.recipes.take(4)) { recipe ->
                            RecipeCard(
                                recipe,
                                container.recipes.photoUrl(recipe.id, recipe.image, 480),
                                meta = freshMeta(recipe),
                                onClick = { nav.recipe(recipe.id) },
                            )
                        }
                    }
                }
            }
        }
        opened?.let { book ->
            OpenBook(
                book = book,
                load = { id -> container.recipes.cookbook(id).value },
                onOpenRecipe = { opened = null; nav.recipe(it) },
                onOpenCookbook = { opened = null; nav.cookbook(it) },
                onClose = { opened = null },
            )
        }
    }
}

/** A recent recipe: back into cook mode mid-way, or just to the page. */
@Composable
private fun PickUpRow(v: Viewed) {
    val c = Crumb.colors
    val nav = LocalNav.current
    val container = AppContainerProvider
    val step = RecipeSession.cookStep(v.id)
    val steps = v.steps
    // Not started, or on the last step (cook mode stays there after Finish): core says which
    val progress = cookProgress(step?.toLong(), steps?.toLong())
    val photoPx = with(LocalDensity.current) { 64.dp.roundToPx() }
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 68.dp)
            .clickable(role = Role.Button) { if (progress != null) nav.cook(v.id) else nav.recipe(v.id) }
            .padding(12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        RecipePhoto(
            container.recipes.photoUrl(v.id, v.image, photoPx),
            v.image,
            Modifier.size(64.dp).clip(ControlShape),
            placeholderId = v.id,
        )
        Column(Modifier.weight(1f)) {
            Text(v.title, color = c.ink, fontFamily = NunitoSans, fontSize = 17.sp, fontWeight = FontWeight.Bold, lineHeight = 21.sp, maxLines = 2, overflow = TextOverflow.Ellipsis)
            if (progress != null) {
                Row(Modifier.padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    ProgressBar(progress.step / progress.of.toFloat(), Modifier.weight(1f))
                    Text("Step ${progress.step} of ${progress.of}", style = CrumbText.meta, color = c.inkMuted)
                }
            } else {
                Text(viewedLine(v.at), color = c.inkMuted, fontFamily = NunitoSans, fontSize = 14.sp, modifier = Modifier.padding(top = 2.dp))
            }
        }
    }
}

/** "What should I cook next?": four recipes worth cooking soon, maybe an idea, and Shuffle. */
@Composable
private fun TryNext(suggestions: Suggestions?, total: Int, onShuffle: (done: () -> Unit) -> Unit) {
    val c = Crumb.colors
    val nav = LocalNav.current
    val container = AppContainerProvider
    val uri = androidx.compose.ui.platform.LocalUriHandler.current
    var loading by remember { mutableStateOf(false) }
    when {
        suggestions == null || loading -> CardGrid<Any>(null) {}
        suggestions.items.size < 2 -> Text(
            "Add a few more recipes and Crumb will start suggesting some.",
            color = c.inkMuted,
            fontFamily = NunitoSans,
            fontSize = 14.sp,
        )
        else -> Column {
            CardGrid(suggestions.items.map { it as Any } + listOfNotNull(suggestions.idea)) { item ->
                when (item) {
                    is app.crumb.android.data.Suggestion -> RecipeCard(
                        item.recipe,
                        container.recipes.photoUrl(item.recipe.id, item.recipe.image, 480),
                        reason = item.reason,
                        aiReason = item.ai == true,
                        onClick = { nav.recipe(item.recipe.id) },
                    )
                    is app.crumb.android.data.Idea -> IdeaCard(item) { runCatching { uri.openUri(item.searchUrl) } }
                }
            }
            if (total > TryNextCount) {
                Btn(
                    "Shuffle",
                    { loading = true; onShuffle { loading = false } },
                    style = BtnStyle.Soft,
                    icon = Lucide.Shuffle,
                    modifier = Modifier.align(Alignment.CenterHorizontally).padding(top = 16.dp),
                )
            }
        }
    }
}

/** A dish Wee Chef suggests that isn't in the box yet; opens a web search for it. */
@Composable
private fun IdeaCard(idea: app.crumb.android.data.Idea, onClick: () -> Unit) {
    val c = Crumb.colors
    Column(Modifier.fillMaxWidth().clickable(role = Role.Button, onClick = onClick)) {
        Column(
            Modifier
                .fillMaxWidth()
                .aspectRatio(4f / 3f)
                .clip(ControlShape)
                .background(c.tint)
                .drawBehind {
                    val w = 1.dp.toPx()
                    drawRoundRect(
                        c.lineStrong,
                        style = Stroke(w, pathEffect = PathEffect.dashPathEffect(floatArrayOf(6.dp.toPx(), 4.dp.toPx()))),
                        cornerRadius = CornerRadius(12.dp.toPx()),
                    )
                },
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterVertically),
        ) {
            Icon(Lucide.ChefHat, contentDescription = null, tint = c.primary, modifier = Modifier.size(44.dp))
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                Icon(Lucide.Sparkles, contentDescription = null, tint = c.primary, modifier = Modifier.size(14.dp))
                Text("Idea from Wee Chef", color = c.primary, fontFamily = NunitoSans, fontSize = 13.sp, fontWeight = FontWeight.Bold)
            }
        }
        Text(idea.title, style = CrumbText.rowTitle, color = c.ink, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 10.dp))
        Text(idea.why, color = c.inkMuted, fontFamily = NunitoSans, fontSize = 14.sp, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 4.dp))
        Row(Modifier.padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            Text("Find it", color = c.primary, fontFamily = NunitoSans, fontSize = 14.sp, fontWeight = FontWeight.Bold)
            Icon(Lucide.ExternalLink, contentDescription = null, tint = c.primary, modifier = Modifier.size(14.dp))
        }
    }
}

/** Two columns of cards; null shows four skeletons. */
@Composable
private fun <T> CardGrid(items: List<T>?, card: @Composable (T) -> Unit) {
    val rows = items?.chunked(2)
    Column(verticalArrangement = Arrangement.spacedBy(20.dp)) {
        if (rows == null) {
            repeat(2) {
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    repeat(2) {
                        Column(Modifier.weight(1f)) {
                            Skeleton(Modifier.fillMaxWidth().aspectRatio(4f / 3f))
                            Skeleton(Modifier.padding(top = 10.dp).fillMaxWidth(0.8f).height(18.dp), RoundedCornerShape(6.dp))
                        }
                    }
                }
            }
        } else {
            rows.forEach { row ->
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    row.forEach { Box(Modifier.weight(1f)) { card(it) } }
                    if (row.size == 1) Spacer(Modifier.weight(1f))
                }
            }
        }
    }
}
