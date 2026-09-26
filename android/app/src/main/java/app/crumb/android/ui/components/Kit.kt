package app.crumb.android.ui.components

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.scale
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import kotlin.math.sqrt
import app.crumb.android.ui.theme.Caveat
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.DmSerif
import app.crumb.android.ui.theme.NunitoSans
import com.composables.icons.lucide.ChevronRight
import com.composables.icons.lucide.Lucide

// The web's shared classes (web/src/styles/app.css) as Compose pieces. Screens build from these
// rather than Material defaults, so the phone app looks like the web app. Sizes are the CSS
// rem values at 16px = 16sp/dp.

/** Text styles: `.page-title`, `.section-title`, `.meta`, `.kicker`, `.label`, `.hint`, `.hand`. */
object CrumbText {
    val pageTitle = TextStyle(fontFamily = DmSerif, fontSize = 30.sp, lineHeight = 31.5.sp)
    val sectionTitle = TextStyle(fontFamily = DmSerif, fontSize = 24.sp, lineHeight = 27.6.sp)
    val body = TextStyle(fontFamily = NunitoSans, fontSize = 16.sp, lineHeight = 24.sp)
    val bodySmall = TextStyle(fontFamily = NunitoSans, fontSize = 14.sp, lineHeight = 20.sp)
    val meta = TextStyle(fontFamily = NunitoSans, fontSize = 13.sp, lineHeight = 18.sp, fontWeight = FontWeight.SemiBold)
    val kicker = TextStyle(
        fontFamily = NunitoSans, fontSize = 13.sp, lineHeight = 18.sp,
        fontWeight = FontWeight.Bold, letterSpacing = 0.04.em,
    )
    val label = TextStyle(fontFamily = NunitoSans, fontSize = 15.sp, lineHeight = 20.sp, fontWeight = FontWeight.Bold)
    val rowTitle = TextStyle(fontFamily = NunitoSans, fontSize = 16.sp, lineHeight = 22.sp, fontWeight = FontWeight.Bold)
    val hint = TextStyle(fontFamily = NunitoSans, fontSize = 13.sp, lineHeight = 18.sp)
    /** Handwriting: greetings and the cook's own notes, nothing else. */
    val hand = TextStyle(fontFamily = Caveat, fontWeight = FontWeight.Bold)
}

enum class BtnStyle { Primary, Tile, Soft, Neutral, Outline, Ghost, Danger, Link }

/** `.btn-sm` 40, `.btn` 44, `.btn-lg` 48, `.btn-xl` 56. */
enum class BtnSize(val height: Dp, val padding: Dp, val font: Int, val icon: Dp, val gap: Dp) {
    Sm(40.dp, 14.dp, 14, 16.dp, 6.dp),
    Md(44.dp, 18.dp, 15, 18.dp, 8.dp),
    Lg(48.dp, 20.dp, 16, 18.dp, 8.dp),
    Xl(56.dp, 24.dp, 17, 20.dp, 8.dp),
}

/**
 * `.btn` in all its variants. Butter ([BtnStyle.Primary]) is only the one main action on a
 * screen. Pass [text] null for a square icon button (`.btn-icon`). Presses scale to 0.97.
 */
