package app.crumb.android.ui.account

import android.content.Intent
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import app.crumb.android.data.AccountApi
import app.crumb.android.data.AccountForms
import app.crumb.android.data.AccountPart
import app.crumb.android.data.ApiException
import app.crumb.android.data.AuthMode
import app.crumb.android.data.AuthStatus
import app.crumb.android.data.ConnectedApp
import app.crumb.android.data.CrumbApi
import app.crumb.android.data.Device
import app.crumb.android.data.Household
import app.crumb.android.data.Member
import app.crumb.android.data.PendingInvite
import app.crumb.android.data.SignInMethods
import app.crumb.android.data.accountParts
import app.crumb.android.data.dayLabel
import app.crumb.android.data.deviceName
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.LocalNav
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnSize
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.Card
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.PageTitle
import app.crumb.android.ui.components.GroupLabel
import app.crumb.android.ui.components.Skeleton
import app.crumb.android.ui.components.ToastTone
import app.crumb.android.ui.components.Toaster
import app.crumb.android.ui.components.VSpace
import app.crumb.android.ui.components.ListRow
import app.crumb.android.ui.crumbViewModel
import app.crumb.android.ui.friendlyMessage
import app.crumb.android.ui.theme.Crumb
import com.composables.icons.lucide.ArrowLeft
import com.composables.icons.lucide.ArrowLeftRight
import com.composables.icons.lucide.Copy
import com.composables.icons.lucide.Download
import com.composables.icons.lucide.DoorOpen
import com.composables.icons.lucide.House
import com.composables.icons.lucide.KeyRound
import com.composables.icons.lucide.Link
import com.composables.icons.lucide.LogOut
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Mail
import com.composables.icons.lucide.MonitorSmartphone
import com.composables.icons.lucide.Pencil
import com.composables.icons.lucide.Plug
import com.composables.icons.lucide.Share2
import com.composables.icons.lucide.Trash2
import com.composables.icons.lucide.Unlink
import com.composables.icons.lucide.UserMinus
import com.composables.icons.lucide.UserPlus
import com.composables.icons.lucide.UserRound
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** Everything More → Account reads from the server; null means not loaded yet. */
data class AccountUiState(
    val status: AuthStatus? = null,
    val methods: SignInMethods? = null,
    val devices: List<Device> = emptyList(),
    val household: Household? = null,
    val apps: List<ConnectedApp>? = null,
    val busy: Boolean = false,
    /** An inline form's error (change email, delete account). */
    val formError: String? = null,
    /** The last invite link made, to copy or share. */
    val madeLink: String? = null,
    val error: String? = null,
    val signedOut: Boolean = false,
)

class AccountViewModel(private val api: AccountApi) : ViewModel() {
    private val _state = MutableStateFlow(AccountUiState())
    val state: StateFlow<AccountUiState> = _state.asStateFlow()

    private val mode: AuthMode get() = _state.value.status?.authMode ?: AuthMode.Password

    init {
        load()
    }

    fun load() {
        viewModelScope.launch {
            try {
                val status = api.status()
                _state.update { it.copy(status = status, error = null) }
                coroutineScope {
                    val apps = async { runCatching { api.connectedApps() }.getOrDefault(emptyList()) }
                    if (status.isAccountSignedIn) {
                        val methods = async { runCatching { api.signInMethods(status.authMode) }.getOrNull() }
                        val devices = async { runCatching { api.devices(status.authMode) }.getOrDefault(emptyList()) }
                        val household = async { runCatching { api.household(status.authMode) }.getOrNull() }
                        _state.update { it.copy(methods = methods.await(), devices = devices.await(), household = household.await()) }
                    }
                    _state.update { it.copy(apps = apps.await()) }
                }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _state.update { it.copy(error = e.friendlyMessage(), signedOut = (e as? ApiException)?.isSignedOut == true, apps = it.apps ?: emptyList()) }
            }
        }
    }

