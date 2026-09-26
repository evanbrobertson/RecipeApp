package app.crumb.android.ui.more

import android.content.Intent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.Logout
import androidx.compose.material.icons.outlined.OpenInBrowser
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.core.net.toUri
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.crumb.android.BuildConfig
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.components.PaperCard
import app.crumb.android.ui.components.SecondaryButton
import app.crumb.android.ui.theme.Crumb
import kotlinx.coroutines.launch

@Composable
fun MoreScreen() {
    val container = AppContainerProvider
    val session by container.session.session.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val colors = Crumb.colors
    val server = session?.server?.toString()?.trimEnd('/')

    Column(
        Modifier.fillMaxSize().statusBarsPadding().verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text("More", style = MaterialTheme.typography.headlineLarge)
        PaperCard(Modifier.widthIn(max = 720.dp).fillMaxWidth()) {
            Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text("Your Crumb", style = MaterialTheme.typography.titleMedium)
                Text(server?.removePrefix("https://") ?: "", style = MaterialTheme.typography.bodyLarge, color = colors.inkMuted)
                if (server != null) {
                    SecondaryButton(
                        "Open on the web",
                        icon = Icons.Outlined.OpenInBrowser,
                        onClick = { context.startActivity(Intent(Intent.ACTION_VIEW, server.toUri())) },
                    )
                }
                SecondaryButton(
                    "Sign out",
                    icon = Icons.AutoMirrored.Outlined.Logout,
                    onClick = { scope.launch { container.signOut() } },
                    modifier = Modifier.testTag("sign-out"),
                )
            }
        }
        Text(
            "Crumb for Android ${BuildConfig.VERSION_NAME}",
            style = MaterialTheme.typography.bodySmall,
            color = colors.inkMuted,
        )
    }
}
