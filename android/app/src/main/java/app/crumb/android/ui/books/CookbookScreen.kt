package app.crumb.android.ui.books

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.crumb.android.data.Cookbook
import app.crumb.android.data.Loaded
import app.crumb.android.data.RecipeRepository
import app.crumb.android.data.RecipeSummary
import app.crumb.android.data.Share
import app.crumb.android.data.ShareKind
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.LoadingViewModel
import app.crumb.android.ui.LocalNav
import app.crumb.android.ui.UiState
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.Card
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbInput
import app.crumb.android.ui.components.CrumbMenu
import app.crumb.android.ui.components.CrumbModal
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.EmptyState
import app.crumb.android.ui.components.FieldLabel
import app.crumb.android.ui.components.MenuItem
import app.crumb.android.ui.components.Message
import app.crumb.android.ui.components.OfflineNote
import app.crumb.android.ui.components.Skeleton
import app.crumb.android.ui.components.ToastTone
import app.crumb.android.ui.components.Toaster
import app.crumb.android.ui.components.VSpace
import app.crumb.android.ui.crumbViewModel
import app.crumb.android.ui.friendlyMessage
import app.crumb.android.ui.recipes.RecipeCard
import app.crumb.android.ui.recipe.ShareSheet
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.DmSerif
import com.composables.icons.lucide.ArrowLeft
import com.composables.icons.lucide.BookOpen
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Pencil
import com.composables.icons.lucide.Share as ShareIcon
import com.composables.icons.lucide.Trash2
import com.composables.icons.lucide.X
import kotlinx.coroutines.launch

class BookViewModel(private val repo: RecipeRepository, private val id: Long) : LoadingViewModel<Cookbook>() {
    init {
        load()
    }

    override suspend fun fetch(): Loaded<Cookbook> = repo.cookbook(id)
}

/** The editable fields of the rename form (web's inline `form`). */
private data class CookbookForm(val name: String, val description: String, val color: String)

/** "1 recipe" / "3 recipes" (web's `{n} recipe{…s}`). */
internal fun recipesLabel(count: Int): String = "$count recipe" + if (count == 1) "" else "s"

/** The delete confirmation's second line; the share clause only when a link exists. */
internal fun deleteCookbookDescription(hasShare: Boolean): String =
    if (hasShare) "Recipes in it are kept. Its share link stops working." else "Recipes in it are kept."

/** The remove button's label for a screen reader. */
internal fun removeRecipeLabel(title: String): String = "Remove $title from cookbook"

/**
 * One cookbook (web/src/islands/CookbookPage.svelte): the standing spine and title with Share
 * and ⋯, an inline rename form, the recipe grid with a remove button on each card, the delete
 * confirmation, and the share sheet's cookbook copy. Reads through the offline cache.
 */