    /** Runs one change, toasting what went wrong (or [done]), then [after] on success. */
    private fun change(failed: String, done: String? = null, description: String? = null, after: suspend () -> Unit = {}, work: suspend () -> Unit) {
        if (_state.value.busy) return
        _state.update { it.copy(busy = true, formError = null) }
        viewModelScope.launch {
            try {
                work()
                after()
                if (done != null) Toaster.show(done, description)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                Toaster.show(failed, e.friendlyMessage(), ToastTone.Error)
            } finally {
                _state.update { it.copy(busy = false) }
            }
        }
    }

    private suspend fun reloadHousehold() {
        val household = runCatching { api.household(mode) }.getOrNull() ?: return
        _state.update { it.copy(household = household) }
    }

    private suspend fun reloadDevices() {
        val devices = runCatching { api.devices(mode) }.getOrNull() ?: return
        _state.update { it.copy(devices = devices) }
    }

    fun signOutDevice(d: Device) = change("Couldn't sign that out", after = ::reloadDevices) { api.signOutDevice(mode, d.id) }

    fun signOutOthers() = change("Couldn't sign out", "Signed out everywhere else", after = ::reloadDevices) { api.signOutOthers(mode) }

    fun rename(name: String, then: () -> Unit) =
        change("Couldn't rename the household", after = { reloadHousehold(); then() }) { api.rename(mode, name) }

    fun invite(email: String?, then: (String) -> Unit) {
        var link = ""
        change(
            "Couldn't make an invite",
            after = {
                _state.update { it.copy(madeLink = link) }
                reloadHousehold()
                then(link)
            },
        ) { link = api.invite(mode, email) }
    }

    fun cancelInvite(i: PendingInvite) =
        change("Couldn't cancel the invite", "Invite cancelled", after = ::reloadHousehold) { api.cancelInvite(mode, i.id) }

    fun removeMember(m: Member) =
        change("Couldn't remove them", "${m.name} was removed", after = ::reloadHousehold) { api.removeMember(mode, m.id) }

    /** Leaving or switching changes whose recipes every page shows: [then] starts over from home. */
    fun leave(h: Household, then: () -> Unit) =
        change("Couldn't leave", "You left ${h.name}", after = { then() }) { api.leave(mode, h.id) }

    fun switchTo(id: String, name: String, then: () -> Unit) =
        change("Couldn't switch", "Now in $name", after = { then() }) { api.switchTo(mode, id) }

    fun unlink(p: app.crumb.android.data.Provider) = change(
        "Couldn't unlink ${p.label}",
        "${p.label} unlinked",
        after = { _state.update { s -> s.copy(methods = runCatching { api.signInMethods(mode) }.getOrNull() ?: s.methods) } },
    ) { api.unlink(mode, p) }

    fun disconnect(app: ConnectedApp) = change(
        "Couldn't disconnect it",
        "${app.name} disconnected",
        "It has to be approved again.",
        after = { _state.update { s -> s.copy(apps = runCatching { api.connectedApps() }.getOrNull() ?: s.apps) } },
    ) { api.disconnect(app.id) }

    /** Changes the email; [then] gets the result so the form can close. */
    fun changeEmail(email: String, password: String, then: (app.crumb.android.data.EmailChange) -> Unit) {
        if (_state.value.busy) return
        val needsPassword = _state.value.methods?.password == true
        _state.update { it.copy(busy = true, formError = null) }
        viewModelScope.launch {
            try {
                val made = api.changeEmail(email, password.takeIf { needsPassword }.orEmpty())
                if (!made.pending) load()
                then(made)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _state.update { it.copy(formError = if ((e as? ApiException)?.status == 401) "That password didn't work." else e.friendlyMessage()) }
            } finally {
                _state.update { it.copy(busy = false) }
            }
        }
    }

    fun deleteAccount(password: String?, typed: String?, then: () -> Unit) {
        if (_state.value.busy) return
        _state.update { it.copy(busy = true, formError = null) }
        viewModelScope.launch {
            try {
                api.deleteAccount(password, typed)
                then()
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _state.update { it.copy(formError = if ((e as? ApiException)?.status == 401) "That password didn't work." else e.friendlyMessage(), busy = false) }
            }
        }
    }

    fun clearFormError() = _state.update { it.copy(formError = null) }
}