@Composable
fun Btn(
    text: String?,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    style: BtnStyle = BtnStyle.Neutral,
    size: BtnSize = BtnSize.Md,
    icon: ImageVector? = null,
    trailingIcon: ImageVector? = null,
    enabled: Boolean = true,
    contentDescription: String? = null,
) {
    val c = Crumb.colors
    val (bg, fg) = when (style) {
        BtnStyle.Primary -> c.butter to c.onButter
        BtnStyle.Tile -> c.tile to c.onTile
        BtnStyle.Soft -> c.tint to c.primary
        BtnStyle.Neutral -> c.tint to c.ink
        BtnStyle.Outline -> c.paper to c.ink
        BtnStyle.Ghost -> Color.Transparent to c.inkMuted
        BtnStyle.Danger -> c.error to c.paper
        BtnStyle.Link -> Color.Transparent to c.primary
    }
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val scale by animateFloatAsState(if (pressed && enabled) 0.97f else 1f, label = "press")
    val pressedBg = when (style) {
        BtnStyle.Primary -> c.butterHover
        BtnStyle.Soft, BtnStyle.Neutral -> c.accented
        BtnStyle.Outline, BtnStyle.Ghost -> c.tint
        BtnStyle.Tile -> lerp(c.tile, Color.Black, 0.12f)
        else -> bg
    }
    val fill = when {
        !enabled && style != BtnStyle.Link && style != BtnStyle.Ghost -> c.tint
        pressed -> pressedBg
        else -> bg
    }
    val ink = if (enabled) fg else c.inkMuted
    val iconOnly = text == null
    Row(
        modifier
            .scale(scale)
            .then(if (style == BtnStyle.Link) Modifier else Modifier.height(size.height))
            .then(if (iconOnly) Modifier.width(size.height) else Modifier)
            .clip(ControlShape)
            .background(fill)
            .then(
                if (style == BtnStyle.Outline && enabled) Modifier.border(1.dp, c.lineStrong, ControlShape) else Modifier,
            )
            .clickable(interaction, indication = null, enabled = enabled, role = Role.Button, onClick = onClick)
            .padding(horizontal = if (iconOnly || style == BtnStyle.Link) 0.dp else size.padding),
        horizontalArrangement = Arrangement.spacedBy(size.gap, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) Icon(icon, contentDescription = if (iconOnly) contentDescription ?: text else null, tint = ink, modifier = Modifier.size(size.icon))
        if (text != null) {
            Text(text, color = ink, fontFamily = NunitoSans, fontSize = size.font.sp, fontWeight = FontWeight.Bold, maxLines = 1)
        }
        if (trailingIcon != null) Icon(trailingIcon, contentDescription = null, tint = ink, modifier = Modifier.size(size.icon))
    }
}

/** `.card`: paper, a 1px line, 16dp corners, no shadow. */
@Composable
fun Card(modifier: Modifier = Modifier, padding: PaddingValues = PaddingValues(0.dp), content: @Composable ColumnScope.() -> Unit) {
    val c = Crumb.colors
    Column(
        modifier
            .clip(CardShape)
            .background(c.paper)
            .border(1.dp, c.line, CardShape)
            .padding(padding),
        content = content,
    )
}

/** `.list-card`: a card of [ListRow]s with inset dividers between them. */
@Composable
fun ListCard(modifier: Modifier = Modifier, rows: List<@Composable () -> Unit>) {
    val c = Crumb.colors
    Card(modifier) {
        rows.forEachIndexed { i, row ->
            if (i > 0) HorizontalDivider(Modifier.padding(horizontal = 12.dp), thickness = 1.dp, color = c.line)
            row()
        }
    }
}

/**
 * `.list-row`: a 44dp icon well (or a plain icon), a bold title with an optional muted line,
 * and a chevron when it opens something (or [trailing] instead). At least 68dp tall.
 */
@Composable
fun ListRow(
    title: String,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    icon: ImageVector? = null,
    plainIcon: Boolean = false,
    onClick: (() -> Unit)? = null,
    chevron: Boolean = onClick != null,
    trailing: (@Composable RowScope.() -> Unit)? = null,
) {
    val c = Crumb.colors
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 68.dp)
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        if (icon != null) {
            if (plainIcon) Icon(icon, null, tint = c.ink, modifier = Modifier.size(22.dp).padding(start = 2.dp))
            else Well(icon)
        }
        Column(Modifier.weight(1f)) {
            Text(title, style = CrumbText.rowTitle, color = c.ink)
            if (subtitle != null) Text(subtitle, style = CrumbText.bodySmall, color = c.inkMuted)
        }
        when {
            trailing != null -> trailing()
            chevron -> Icon(Lucide.ChevronRight, null, tint = c.inkMuted, modifier = Modifier.size(20.dp))
        }
    }
}

/** `.well`: a 44dp tint square holding a 22dp tile-green icon. */
@Composable
fun Well(icon: ImageVector, modifier: Modifier = Modifier, size: Dp = 44.dp) {
    val c = Crumb.colors
    Box(modifier.size(size).clip(ControlShape).background(c.tint), contentAlignment = Alignment.Center) {
        Icon(icon, null, tint = c.primary, modifier = Modifier.size(size / 2))
    }
}

/** `.chip`: a 40dp pill. Selected chips are tile green. */
@Composable
fun Chip(text: String, selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier, icon: ImageVector? = null) {
    val c = Crumb.colors
    Row(
        modifier
            .height(40.dp)
            .clip(RoundedCornerShape(50))
            .background(if (selected) c.tile else c.tint)
            .clickable(role = Role.Checkbox, onClick = onClick)
            .padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        val ink = if (selected) c.onTile else c.ink
        if (icon != null) Icon(icon, null, tint = ink, modifier = Modifier.size(16.dp))
        Text(text, color = ink, fontFamily = NunitoSans, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1)
    }
}

