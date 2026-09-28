package app.crumb.android.ui.recipe

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.crumb.android.data.RecipeChecks
import app.crumb.android.ui.components.Card
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.NunitoSans
import com.composables.icons.lucide.ChefHat
import com.composables.icons.lucide.Lucide

/**
 * The quiet note on a recipe page when Wee Chef tidied or flagged lines on import
 * (web/src/components/WeeChefCard.svelte). Plain paper card, a single ChefHat icon, no
 * colour-coded alert; "See"/"Hide" reveals what each fix was, and Undo appears when the
 * recipe hasn't been edited since.
 */
@Composable
fun WeeChefCard(
    checks: RecipeChecks,
    onUndo: () -> Unit,
    onEdit: () -> Unit,
    modifier: Modifier = Modifier,
    undoing: Boolean = false,
) {
    val c = Crumb.colors
    val fixed = fixedFlags(checks)
    val review = reviewFlags(checks)
    if (fixed.isEmpty() && review.isEmpty()) return
    var open by rememberSaveable(checks.flags.size) { mutableStateOf(false) }

    Card(modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 16.dp, vertical = 4.dp)) {
        if (fixed.isNotEmpty()) {
            Row(Modifier.fillMaxWidth().padding(vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(Lucide.ChefHat, contentDescription = null, tint = c.primary, modifier = Modifier.size(18.dp))
                Text(
                    "Wee Chef tidied ${plural(fixed.size, "thing")}",
                    color = c.ink,
                    fontFamily = NunitoSans,
                    fontSize = 15.sp,
                    fontWeight = FontWeight.Bold,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f).padding(start = 10.dp),
                )
                LinkText(if (open) "Hide" else "See") { open = !open }
                if (checks.canUndo) {
                    Text("·", color = c.inkMuted, modifier = Modifier.padding(horizontal = 6.dp))
                    LinkText("Undo", enabled = !undoing, onClick = onUndo)
                }
            }
            if (open) {
                Column(
                    Modifier.fillMaxWidth().padding(start = 28.dp, bottom = 12.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    fixed.forEach { flag ->
                        Text(fixText(flag), color = c.inkMuted, fontFamily = NunitoSans, fontSize = 14.sp, lineHeight = 20.sp)
                    }
                }
            }
        }
        if (review.isNotEmpty()) {
            if (fixed.isNotEmpty()) HorizontalDivider(color = c.line)
            Row(Modifier.fillMaxWidth().padding(vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                if (fixed.isNotEmpty()) {
                    Spacer(Modifier.width(18.dp))
                } else {
                    Icon(Lucide.ChefHat, contentDescription = null, tint = c.primary, modifier = Modifier.size(18.dp))
                }
                Text(
                    "${plural(review.size, "line")} might need a look",
                    color = c.ink,
                    fontFamily = NunitoSans,
                    fontSize = 15.sp,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f).padding(start = 10.dp),
                )
                LinkText("Edit", onClick = onEdit)
            }
        }
    }
}

/** A `.link`: primary, bold, at least 44dp of tap target. */
@Composable
private fun LinkText(text: String, enabled: Boolean = true, onClick: () -> Unit) {
    val c = Crumb.colors
    Row(
        Modifier
            .heightIn(min = 44.dp)
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
            .padding(horizontal = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text,
            color = if (enabled) c.primary else c.inkMuted,
            fontFamily = NunitoSans,
            fontSize = 15.sp,
            fontWeight = FontWeight.Bold,
        )
    }
}