/**
 * More → Account (web/src/islands/AccountPage.svelte + AccountSection.svelte): who's signed
 * in and how, their devices and household, the apps connected to Crumb, their data and
 * signing out. With one password it's just the connected apps and signing out.
 */
@Composable
fun AccountScreen() {
    val container = AppContainerProvider
    val nav = LocalNav.current
    val scope = rememberCoroutineScope()
    val vm: AccountViewModel = crumbViewModel { AccountViewModel(it.accounts) }
    val state by vm.state.collectAsStateWithLifecycle()
    val saveFile = rememberFileSaver("Your data was saved")

    LaunchedEffect(state.signedOut) {
        if (state.signedOut) nav.signedOut()
    }

    /** A new household means different recipes everywhere: forget what's cached and start over. */
    val restart: () -> Unit = {
        container.cache.clear()
        nav.restart()
    }
    val signOut: () -> Unit = { scope.launch { container.signOut() } }

    Column(
        Modifier.fillMaxSize().statusBarsPadding().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp),
        verticalArrangement = Arrangement.spacedBy(28.dp),
    ) {
        Column {
            BackLink("More") { nav.back() }
            PageTitle("Account", Crumb.colors.ink, modifier = Modifier.padding(top = 12.dp))
            Text(
                "How you sign in, your household and connected apps.",
                style = CrumbText.body,
                color = Crumb.colors.inkMuted,
                modifier = Modifier.padding(top = 8.dp),
            )
        }
        val status = state.status
        if (status == null) {
            if (state.error != null) {
                Text(state.error.orEmpty(), style = CrumbText.bodySmall, color = Crumb.colors.error)
                Btn("Try again", vm::load, style = BtnStyle.Outline)
            } else {
                Skeleton(Modifier.fillMaxWidth().size(160.dp), shape = app.crumb.android.ui.components.CardShape)
                Skeleton(Modifier.fillMaxWidth().size(112.dp), shape = app.crumb.android.ui.components.CardShape)
            }
        } else {
            for (part in accountParts(status)) {
                when (part) {
                    AccountPart.Account -> AccountCard(status, state, vm)
                    AccountPart.SignInMethods -> state.methods?.let { SignInMethodsCard(status, it, state, vm) }
                    AccountPart.Household -> state.household?.let { HouseholdCard(it, state, vm, restart) }
                    AccountPart.ConnectedApps -> ConnectedAppsCard(state, vm) { nav.connect() }
                    AccountPart.Data -> DataCard(status, state, vm, saveFile = saveFile, container = container, onDeleted = signOut)
                    AccountPart.SignOut -> Card {
                        ListRow(title = "Sign out", icon = Lucide.LogOut, plainIcon = true, onClick = signOut, chevron = false)
                    }
                }
            }
        }
        VSpace(24.dp)
    }
}

@Composable
private fun BackLink(label: String, onClick: () -> Unit) {
    val c = Crumb.colors
    Row(
        Modifier.padding(top = 8.dp).clip(ControlShape).clickable(onClick = onClick).padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Icon(Lucide.ArrowLeft, contentDescription = null, tint = c.inkMuted, modifier = Modifier.size(18.dp))
        Text(label, style = CrumbText.label, color = c.inkMuted)
    }
}

/** A heading and its rows in one card. */
@Composable
private fun Section(title: String, content: @Composable () -> Unit) {
    Column {
        GroupLabel(title)
        Card { content() }
    }
}

@Composable
private fun Divider() {
    HorizontalDivider(Modifier.padding(horizontal = 12.dp), thickness = 1.dp, color = Crumb.colors.line)
}

