package app.crumb.android.ui.edit

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
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
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.crumb.android.AppContainer
import app.crumb.android.data.ApiException
import app.crumb.android.data.Loaded
import app.crumb.android.data.Recipe
import app.crumb.android.data.RecipeChecks
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.LoadingViewModel
import app.crumb.android.ui.LocalNav
import app.crumb.android.ui.UiState
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.CardShape
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.Message
import app.crumb.android.ui.components.Skeleton
import app.crumb.android.ui.components.ToastTone
import app.crumb.android.ui.components.Toaster
import app.crumb.android.ui.components.VSpace
import app.crumb.android.ui.crumbViewModel
import app.crumb.android.ui.friendlyMessage
import app.crumb.android.ui.theme.Crumb
import com.composables.icons.lucide.ArrowLeft
import com.composables.icons.lucide.Lucide
import kotlinx.coroutines.launch

/** The recipe and Wee Chef's check of it, as the edit page loads them. */
data class EditData(val recipe: Recipe, val checks: RecipeChecks?)

/** Loads the recipe through the offline cache, with its check alongside (tolerating none). */
class EditViewModel(container: AppContainer, private val recipeId: Long) : LoadingViewModel<EditData>() {
    private val repo = container.recipes
    private val api = container.api

    init {
        load()
    }

    override suspend fun fetch(): Loaded<EditData> {
        val loaded = repo.recipe(recipeId)
        val checks = runCatching { api.recipeChecks(recipeId) }.getOrNull()
        return Loaded(EditData(loaded.value, checks), loaded.offline)
    }
}

/** Edit a recipe (web/src/islands/EditPage.svelte, spec §7.2). */
@Composable
fun EditScreen(id: Long) {
    val vm: EditViewModel = crumbViewModel(key = "edit-$id") { EditViewModel(it, id) }
    val state by vm.state.collectAsStateWithLifecycle()
    val nav = LocalNav.current

    LaunchedEffect(state) {
        if ((state as? UiState.Failed)?.signedOut == true) nav.signedOut()
    }

    Box(Modifier.fillMaxSize()) {
        when (val s = state) {
            UiState.Loading -> EditSkeleton()
            is UiState.Failed -> Message(
                "Couldn't open this recipe",
                s.message,
                action = { Btn("Try again", { vm.load() }, style = BtnStyle.Outline) },
            )
            is UiState.Ready -> EditContent(s.value)
        }
    }
}

@Composable
private fun EditContent(data: EditData) {
    val container = AppContainerProvider
    val nav = LocalNav.current
    val scope = rememberCoroutineScope()
    val c = Crumb.colors
    val id = data.recipe.id
    var saving by remember { mutableStateOf(false) }
    // "Keep as is" is remembered, so Wee Chef won't raise that line again
    var flags by remember(id) { mutableStateOf(data.checks?.flags.orEmpty()) }

    Column(
        Modifier.fillMaxSize().statusBarsPadding().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp),
    ) {
        VSpace(8.dp)
        BackLink(data.recipe.title) { nav.back() }
        Text("Edit recipe", style = CrumbText.pageTitle, color = c.ink, modifier = Modifier.padding(top = 4.dp, bottom = 28.dp))
        RecipeEditor(
            initial = recipeDraft(data.recipe),
            saving = saving,
            flags = flags,
            onDismissFlag = { flag ->
                // Optimistic: drop it now, put it back with a toast if the server says no
                flags = flags.filter { it.id != flag.id }
                scope.launch {
                    try {
                        container.api.dismissFlag(id, flag.id)
                    } catch (e: Exception) {
                        flags = flags + flag
                        Toaster.show("Couldn't save that", e.friendlyMessage(), ToastTone.Error)
                    }
                }
            },
            onSubmit = { fields ->
                saving = true
                scope.launch {
                    try {
                        container.api.updateRecipe(id, fields)
                        Toaster.show("Saved", tone = ToastTone.Success)
                        nav.back()
                    } catch (e: Exception) {
                        if ((e as? ApiException)?.isSignedOut == true) nav.signedOut()
                        else Toaster.show("Couldn't save", e.friendlyMessage(), ToastTone.Error)
                        saving = false
                    }
                }
            },
            onCancel = { nav.back() },
        )
        VSpace(24.dp)
    }
}

/** Write a recipe from scratch (web/src/islands/NewRecipePage.svelte, spec §7.1). */
@Composable
fun NewRecipeScreen(title: String?) {
    val container = AppContainerProvider
    val nav = LocalNav.current
    val scope = rememberCoroutineScope()
    val c = Crumb.colors
    var saving by remember { mutableStateOf(false) }

    Column(
        Modifier.fillMaxSize().statusBarsPadding().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp),
    ) {
        VSpace(8.dp)
        Text("New recipe", style = CrumbText.pageTitle, color = c.ink, modifier = Modifier.padding(bottom = 28.dp))
        RecipeEditor(
            initial = newDraft(title.orEmpty()),
            saving = saving,
            submitLabel = "Save recipe",
            onSubmit = { fields ->
                saving = true
                scope.launch {
                    try {
                        val recipe = container.api.createRecipe(fields)
                        nav.replaceWithRecipe(recipe.id)
                    } catch (e: Exception) {
                        if ((e as? ApiException)?.isSignedOut == true) nav.signedOut()
                        else Toaster.show("Couldn't save", e.friendlyMessage(), ToastTone.Error)
                        saving = false
                    }
                }
            },
            onCancel = { nav.back() },
        )
        VSpace(24.dp)
    }
}

/** "← Recipe title", muted and truncated, back to the recipe. */
@Composable
private fun BackLink(title: String, onClick: () -> Unit) {
    val c = Crumb.colors
    Row(
        Modifier
            .clip(ControlShape)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Icon(Lucide.ArrowLeft, null, tint = c.inkMuted, modifier = Modifier.size(18.dp))
        Text(
            title,
            style = CrumbText.bodySmall.copy(fontSize = 15.sp, fontWeight = FontWeight.SemiBold),
            color = c.inkMuted,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/** The web's loading skeleton (h-5 w-40, h-9 w-1/2, h-64 w-full). */
@Composable
private fun EditSkeleton() {
    Column(
        Modifier.fillMaxSize().statusBarsPadding().padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Skeleton(Modifier.width(160.dp).height(20.dp))
        Skeleton(Modifier.fillMaxWidth(0.5f).height(36.dp))
        Skeleton(Modifier.fillMaxWidth().height(256.dp), shape = CardShape)
    }
}