@Composable
fun CookbookScreen(id: Long, onBack: () -> Unit, onOpenRecipe: (Long) -> Unit, onSignedOut: () -> Unit) {
    val vm = crumbViewModel(key = "book-$id") { BookViewModel(it.recipes, id) }
    val state by vm.state.collectAsStateWithLifecycle()
    val container = AppContainerProvider
    val nav = LocalNav.current
    val scope = rememberCoroutineScope()
    val photoPx = with(LocalDensity.current) { 400.dp.roundToPx() }

    var editing by rememberSaveable { mutableStateOf(false) }
    var form by remember { mutableStateOf(CookbookForm("", "", "tile")) }
    var showDelete by rememberSaveable { mutableStateOf(false) }
    var showShare by rememberSaveable { mutableStateOf(false) }
    var share by remember { mutableStateOf<Share?>(null) }
    LaunchedEffect(id) {
        share = runCatching { container.api.existingShare(ShareKind.Cookbook, id) }.getOrNull()
    }
    var busy by remember { mutableStateOf(false) }

    LaunchedEffect(state) {
        if ((state as? UiState.Failed)?.signedOut == true) onSignedOut()
    }

    when (val s = state) {
        UiState.Loading -> CookbookSkeleton()
        is UiState.Failed -> Message(
            "Couldn't open this cookbook",
            s.message,
            modifier = Modifier.statusBarsPadding(),
            action = { Btn("Try again", { vm.load() }, style = BtnStyle.Outline) },
        )
        is UiState.Ready -> {
            val book = s.value
            LazyVerticalGrid(
                columns = GridCells.Fixed(2),
                modifier = Modifier.fillMaxSize().statusBarsPadding(),
                contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 4.dp, bottom = 32.dp),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalArrangement = Arrangement.spacedBy(20.dp),
            ) {
                item(span = { GridItemSpan(maxLineSpan) }) {
                    Column(Modifier.fillMaxWidth().padding(bottom = 8.dp)) {
                        if (s.offline) OfflineNote(Modifier.padding(bottom = 12.dp))
                        Btn(
                            "Shelf",
                            onBack,
                            style = BtnStyle.Ghost,
                            icon = Lucide.ArrowLeft,
                            padding = 12.dp,
                            modifier = Modifier.offset(x = (-12).dp),
                        )
                        VSpace(4.dp)
                        if (editing) {
                            CookbookEditForm(
                                form = form,
                                busy = busy,
                                onChange = { form = it },
                                onCancel = { editing = false },
                                onSave = {
                                    if (busy) return@CookbookEditForm
                                    scope.launch {
                                        busy = true
                                        try {
                                            container.api.updateCookbook(book.id, form.name.trim(), form.description.trim(), form.color)
                                            editing = false
                                            vm.load(showSpinner = false)
                                        } catch (e: Exception) {
                                            Toaster.show("Couldn't save", e.friendlyMessage(), ToastTone.Error)
                                        } finally {
                                            busy = false
                                        }
                                    }
                                },
                            )
                        } else {
                            CookbookHeader(
                                book = book,
                                onShare = { showShare = true },
                                onRename = {
                                    form = CookbookForm(book.name, book.description.orEmpty(), bookColor(book.color))
                                    editing = true
                                },
                                onDelete = { showDelete = true },
                            )
                            book.description?.takeIf { it.isNotBlank() }?.let { description ->
                                Text(
                                    description,
                                    style = CrumbText.body.copy(fontSize = 17.sp, lineHeight = 26.sp),
                                    color = Crumb.colors.inkMuted,
                                    modifier = Modifier.padding(top = 16.dp),
                                )
                            }
                        }
                    }
                }

                if (book.recipes.isEmpty()) {
                    item(span = { GridItemSpan(maxLineSpan) }) {
                        EmptyState(
                            icon = Lucide.BookOpen,
                            title = "This cookbook is empty",
                            description = "Open a recipe and tap the cookbook name, or use Select on the recipes page.",
                        ) {
                            Btn("Browse recipes", { nav.recipes() }, style = BtnStyle.Soft)
                        }
                    }
                } else {
                    items(book.recipes, key = { it.id }) { recipe ->
                        RecipeCardWithRemove(
                            recipe = recipe,
                            photoUrl = container.recipes.photoUrl(recipe.id, recipe.image, photoPx),
                            onOpen = { onOpenRecipe(recipe.id) },
                            onRemove = {
                                scope.launch {
                                    try {
                                        container.api.removeFromCookbook(book.id, recipe.id)
                                        vm.load(showSpinner = false)
                                    } catch (e: Exception) {
                                        Toaster.show("Couldn't remove recipe", e.friendlyMessage(), ToastTone.Error)
                                    }
                                }
                            },
                        )
                    }
                }
            }
        }
    }

    if (showShare) {
        val book = (state as? UiState.Ready)?.value ?: return
        ShareSheet(
            kind = ShareKind.Cookbook,
            id = book.id,
            title = book.name,
            share = share,
            onShareChange = { share = it },
            onDismiss = { showShare = false },
            cookbook = book,
        )
    }

    if (showDelete) {
        val book = (state as? UiState.Ready)?.value ?: return
        CrumbModal(
            title = "Delete this cookbook?",
            description = deleteCookbookDescription(hasShare = share != null),
            onDismiss = { showDelete = false },
            footer = {
                Btn("Cancel", { showDelete = false }, style = BtnStyle.Ghost)
                Btn(
                    "Delete",
                    {
                        if (busy) return@Btn
                        scope.launch {
                            busy = true
                            try {
                                container.api.deleteCookbook(book.id)
                                Toaster.show("Cookbook deleted")
                                showDelete = false
                                nav.back()
                            } catch (e: Exception) {
                                Toaster.show("Couldn't delete", e.friendlyMessage(), ToastTone.Error)
                            } finally {
                                busy = false
                            }
                        }
                    },
                    style = BtnStyle.Danger,
                    enabled = !busy,
                )
            },
        ) {}
    }
}

/** The spine, name, recipe count, Share and ⋯ (or the rename form, which replaces all this). */
@Composable
private fun CookbookHeader(book: Cookbook, onShare: () -> Unit, onRename: () -> Unit, onDelete: () -> Unit) {
    val c = Crumb.colors
    // The web keeps the buttons beside the title; a phone is too narrow for that, so they sit
    // under the count and the name gets the full width.
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Bottom) {
        StandingSpine(book.name, bookLook(book.color))
        Column(Modifier.weight(1f).padding(start = 16.dp)) {
            Text(book.name, style = CrumbText.pageTitle, color = c.ink)
            Text(recipesLabel(book.recipes.size), style = CrumbText.meta, color = c.inkMuted, modifier = Modifier.padding(top = 6.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(top = 12.dp)) {
                Btn(null, onShare, style = BtnStyle.Outline, icon = Lucide.ShareIcon, contentDescription = "Share")
                CrumbMenu(
                    groups = listOf(
                        listOf(MenuItem("Rename", Lucide.Pencil, onSelect = onRename)),
                        listOf(MenuItem("Delete cookbook", Lucide.Trash2, danger = true, onSelect = onDelete)),
                    ),
                )
            }
        }
    }
}

/**
 * The book's spine standing on a strip of the shelf plank: cloth with a darker fore-edge,
 * two foil bands 10dp from either end, an inset edge for pale cloths, and the title written
 * bottom-to-top. (CookbookPage.svelte's `.spine`.)
 */