/** `.skeleton`: a tint block that pulses while something loads. */
@Composable
fun Skeleton(modifier: Modifier = Modifier, shape: androidx.compose.ui.graphics.Shape = ControlShape) {
    val pulse = rememberInfiniteTransition(label = "skeleton")
    val alpha by pulse.animateFloat(1f, 0.55f, infiniteRepeatable(tween(800), RepeatMode.Reverse), label = "alpha")
    Box(modifier.alpha(alpha).clip(shape).background(Crumb.colors.tint))
}

/** `.progress`: a 4dp tint track with a tile-green fill. */
@Composable
fun ProgressBar(fraction: Float, modifier: Modifier = Modifier) {
    val c = Crumb.colors
    Box(modifier.fillMaxWidth().height(4.dp).clip(RoundedCornerShape(50)).background(c.tint)) {
        Box(Modifier.fillMaxWidth(fraction.coerceIn(0f, 1f)).height(4.dp).clip(RoundedCornerShape(50)).background(c.tile))
    }
}

/** A section heading with an optional "See all"-style link on the right, as on Home. */
@Composable
fun SectionHeader(title: String, modifier: Modifier = Modifier, action: String? = null, onAction: (() -> Unit)? = null) {
    val c = Crumb.colors
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.Bottom) {
        Text(title, style = CrumbText.sectionTitle, color = c.ink, modifier = Modifier.weight(1f))
        if (action != null && onAction != null) {
            Text(
                action,
                color = c.primary, fontFamily = NunitoSans, fontSize = 15.sp, fontWeight = FontWeight.Bold,
                modifier = Modifier.clip(ControlShape).clickable(onClick = onAction).padding(4.dp),
                maxLines = 1, overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/** Uppercase muted label over a group of rows (the More page's "YOUR RECIPES"). */
@Composable
fun GroupLabel(text: String, modifier: Modifier = Modifier) {
    Text(
        text.uppercase(),
        style = CrumbText.kicker.copy(letterSpacing = 0.08.em),
        color = Crumb.colors.inkMuted,
        modifier = modifier.padding(start = 24.dp, bottom = 10.dp),
    )
}

/**
 * `.photo-empty`: the striped stand-in for a recipe without a photo. 135° stripes, 10dp of
 * tint then 10dp of tint mixed 45% towards paper; 45° lines repeat every 20dp·√2 across.
 */
fun Modifier.photoEmpty(tint: Color, paper: Color): Modifier = drawBehind {
    drawRect(tint)
    val stripe = lerp(tint, paper, 0.45f)
    val band = 10.dp.toPx()
    val across = 2 * band * sqrt(2f)
    var x = -size.height
    while (x < size.width + size.height) {
        drawLine(stripe, Offset(x, size.height), Offset(x + size.height, 0f), strokeWidth = band)
        x += across
    }
}

@Composable
fun VSpace(height: Dp) = Spacer(Modifier.height(height))

@Composable
fun HSpace(width: Dp) = Spacer(Modifier.width(width))

/** A 1px line border in the theme's line colour, for ad-hoc surfaces. */
@Composable
fun lineBorder(): BorderStroke = BorderStroke(1.dp, Crumb.colors.line)

/** components/EmptyState.svelte: a card with a 64dp icon well, a serif title, a line and actions. */
@Composable
fun EmptyState(
    icon: ImageVector,
    title: String,
    modifier: Modifier = Modifier,
    description: String? = null,
    actions: (@Composable RowScope.() -> Unit)? = null,
) {
    val c = Crumb.colors
    Card(modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 24.dp, vertical = 48.dp)) {
        Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
            Well(icon, size = 64.dp)
            Text(title, style = CrumbText.sectionTitle, color = c.ink, modifier = Modifier.padding(top = 16.dp), textAlign = TextAlign.Center)
            if (description != null) {
                Text(
                    description, style = CrumbText.bodySmall, color = c.inkMuted, textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 6.dp).widthIn(max = 384.dp),
                )
            }
            if (actions != null) {
                Row(Modifier.padding(top = 20.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), content = actions)
            }
        }
    }
}
