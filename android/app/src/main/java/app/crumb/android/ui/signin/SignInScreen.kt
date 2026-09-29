package app.crumb.android.ui.signin

import android.security.NetworkSecurityPolicy
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import app.crumb.android.R
import app.crumb.android.data.AccountApi
import app.crumb.android.data.AccountForms
import app.crumb.android.data.ApiException
import app.crumb.android.data.AuthMode
import app.crumb.android.data.AuthStatus
import app.crumb.android.data.SignInForm
import app.crumb.android.data.signInForms
import app.crumb.android.data.CrumbApi
import app.crumb.android.data.OfflineException
import app.crumb.android.data.SessionStore
import app.crumb.android.ui.components.CardShape
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnSize
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.PaperCard
import app.crumb.android.ui.components.PrimaryButton
import app.crumb.android.ui.crumbViewModel
import app.crumb.android.ui.theme.Caveat
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.LightColors
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import okhttp3.HttpUrl

/**
 * Two steps, as the server decides: the address first, then whichever form its `AUTH_MODE`
 * offers ([forms]): one password, email and password, sign-up, the first account's setup, or
 * a reset email. [checkEmail] and [resetSentTo] are the "look in your inbox" end states.
 */
data class SignInState(
    val busy: Boolean = false,
    val error: String? = null,
    val server: HttpUrl? = null,
    val status: AuthStatus? = null,
    val form: SignInForm? = null,
    val checkEmail: String? = null,
    val resetSentTo: String? = null,
) {
    val forms: List<SignInForm> get() = status?.let(::signInForms).orEmpty()
}

class SignInViewModel(
    private val api: CrumbApi,
    private val accounts: AccountApi,
    private val session: SessionStore,
) : ViewModel() {
    private val _state = MutableStateFlow(SignInState())
    val state = _state.asStateFlow()

    val lastServer: String = session.lastServer?.trimEnd('/')?.removePrefix("https://").orEmpty()

    private val mode: AuthMode get() = _state.value.status?.authMode ?: AuthMode.Password

    /** Checks the address and asks the server how it signs people in. */
    fun connect(serverInput: String) {
        val server = CrumbApi.serverUrl(serverInput)
        if (server == null) {
            _state.value = SignInState(error = "That doesn't look like a web address.")
            return
        }
        if (!NetworkSecurityPolicy.getInstance().isCleartextTrafficPermitted(server.host) && !server.isHttps) {
            _state.value = SignInState(error = "Use an https:// address for your Crumb.")
            return
        }
        _state.value = SignInState(busy = true)
        viewModelScope.launch {
            _state.value = try {
                api.health(server)
                val status = accounts.status(server)
                SignInState(server = server, status = status, form = signInForms(status).first())
            } catch (e: ApiException) {
                SignInState(error = e.message)
            } catch (e: OfflineException) {
                SignInState(error = "Couldn't reach a Crumb server there. Check the address.")
            }
        }
    }

    fun changeServer() {
        _state.value = SignInState()
    }

    fun show(form: SignInForm) {
        _state.update { it.copy(form = form, error = null, checkEmail = null, resetSentTo = null) }
    }

    fun signIn(password: String) = attempt("That password didn't work.") { server ->
        // A server without APP_PASSWORD accepts anything, but the field can't be empty
        api.login(server, password.ifEmpty { " " })
    }

    fun signIn(email: String, password: String) = attempt("That email or password didn't work.", AccountForms.signIn(email, password)) { server ->
        accounts.signIn(server, mode, email, password)
    }

    fun signUp(name: String, email: String, password: String) =
        perform("Couldn't make the account.", AccountForms.signUp(name, email, password), onDone = { server ->
            val made = accounts.signUp(server, mode, name, email, password)
            if (made.verify) {
                _state.update { it.copy(busy = false, checkEmail = email.trim()) }
                false
            } else {
                session.signIn(server, made.cookie)
                true
            }
        })

    fun setUp(name: String, email: String, password: String, appPassword: String) {
        val needs = _state.value.status?.setupNeedsAppPassword == true
        attempt(
            "Couldn't make the account.",
            AccountForms.setUp(name, email, password, needs, appPassword),
        ) { server -> accounts.setUp(server, name, email, password, appPassword) }
    }

    fun forgot(email: String) {
        val invalid = AccountForms.forgot(email)
        val server = _state.value.server
        if (invalid != null || server == null) {
            _state.update { it.copy(error = invalid) }
            return
        }
        _state.update { it.copy(busy = true, error = null) }
        viewModelScope.launch {
            try {
                accounts.requestReset(server, email)
                _state.update { it.copy(busy = false, resetSentTo = email.trim()) }
            } catch (e: ApiException) {
                _state.update { it.copy(busy = false, error = e.message) }
            } catch (e: OfflineException) {
                _state.update { it.copy(busy = false, error = "Couldn't reach your Crumb server. Check your connection.") }
            }
        }
    }

    /** Runs a sign-in that ends with a cookie to keep; a 401 is a wrong password, not a lapsed session. */
    private fun attempt(
        wrong: String,
        invalid: String? = null,
        get: suspend (HttpUrl) -> String?,
    ) = perform(wrong, invalid, onDone = { server ->
        session.signIn(server, get(server))
        true
    })

    private fun perform(wrong: String, invalid: String?, onDone: suspend (HttpUrl) -> Boolean) {
        val server = _state.value.server ?: return
        if (invalid != null) {
            _state.update { it.copy(error = invalid) }
            return
        }
        _state.update { it.copy(busy = true, error = null) }
        viewModelScope.launch {
            try {
                // Signing in swaps this screen out, so there's nothing else to update
                onDone(server)
            } catch (e: ApiException) {
                _state.update { it.copy(busy = false, error = if (e.status == 401) wrong else e.message) }
            } catch (e: OfflineException) {
                _state.update { it.copy(busy = false, error = "Couldn't reach your Crumb server. Check your connection.") }
            }
        }
    }
}

