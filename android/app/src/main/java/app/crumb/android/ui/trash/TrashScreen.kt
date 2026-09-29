package app.crumb.android.ui.trash

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import app.crumb.android.AppContainer
import app.crumb.android.data.ApiException
import app.crumb.android.data.Trashed
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.LocalNav
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnSize
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.Card
import app.crumb.android.ui.components.CardShape
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbModal
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.ListCard
import app.crumb.android.ui.components.RecipePhoto
import app.crumb.android.ui.components.Skeleton
import app.crumb.android.ui.components.ToastAction
import app.crumb.android.ui.components.ToastTone
import app.crumb.android.ui.components.Toaster
import app.crumb.android.ui.components.VSpace
import app.crumb.android.ui.crumbViewModel
import app.crumb.android.ui.friendlyMessage
import app.crumb.android.ui.theme.Crumb
import com.composables.icons.lucide.ArrowLeft
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.RotateCcw
import com.composables.icons.lucide.Trash2
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** What the trash page shows; [items] is null until the list lands. */
data class TrashUiState(
    val items: List<Trashed>? = null,
    /** The recipe being put back or deleted, or [ALL] for emptying; the other buttons wait. */
    val busy: Long? = null,
    val signedOut: Boolean = false,
) {
    companion object {
        const val ALL = -1L
    }
}

class TrashViewModel(private val container: AppContainer) : ViewModel() {
    private val _state = MutableStateFlow(TrashUiState())
    val state: StateFlow<TrashUiState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            try {
                val items = container.api.trash()
                _state.update { it.copy(items = items) }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _state.update { it.copy(items = emptyList(), signedOut = (e as? ApiException)?.isSignedOut == true) }
                Toaster.show("Couldn't open the trash", e.friendlyMessage(), ToastTone.Error)
            }
        }
    }

    /** Puts [item] back; [open] goes to the recipe that came back (it may be another one). */
    fun restore(item: Trashed, open: (Long) -> Unit) = act(item.id, "Couldn't put it back") {
        val restored = container.recipes.restore(item.id)
        _state.update { s -> s.copy(items = s.items?.without(item.id)) }
        val (title, description) = restoredToast(restored, item.title)
        Toaster.show(title, description, ToastTone.Success, ToastAction("Open") { open(restored.id) })
    }

    fun purge(item: Trashed) = act(item.id, "Couldn't delete it") {
        container.api.purgeTrashed(item.id)
        _state.update { s -> s.copy(items = s.items?.without(item.id)) }
    }

    fun empty(onDone: () -> Unit) = act(TrashUiState.ALL, "Couldn't empty the trash") {
        container.api.emptyTrash()
        _state.update { it.copy(items = emptyList()) }
        onDone()
        Toaster.show("Trash emptied")
    }

    private fun act(busy: Long, failure: String, block: suspend () -> Unit) {
        _state.update { it.copy(busy = busy) }
        viewModelScope.launch {
            try {
                block()
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                Toaster.show(failure, e.friendlyMessage(), ToastTone.Error)
            } finally {
                _state.update { it.copy(busy = null) }
            }
        }
    }
}

/**
 * Undo on a "Moved to the trash" toast: puts [ids] back from the trash, then [onDone] gets the
 * recipes that came back. Runs on the app's scope, since the screen that deleted is often gone.
 */
fun undoDelete(container: AppContainer, ids: List<Long>, onDone: (List<Long>) -> Unit) {
    container.scope.launch {
        try {
            val back = ids.map { container.recipes.restore(it).id }
            Toaster.show(putBackTitle(ids.size))
            onDone(back)
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            Toaster.show("Couldn't put them back", e.friendlyMessage(), ToastTone.Error)
        }
    }
}