/** A row with an icon, bold title, muted line, and buttons at the end. */
@Composable
private fun InfoRow(
    icon: ImageVector,
    title: String,
    subtitle: String? = null,
    titleColor: androidx.compose.ui.graphics.Color? = null,
    onClick: (() -> Unit)? = null,
    /** The start of [subtitle] drawn as the web's link (primary, bold); the row's tap is its target. */
    subtitleLink: String? = null,
    trailing: (@Composable RowScope.() -> Unit)? = null,
) {
    val c = Crumb.colors
    Row(
        Modifier
            .fillMaxWidth()
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(horizontal = 12.dp, vertical = 10.dp)
            .padding(vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Icon(icon, null, tint = titleColor ?: c.ink, modifier = Modifier.size(22.dp).padding(start = 2.dp))
        Column(Modifier.weight(1f)) {
            Text(title, style = CrumbText.rowTitle, color = titleColor ?: c.ink)
            if (subtitle != null && subtitleLink != null && subtitle.startsWith(subtitleLink)) {
                Text(
                    buildAnnotatedString {
                        withStyle(SpanStyle(color = c.primary, fontWeight = FontWeight.Bold)) { append(subtitleLink) }
                        append(subtitle.removePrefix(subtitleLink))
                    },
                    style = CrumbText.bodySmall,
                    color = c.inkMuted,
                )
            } else if (subtitle != null) {
                Text(subtitle, style = CrumbText.bodySmall, color = c.inkMuted)
            }
        }
        trailing?.invoke(this)
    }
}

/** Under a row: what confirming does, and the Keep / do-it buttons. */
@Composable
private fun ConfirmBar(message: String, keep: String, act: String, busy: Boolean, onKeep: () -> Unit, onAct: () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(start = 50.dp, end = 12.dp, bottom = 12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(message, style = CrumbText.bodySmall, color = Crumb.colors.inkMuted)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Btn(keep, onKeep, style = BtnStyle.Ghost, size = BtnSize.Sm)
            Btn(act, onAct, style = BtnStyle.Danger, size = BtnSize.Sm, enabled = !busy)
        }
    }
}

@Composable
private fun Field(
    label: String,
    value: String,
    onChange: (String) -> Unit,
    type: KeyboardType = KeyboardType.Text,
    secret: Boolean = false,
    modifier: Modifier = Modifier,
) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        singleLine = true,
        visualTransformation = if (secret) PasswordVisualTransformation() else androidx.compose.ui.text.input.VisualTransformation.None,
        keyboardOptions = KeyboardOptions(keyboardType = type, autoCorrectEnabled = false),
        modifier = modifier.fillMaxWidth(),
    )
}

// ─── Account: who's signed in, and where ───

@Composable
private fun AccountCard(status: AuthStatus, state: AccountUiState, vm: AccountViewModel) {
    Section("Account") {
        InfoRow(Lucide.UserRound, status.user?.name.orEmpty(), status.user?.email)
        for (d in state.devices) {
            Divider()
            InfoRow(
                Lucide.MonitorSmartphone,
                deviceName(d.userAgent),
                if (d.current) "This device" else "Last used ${dayLabel(d.lastSeenAt)}",
                trailing = if (d.current) null else {
                    { Btn(null, { vm.signOutDevice(d) }, style = BtnStyle.Ghost, icon = Lucide.LogOut, enabled = !state.busy, contentDescription = "Sign out ${deviceName(d.userAgent)}") }
                },
            )
        }
        if (state.devices.size > 1) {
            Divider()
            InfoRow(Lucide.LogOut, "Sign out everywhere else", onClick = { if (!state.busy) vm.signOutOthers() })
        }
    }
}

