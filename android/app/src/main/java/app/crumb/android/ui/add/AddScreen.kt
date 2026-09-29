package app.crumb.android.ui.add

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.crumb.android.ui.LocalNav
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.ListCard
import app.crumb.android.ui.components.ListRow
import app.crumb.android.ui.components.SectionHeader
import app.crumb.android.ui.home.TopBox
import app.crumb.android.ui.theme.Crumb
import com.composables.icons.lucide.ClipboardType
import com.composables.icons.lucide.FileDown
import com.composables.icons.lucide.Link
import com.composables.icons.lucide.List
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.PenLine

/**
 * "Add a recipe" (web/src/pages/add.astro): the Add box on its own, what it takes, and the
 * other ways in. [shared] (Share → Crumb text, or the route's text) prefills it once.
 */
@Composable
fun AddScreen(shared: String?, onSharedUsed: () -> Unit, textMode: Boolean = false) {
    val c = Crumb.colors
    val nav = LocalNav.current
    Column(
        Modifier
            .fillMaxSize()
            .statusBarsPadding()
            .imePadding()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 20.dp, vertical = 24.dp),
        verticalArrangement = Arrangement.spacedBy(28.dp),
    ) {
        Column {
            Text("Add a recipe", style = CrumbText.pageTitle, color = c.ink)
            Text("Paste a link or the recipe itself.", style = CrumbText.body, color = c.inkMuted, modifier = Modifier.padding(top = 8.dp))
            TopBox(
                Modifier.fillMaxWidth().padding(top = 20.dp),
                tabs = false,
                onTile = false,
                shared = shared,
                onSharedUsed = onSharedUsed,
                autofocus = shared == null,
                startInTextMode = textMode,
            )
        }
        PopularLinks()
        Column {
            SectionHeader("What you can paste", Modifier.padding(bottom = 14.dp))
            ListCard(
                rows = listOf(
                    { ListRow("A link", subtitle = "Most recipe sites work. If one blocks us, a real browser has a go.", icon = Lucide.Link) },
                    {
                        ListRow(
                            "Any recipe text",
                            subtitle = "From an email, notes, a PDF or a photo's text. Headings help but aren't required.",
                            icon = Lucide.ClipboardType,
                        )
                    },
                    { ListRow("Lots of links", subtitle = "Paste several at once and they're imported one after another.", icon = Lucide.List) },
                ),
            )
        }
        Column {
            SectionHeader("Other ways in", Modifier.padding(bottom = 14.dp))
            ListCard(
                rows = listOf(
                    {
                        ListRow(
                            "Write from scratch",
                            subtitle = "Grandma's card, your own creation",
                            icon = Lucide.PenLine,
                            onClick = { nav.newRecipe() },
                        )
                    },
                    {
                        ListRow(
                            "Import from another app",
                            subtitle = "Just the Recipe, Paprika, Mealie…",
                            icon = Lucide.FileDown,
                            onClick = { nav.import() },
                        )
                    },
                ),
            )
        }
    }
}
