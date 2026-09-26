package app.crumb.android.ui.recipe

import android.content.Intent
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.outlined.CheckBox
import androidx.compose.material.icons.outlined.CheckBoxOutlineBlank
import androidx.compose.material.icons.outlined.LocalDining
import androidx.compose.material.icons.outlined.OpenInBrowser
import androidx.compose.material.icons.outlined.Restaurant
import androidx.compose.material.icons.outlined.Schedule
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.core.net.toUri
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.crumb.android.data.Durations
import app.crumb.android.data.Loaded
import app.crumb.android.data.Recipe
import app.crumb.android.data.RecipeRepository
import app.crumb.android.data.Section
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.LoadingViewModel
import app.crumb.android.ui.UiState
import app.crumb.android.ui.components.Loading
import app.crumb.android.ui.components.Message
import app.crumb.android.ui.components.OfflineNote
import app.crumb.android.ui.components.Pill
import app.crumb.android.ui.components.PrimaryButton
import app.crumb.android.ui.components.RecipePhoto
import app.crumb.android.ui.components.SecondaryButton
import app.crumb.android.ui.crumbViewModel
import app.crumb.android.ui.theme.Caveat
import app.crumb.android.ui.theme.Crumb

class RecipeViewModel(private val repo: RecipeRepository, private val id: Long) : LoadingViewModel<Recipe>() {
    init {
        load()
    }

    override suspend fun fetch(): Loaded<Recipe> = repo.recipe(id)
}

@Composable
fun recipeViewModel(id: Long): RecipeViewModel = crumbViewModel(key = "recipe-$id") { RecipeViewModel(it.recipes, id) }

