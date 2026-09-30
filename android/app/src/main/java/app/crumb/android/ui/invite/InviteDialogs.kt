package app.crumb.android.ui.invite

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import app.crumb.android.data.AccountApi
import app.crumb.android.data.ApiException
import app.crumb.android.data.AuthMode
import app.crumb.android.data.InvitePreview
import app.crumb.android.data.InviteStep
import app.crumb.android.data.InviteText
import app.crumb.android.data.OfflineException
import app.crumb.android.data.inviteStep
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.LocalNav
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.CrumbModal
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.Toaster
import app.crumb.android.ui.crumbViewModel
import app.crumb.android.ui.friendlyMessage
import app.crumb.android.ui.theme.Crumb
import app.crumb.core.InviteLink
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** The join dialog's state: reading what the invite is for, then joining it. */
data class JoinState(
    val preview: InvitePreview? = null,
    /** Why it can't be joined (used up, expired), or why joining failed. */
    val error: String? = null,
    val joining: Boolean = false,
    /** The invite can't be used at all, so there's nothing to offer but closing. */
    val dead: Boolean = false,
)

class JoinViewModel(private val accounts: AccountApi, private val invite: InviteLink) : ViewModel() {
    private val _state = MutableStateFlow(JoinState())
    val state: StateFlow<JoinState> = _state.asStateFlow()

    private var mode: AuthMode = AuthMode.Password

    init {
        viewModelScope.launch {
            try {
                mode = accounts.status().authMode
                if (mode == AuthMode.Password) throw ApiException(410, InviteText.GONE)
                _state.update { it.copy(preview = accounts.previewInvite(mode, invite.token)) }
            } catch (e: CancellationException) {
                throw e
            } catch (e: ApiException) {
                _state.update { it.copy(error = e.message, dead = true) }
            } catch (e: OfflineException) {
                _state.update { it.copy(error = InviteText.OFFLINE, dead = true) }
            }
        }
    }

    /** Joins; [then] runs once it has, with the household's name. */
    fun join(then: (String) -> Unit) {
        val preview = _state.value.preview ?: return
        if (_state.value.joining) return
        _state.update { it.copy(joining = true, error = null) }
        viewModelScope.launch {
            try {
                accounts.acceptInvite(mode, invite.token)
                then(preview.household)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _state.update { it.copy(error = e.friendlyMessage()) }
            } finally {
                _state.update { it.copy(joining = false) }
            }
        }
    }
}

/**
 * What a waiting invite leads to once someone is signed in: the join dialog on the invite's
 * own server, or the offer to sign out of another one first. (Signed out, the sign-in screen
 * takes it.)
 */
@Composable
fun InviteDialogs() {
    val container = AppContainerProvider
    val session by container.session.session.collectAsStateWithLifecycle()
    val waiting by container.links.invite.collectAsStateWithLifecycle()
    when (val step = inviteStep(session, waiting)) {
        is InviteStep.Join -> JoinDialog(step.invite)
        is InviteStep.OtherServer -> {
            val scope = rememberCoroutineScope()
            CrumbModal(
                title = "Join a household",
                onDismiss = container.links::dismissInvite,
                description = InviteText.otherServer(step.invite, step.current),
                footer = {
                    Btn("Cancel", container.links::dismissInvite, style = BtnStyle.Ghost)
                    // The invite stays waiting, so the sign-in screen that follows picks it up
                    Btn("Sign out", { scope.launch { container.signOut() } }, style = BtnStyle.Tile, modifier = Modifier.testTag("invite-sign-out"))
                },
            ) {}
        }
        else -> Unit
    }
}

@Composable
private fun JoinDialog(invite: InviteLink) {
    val container = AppContainerProvider
    val nav = LocalNav.current
    val vm: JoinViewModel = crumbViewModel(key = invite.token) { JoinViewModel(it.accounts, invite) }
    val state by vm.state.collectAsStateWithLifecycle()
    val preview = state.preview

    CrumbModal(
        title = if (preview != null) "Join ${preview.household}" else "Join a household",
        onDismiss = container.links::dismissInvite,
        footer = {
            Btn(if (state.dead) "Close" else "Not now", container.links::dismissInvite, style = BtnStyle.Ghost)
            if (preview != null) {
                Btn(
                    if (state.joining) "Joining…" else "Join ${preview.household}",
                    {
                        vm.join { household ->
                            // A new household means different recipes everywhere: forget what's cached and start over
                            container.links.dismissInvite()
                            container.cache.clear()
                            nav.restart()
                            Toaster.show("Now in $household")
                        }
                    },
                    style = BtnStyle.Tile,
                    enabled = !state.joining,
                    modifier = Modifier.testTag("invite-join"),
                )
            }
        },
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            when {
                preview != null -> Text(preview.headline, style = CrumbText.body, color = Crumb.colors.ink)
                state.error == null -> Text("Reading the invite…", style = CrumbText.bodySmall, color = Crumb.colors.inkMuted)
            }
            state.error?.let { Text(it, style = CrumbText.bodySmall, color = Crumb.colors.error) }
        }
    }
}
