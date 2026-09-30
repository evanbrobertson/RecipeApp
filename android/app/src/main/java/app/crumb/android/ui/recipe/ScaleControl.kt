package app.crumb.android.ui.recipe

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.crumb.android.data.RecipeSession
import app.crumb.android.ui.components.CardShape
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.NunitoSans

/**
 * The scale toggle (web/src/components/ScaleControl.svelte): a tint pill holding ½× 1× 2× 3×,
 * the chosen one a tile-green pill. Shown beside "Ingredients" only when there are any.
 */
@Composable
fun ScaleControl(scale: Double, onChange: (Double) -> Unit, modifier: Modifier = Modifier) {
    val c = Crumb.colors
    Row(
        modifier
            .clip(CardShape)
            .background(c.tint)
            .padding(4.dp),
        horizontalArrangement = Arrangement.spacedBy(2.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RecipeSession.Presets.forEach { (value, label) ->
            val selected = scale == value
            Box(
                Modifier
                    .height(44.dp)
                    .defaultMinSize(minWidth = 44.dp)
                    .clip(ControlShape)
                    .background(if (selected) c.tile else c.tint)
                    .clickable(role = Role.RadioButton) { onChange(value) }
                    .semantics { this.selected = selected }
                    .padding(horizontal = 10.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    label,
                    color = if (selected) c.onTile else c.inkMuted,
                    fontFamily = NunitoSans,
                    fontSize = 15.sp,
                    fontWeight = FontWeight.Bold,
                )
            }
        }
    }
}