@Composable
private fun SignInMethodsCard(status: AuthStatus, methods: SignInMethods, state: AccountUiState, vm: AccountViewModel) {
    var confirming by rememberSaveable { mutableStateOf<String?>(null) }
    var editing by rememberSaveable { mutableStateOf(false) }
    var email by rememberSaveable { mutableStateOf("") }
    var again by rememberSaveable { mutableStateOf("") }
    var password by rememberSaveable { mutableStateOf("") }
    var localError by rememberSaveable { mutableStateOf<String?>(null) }
    val hosted = status.authMode == AuthMode.Hosted

    fun close() {
        editing = false
        email = ""; again = ""; password = ""; localError = null
        vm.clearFormError()
    }

    Section("Sign-in methods") {
        if (editing) {
            Column(Modifier.fillMaxWidth().padding(12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Text("Change your email", style = CrumbText.rowTitle, color = Crumb.colors.ink)
                Text(
                    if (hosted) "We'll send the new address a link, and switch once it's opened."
                    else "You'll sign in with the new address, and be signed out on other devices.",
                    style = CrumbText.bodySmall,
                    color = Crumb.colors.inkMuted,
                )
                Field("New email", email, { email = it }, KeyboardType.Email)
                Field("New email again", again, { again = it }, KeyboardType.Email)
                if (methods.password) Field("Your password", password, { password = it }, KeyboardType.Password, secret = true)
                (localError ?: state.formError)?.let { Text(it, style = CrumbText.bodySmall, color = Crumb.colors.error) }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End), modifier = Modifier.fillMaxWidth()) {
                    Btn("Cancel", ::close, style = BtnStyle.Ghost)
                    Btn(
                        if (state.busy) "Changing…" else "Change email",
                        {
                            localError = AccountForms.changeEmail(email, again, password, methods.password)
                            if (localError == null) {
                                vm.changeEmail(email, password) { made ->
                                    close()
                                    if (made.pending) {
                                        Toaster.show(
                                            "Check ${made.email}",
                                            "Open the link sent there to switch. Until then, sign in with ${status.user?.email}.",
                                        )
                                    } else {
                                        Toaster.show("Email changed", "Sign in with ${made.email} from now on.")
                                    }
                                }
                            }
                        },
                        style = BtnStyle.Tile,
                        enabled = !state.busy,
                    )
                }
            }
        } else {
            InfoRow(
                Lucide.Mail,
                "Email",
                status.user?.email,
                trailing = { Btn(null, { editing = true }, style = BtnStyle.Ghost, icon = Lucide.Pencil, contentDescription = "Change your email") },
            )
        }
        if (methods.password) {
            Divider()
            InfoRow(Lucide.KeyRound, "Password", "With your email")
        }
        for (l in methods.linked) {
            Divider()
            InfoRow(
                Lucide.Link,
                l.provider.label,
                l.email ?: "Linked ${dayLabel(l.createdAt)}",
                trailing = if (methods.canUnlink && confirming != "unlink:${l.provider.id}") {
                    { Btn(null, { confirming = "unlink:${l.provider.id}" }, style = BtnStyle.Ghost, icon = Lucide.Unlink, contentDescription = "Unlink ${l.provider.label}") }
                } else null,
            )
            if (confirming == "unlink:${l.provider.id}") {
                ConfirmBar("You won't be able to sign in with it.", "Keep", "Unlink", state.busy, { confirming = null }) {
                    vm.unlink(l.provider)
                    confirming = null
                }
            }
        }
    }
}

