package app.crumb.android.ui.components

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupProperties
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.NunitoSans
import com.composables.icons.lucide.Ellipsis
import com.composables.icons.lucide.Lucide

/** One row of a [CrumbMenu]. Danger items are drawn in the error colour. */
data class MenuItem(val label: String, val icon: ImageVector, val danger: Boolean = false, val onSelect: () -> Unit)

/**
 * The web's ⋯ menu (components/Menu.svelte): an outline icon button that drops a paper menu
 * under itself, right-aligned. [groups] are separated by an inset line. Choosing an item
 * closes the menu first, then runs it. Back, or a tap outside, closes it.
 */
@Composable
fun CrumbMenu(
    groups: List<List<MenuItem>>,
    modifier: Modifier = Modifier,
    label: String = "More actions",
    trigger: (@Composable (open: () -> Unit) -> Unit)? = null,
) {
    var open by remember { mutableStateOf(false) }
    val c = Crumb.colors
    Box(modifier) {
        if (trigger != null) trigger { open = true }
        else Btn(null, { open = true }, style = BtnStyle.Outline, icon = Lucide.Ellipsis, contentDescription = label)
        if (open) {
            Popup(
                alignment = Alignment.TopEnd,
                offset = IntOffset(0, with(androidx.compose.ui.platform.LocalDensity.current) { 50.dp.roundToPx() }),
                onDismissRequest = { open = false },
                properties = PopupProperties(focusable = true),
            ) {
                val shown = remember { MutableTransitionState(false).apply { targetState = true } }
                AnimatedVisibility(shown, enter = fadeIn(tween(160)) + slideInVertically(tween(160)) { 12 }, exit = fadeOut(tween(100))) {
                    Column(
                        Modifier
                            .widthIn(min = 208.dp, max = 288.dp)
                            .shadow(18.dp, CardShape, ambientColor = c.ink, spotColor = c.ink)
                            .clip(CardShape)
                            .background(c.paper)
                            .border(1.dp, c.line, CardShape)
                            .padding(vertical = 4.dp),
                    ) {
                        groups.filter { it.isNotEmpty() }.forEachIndexed { gi, group ->
                            if (gi > 0) HorizontalDivider(Modifier.padding(horizontal = 12.dp, vertical = 4.dp), color = c.line)
                            group.forEach { item ->
                                val ink = if (item.danger) c.error else c.ink
                                Row(
                                    Modifier
                                        .heightIn(min = 44.dp)
                                        .clickable(role = Role.Button) {
                                            open = false
                                            item.onSelect()
                                        }
                                        .padding(horizontal = 14.dp)
                                        .width(IntrinsicMenuWidth),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                                ) {
                                    Icon(item.icon, null, tint = if (item.danger) c.error else c.primary, modifier = Modifier.size(18.dp))
                                    Text(item.label, color = ink, fontFamily = NunitoSans, fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

private val IntrinsicMenuWidth = 240.dp
