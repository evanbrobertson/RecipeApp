package app.crumb.android.ui.recipes

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.automirrored.outlined.MenuBook
import androidx.compose.material.icons.outlined.Schedule
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.createSavedStateHandle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.crumb.android.data.Durations
import app.crumb.android.data.Loaded
import app.crumb.android.data.RecipeRepository
import app.crumb.android.data.RecipeSummary
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.LoadingViewModel
import app.crumb.android.ui.UiState
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.Loading
import app.crumb.android.ui.components.Message
import app.crumb.android.ui.components.OfflineNote
import app.crumb.android.ui.components.PaperCard
import app.crumb.android.ui.components.RecipePhoto
import app.crumb.android.ui.components.SecondaryButton
import app.crumb.android.ui.theme.Crumb
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.onEach

@OptIn(FlowPreview::class)
class RecipesViewModel(
    private val repo: RecipeRepository,
    private val saved: SavedStateHandle,
) : LoadingViewModel<List<RecipeSummary>>() {
    val query: StateFlow<String> = saved.getStateFlow("q", "")

    init {
        load()
        query.drop(1).debounce(250).distinctUntilChanged().onEach { load(showSpinner = false) }.launchIn(viewModelScope)
    }

    fun setQuery(text: String) {
        saved["q"] = text
    }

    override suspend fun fetch(): Loaded<List<RecipeSummary>> = repo.recipes(query.value)
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RecipesScreen(onOpen: (Long) -> Unit, onSignedOut: () -> Unit) {
    val container = AppContainerProvider
    val vm: RecipesViewModel = viewModel(
        factory = viewModelFactory { initializer { RecipesViewModel(container.recipes, createSavedStateHandle()) } },
    )
    val state by vm.state.collectAsStateWithLifecycle()
    val query by vm.query.collectAsStateWithLifecycle()
    val colors = Crumb.colors
    // Photo width in pixels for a two-column card
    val photoPx = with(LocalDensity.current) { 200.dp.roundToPx() }

    LaunchedEffect(state) {
        if ((state as? UiState.Failed)?.signedOut == true) onSignedOut()
    }

    Column(Modifier.fillMaxSize().statusBarsPadding()) {
        Text(
            "Recipes",
            style = MaterialTheme.typography.headlineLarge,
            modifier = Modifier.padding(start = 20.dp, end = 20.dp, top = 16.dp, bottom = 8.dp),
        )
        OutlinedTextField(
            value = query,
            onValueChange = vm::setQuery,
            placeholder = { Text("Search recipes") },
            leadingIcon = { Icon(Icons.Outlined.Search, contentDescription = null) },
            trailingIcon = {
                if (query.isNotEmpty()) {
                    IconButton(onClick = { vm.setQuery("") }) {
                        Icon(Icons.Outlined.Close, contentDescription = "Clear search")
                    }
                }
            },
            singleLine = true,
            shape = ControlShape,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            colors = OutlinedTextFieldDefaults.colors(
                unfocusedContainerColor = colors.paper,
                focusedContainerColor = colors.paper,
                unfocusedBorderColor = colors.line,
            ),
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp).testTag("search"),
        )

        when (val s = state) {
            UiState.Loading -> Loading()
            is UiState.Failed -> Message(
                "Couldn't load your recipes",
                s.message,
                action = { SecondaryButton("Try again", onClick = { vm.load() }) },
            )
            is UiState.Ready -> PullToRefreshBox(
                isRefreshing = false,
                onRefresh = { vm.load(showSpinner = false) },
                modifier = Modifier.fillMaxSize(),
            ) {
                if (s.value.isEmpty()) {
                    Message(
                        if (query.isBlank()) "Your recipe box is empty" else "Nothing matches “${query.trim()}”",
                        if (query.isBlank()) "Tap Add to save a recipe from a link or paste one in." else null,
                        icon = Icons.AutoMirrored.Outlined.MenuBook,
                    )
                } else {
                    LazyVerticalGrid(
                        columns = GridCells.Adaptive(minSize = 150.dp),
                        contentPadding = PaddingValues(16.dp),
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                        modifier = Modifier.fillMaxSize().testTag("recipe-grid"),
                    ) {
                        if (s.offline) {
                            item(span = { GridItemSpan(maxLineSpan) }) { OfflineNote() }
                        }
                        items(s.value, key = { it.id }) { recipe ->
                            RecipeCard(recipe, container.recipes.photoUrl(recipe.id, recipe.image, photoPx)) {
                                onOpen(recipe.id)
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun RecipeCard(recipe: RecipeSummary, photoUrl: String?, onClick: () -> Unit) {
    val colors = Crumb.colors
    PaperCard(onClick = onClick, modifier = Modifier.fillMaxWidth().testTag("recipe-card")) {
        Column {
            RecipePhoto(photoUrl, recipe.image, Modifier.fillMaxWidth().aspectRatio(4f / 3f).padding(6.dp))
            Column(Modifier.padding(start = 12.dp, end = 12.dp, bottom = 12.dp, top = 4.dp)) {
                Text(
                    recipe.title,
                    style = MaterialTheme.typography.titleSmall,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                val time = Durations.display(recipe.totalTime)
                val meta = listOfNotNull(time, recipe.recipeCategory).joinToString(" · ")
                if (meta.isNotEmpty()) {
                    Row(Modifier.padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                        if (time != null) {
                            Icon(
                                Icons.Outlined.Schedule,
                                contentDescription = null,
                                tint = colors.inkMuted,
                                modifier = Modifier.padding(end = 4.dp).size(14.dp),
                            )
                        }
                        Text(meta, style = MaterialTheme.typography.bodySmall, color = colors.inkMuted, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
        }
    }
}