// ─── Household ───

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun HouseholdCard(h: Household, state: AccountUiState, vm: AccountViewModel, restart: () -> Unit) {
    val context = LocalContext.current
    val clipboard = LocalClipboardManager.current
    val hosted = state.status?.authMode == AuthMode.Hosted
    var confirming by rememberSaveable { mutableStateOf<String?>(null) }
    var renaming by rememberSaveable { mutableStateOf(false) }
    var newName by rememberSaveable { mutableStateOf("") }
    var inviting by rememberSaveable { mutableStateOf(false) }
    var inviteEmail by rememberSaveable { mutableStateOf("") }

    fun copy(url: String) {
        runCatching {
            clipboard.setText(AnnotatedString(url))
            Toaster.show("Invite link copied")
        }.onFailure { Toaster.show("Couldn't copy", tone = ToastTone.Error) }
    }

    fun share(url: String) {
        val send = Intent(Intent.ACTION_SEND).apply {
            type = "text/plain"
            putExtra(Intent.EXTRA_TEXT, "Join ${h.name} on Crumb: $url")
        }
        context.startActivity(Intent.createChooser(send, "Send the invite"))
    }

    Section("Household") {
        if (renaming) {
            Column(Modifier.fillMaxWidth().padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Field("Household name", newName, { newName = it.take(80) })
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End), modifier = Modifier.fillMaxWidth()) {
                    Btn("Cancel", { renaming = false }, style = BtnStyle.Ghost)
                    Btn(
                        "Save",
                        {
                            val name = newName.trim()
                            if (name.isEmpty() || name == h.name) renaming = false else vm.rename(name) { renaming = false }
                        },
                        style = BtnStyle.Tile,
                        enabled = !state.busy,
                    )
                }
            }
        } else {
            InfoRow(
                Lucide.House,
                h.name,
                if (h.members.size == 1) "Just you" else "${h.members.size} people share these recipes",
                trailing = if (h.isOwner) {
                    { Btn(null, { newName = h.name; renaming = true }, style = BtnStyle.Ghost, icon = Lucide.Pencil, contentDescription = "Rename the household") }
                } else null,
            )
        }
        for (m in h.members) {
            Divider()
            InfoRow(
                Lucide.UserRound,
                if (m.you) "${m.name} (you)" else m.name,
                "${if (m.role == "owner") "Owner" else "Member"} · ${m.email}",
                trailing = if (h.isOwner && !m.you && m.role != "owner" && confirming != "remove:${m.id}") {
                    { Btn(null, { confirming = "remove:${m.id}" }, style = BtnStyle.Ghost, icon = Lucide.UserMinus, contentDescription = "Remove ${m.name} from the household") }
                } else null,
            )
            if (confirming == "remove:${m.id}") {
                ConfirmBar("They lose these recipes. Their account stays.", "Keep", "Remove", state.busy, { confirming = null }) {
                    vm.removeMember(m)
                    confirming = null
                }
            }
        }
        if (h.isOwner) {
            for (i in h.invites) {
                Divider()
                InfoRow(
                    if (i.email != null) Lucide.Mail else Lucide.Link,
                    "Invited",
                    listOf(i.email ?: i.createdBy?.let { "Link made by $it" } ?: "Invite link", "until ${dayLabel(i.expiresAt)}").joinToString(" · "),
                    trailing = { Btn("Cancel", { vm.cancelInvite(i) }, style = BtnStyle.Ghost, size = BtnSize.Sm, enabled = !state.busy) },
                )
            }
            state.madeLink?.let { link ->
                Divider()
                InfoRow(
                    Lucide.Link,
                    "New invite link",
                    if (hosted) "Only works for the address it went to." else "Works once, for a week. Send it to who you're inviting.",
                    trailing = {
                        Btn(null, { copy(link) }, style = BtnStyle.Ghost, icon = Lucide.Copy, contentDescription = "Copy the invite link")
                        Btn(null, { share(link) }, style = BtnStyle.Ghost, icon = Lucide.Share2, contentDescription = "Share the invite link")
                    },
                )
            }
            Divider()
            if (hosted && inviting) {
                Column(Modifier.fillMaxWidth().padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Field("Their email", inviteEmail, { inviteEmail = it }, KeyboardType.Email)
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End), modifier = Modifier.fillMaxWidth()) {
                        Btn("Cancel", { inviting = false }, style = BtnStyle.Ghost)
                        Btn(
                            "Send",
                            {
                                if (!AccountForms.looksLikeEmail(inviteEmail)) {
                                    Toaster.show("That doesn't look like an email address.", tone = ToastTone.Error)
                                } else {
                                    val to = inviteEmail.trim()
                                    vm.invite(to) {
                                        Toaster.show("Invite sent to $to")
                                        inviteEmail = ""
                                        inviting = false
                                    }
                                }
                            },
                            style = BtnStyle.Tile,
                            enabled = !state.busy,
                        )
                    }
                }
            } else {
                InfoRow(
                    Lucide.UserPlus,
                    "Invite someone",
                    if (hosted) "By email" else "Makes a link to copy and send",
                    onClick = {
                        if (!state.busy) {
                            if (hosted) inviting = true else vm.invite(null) { copy(it) }
                        }
                    },
                )
            }
        }
        for (other in h.others) {
            Divider()
            InfoRow(
                Lucide.ArrowLeftRight,
                "Switch to ${other.name}",
                "Another household you're in",
                onClick = { if (!state.busy) vm.switchTo(other.id, other.name, restart) },
            )
        }
        if (!h.isOwner) {
            Divider()
            if (confirming == "leave") {
                InfoRow(Lucide.DoorOpen, "Leave ${h.name}?", "You'll stop seeing its recipes.")
                ConfirmBar("You can be invited back.", "Stay", "Leave", state.busy, { confirming = null }) { vm.leave(h, restart) }
            } else {
                InfoRow(Lucide.DoorOpen, "Leave household", onClick = { confirming = "leave" })
            }
        }
    }
}

