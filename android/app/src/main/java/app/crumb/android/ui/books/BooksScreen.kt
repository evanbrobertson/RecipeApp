package app.crumb.android.ui.books

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.outlined.CollectionsBookmark
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.crumb.android.data.Cookbook
import app.crumb.android.data.CookbookListItem
import app.crumb.android.data.Loaded
import app.crumb.android.data.RecipeRepository
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.LoadingViewModel
import app.crumb.android.ui.UiState
import app.crumb.android.ui.components.Loading
import app.crumb.android.ui.components.Message
import app.crumb.android.ui.components.OfflineNote
import app.crumb.android.ui.components.SecondaryButton
import app.crumb.android.ui.crumbViewModel
import app.crumb.android.ui.recipes.RecipeCard
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.DmSerif

/** Cover cloth and title foil, from web/src/lib/books.ts. The same in light and dark. */
data class BookLook(val cloth: Color, val foil: Color, val band: Color, val outlined: Boolean = false)

private val Forest = Color(0xFF1C2B22)
private val Tile = Color(0xFF2F6B4F)
private val Sage = Color(0xFFA9C4AE)
private val Butter = Color(0xFFF3DA8B)
private val Clay = Color(0xFFA55A40)
private val Cream = Color(0xFFF8F3E6)

fun bookLook(color: String?): BookLook = when (bookColor(color)) {
    "forest" -> BookLook(Forest, Butter, Butter)
    "sage" -> BookLook(Sage, Forest, Forest)
    "butter" -> BookLook(Butter, Forest, Forest)
    "clay" -> BookLook(Clay, Color(0xFFFFF8EA), Butter)
    "cream" -> BookLook(Cream, Forest, Tile, outlined = true)
    else -> BookLook(Tile, Color(0xFFFFFDF8), Butter)
}

/** Any stored colour name as one of the six, like `bookColor` in books.ts. */
fun bookColor(color: String?): String {
    val c = color.orEmpty().lowercase()
    return when (c) {
        "forest", "tile", "sage", "butter", "clay", "cream" -> c
        "tomato", "terracotta" -> "clay"
        "mustard" -> "butter"
        "ocean" -> "tile"
        "plum", "navy", "charcoal" -> "forest"
        "rose" -> "cream"
        else -> "tile"
    }
}

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

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BooksScreen(onOpen: (Long) -> Unit, onSignedOut: () -> Unit) {
    val vm = crumbViewModel { BooksViewModel(it.recipes) }
    val state by vm.state.collectAsStateWithLifecycle()
    LaunchedEffect(state) {
        if ((state as? UiState.Failed)?.signedOut == true) onSignedOut()
    }
    Column(Modifier.fillMaxSize().statusBarsPadding()) {
        Text(
            "Cookbooks",
            style = MaterialTheme.typography.headlineLarge,
            modifier = Modifier.padding(start = 20.dp, end = 20.dp, top = 16.dp, bottom = 4.dp),
        )
        when (val s = state) {
            UiState.Loading -> Loading()
            is UiState.Failed -> Message("Couldn't load your cookbooks", s.message, action = {
                SecondaryButton("Try again", onClick = { vm.load() })
            })
            is UiState.Ready -> PullToRefreshBox(isRefreshing = false, onRefresh = { vm.load(showSpinner = false) }) {
                if (s.value.isEmpty()) {
                    Message(
                        "No cookbooks yet",
                        "Gather recipes into cookbooks from Crumb on the web.",
                        icon = Icons.Outlined.CollectionsBookmark,
                    )
                } else {
                    LazyVerticalGrid(
                        columns = GridCells.Adaptive(140.dp),
                        contentPadding = PaddingValues(16.dp),
                        horizontalArrangement = Arrangement.spacedBy(16.dp),
                        verticalArrangement = Arrangement.spacedBy(20.dp),
                        modifier = Modifier.fillMaxSize().testTag("book-grid"),
                    ) {
                        if (s.offline) item(span = { GridItemSpan(maxLineSpan) }) { OfflineNote() }
                        items(s.value, key = { it.id }) { book -> BookCover(book) { onOpen(book.id) } }
                    }
                }
            }
        }
    }
}

@Composable
private fun BookCover(book: CookbookListItem, onClick: () -> Unit) {
    val look = bookLook(book.color)
    val colors = Crumb.colors
    Column(Modifier.fillMaxWidth()) {
        Surface(
            onClick = onClick,
            // Book spine: square corners on the left, rounded on the right
            shape = RoundedCornerShape(topStart = 3.dp, bottomStart = 3.dp, topEnd = 12.dp, bottomEnd = 12.dp),
            color = look.cloth,
            border = if (look.outlined) BorderStroke(1.dp, Forest.copy(alpha = 0.22f)) else null,
            modifier = Modifier.fillMaxWidth().aspectRatio(3f / 4f),
        ) {
            Row {
                Box(Modifier.fillMaxHeight().width(14.dp).background(Color.Black.copy(alpha = 0.12f)))
                Box(Modifier.fillMaxHeight().width(3.dp).background(look.band.copy(alpha = 0.8f)))
                Box(Modifier.fillMaxSize().padding(14.dp), contentAlignment = Alignment.Center) {
                    Text(
                        book.name,
                        fontFamily = DmSerif,
                        style = MaterialTheme.typography.titleLarge,
                        color = look.foil,
                        maxLines = 4,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
        Text(
            if (book.recipeCount == 1L) "1 recipe" else "${book.recipeCount} recipes",
            style = MaterialTheme.typography.bodySmall,
            color = colors.inkMuted,
            modifier = Modifier.padding(top = 6.dp, start = 2.dp),
        )
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
