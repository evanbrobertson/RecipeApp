package app.crumb.android.ui.timers

import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Timer
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import app.crumb.android.ui.components.Message

@Composable
fun TimersScreen() {
    Message(
        "Kitchen timers",
        "Timers that ring even with the screen off are on their way in the next update.",
        icon = Icons.Outlined.Timer,
        modifier = Modifier.statusBarsPadding(),
    )
}
