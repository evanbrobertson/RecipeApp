package app.crumb.android

import android.content.Intent
import android.os.Bundle
import android.graphics.Color
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.crumb.android.ui.theme.rememberDark
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import app.crumb.android.data.Incoming
import app.crumb.android.ui.CrumbApp
import app.crumb.android.ui.theme.CrumbTheme
import kotlinx.coroutines.flow.MutableStateFlow

class MainActivity : ComponentActivity() {
    /** What came in through Share → Crumb, until the Add screen takes it. */
    private val shared = MutableStateFlow<Incoming?>(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        // Only a fresh launch; after recreation the Add screen already has the draft
        if (savedInstanceState == null) receive(intent)
        val themeStore = (application as CrumbApplication).container.theme
        setContent {
            val settings by themeStore.settings.collectAsStateWithLifecycle(initialValue = null)
            // Wait for the saved theme so a dark kitchen never flashes light on launch
            val current = settings ?: return@setContent
            val dark = rememberDark(current)
            DisposableEffect(dark) {
                val bars = if (dark) {
                    SystemBarStyle.dark(Color.TRANSPARENT)
                } else {
                    SystemBarStyle.light(Color.TRANSPARENT, Color.TRANSPARENT)
                }
                enableEdgeToEdge(statusBarStyle = bars, navigationBarStyle = bars)
                onDispose {}
            }
            CrumbTheme(dark = dark) {
                CrumbApp(shared, onSharedUsed = { shared.value = null })
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        receive(intent)
    }

    private fun receive(intent: Intent?) {
        Incoming.from(intent, contentResolver)?.let { shared.value = it }
    }
}