@Composable
fun SignInScreen() {
    val vm = crumbViewModel { SignInViewModel(it.api, it.accounts, it.session) }
    val state by vm.state.collectAsStateWithLifecycle()
    var address by rememberSaveable { mutableStateOf(vm.lastServer) }
    val colors = Crumb.colors

    Box(Modifier.fillMaxSize().imePadding().verticalScroll(rememberScrollState()), contentAlignment = Alignment.Center) {
        Column(
            Modifier.widthIn(max = 440.dp).fillMaxWidth().padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Image(
                painterResource(R.drawable.ic_launcher_foreground),
                contentDescription = null,
                modifier = Modifier.size(88.dp).clip(CardShape).background(LightColors.tile),
            )
            Text("Welcome back", fontFamily = Caveat, style = MaterialTheme.typography.headlineMedium, color = colors.primary)
            Text("Crumb", style = MaterialTheme.typography.displaySmall)
            PaperCard(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                    if (state.status == null) {
                        AddressStep(address, { address = it }, state, onContinue = { vm.connect(address) })
                    } else {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(
                                address.trim().trimEnd('/').removePrefix("https://"),
                                style = CrumbText.bodySmall,
                                color = colors.inkMuted,
                                maxLines = 1,
                                modifier = Modifier.weight(1f),
                            )
                            Btn("Change", vm::changeServer, style = BtnStyle.Ghost, size = BtnSize.Sm)
                        }
                        FormStep(vm, state)
                    }
                }
            }
            Text(
                "Crumb keeps your recipes on your own server. Your password isn't stored on this phone.",
                style = MaterialTheme.typography.bodySmall,
                color = colors.inkMuted,
            )
        }
    }
}

@Composable
private fun AddressStep(address: String, onChange: (String) -> Unit, state: SignInState, onContinue: () -> Unit) {
    val go = { if (!state.busy && address.isNotBlank()) onContinue() }
    OutlinedTextField(
        value = address,
        onValueChange = onChange,
        label = { Text("Your Crumb address") },
        placeholder = { Text("crumb.example.com") },
        singleLine = true,
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go, autoCorrectEnabled = false),
        keyboardActions = KeyboardActions(onGo = { go() }),
        modifier = Modifier.fillMaxWidth().testTag("server"),
    )
    state.error?.let { ErrorText(it) }
    PrimaryButton(
        text = if (state.busy) "Connecting\u2026" else "Continue",
        onClick = go,
        enabled = !state.busy && address.isNotBlank(),
        modifier = Modifier.fillMaxWidth().testTag("continue"),
    )
}

@Composable
private fun ErrorText(text: String) {
    Text(text, color = Crumb.colors.error, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite })
}

