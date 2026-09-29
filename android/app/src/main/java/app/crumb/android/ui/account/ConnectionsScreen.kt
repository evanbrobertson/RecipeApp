package app.crumb.android.ui.account

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import app.crumb.android.ui.LocalNav
import app.crumb.android.ui.components.Card
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.PageTitle
import app.crumb.android.ui.components.GroupLabel
import app.crumb.android.ui.components.ListRow
import app.crumb.android.ui.theme.Crumb
import com.composables.icons.lucide.ArrowLeft
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.MessagesSquare

/**
 * More → Connections (web/src/pages/more/connections.astro): one row per app that can reach
 * your recipe box. For now that's Claude.
 */
@Composable
fun ConnectionsScreen() {
    val nav = LocalNav.current
    val c = Crumb.colors
    Column(
        Modifier.fillMaxSize().statusBarsPadding().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp),
        verticalArrangement = Arrangement.spacedBy(28.dp),
    ) {
        Column {
            Row(
                Modifier.padding(top = 8.dp).clip(ControlShape).clickable { nav.back() }.padding(vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Icon(Lucide.ArrowLeft, contentDescription = null, tint = c.inkMuted, modifier = Modifier.size(18.dp))
                Text("More", style = CrumbText.label, color = c.inkMuted)
            }
            PageTitle("Connections", c.ink, modifier = Modifier.padding(top = 12.dp))
            Text("Use your recipe box from other apps.", style = CrumbText.body, color = c.inkMuted, modifier = Modifier.padding(top = 8.dp))
        }
        Column {
            GroupLabel("Apps")
            Card {
                ListRow(
                    title = "Claude",
                    subtitle = "Save, search and tweak recipes from a chat",
                    icon = Lucide.MessagesSquare,
                    plainIcon = true,
                    onClick = { nav.connect() },
                )
            }
        }
    }
}