@Composable
private fun StandingSpine(name: String, look: BookLook) {
    val c = Crumb.colors
    val spineShape = RoundedCornerShape(topStart = 3.dp, topEnd = 3.dp)
    val inset = if (c.dark) Color.White.copy(alpha = 0.14f) else c.ink.copy(alpha = 0.22f)
    Column(Modifier.width(48.dp)) {
        Box(
            Modifier
                .size(48.dp, 128.dp)
                .clip(spineShape)
                .background(look.cloth)
                .drawBehind {
                    drawRect(
                        Brush.horizontalGradient(
                            0f to Color.Transparent,
                            0.78f to Color.Transparent,
                            1f to look.shade.copy(alpha = 0.55f),
                        ),
                    )
                }
                .border(1.dp, inset, spineShape),
            contentAlignment = Alignment.Center,
        ) {
            Box(Modifier.align(Alignment.TopStart).fillMaxWidth().offset(y = 10.dp).height(2.dp).background(look.foil.copy(alpha = 0.85f)))
            Box(Modifier.align(Alignment.BottomStart).fillMaxWidth().offset(y = (-10).dp).height(2.dp).background(look.foil.copy(alpha = 0.85f)))
            Text(
                name,
                color = look.foil,
                fontFamily = DmSerif,
                fontSize = 15.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier
                    .width(100.dp)
                    .graphicsLayer { rotationZ = -90f },
            )
        }
        Box(
            Modifier
                .width(60.dp)
                .offset(x = (-6).dp)
                .height(8.dp)
                .clip(RoundedCornerShape(50))
                .background(c.tile),
        )
    }
}

/** The inline rename form: name, description, cover colour, Save/Cancel. */
@Composable
private fun CookbookEditForm(
    form: CookbookForm,
    busy: Boolean,
    onChange: (CookbookForm) -> Unit,
    onCancel: () -> Unit,
    onSave: () -> Unit,
) {
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    Card(Modifier.fillMaxWidth(), padding = PaddingValues(16.dp)) {
        FieldLabel("Name")
        CrumbInput(form.name, { onChange(form.copy(name = it)) }, modifier = Modifier.focusRequester(focus))
        VSpace(16.dp)
        FieldLabel("Description")
        CrumbInput(form.description, { onChange(form.copy(description = it)) }, placeholder = "Optional", minLines = 2)
        VSpace(16.dp)
        FieldLabel("Cover colour")
        BookColorPicker(form.color, { onChange(form.copy(color = it)) })
        Row(Modifier.padding(top = 20.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Btn("Save", onSave, style = BtnStyle.Primary, enabled = form.name.isNotBlank() && !busy)
            Btn("Cancel", onCancel, style = BtnStyle.Ghost)
        }
    }
}

/** A recipe card with the paper × remove button over its top-right corner. */
@Composable
private fun RecipeCardWithRemove(recipe: RecipeSummary, photoUrl: String?, onOpen: () -> Unit, onRemove: () -> Unit) {
    val c = Crumb.colors
    Box(Modifier.fillMaxWidth()) {
        RecipeCard(recipe, photoUrl, onClick = onOpen)
        Box(
            Modifier
                .align(Alignment.TopEnd)
                .padding(8.dp)
                .size(44.dp)
                .clip(CircleShape)
                .background(c.paper)
                .border(1.dp, c.line, CircleShape)
                .clickable(role = Role.Button, onClick = onRemove)
                .semantics { contentDescription = removeRecipeLabel(recipe.title) },
            contentAlignment = Alignment.Center,
        ) {
            Icon(Lucide.X, contentDescription = null, tint = c.ink, modifier = Modifier.size(18.dp))
        }
    }
}

/** The page's loading shape: a standing spine beside two title lines, then a 2-column grid. */
@Composable
private fun CookbookSkeleton() {
    Column(Modifier.fillMaxSize().statusBarsPadding().padding(horizontal = 20.dp, vertical = 4.dp)) {
        Row(Modifier.fillMaxWidth().padding(top = 12.dp), verticalAlignment = Alignment.Bottom) {
            Skeleton(Modifier.size(60.dp, 136.dp), RoundedCornerShape(topStart = 3.dp, topEnd = 3.dp))
            Column(Modifier.weight(1f).padding(start = 16.dp, bottom = 8.dp)) {
                Skeleton(Modifier.fillMaxWidth(0.6f).height(28.dp), ControlShape)
                VSpace(10.dp)
                Skeleton(Modifier.fillMaxWidth(0.3f).height(14.dp), ControlShape)
            }
        }
        VSpace(28.dp)
        repeat(2) {
            Row(Modifier.fillMaxWidth().padding(bottom = 20.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                repeat(2) {
                    Column(Modifier.weight(1f)) {
                        Skeleton(Modifier.fillMaxWidth().aspectRatio(4f / 3f), ControlShape)
                        VSpace(10.dp)
                        Skeleton(Modifier.fillMaxWidth(0.8f).height(16.dp), ControlShape)
                        VSpace(8.dp)
                        Skeleton(Modifier.fillMaxWidth(0.4f).height(14.dp), ControlShape)
                    }
                }
            }
        }
    }
}