/** The trash (web/src/islands/TrashPage.svelte, pages/more/trash.astro). */
@Composable
fun TrashScreen() {
    val container = AppContainerProvider
    val nav = LocalNav.current
    val vm: TrashViewModel = crumbViewModel { TrashViewModel(it) }
    val state by vm.state.collectAsStateWithLifecycle()
    val c = Crumb.colors
    var confirmEmpty by remember { mutableStateOf(false) }

    LaunchedEffect(state.signedOut) {
        if (state.signedOut) nav.signedOut()
    }

    Column(
        Modifier.fillMaxSize().statusBarsPadding().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp),
        verticalArrangement = Arrangement.spacedBy(28.dp),
    ) {
        Column {
            VSpace(8.dp)
            BackLink { nav.back() }
            Text("Trash", style = CrumbText.pageTitle, color = c.ink, modifier = Modifier.padding(top = 4.dp))
            Text(
                "Deleted recipes stay here for 30 days, with their cookbooks and cook log, then go for good.",
                style = CrumbText.body,
                color = c.inkMuted,
                modifier = Modifier.padding(top = 8.dp),
            )
        }

        val items = state.items
        when {
            items == null -> Skeleton(Modifier.fillMaxWidth().size(160.dp), shape = CardShape)
            items.isEmpty() -> Card(Modifier.fillMaxWidth(), padding = PaddingValues(24.dp)) {
                Text("Nothing in the trash.", style = CrumbText.body, color = c.inkMuted, textAlign = TextAlign.Center, modifier = Modifier.fillMaxWidth())
            }
            else -> Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                ListCard(
                    rows = items.map { item ->
                        @Composable {
                            TrashRow(
                                item,
                                photoUrl = container.recipes.photoUrl(item.id, item.image, 168),
                                enabled = state.busy == null,
                                onRestore = { vm.restore(item) { id -> nav.recipe(id) } },
                                onPurge = { vm.purge(item) },
                            )
                        }
                    },
                )
                Row {
                    Btn("Empty the trash", { confirmEmpty = true }, style = BtnStyle.Ghost, enabled = state.busy == null)
                }
            }
        }
        VSpace(24.dp)
    }

    if (confirmEmpty) {
        CrumbModal(
            title = "Empty the trash?",
            description = emptyTrashDescription(state.items?.size ?: 0),
            onDismiss = { confirmEmpty = false },
            footer = {
                Btn("Cancel", { confirmEmpty = false }, style = BtnStyle.Ghost)
                Btn(
                    "Empty the trash",
                    { vm.empty { confirmEmpty = false } },
                    style = BtnStyle.Danger,
                    enabled = state.busy == null,
                )
            },
        ) {}
    }
}

/** A photo, the title with its days left, "Put back" and a delete-for-good button. */
@Composable
private fun TrashRow(item: Trashed, photoUrl: String?, enabled: Boolean, onRestore: () -> Unit, onPurge: () -> Unit) {
    val c = Crumb.colors
    Row(
        Modifier.fillMaxWidth().padding(12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        RecipePhoto(photoUrl, item.image, Modifier.size(56.dp), placeholderId = item.id)
        // Small buttons keep their width, so the title gets what's left (two lines) and the
        // days left stay on one
        Column(Modifier.weight(1f)) {
            Text(item.title, style = CrumbText.rowTitle, color = c.ink, maxLines = 2, overflow = TextOverflow.Ellipsis)
            daysLeftLabel(item.purgeAt, System.currentTimeMillis())?.let {
                Text(it, style = CrumbText.bodySmall, color = c.inkMuted, maxLines = 1)
            }
        }
        Btn("Put back", onRestore, style = BtnStyle.Soft, size = BtnSize.Sm, padding = 12.dp, icon = Lucide.RotateCcw, enabled = enabled)
        Btn(
            null,
            onPurge,
            style = BtnStyle.Ghost,
            size = BtnSize.Sm,
            icon = Lucide.Trash2,
            contentDescription = "Delete ${item.title} for good",
            enabled = enabled,
        )
    }
}

/** "← More", 15sp muted. */
@Composable
private fun BackLink(onClick: () -> Unit) {
    val c = Crumb.colors
    Row(
        Modifier.clip(ControlShape).clickable(role = Role.Button, onClick = onClick).padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Icon(Lucide.ArrowLeft, contentDescription = null, tint = c.inkMuted, modifier = Modifier.size(18.dp))
        Text("More", style = CrumbText.bodySmall, color = c.inkMuted)
    }
}