@Composable
fun RecipeScreen(id: Long, onBack: () -> Unit, onCook: () -> Unit, onSignedOut: () -> Unit) {
    val vm = recipeViewModel(id)
    val state by vm.state.collectAsStateWithLifecycle()
    LaunchedEffect(state) {
        if ((state as? UiState.Failed)?.signedOut == true) onSignedOut()
    }
    Box(Modifier.fillMaxSize()) {
        when (val s = state) {
            UiState.Loading -> Loading()
            is UiState.Failed -> Message(
                "Couldn't open this recipe",
                s.message,
                action = { SecondaryButton("Try again", onClick = { vm.load() }) },
            )
            is UiState.Ready -> RecipeContent(s.value, s.offline, onCook)
        }
        FilledTonalIconButton(
            onClick = onBack,
            colors = IconButtonDefaults.filledTonalIconButtonColors(containerColor = Crumb.colors.paper.copy(alpha = 0.92f)),
            modifier = Modifier.statusBarsPadding().padding(12.dp).testTag("back"),
        ) {
            Icon(Icons.AutoMirrored.Outlined.ArrowBack, contentDescription = "Back")
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun RecipeContent(recipe: Recipe, offline: Boolean, onCook: () -> Unit) {
    val colors = Crumb.colors
    val context = LocalContext.current
    val container = AppContainerProvider
    val widthPx = LocalWindowInfo.current.containerSize.width
    val photo = container.recipes.photoUrl(recipe.id, recipe.image, widthPx)
    // Ticked ingredients, as "section:item" indexes; survives rotation
    var ticked by rememberSaveable(recipe.id) { mutableStateOf(setOf<String>()) }

    LazyColumn(Modifier.fillMaxSize().testTag("recipe"), horizontalAlignment = Alignment.CenterHorizontally) {
        item {
            if (photo != null) {
                RecipePhoto(photo, recipe.image, Modifier.fillMaxWidth().aspectRatio(4f / 3f), contentDescription = recipe.title)
            } else {
                Box(Modifier.statusBarsPadding().padding(top = 56.dp))
            }
        }
        item {
            Column(
                Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(20.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                if (offline) OfflineNote()
                Text(recipe.title, style = MaterialTheme.typography.headlineMedium)
                val pills = listOfNotNull(
                    Durations.display(recipe.totalTime)?.let { "Total $it" to Icons.Outlined.Schedule },
                    Durations.display(recipe.prepTime)?.let { "Prep $it" to null },
                    Durations.display(recipe.cookTime)?.let { "Cook $it" to null },
                    recipe.recipeYield?.takeIf { it.isNotBlank() }?.let { it to Icons.Outlined.LocalDining },
                    recipe.recipeCategory?.let { it to null },
                )
                if (pills.isNotEmpty()) {
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        pills.forEach { (text, icon) -> Pill(text, icon = icon) }
                    }
                }
                recipe.description?.takeIf { it.isNotBlank() }?.let {
                    Text(it, style = MaterialTheme.typography.bodyLarge, color = colors.inkMuted)
                }
                if (recipe.instructions.any { it.items.isNotEmpty() }) {
                    PrimaryButton(
                        "Start cooking",
                        onClick = onCook,
                        icon = Icons.Outlined.Restaurant,
                        modifier = Modifier.fillMaxWidth().testTag("start-cooking"),
                    )
                }
            }
        }
        heading("Ingredients")
        sections(recipe.ingredients) { sectionIndex, itemIndex, text ->
            val key = "$sectionIndex:$itemIndex"
            val done = key in ticked
            Row(
                Modifier
                    .widthIn(max = 720.dp)
                    .fillMaxWidth()
                    .heightIn(min = 48.dp)
                    .clickable(role = Role.Checkbox) { ticked = if (done) ticked - key else ticked + key }
                    .padding(horizontal = 20.dp, vertical = 8.dp),
                verticalAlignment = Alignment.Top,
            ) {
                Icon(
                    if (done) Icons.Outlined.CheckBox else Icons.Outlined.CheckBoxOutlineBlank,
                    contentDescription = null,
                    tint = if (done) colors.primary else colors.inkMuted,
                    modifier = Modifier.padding(end = 12.dp, top = 1.dp).size(22.dp),
                )
                Text(
                    text,
                    style = MaterialTheme.typography.bodyLarge,
                    color = if (done) colors.inkMuted else colors.ink,
                    textDecoration = if (done) TextDecoration.LineThrough else null,
                )
            }
        }
        heading("Method")
        sections(recipe.instructions) { _, itemIndex, text ->
            Row(
                Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(horizontal = 20.dp, vertical = 10.dp),
                verticalAlignment = Alignment.Top,
            ) {
                Surface(shape = CircleShape, color = colors.tile, modifier = Modifier.padding(end = 14.dp).size(30.dp)) {
                    Box(contentAlignment = Alignment.Center) {
                        Text("${itemIndex + 1}", style = MaterialTheme.typography.labelMedium, color = colors.onTile)
                    }
                }
                Text(text, style = MaterialTheme.typography.bodyLarge)
            }
        }
        recipe.notes?.takeIf { it.isNotBlank() }?.let { notes ->
            heading("Cook's notes")
            item {
                Text(
                    notes,
                    fontFamily = Caveat,
                    style = MaterialTheme.typography.headlineSmall,
                    color = colors.primary,
                    modifier = Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(horizontal = 20.dp),
                )
            }
        }
        recipe.url?.let { url ->
            item {
                Column(Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(20.dp)) {
                    SecondaryButton(
                        "View original" + (url.toUri().host?.removePrefix("www.")?.let { " on $it" } ?: ""),
                        onClick = { context.startActivity(Intent(Intent.ACTION_VIEW, url.toUri())) },
                        icon = Icons.Outlined.OpenInBrowser,
                    )
                }
            }
        }
        item { Box(Modifier.navigationBarsPadding().padding(bottom = 24.dp)) }
    }
}

private fun LazyListScope.heading(text: String) {
    item {
        Text(
            text,
            style = MaterialTheme.typography.titleLarge,
            modifier = Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 20.dp, bottom = 6.dp),
        )
    }
}

private fun LazyListScope.sections(
    sections: List<Section>,
    row: @Composable (sectionIndex: Int, itemIndex: Int, text: String) -> Unit,
) {
    sections.forEachIndexed { sectionIndex, section ->
        section.name?.takeIf { it.isNotBlank() }?.let { name ->
            item(key = "section-$sectionIndex-${sections.hashCode()}") {
                Text(
                    name,
                    style = MaterialTheme.typography.titleSmall,
                    color = Crumb.colors.primary,
                    modifier = Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 12.dp, bottom = 2.dp),
                )
            }
        }
        itemsIndexed(section.items) { itemIndex, text -> row(sectionIndex, itemIndex, text) }
    }
}
