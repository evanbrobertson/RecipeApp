package app.crumb.android.ui.recipes

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
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
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.createSavedStateHandle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.crumb.android.data.ApiException
import app.crumb.android.data.CookbookListItem
import app.crumb.android.data.RecipeRepository
import app.crumb.android.data.RecipeSummary
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.LocalNav
import app.crumb.android.ui.books.bookLook
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.CardShape
import app.crumb.android.ui.components.Chip
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbModal
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.EmptyState
import app.crumb.android.ui.components.Message
import app.crumb.android.ui.components.OfflineNote
import app.crumb.android.ui.components.Skeleton
import app.crumb.android.ui.components.ToastTone
import app.crumb.android.ui.components.Toaster
import app.crumb.android.ui.components.VSpace
import app.crumb.android.ui.friendlyMessage
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.NunitoSans
import app.crumb.core.categories
import com.composables.icons.lucide.BookOpen
import com.composables.icons.lucide.BookPlus
import com.composables.icons.lucide.LoaderCircle
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Plus
import com.composables.icons.lucide.Search
import com.composables.icons.lucide.SearchX
import com.composables.icons.lucide.SquareCheck
import com.composables.icons.lucide.Trash2
import com.composables.icons.lucide.X
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

private const val QueryKey = "q"
private const val PhotoWidthPx = 320

data class RecipesUiState(
    val recipes: List<RecipeSummary> = emptyList(),
    val cookbooks: List<CookbookListItem> = emptyList(),
    val loading: Boolean = true,
    val searching: Boolean = false,
    val offline: Boolean = false,
    val error: String? = null,
    val signedOut: Boolean = false,
)

/**
 * The recipes list's state (web/src/islands/RecipesPage.svelte): one page load of recipes and
 * cookbooks, then a debounced (250ms) server-side search whose `requestId` guard drops any
 * answer that arrives out of order. The query lives in [SavedStateHandle] so it survives
 * rotation; a RecipesRoute query seeds it on first open.
 */
class RecipesViewModel(
    private val repo: RecipeRepository,
    private val saved: SavedStateHandle,
) : ViewModel() {
    val query: StateFlow<String> = saved.getStateFlow(QueryKey, "")

    private val _state = MutableStateFlow(RecipesUiState())
    val state: StateFlow<RecipesUiState> = _state.asStateFlow()

    private val gate = SearchGate()
    private var applied = query.value.trim()
    private var timer: Job? = null
    private var loadJob: Job? = null

    init {
        load(showSpinner = true)
        query.drop(1).onEach { typed ->
            timer?.cancel()
            timer = viewModelScope.launch {
                delay(250)
                search(typed)
            }
        }.launchIn(viewModelScope)
    }

    fun setQuery(text: String) {
        saved[QueryKey] = text
    }

    /** Load (or reload, for pull-to-refresh) the current query and the cookbook list. */
    fun load(showSpinner: Boolean = false) {
        if (showSpinner) _state.update { it.copy(loading = true, error = null) }
        loadJob?.cancel()
        loadJob = viewModelScope.launch {
            try {
                applied = query.value.trim()
                val recipes = repo.recipes(applied.ifEmpty { null })
                val books = runCatching { repo.cookbooks() }.getOrNull()
                _state.update {
                    it.copy(
                        recipes = recipes.value,
                        cookbooks = books?.value ?: it.cookbooks,
                        loading = false,
                        offline = recipes.offline || books?.offline == true,
                        error = null,
                        signedOut = false,
                    )
                }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _state.update {
                    it.copy(loading = false, error = e.friendlyMessage(), signedOut = (e as? ApiException)?.isSignedOut == true)
                }
            }
        }
    }

    /** Drop the deleted ids from the list after a bulk delete. */
    fun remove(ids: List<Long>) {
        _state.update { s -> s.copy(recipes = s.recipes.filter { it.id !in ids }) }
    }

    private suspend fun search(raw: String) {
        val value = raw.trim()
        if (value == applied) return
        applied = value
        val token = gate.begin()
        _state.update { it.copy(searching = true) }
        try {
            val loaded = repo.recipes(value.ifEmpty { null })
            if (gate.isCurrent(token)) _state.update { it.copy(recipes = loaded.value, offline = loaded.offline) }
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            if (gate.isCurrent(token)) Toaster.show("Search failed", e.friendlyMessage(), ToastTone.Error)
        } finally {
            if (gate.isCurrent(token)) _state.update { it.copy(searching = false) }
        }
    }
}

