package app.crumb.android

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import app.crumb.android.ui.CrumbApp
import app.crumb.android.ui.theme.CrumbTheme
import kotlinx.coroutines.flow.MutableStateFlow

class MainActivity : ComponentActivity() {
    /** Text from Share → Crumb until the Add screen takes it. */
    private val sharedText = MutableStateFlow<String?>(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        // Only a fresh launch; after recreation the Add screen already has the draft
        if (savedInstanceState == null) receive(intent)
        setContent {
            CrumbTheme {
                CrumbApp(sharedText, onSharedUsed = { sharedText.value = null })
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        receive(intent)
    }

    private fun receive(intent: Intent?) {
        if (intent?.action != Intent.ACTION_SEND || intent.type != "text/plain") return
        val text = intent.getStringExtra(Intent.EXTRA_TEXT)?.takeIf { it.isNotBlank() } ?: return
        val subject = intent.getStringExtra(Intent.EXTRA_SUBJECT)
        // Some apps put the page title in the subject and only the link in the text
        sharedText.value = if (subject != null && subject !in text && !text.trim().startsWith("http")) {
            "$subject\n\n$text"
        } else {
            text
        }
    }
}
