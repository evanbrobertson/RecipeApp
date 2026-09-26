package app.crumb.android.ui.signin

import android.security.NetworkSecurityPolicy
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
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
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import app.crumb.android.R
import app.crumb.android.data.ApiException
import app.crumb.android.data.CrumbApi
import app.crumb.android.data.OfflineException
import app.crumb.android.data.SessionStore
import app.crumb.android.ui.components.CardShape
import app.crumb.android.ui.components.PaperCard
import app.crumb.android.ui.components.PrimaryButton
import app.crumb.android.ui.crumbViewModel
import app.crumb.android.ui.theme.Caveat
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.LightColors
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

data class SignInState(val busy: Boolean = false, val error: String? = null)

class SignInViewModel(private val api: CrumbApi, private val session: SessionStore) : ViewModel() {
    private val _state = MutableStateFlow(SignInState())
    val state = _state.asStateFlow()

    val lastServer: String = session.lastServer?.trimEnd('/')?.removePrefix("https://").orEmpty()

    fun signIn(serverInput: String, password: String) {
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
                // A server without APP_PASSWORD accepts anything, but the field can't be empty
                val cookie = api.login(server, password.ifEmpty { " " })
                session.signIn(server, cookie)
                SignInState()
            } catch (e: ApiException) {
                SignInState(error = if (e.status == 401) "That password didn't work." else e.message)
            } catch (e: OfflineException) {
                SignInState(error = "Couldn't reach a Crumb server there. Check the address.")
            }
        }
    }
}

@Composable
fun SignInScreen() {
    val vm = crumbViewModel { SignInViewModel(it.api, it.session) }
    val state by vm.state.collectAsStateWithLifecycle()
    var server by rememberSaveable { mutableStateOf(vm.lastServer) }
    var password by rememberSaveable { mutableStateOf("") }
    val colors = Crumb.colors
    val submit = { if (!state.busy) vm.signIn(server, password) }

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
                    OutlinedTextField(
                        value = server,
                        onValueChange = { server = it },
                        label = { Text("Your Crumb address") },
                        placeholder = { Text("crumb.example.com") },
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Next, autoCorrectEnabled = false),
                        modifier = Modifier.fillMaxWidth().testTag("server"),
                    )
                    OutlinedTextField(
                        value = password,
                        onValueChange = { password = it },
                        label = { Text("Password") },
                        singleLine = true,
                        visualTransformation = PasswordVisualTransformation(),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Go),
                        keyboardActions = KeyboardActions(onGo = { submit() }),
                        modifier = Modifier.fillMaxWidth().testTag("password"),
                    )
                    state.error?.let { Text(it, color = colors.error, style = MaterialTheme.typography.bodyMedium) }
                    PrimaryButton(
                        text = if (state.busy) "Signing in…" else "Sign in",
                        onClick = submit,
                        enabled = !state.busy && server.isNotBlank(),
                        modifier = Modifier.fillMaxWidth().testTag("sign-in"),
                    )
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