/**
 * The recipe box (web/src/islands/RecipesPage.svelte): a debounced search, client-side
 * category chips, a two-column grid, and select mode with bulk delete and add-to-cookbook.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RecipesScreen(initialQuery: String? = null) {
    val container = AppContainerProvider
    val nav = LocalNav.current
    val vm: RecipesViewModel = viewModel(
        factory = viewModelFactory {
            initializer {
                val saved = createSavedStateHandle()
                if (saved.get<String>(QueryKey) == null && !initialQuery.isNullOrBlank()) saved[QueryKey] = initialQuery
                RecipesViewModel(container.recipes, saved)
            }
        },
    )
    val state by vm.state.collectAsStateWithLifecycle()
    val query by vm.query.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    val c = Crumb.colors
    val order = remember { categories() }

    var selecting by remember { mutableStateOf(false) }
    var selected by remember { mutableStateOf<Set<Long>>(emptySet()) }
    var category by remember { mutableStateOf("all") }
    var showDelete by remember { mutableStateOf(false) }
    var showCookbooks by remember { mutableStateOf(false) }
    var busy by remember { mutableStateOf(false) }

    LaunchedEffect(state.signedOut) {
        if (state.signedOut) nav.signedOut()
    }

    val q = query.trim()
    val present = categoriesIn(state.recipes, order)
    val visible = if (category == "all") state.recipes else state.recipes.filter { it.recipeCategory == category }
    val visibleIds = visible.map { it.id }

    fun clearSearch() {
        vm.setQuery("")
        category = "all"
    }

    fun deleteSelected() {
        val ids = selected.toList()
        busy = true
        scope.launch {
            try {
                container.api.deleteRecipes(ids)
                container.local.forgetViewed(ids)
                vm.remove(ids)
                Toaster.show(deletedTitle(ids.size), tone = ToastTone.Success)
                showDelete = false
                selecting = false
                selected = emptySet()
            } catch (e: Exception) {
                Toaster.show("Couldn't delete", e.friendlyMessage(), ToastTone.Error)
            } finally {
                busy = false
            }
        }
    }

    fun addToCookbook(book: CookbookListItem) {
        val ids = selected.toList()
        busy = true
        scope.launch {
            try {
                container.api.addToCookbook(book.id, ids)
                Toaster.show(addedTitle(ids.size, book.name), tone = ToastTone.Success)
                showCookbooks = false
                selecting = false
                selected = emptySet()
            } catch (e: Exception) {
                Toaster.show("Couldn't add to cookbook", e.friendlyMessage(), ToastTone.Error)
            } finally {
                busy = false
            }
        }
    }

    Box(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxSize().statusBarsPadding()) {
            Row(
                Modifier.fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 16.dp, bottom = 20.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text("Recipes", style = CrumbText.pageTitle, color = c.ink, modifier = Modifier.weight(1f))
                if (state.recipes.isNotEmpty()) {
                    Btn(
                        text = if (selecting) "Done" else "Select",
                        onClick = { selecting = !selecting; selected = emptySet() },
                        style = if (selecting) BtnStyle.Tile else BtnStyle.Outline,
                        icon = if (selecting) Lucide.X else Lucide.SquareCheck,
                    )
                }
                Btn("Add", onClick = { nav.add() }, style = BtnStyle.Primary, icon = Lucide.Plus)
            }

            Column(Modifier.fillMaxWidth().padding(horizontal = 20.dp)) {
                SearchField(
                    value = query,
                    onValueChange = vm::setQuery,
                    placeholder = searchPlaceholder(state.recipes.size, q.isNotEmpty()),
                    searching = state.searching,
                    onClear = ::clearSearch,
                )
                if (present.size > 1) {
                    VSpace(12.dp)
                    LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        item(key = "all") { Chip("Everything", category == "all", onClick = { category = "all" }) }
                        present.forEach { name ->
                            item(key = name) { Chip(name, category == name, onClick = { category = name }) }
                        }
                    }
                }
                VSpace(28.dp)
            }

            Box(Modifier.weight(1f).fillMaxWidth()) {
                when {
                    state.loading -> GridSkeleton()
                    state.error != null -> Message(
                        "Couldn't load your recipes",
                        state.error,
                        action = { Btn("Try again", { vm.load(showSpinner = true) }, style = BtnStyle.Outline) },
                    )
                    state.recipes.isEmpty() && q.isEmpty() -> Column(Modifier.fillMaxSize().padding(horizontal = 20.dp)) {
                        EmptyState(
                            icon = Lucide.BookOpen,
                            title = "No recipes yet",
                            description = "Paste a link or some recipe text to save your first one.",
                        ) {
                            Btn("Add a recipe", { nav.add() }, style = BtnStyle.Primary, icon = Lucide.Plus)
                        }
                    }
                    visible.isEmpty() -> Column(Modifier.fillMaxSize().padding(horizontal = 20.dp)) {
                        EmptyState(icon = Lucide.SearchX, title = "Nothing matches", description = noMatchesDescription(q)) {
                            Btn("Clear search", ::clearSearch, style = BtnStyle.Soft)
                        }
                    }
                    else -> PullToRefreshBox(
                        isRefreshing = false,
                        onRefresh = { vm.load() },
                        modifier = Modifier.fillMaxSize(),
                    ) {
                        LazyVerticalGrid(
                            columns = GridCells.Fixed(2),
                            contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 4.dp, bottom = if (selecting) 104.dp else 28.dp),
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                            verticalArrangement = Arrangement.spacedBy(20.dp),
                            modifier = Modifier.fillMaxSize().testTag("recipe-grid"),
                        ) {
                            if (state.offline) {
                                item(span = { GridItemSpan(maxLineSpan) }) { OfflineNote(Modifier.padding(bottom = 4.dp)) }
                            }
                            items(visible, key = { it.id }) { recipe ->
                                RecipeCard(
                                    recipe = recipe,
                                    photoUrl = container.recipes.photoUrl(recipe.id, recipe.image, PhotoWidthPx),
                                    onClick = { nav.recipe(recipe.id) },
                                    selectable = selecting,
                                    selected = recipe.id in selected,
                                    onToggle = { selected = toggleSelection(selected, it) },
                                )
                            }
                        }
                    }
                }
            }
        }

        AnimatedVisibility(
            visible = selecting,
            enter = slideInVertically(tween(200)) { it } + fadeIn(tween(200)),
            exit = slideOutVertically(tween(200)) { it } + fadeOut(tween(200)),
            modifier = Modifier.align(Alignment.BottomCenter),
        ) {
            SelectToolbar(
                selectedCount = selected.size,
                allSelected = allSelected(visibleIds, selected),
                onToggleAll = { selected = toggleAllSelection(visibleIds, selected) },
                onCookbook = { showCookbooks = true },
                onDelete = { showDelete = true },
            )
        }
    }

    if (showDelete) {
        CrumbModal(
            title = "Delete recipes?",
            description = deleteDescription(selected.size),
            onDismiss = { if (!busy) showDelete = false },
            footer = {
                Btn("Cancel", { showDelete = false }, style = BtnStyle.Ghost, enabled = !busy)
                Btn("Delete", { deleteSelected() }, style = BtnStyle.Danger, enabled = !busy)
            },
        ) {}
    }

    if (showCookbooks) {
        CrumbModal(title = "Add to cookbook", onDismiss = { if (!busy) showCookbooks = false }) {
            if (state.cookbooks.isEmpty()) {
                Column(Modifier.fillMaxWidth().padding(vertical = 8.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text("You don't have any cookbooks yet.", style = CrumbText.body, color = c.inkMuted, textAlign = TextAlign.Center)
                    VSpace(12.dp)
                    Btn("Create a cookbook", { nav.shelf() }, style = BtnStyle.Soft)
                }
            } else {
                Column(Modifier.fillMaxWidth()) {
                    state.cookbooks.forEach { book ->
                        CookbookRow(book, enabled = !busy) { addToCookbook(book) }
                    }
                }
            }
        }
    }
}

/** The web's search input: a leading Search/loader icon and a trailing clear button. */
@Composable
private fun SearchField(
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    searching: Boolean,
    onClear: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val c = Crumb.colors
    var focused by remember { mutableStateOf(false) }
    val spin = rememberInfiniteTransition(label = "search")
    val angle by spin.animateFloat(0f, 360f, infiniteRepeatable(tween(900, easing = LinearEasing)), label = "angle")
    BasicTextField(
        value = value,
        onValueChange = onValueChange,
        singleLine = true,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        textStyle = TextStyle(fontFamily = NunitoSans, fontSize = 16.sp, color = c.ink),
        cursorBrush = SolidColor(c.tile),
        modifier = modifier.fillMaxWidth().onFocusChanged { focused = it.isFocused }.testTag("search"),
        decorationBox = { inner ->
            Box(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 48.dp)
                    .clip(CardShape)
                    .background(c.paper)
                    .border(1.dp, if (focused) c.tile else c.lineStrong, CardShape),
                contentAlignment = Alignment.CenterStart,
            ) {
                Box(Modifier.align(Alignment.CenterStart).padding(start = 14.dp)) {
                    if (searching) {
                        Icon(Lucide.LoaderCircle, contentDescription = null, tint = c.inkMuted, modifier = Modifier.size(20.dp).rotate(angle))
                    } else {
                        Icon(Lucide.Search, contentDescription = null, tint = c.inkMuted, modifier = Modifier.size(20.dp))
                    }
                }
                Box(Modifier.fillMaxWidth().padding(start = 46.dp, end = 52.dp)) {
                    if (value.isEmpty()) {
                        Text(placeholder, style = TextStyle(fontFamily = NunitoSans, fontSize = 16.sp), color = c.inkMuted, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                    inner()
                }
                if (value.isNotEmpty()) {
                    Box(
                        Modifier
                            .align(Alignment.CenterEnd)
                            .padding(end = 6.dp)
                            .size(40.dp)
                            .clip(ControlShape)
                            .clickable(role = Role.Button, onClick = onClear),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(Lucide.X, contentDescription = "Clear search", tint = c.inkMuted, modifier = Modifier.size(18.dp))
                    }
                }
            }
        },
    )
}

/** The web's flying-in selection bar, sitting just above the tab bar. */
@Composable
private fun SelectToolbar(
    selectedCount: Int,
    allSelected: Boolean,
    onToggleAll: () -> Unit,
    onCookbook: () -> Unit,
    onDelete: () -> Unit,
) {
    val c = Crumb.colors
    Column(Modifier.fillMaxWidth().background(c.paper)) {
        Box(Modifier.fillMaxWidth().height(1.dp).background(c.line))
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
                Btn(if (allSelected) "Clear" else "Select all", onToggleAll, style = BtnStyle.Ghost)
            }
            Text("$selectedCount selected", style = CrumbText.label, color = c.ink)
            Box(Modifier.weight(1f), contentAlignment = Alignment.CenterEnd) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Btn("Cookbook", onCookbook, style = BtnStyle.Soft, icon = Lucide.BookPlus, enabled = selectedCount > 0)
                    DeleteIconButton(onDelete, enabled = selectedCount > 0)
                }
            }
        }
    }
}

