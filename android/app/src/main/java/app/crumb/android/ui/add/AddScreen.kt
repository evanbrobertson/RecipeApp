package app.crumb.android.ui.add

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.ContentPaste
import androidx.compose.material.icons.outlined.Download
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.createSavedStateHandle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.crumb.android.data.ImportInput
import app.crumb.android.data.ImportResult
import app.crumb.android.data.RecipeRepository
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.PrimaryButton
import app.crumb.android.ui.components.SecondaryButton
import app.crumb.android.ui.friendlyMessage
import app.crumb.android.ui.theme.Crumb
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

data class AddState(val busy: Boolean = false, val error: String? = null, val saved: ImportResult? = null)

class AddViewModel(private val repo: RecipeRepository, private val saved: SavedStateHandle) : ViewModel() {
    /** The draft survives rotation and the app being closed in the background. */
    val draft = saved.getStateFlow("draft", "")
    private val _state = MutableStateFlow(AddState())
    val state = _state.asStateFlow()

    fun setDraft(text: String) {
        saved["draft"] = text
        if (_state.value.error != null) _state.value = _state.value.copy(error = null)
    }

    fun save() {
        val input = ImportInput.classify(draft.value) ?: return
        _state.value = AddState(busy = true)
        viewModelScope.launch {
            _state.value = try {
                val result = when (input) {
                    is ImportInput.Link -> repo.importLink(input.url)
                    is ImportInput.Text -> repo.importText(input.text)
                }
                saved["draft"] = ""
                AddState(saved = result)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                AddState(error = e.friendlyMessage())
            }
        }
    }

    fun consumeSaved() {
        _state.value = _state.value.copy(saved = null)
    }
}

/**
 * Save a recipe from a link or pasted text. [shared] is text from Share → Crumb; it
 * replaces the draft once, then [onSharedUsed] clears it.
 */
@Composable
fun AddScreen(
    shared: String?,
    onSharedUsed: () -> Unit,
    onSaved: (recipeId: Long, cookbookId: Long?) -> Unit,
) {
    val container = AppContainerProvider
    val vm: AddViewModel = viewModel(
        factory = viewModelFactory { initializer { AddViewModel(container.recipes, createSavedStateHandle()) } },
    )
    val draft by vm.draft.collectAsStateWithLifecycle()
    val state by vm.state.collectAsStateWithLifecycle()
    val colors = Crumb.colors
    val clipboard = LocalClipboard.current
    val scope = rememberCoroutineScope()

    LaunchedEffect(shared) {
        if (shared != null) {
            vm.setDraft(shared)
            onSharedUsed()
        }
    }
    LaunchedEffect(state.saved) {
        state.saved?.let {
            vm.consumeSaved()
            onSaved(it.id, it.cookbook?.id)
        }
    }

    val input = ImportInput.classify(draft)
    Column(
        Modifier.fillMaxSize().statusBarsPadding().imePadding().verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Text("Add a recipe", style = MaterialTheme.typography.headlineLarge)
        Text(
            "Paste a link to a recipe, or the recipe itself. You can also share a page to Crumb from your browser.",
            style = MaterialTheme.typography.bodyMedium,
            color = colors.inkMuted,
        )
        OutlinedTextField(
            value = draft,
            onValueChange = vm::setDraft,
            placeholder = { Text("https://… or paste the ingredients and method") },
            shape = ControlShape,
            enabled = !state.busy,
            colors = OutlinedTextFieldDefaults.colors(
                unfocusedContainerColor = colors.paper,
                focusedContainerColor = colors.paper,
                disabledContainerColor = colors.sunk,
                unfocusedBorderColor = colors.line,
            ),
            modifier = Modifier.widthIn(max = 720.dp).fillMaxWidth().heightIn(min = 160.dp).testTag("import-draft"),
        )
        state.error?.let { Text(it, color = colors.error, style = MaterialTheme.typography.bodyMedium) }
        PrimaryButton(
            text = when {
                state.busy && input is ImportInput.Link -> "Fetching the recipe…"
                state.busy -> "Reading the recipe…"
                input is ImportInput.Text -> "Save pasted recipe"
                else -> "Save recipe"
            },
            icon = Icons.Outlined.Download,
            enabled = input != null && !state.busy,
            onClick = vm::save,
            modifier = Modifier.widthIn(max = 720.dp).fillMaxWidth().testTag("import-save"),
        )
        if (draft.isEmpty()) {
            SecondaryButton(
                "Paste",
                icon = Icons.Outlined.ContentPaste,
                onClick = {
                    scope.launch {
                        val clip = clipboard.getClipEntry()?.clipData
                        val text = clip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.text?.toString()
                        if (!text.isNullOrBlank()) vm.setDraft(text)
                    }
                },
            )
        }
    }
}