/** The mode's form: what the web's LoginForm.svelte, and the reset page, show. */
@Composable
private fun ColumnScope.FormStep(vm: SignInViewModel, state: SignInState) {
    val colors = Crumb.colors
    val form = state.form ?: return
    var name by rememberSaveable { mutableStateOf("") }
    var email by rememberSaveable { mutableStateOf("") }
    var password by rememberSaveable { mutableStateOf("") }
    var appPassword by rememberSaveable { mutableStateOf("") }
    val status = state.status
    val forms = state.forms

    if (state.checkEmail != null || state.resetSentTo != null) {
        Text("Check your email", style = CrumbText.rowTitle, color = colors.ink)
        Text(
            if (state.checkEmail != null) {
                "We sent a link to ${state.checkEmail}. Open it to confirm your address and your recipe box is ready."
            } else {
                "If ${state.resetSentTo} has an account, a link to choose a new password is on its way. Open it in your browser, then sign in here."
            },
            style = CrumbText.bodySmall,
            color = colors.inkMuted,
        )
        Btn("Back to sign in", { vm.show(SignInForm.SignIn) }, style = BtnStyle.Ghost)
        return
    }

    Text(
        when (form) {
            SignInForm.Password -> "Enter your password to open your recipe box."
            SignInForm.SignIn -> "Sign in to open your recipe box."
            SignInForm.SignUp -> "Make an account and a recipe box of your own."
            SignInForm.SetUp -> "Make the first account. It owns the recipes already here."
            SignInForm.Forgot -> "Enter your email and we'll send you a link to choose a new password."
        },
        style = CrumbText.bodySmall,
        color = colors.inkMuted,
    )
    if (form == SignInForm.SignUp || form == SignInForm.SetUp) {
        Field("Your name", name, { name = it }, "name", KeyboardType.Text, ImeAction.Next)
    }
    if (form != SignInForm.Password) {
        Field("Email", email, { email = it }, "email", KeyboardType.Email, if (form == SignInForm.Forgot) ImeAction.Go else ImeAction.Next)
    }
    val submit: () -> Unit = {
        if (!state.busy) when (form) {
            SignInForm.Password -> vm.signIn(password)
            SignInForm.SignIn -> vm.signIn(email, password)
            SignInForm.SignUp -> vm.signUp(name, email, password)
            SignInForm.SetUp -> vm.setUp(name, email, password, appPassword)
            SignInForm.Forgot -> vm.forgot(email)
        }
    }
    if (form != SignInForm.Forgot) {
        Field(
            "Password",
            password,
            { password = it },
            "password",
            KeyboardType.Password,
            if (form == SignInForm.SetUp && status?.setupNeedsAppPassword == true) ImeAction.Next else ImeAction.Go,
            secret = true,
            onGo = submit,
        )
        if (form == SignInForm.SignUp || form == SignInForm.SetUp) {
            Text("At least ${AccountForms.MIN_PASSWORD} characters.", style = CrumbText.hint, color = colors.inkMuted)
        }
    }
    if (form == SignInForm.SetUp && status?.setupNeedsAppPassword == true) {
        Field("Current app password", appPassword, { appPassword = it }, "app-password", KeyboardType.Password, ImeAction.Go, secret = true, onGo = submit)
        Text("The one this box used before accounts.", style = CrumbText.hint, color = colors.inkMuted)
    }
    state.error?.let { ErrorText(it) }
    PrimaryButton(
        text = when {
            state.busy -> "Working\u2026"
            form == SignInForm.Forgot -> "Send the link"
            form == SignInForm.SignUp || form == SignInForm.SetUp -> "Create account"
            else -> "Sign in"
        },
        onClick = submit,
        enabled = !state.busy,
        modifier = Modifier.fillMaxWidth().testTag("sign-in"),
    )
    // Links between the forms this server offers
    if (form == SignInForm.SignIn && SignInForm.Forgot in forms) {
        Btn("Forgot your password?", { vm.show(SignInForm.Forgot) }, style = BtnStyle.Link, modifier = Modifier.align(Alignment.CenterHorizontally))
    }
    if (form == SignInForm.SignIn && SignInForm.SignUp in forms) {
        Btn("New here? Create an account", { vm.show(SignInForm.SignUp) }, style = BtnStyle.Link, modifier = Modifier.align(Alignment.CenterHorizontally))
    }
    if (form == SignInForm.SignUp || form == SignInForm.Forgot) {
        Btn("Have an account? Sign in", { vm.show(SignInForm.SignIn) }, style = BtnStyle.Link, modifier = Modifier.align(Alignment.CenterHorizontally))
    }
}

@Composable
private fun Field(
    label: String,
    value: String,
    onChange: (String) -> Unit,
    tag: String,
    type: KeyboardType,
    ime: ImeAction,
    secret: Boolean = false,
    onGo: () -> Unit = {},
) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        singleLine = true,
        visualTransformation = if (secret) PasswordVisualTransformation() else VisualTransformation.None,
        keyboardOptions = KeyboardOptions(keyboardType = type, imeAction = ime, autoCorrectEnabled = false),
        keyboardActions = KeyboardActions(onGo = { onGo() }),
        modifier = Modifier.fillMaxWidth().testTag(tag),
    )
}