/** `.btn-outline.btn-icon.text-error`: an outline square holding a red Trash2. */
@Composable
private fun DeleteIconButton(onClick: () -> Unit, enabled: Boolean) {
    val c = Crumb.colors
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    Box(
        Modifier
            .size(44.dp)
            .clip(ControlShape)
            .background(if (pressed && enabled) c.tint else c.paper)
            .border(1.dp, if (enabled) c.lineStrong else c.line, ControlShape)
            .clickable(interaction, indication = null, enabled = enabled, role = Role.Button, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(Lucide.Trash2, contentDescription = "Delete selected", tint = if (enabled) c.error else c.inkMuted, modifier = Modifier.size(18.dp))
    }
}

/** One cookbook in the add-to-cookbook modal, with a tiny spine in its cloth colour. */
@Composable
private fun CookbookRow(book: CookbookListItem, enabled: Boolean, onClick: () -> Unit) {
    val c = Crumb.colors
    val spine = RoundedCornerShape(topStart = 3.dp, topEnd = 3.dp)
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .clip(ControlShape)
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
            .padding(horizontal = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(Modifier.size(width = 16.dp, height = 36.dp).clip(spine).background(bookLook(book.color).cloth).border(1.dp, c.ink.copy(alpha = 0.22f), spine))
        Column(Modifier.weight(1f)) {
            Text(book.name, style = CrumbText.rowTitle, color = c.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(recipeCountLabel(book.recipeCount.toInt()), style = CrumbText.meta, color = c.inkMuted)
        }
    }
}

/** A grid of skeleton cards while the first load runs (web/src/components/GridSkeleton.svelte). */
@Composable
private fun GridSkeleton() {
    LazyVerticalGrid(
        columns = GridCells.Fixed(2),
        contentPadding = PaddingValues(horizontal = 20.dp, vertical = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
        modifier = Modifier.fillMaxSize(),
    ) {
        items(List(4) { it }) {
            Column(Modifier.fillMaxWidth()) {
                Skeleton(Modifier.fillMaxWidth().aspectRatio(4f / 3f), ControlShape)
                VSpace(10.dp)
                Skeleton(Modifier.fillMaxWidth(0.8f).height(16.dp))
                VSpace(8.dp)
                Skeleton(Modifier.fillMaxWidth(0.4f).height(14.dp))
            }
        }
    }
}