// ─── Connected apps ───

@Composable
private fun ConnectedAppsCard(state: AccountUiState, vm: AccountViewModel, onConnect: () -> Unit) {
    val c = Crumb.colors
    var confirming by rememberSaveable { mutableStateOf<String?>(null) }
    val apps = state.apps
    Section("Connected apps") {
        when {
            apps == null -> InfoRow(Lucide.Plug, "Loading…")
            apps.isEmpty() -> InfoRow(Lucide.Plug, "No apps connected", "Connect Claude to save and find recipes from a chat.", onClick = onConnect, subtitleLink = "Connect Claude")
            else -> apps.forEachIndexed { i, app ->
                if (i > 0) Divider()
                val key = "app:${app.id}:${app.household?.id}"
                InfoRow(
                    Lucide.Plug,
                    app.name,
                    listOfNotNull(app.household?.name, "connected ${dayLabel(app.connectedAt)}").joinToString(" · "),
                    trailing = if (confirming != key) {
                        { Btn(null, { confirming = key }, style = BtnStyle.Ghost, icon = Lucide.Unlink, contentDescription = "Disconnect ${app.name}") }
                    } else null,
                )
                if (confirming == key) {
                    ConfirmBar("It stops working until it's approved again.", "Keep", "Disconnect", state.busy, { confirming = null }) {
                        vm.disconnect(app)
                        confirming = null
                    }
                }
            }
        }
    }
}

// ─── Your data ───

@Composable
private fun DataCard(
    status: AuthStatus,
    state: AccountUiState,
    vm: AccountViewModel,
    saveFile: (app.crumb.android.data.Download) -> Unit,
    container: app.crumb.android.AppContainer,
    onDeleted: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    var deleting by rememberSaveable { mutableStateOf(false) }
    var password by rememberSaveable { mutableStateOf("") }
    var typed by rememberSaveable { mutableStateOf("") }
    var localError by rememberSaveable { mutableStateOf<String?>(null) }
    val hasPassword = state.methods?.password == true
    val email = status.user?.email.orEmpty()

    Section("Your data") {
        InfoRow(
            Lucide.Download,
            "Download my data",
            "Your account and every household's recipes, as one JSON file",
            onClick = {
                scope.launch {
                    try {
                        saveFile(container.api.exportAccount())
                    } catch (e: Exception) {
                        Toaster.show("Couldn't download that", e.friendlyMessage(), ToastTone.Error)
                    }
                }
            },
        )
        Divider()
        if (deleting) {
            Column(Modifier.fillMaxWidth().padding(12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Text("Delete your account?", style = CrumbText.rowTitle, color = Crumb.colors.ink)
                Text(
                    "This can't be undone. Households you share pass to the next person in them; ones that are only yours are deleted with their recipes. Download your data first if you want a copy.",
                    style = CrumbText.bodySmall,
                    color = Crumb.colors.inkMuted,
                )
                if (hasPassword) Field("Your password", password, { password = it }, KeyboardType.Password, secret = true)
                else Field("Type $email to confirm", typed, { typed = it }, KeyboardType.Email)
                (localError ?: state.formError)?.let { Text(it, style = CrumbText.bodySmall, color = Crumb.colors.error) }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End), modifier = Modifier.fillMaxWidth()) {
                    Btn("Keep my account", { deleting = false; localError = null; vm.clearFormError() }, style = BtnStyle.Ghost)
                    Btn(
                        if (state.busy) "Deleting…" else "Delete account",
                        {
                            localError = AccountForms.deleteAccount(hasPassword, password, typed, email)
                            if (localError == null) {
                                vm.deleteAccount(password.takeIf { hasPassword }, typed.takeIf { !hasPassword }, onDeleted)
                            }
                        },
                        style = BtnStyle.Danger,
                        enabled = !state.busy,
                    )
                }
            }
        } else {
            InfoRow(
                Lucide.Trash2,
                "Delete account",
                "And the households only you are in",
                titleColor = Crumb.colors.error,
                onClick = { deleting = true },
            )
        }
    }
}
