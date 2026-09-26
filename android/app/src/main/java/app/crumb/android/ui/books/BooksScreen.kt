package app.crumb.android.ui.books

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.crumb.android.data.Cookbook
import app.crumb.android.data.CookbookListItem
import app.crumb.android.data.Loaded
import app.crumb.android.data.RecipeRepository
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.LoadingViewModel
import app.crumb.android.ui.UiState
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.CardShape
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbInput
import app.crumb.android.ui.components.CrumbModal
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.EmptyState
import app.crumb.android.ui.components.FieldLabel
import app.crumb.android.ui.components.Loading
import app.crumb.android.ui.components.Message
import app.crumb.android.ui.components.OfflineNote
import app.crumb.android.ui.components.SecondaryButton
import app.crumb.android.ui.components.Skeleton
import app.crumb.android.ui.components.ToastTone
import app.crumb.android.ui.components.Toaster
import app.crumb.android.ui.components.VSpace
import app.crumb.android.ui.crumbViewModel
import app.crumb.android.ui.friendlyMessage
import app.crumb.android.ui.recipes.RecipeCard
import app.crumb.android.ui.theme.Crumb
import com.composables.icons.lucide.Check
import com.composables.icons.lucide.LibraryBig
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Plus
import kotlinx.coroutines.launch

class BooksViewModel(private val repo: RecipeRepository) : LoadingViewModel<List<CookbookListItem>>() {
    init {
        load()
    }

    override suspend fun fetch(): Loaded<List<CookbookListItem>> = repo.cookbooks()
}

class BookViewModel(private val repo: RecipeRepository, private val id: Long) : LoadingViewModel<Cookbook>() {
    init {
        load()
    }

    override suspend fun fetch(): Loaded<Cookbook> = repo.cookbook(id)
}

/** "Your shelf": the cookbooks as a stack of books (web/src/islands/ShelfPage.svelte). */
@Composable
fun ShelfScreen(onOpenRecipe: (Long) -> Unit, onOpenCookbook: (Long) -> Unit, onSignedOut: () -> Unit) {
    val vm = crumbViewModel { BooksViewModel(it.recipes) }
    val state by vm.state.collectAsStateWithLifecycle()
    val container = AppContainerProvider
    val scope = rememberCoroutineScope()
    val c = Crumb.colors
    var opened by remember { mutableStateOf<CookbookListItem?>(null) }
    var creating by remember { mutableStateOf(false) }
    LaunchedEffect(state) {
        if ((state as? UiState.Failed)?.signedOut == true) onSignedOut()
    }
    val books = (state as? UiState.Ready)?.value.orEmpty()

    Column(
        Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .statusBarsPadding()
            .padding(horizontal = 20.dp, vertical = 24.dp),
    ) {
        Row(Modifier.fillMaxWidth().padding(bottom = 28.dp), verticalAlignment = Alignment.Bottom) {
            Column(Modifier.weight(1f)) {
                Text("Your shelf", style = CrumbText.pageTitle, color = c.ink)
                Text(
                    "Pull a book off the shelf to open it.",
                    style = CrumbText.bodySmall, color = c.inkMuted, modifier = Modifier.padding(top = 8.dp),
                )
            }
            // An empty shelf has its own call to action
            if (books.isNotEmpty()) {
                Btn("New book", { creating = true }, style = BtnStyle.Primary, icon = Lucide.Plus, modifier = Modifier.padding(start = 12.dp))
            }
        }
        when (val s = state) {
            UiState.Loading -> Skeleton(Modifier.fillMaxWidth().height(208.dp), CardShape)
            is UiState.Failed -> Message("Couldn't load your shelf", s.message, action = {
                Btn("Try again", { vm.load() }, style = BtnStyle.Outline)
            })
            is UiState.Ready -> {
                if (s.offline) OfflineNote(Modifier.padding(bottom = 16.dp))
                if (s.value.isEmpty()) {
                    EmptyState(
                        icon = Lucide.LibraryBig,
                        title = "An empty shelf",
                        description = "Cookbooks group recipes, like “Weeknight dinners” or “Christmas baking”.",
                    ) {
                        Btn("Make your first cookbook", { creating = true }, style = BtnStyle.Primary)
                    }
                } else {
                    Box(Modifier.fillMaxWidth().clip(CardShape).background(c.tint).padding(top = 20.dp)) {
                        Bookshelf(
                            books = s.value,
                            pulledId = opened?.id,
                            onOpen = { opened = it },
                            onAdd = { creating = true },
                        )
                    }
                }
            }
        }
    }

    opened?.let { book ->
        OpenBook(
            book = book,
            load = { container.recipes.cookbook(it).value },
            onOpenRecipe = { opened = null; onOpenRecipe(it) },
            onOpenCookbook = { opened = null; onOpenCookbook(it) },
            onClose = { opened = null },
        )
    }

    if (creating) {
        NewBookModal(
            onDismiss = { creating = false },
            onCreate = { name, description, color ->
                scope.launch {
                    try {
                        val book = container.api.createCookbook(name, description, color)
                        creating = false
                        vm.load(showSpinner = false)
                        Toaster.show("“${book.name}” is on the shelf", tone = ToastTone.Success)
                    } catch (e: Exception) {
                        Toaster.show("Couldn't create cookbook", e.friendlyMessage(), ToastTone.Error)
                    }
                }
            },
        )
    }
}

