package app.crumb.android.ui.recipes

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.crumb.android.data.RecipeSummary
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.RecipePhoto
import app.crumb.android.ui.components.VSpace
import app.crumb.android.ui.theme.Crumb
import com.composables.icons.lucide.Check
import com.composables.icons.lucide.Lucide

/**
 * One card in a recipes grid (web/src/components/RecipeCard.svelte): a bare 4:3 photo, a
 * two-line bold title, then the "Breakfast · British · 30m" line. In select mode a round
 * check badge sits on the photo and a tap toggles instead of opening.
 */
@Composable
fun RecipeCard(
    recipe: RecipeSummary,
    photoUrl: String?,
    modifier: Modifier = Modifier,
    selectable: Boolean = false,
    selected: Boolean = false,
    onToggle: (Long) -> Unit = {},
    onClick: () -> Unit,
) {
    val c = Crumb.colors
    Column(
        modifier
            .fillMaxWidth()
            .clickable(role = Role.Button) { if (selectable) onToggle(recipe.id) else onClick() },
    ) {
        Box(Modifier.fillMaxWidth().aspectRatio(4f / 3f).clip(ControlShape)) {
            RecipePhoto(photoUrl, recipe.image, Modifier.fillMaxSize(), contentDescription = recipe.title, placeholderId = recipe.id)
            if (selectable) {
                Box(
                    Modifier
                        .align(Alignment.TopStart)
                        .padding(10.dp)
                        .size(28.dp)
                        .clip(CircleShape)
                        .background(if (selected) c.tile else Color.Black.copy(alpha = 0.25f))
                        .border(2.dp, Color.White, CircleShape),
                    contentAlignment = Alignment.Center,
                ) {
                    if (selected) Icon(Lucide.Check, contentDescription = null, tint = c.onTile, modifier = Modifier.size(16.dp))
                }
            }
        }
        VSpace(10.dp)
        Text(recipe.title, style = CrumbText.rowTitle, color = c.ink, maxLines = 2, overflow = TextOverflow.Ellipsis)
        val sub = cardSubtitle(recipe)
        if (sub.isNotEmpty()) {
            Text(
                sub,
                style = CrumbText.meta,
                color = c.inkMuted,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(top = 2.dp),
            )
        }
    }
}