/** "New book": name, description and a cover colour (a random one to start). */
@Composable
fun NewBookModal(onDismiss: () -> Unit, onCreate: suspend (String, String, String) -> Unit) {
    var name by remember { mutableStateOf("") }
    var description by remember { mutableStateOf("") }
    var color by remember { mutableStateOf(randomBookColor()) }
    var busy by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    CrumbModal(
        title = "New book",
        onDismiss = onDismiss,
        footer = {
            Btn("Cancel", onDismiss, style = BtnStyle.Ghost)
            Btn(
                "Put it on the shelf",
                { busy = true; scope.launch { onCreate(name.trim(), description.trim(), color); busy = false } },
                style = BtnStyle.Primary,
                enabled = !busy && name.isNotBlank(),
            )
        },
    ) {
        FieldLabel("Name")
        CrumbInput(name, { name = it }, placeholder = "Weeknight dinners", modifier = Modifier.focusRequester(focus))
        VSpace(16.dp)
        FieldLabel("Description")
        CrumbInput(description, { description = it }, placeholder = "Optional", minLines = 2)
        VSpace(16.dp)
        FieldLabel("Cover colour")
        BookColorPicker(color, { color = it })
    }
}

/** Six little books standing on their feet; the chosen one stands a little taller with a tick. */
@Composable
fun BookColorPicker(value: String, onChange: (String) -> Unit) {
    val c = Crumb.colors
    val current = bookColor(value)
    Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        BookColors.forEach { name ->
            val look = bookLook(name)
            val selected = name == current
            val lift by animateDpAsState(if (selected) (-3).dp else 0.dp, tween(180), label = "lift")
            Box(
                Modifier
                    .size(48.dp, 56.dp)
                    .clip(ControlShape)
                    .selectable(selected, role = Role.RadioButton) { onChange(name) }
                    .semantics { contentDescription = name.replaceFirstChar { it.uppercase() } }
                    .padding(bottom = 6.dp),
                contentAlignment = Alignment.BottomCenter,
            ) {
                Box(
                    Modifier
                        .offset(y = lift)
                        .size(26.dp, 40.dp)
                        .clip(RoundedCornerShape(topStart = 3.dp, topEnd = 3.dp))
                        .background(look.cloth)
                        .border(1.dp, c.ink.copy(alpha = if (look.outlined) 0.22f else 0.16f), RoundedCornerShape(topStart = 3.dp, topEnd = 3.dp)),
                )
                if (selected) {
                    Box(
                        Modifier
                            .align(Alignment.TopEnd)
                            .offset(x = (-4).dp, y = (-4).dp)
                            .size(24.dp)
                            .clip(CircleShape)
                            .background(c.paper)
                            .padding(2.dp)
                            .clip(CircleShape)
                            .background(c.tile),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(Lucide.Check, null, tint = c.onTile, modifier = Modifier.size(14.dp))
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BookScreen(id: Long, onBack: () -> Unit, onOpenRecipe: (Long) -> Unit, onSignedOut: () -> Unit) {
    val vm = crumbViewModel(key = "book-$id") { BookViewModel(it.recipes, id) }
    val state by vm.state.collectAsStateWithLifecycle()
    val container = AppContainerProvider
    val photoPx = with(LocalDensity.current) { 200.dp.roundToPx() }
    LaunchedEffect(state) {
        if ((state as? UiState.Failed)?.signedOut == true) onSignedOut()
    }
    Column(Modifier.fillMaxSize().statusBarsPadding()) {
        Row(Modifier.fillMaxWidth().padding(4.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = onBack, modifier = Modifier.testTag("back")) {
                Icon(Icons.AutoMirrored.Outlined.ArrowBack, contentDescription = "Back")
            }
        }
        when (val s = state) {
            UiState.Loading -> Loading()
            is UiState.Failed -> Message("Couldn't open this cookbook", s.message, action = {
                SecondaryButton("Try again", onClick = { vm.load() })
            })
            is UiState.Ready -> {
                val book = s.value
                LazyVerticalGrid(
                    columns = GridCells.Adaptive(150.dp),
                    contentPadding = PaddingValues(16.dp),
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                    modifier = Modifier.fillMaxSize(),
                ) {
                    item(span = { GridItemSpan(maxLineSpan) }) {
                        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                            if (s.offline) OfflineNote()
                            Text(book.name, style = MaterialTheme.typography.headlineMedium)
                            book.description?.takeIf { it.isNotBlank() }?.let {
                                Text(it, style = MaterialTheme.typography.bodyLarge, color = Crumb.colors.inkMuted)
                            }
                        }
                    }
                    if (book.recipes.isEmpty()) {
                        item(span = { GridItemSpan(maxLineSpan) }) {
                            Text("No recipes in this cookbook yet.", color = Crumb.colors.inkMuted)
                        }
                    }
                    items(book.recipes, key = { it.id }) { recipe ->
                        RecipeCard(recipe, container.recipes.photoUrl(recipe.id, recipe.image, photoPx)) {
                            onOpenRecipe(recipe.id)
                        }
                    }
                }
            }
        }
    }
}
