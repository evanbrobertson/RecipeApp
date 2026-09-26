package app.crumb.android.ui.prep

import android.content.Context
import android.os.VibrationEffect
import android.os.Vibrator
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.crumb.android.data.Recipe
import app.crumb.android.data.RecipeSession
import app.crumb.android.ui.LocalNav
import app.crumb.android.ui.UiState
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnSize
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.Card
import app.crumb.android.ui.components.CardShape
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.EmptyState
import app.crumb.android.ui.components.Loading
import app.crumb.android.ui.components.Message
import app.crumb.android.ui.components.OfflineNote
import app.crumb.android.ui.components.ProgressBar
import app.crumb.android.ui.components.VSpace
import app.crumb.android.ui.components.Well
import app.crumb.android.ui.recipe.ScaleControl
import app.crumb.android.ui.recipe.recipeViewModel
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.NunitoSans
import app.crumb.core.MiseItem
import app.crumb.core.miseEnPlace
import com.composables.icons.lucide.ArrowLeft
import com.composables.icons.lucide.Check
import com.composables.icons.lucide.Flame
import com.composables.icons.lucide.Hand
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Slice
import com.composables.icons.lucide.Soup

/**
 * Mise en place (web/src/islands/PrepPage.svelte, spec §6): every ingredient in a vessel sized
 * to its quantity, grouped by what to get out, tapped ready before the heat goes on.
 */
@Composable
fun PrepScreen(id: Long) {
    val vm = recipeViewModel(id)
    val state by vm.state.collectAsStateWithLifecycle()
    val nav = LocalNav.current

    // Keep the screen on for the whole of prep; Android has no wake-lock gesture to tap
    val view = LocalView.current
    DisposableEffect(view) {
        view.keepScreenOn = true
        onDispose { view.keepScreenOn = false }
    }

    LaunchedEffect(state) {
        if ((state as? UiState.Failed)?.signedOut == true) nav.signedOut()
    }

    when (val s = state) {
        UiState.Loading -> Loading()
        is UiState.Failed -> Message(
            "Couldn't open this recipe",
            s.message,
            action = { Btn("Try again", { vm.load() }, style = BtnStyle.Outline) },
        )
        is UiState.Ready -> PrepContent(s.value, s.offline)
    }
}

@Composable
private fun PrepContent(recipe: Recipe, offline: Boolean) {
    val c = Crumb.colors
    val nav = LocalNav.current
    val context = LocalContext.current

    // Vessel classification is crumb-core's miseEnPlace, never re-done here
    val items = remember(recipe) {
        val lines = recipe.ingredients.flatMap { it.items }
        runCatching { miseEnPlace(lines) }.getOrElse { emptyList() }
    }
    val ready = RecipeSession.prepReady(recipe.id)
    val scale = RecipeSession.scale(recipe.id)
    val progress = if (items.isEmpty()) 0f else ready.size.toFloat() / items.size
    val allReady = items.isNotEmpty() && ready.size == items.size

    fun toggle(index: Int) {
        val key = index.toString()
        val next = if (key in ready) ready - key else ready + key
        RecipeSession.setPrepReady(recipe.id, next)
        if (items.isNotEmpty() && next.size == items.size) vibrate(context)
    }

    Box(Modifier.fillMaxSize()) {
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .statusBarsPadding()
                .padding(horizontal = 20.dp),
        ) {
            VSpace(8.dp)
            if (offline) {
                OfflineNote()
                VSpace(12.dp)
            }
            BackLink(recipe.title) { nav.back() }
            Text("Mise en place", style = CrumbText.pageTitle, color = c.ink, modifier = Modifier.padding(top = 4.dp))
            Quote()
            val counts = getOutCounts(items)
            if (counts.isNotEmpty()) {
                GetOutCard(counts, scale) { RecipeSession.setScale(recipe.id, it) }
            }
            VSpace(16.dp)
            ProgressRow(ready.size, items.size, progress)
            VSpace(20.dp)
            if (items.isEmpty()) {
                EmptyState(Lucide.Soup, "No ingredients to prep", modifier = Modifier.fillMaxWidth()) {
                    Btn("Back to recipe", { nav.recipe(recipe.id) }, style = BtnStyle.Soft)
                }
            } else {
                Countertop(items, ready, scale) { toggle(it) }
            }
            VSpace(96.dp)
        }
        if (allReady) {
            Btn(
                "Everything's in its place. Start cooking",
                { nav.cook(recipe.id) },
                style = BtnStyle.Primary,
                size = BtnSize.Xl,
                icon = Lucide.Flame,
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .padding(bottom = 16.dp)
                    .shadow(8.dp, ControlShape),
            )
        }
    }
}

/** "← Recipe title", 15sp muted, back to the recipe. */
@Composable
private fun BackLink(title: String, onClick: () -> Unit) {
    val c = Crumb.colors
    Row(
        Modifier
            .clip(ControlShape)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Icon(Lucide.ArrowLeft, null, tint = c.inkMuted, modifier = Modifier.size(18.dp))
        Text(
            title,
            style = CrumbText.bodySmall.copy(fontSize = 15.sp, fontWeight = FontWeight.SemiBold),
            color = c.inkMuted,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
private fun Quote() {
    val c = Crumb.colors
    val text = buildAnnotatedString {
        withStyle(SpanStyle(fontStyle = FontStyle.Italic)) { append("“Everything in its place.”") }
        append(" Prep and measure it all before you start cooking. Tap each one when it's ready.")
    }
    Text(text, style = CrumbText.body, color = c.inkMuted, modifier = Modifier.padding(top = 8.dp))
}

/** The "Get out:" card (spec §6.2), with the scale control bottom-right on a phone. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun GetOutCard(counts: List<VesselCount>, scale: Double, onScale: (Double) -> Unit) {
    val c = Crumb.colors
    Card(
        Modifier.fillMaxWidth().padding(top = 16.dp),
        padding = PaddingValues(horizontal = 16.dp, vertical = 12.dp),
    ) {
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text("Get out:", style = CrumbText.label, color = c.ink)
            counts.forEach { count ->
                val text = buildAnnotatedString {
                    withStyle(SpanStyle(fontWeight = FontWeight.Bold, color = c.ink)) { append("${count.count}") }
                    append(" × ${getOutNoun(count)}")
                }
                Text(text, style = CrumbText.label.copy(fontWeight = FontWeight.Normal), color = c.inkMuted)
            }
        }
        Row(Modifier.fillMaxWidth().padding(top = 10.dp), horizontalArrangement = Arrangement.End) {
            ScaleControl(scale, onScale)
        }
    }
}

@Composable
private fun ProgressRow(done: Int, total: Int, progress: Float) {
    val c = Crumb.colors
    Row(
        Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        ProgressBar(progress, Modifier.weight(1f))
        Text(
            "$done/$total ready",
            style = CrumbText.label.copy(fontFeatureSettings = "tnum"),
            color = c.ink,
        )
    }
}

/** The speckled sunk countertop all three sections sit on, as the web's `.counter`. */
@Composable
private fun Countertop(items: List<MiseItem>, ready: Set<String>, scale: Double, onToggle: (Int) -> Unit) {
    val c = Crumb.colors
    val groups = remember(items) { prepGroups(items) }
    Column(
        Modifier
            .fillMaxWidth()
            .clip(CardShape)
            .background(c.sunk)
            .border(1.dp, c.line, CardShape)
            .counterTexture(c.dark)
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(24.dp),
    ) {
        groups.forEach { group ->
            Column {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    Well(groupIcon(group.kind))
                    Text(groupTitle(group.kind), style = CrumbText.sectionTitle, color = c.ink)
                }
                VSpace(12.dp)
                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    group.entries.chunked(2).forEach { row ->
                        Row(
                            Modifier.fillMaxWidth().height(IntrinsicSize.Max),
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            row.forEach { entry ->
                                ItemCard(
                                    entry = entry,
                                    ready = entry.index.toString() in ready,
                                    scale = scale,
                                    onClick = { onToggle(entry.index) },
                                    modifier = Modifier.weight(1f).fillMaxHeight(),
                                )
                            }
                            if (row.size == 1) Spacer(Modifier.weight(1f))
                        }
                    }
                }
            }
        }
    }
}

/** One ingredient card: vessel drawing, amount, name, task and vessel label. Tapping readies it. */
@Composable
private fun ItemCard(entry: PrepEntry, ready: Boolean, scale: Double, onClick: () -> Unit, modifier: Modifier) {
    val c = Crumb.colors
    val item = entry.item
    val badgeScale by animateFloatAsState(if (ready) 1f else 0.75f, tween(150), label = "prep-badge")
    Box(
        modifier
            .clip(CardShape)
            .background(if (ready) c.tint else c.paper)
            .then(if (ready) Modifier else Modifier.border(1.dp, c.line, CardShape))
            .clickable(role = Role.Checkbox, onClick = onClick)
            .padding(horizontal = 8.dp, vertical = 12.dp),
    ) {
        val badgeShape = RoundedCornerShape(50)
        Box(
            Modifier
                .align(Alignment.TopEnd)
                .scale(badgeScale)
                .size(28.dp)
                .clip(badgeShape)
                .then(if (ready) Modifier.background(c.tile) else Modifier.border(2.dp, c.lineStrong, badgeShape)),
            contentAlignment = Alignment.Center,
        ) {
            if (ready) Icon(Lucide.Check, null, tint = c.onTile, modifier = Modifier.size(16.dp))
        }
        Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
            Box(Modifier.height(112.dp), contentAlignment = Alignment.BottomCenter) {
                PrepBowl(item.vessel, ready, ingredientColor(item.name))
            }
            Text(
                prepAmount(item, scale).ifEmpty { " " },
                style = TextStyle(
                    fontFamily = NunitoSans,
                    fontSize = 18.sp,
                    lineHeight = 22.sp,
                    fontWeight = FontWeight.Bold,
                    fontFeatureSettings = "tnum",
                ),
                color = c.ink,
                textAlign = TextAlign.Center,
                maxLines = 1,
                modifier = Modifier.padding(top = 8.dp),
            )
            Text(
                item.name,
                style = CrumbText.body,
                color = if (ready) c.inkMuted else c.ink,
                textDecoration = if (ready) TextDecoration.LineThrough else null,
                textAlign = TextAlign.Center,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(top = 2.dp),
            )
            when {
                item.task != null -> TaskPill(item.task.orEmpty())
                item.prep != null -> Text(
                    item.prep.orEmpty(),
                    style = CrumbText.hint,
                    color = c.inkMuted,
                    textAlign = TextAlign.Center,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(top = 4.dp),
                )
            }
            Text(
                vesselLabel(item.vessel).uppercase(),
                style = CrumbText.meta.copy(letterSpacing = 0.05.em),
                color = c.inkMuted,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(top = 6.dp),
            )
        }
    }
}

/** The clay "Dice"-style pill for a knife task (web `.task`). */
@Composable
private fun TaskPill(task: String) {
    val dark = Crumb.colors.dark
    val label = if (dark) Color(0xFFE6AD95) else Color(0xFF8F4B34)
    val bg = if (dark) Color(0x42A55A40) else Color(0x1FA55A40)
    Box(
        Modifier
            .padding(top = 6.dp)
            .clip(RoundedCornerShape(50))
            .background(bg)
            .padding(horizontal = 10.dp, vertical = 2.dp),
    ) {
        Text(task.capitalizeWords(), style = CrumbText.hint.copy(fontWeight = FontWeight.Bold), color = label)
    }
}

private fun String.capitalizeWords(): String =
    split(" ").joinToString(" ") { it.replaceFirstChar { ch -> ch.uppercase() } }

private fun groupTitle(kind: PrepGroupKind): String = when (kind) {
    PrepGroupKind.Chop -> "Chop & prep"
    PrepGroupKind.Measure -> "Measure into bowls"
    PrepGroupKind.Reach -> "Keep within reach"
}

private fun groupIcon(kind: PrepGroupKind): ImageVector = when (kind) {
    PrepGroupKind.Chop -> Lucide.Slice
    PrepGroupKind.Measure -> Lucide.Soup
    PrepGroupKind.Reach -> Lucide.Hand
}

/** A short buzz when everything is ready, as the web's `navigator.vibrate(60)`. */
private fun vibrate(context: Context) {
    val vibrator = context.getSystemService(Context.VIBRATOR_SERVICE) as? Vibrator ?: return
    vibrator.vibrate(VibrationEffect.createOneShot(60, VibrationEffect.DEFAULT_AMPLITUDE))
}

/** The cream speckled countertop texture (web `.counter`), two dot grids at 14dp and 19dp. */
private fun Modifier.counterTexture(dark: Boolean): Modifier = drawBehind {
    val base = if (dark) Color.White else Color(0xFF1C2B22)
    drawDotGrid(14.dp.toPx(), 2.8f, 4.2f, 1.dp.toPx(), base.copy(alpha = 0.05f))
    drawDotGrid(19.dp.toPx(), 3.8f, 5.7f, 1.dp.toPx(), base.copy(alpha = 0.04f))
}

private fun DrawScope.drawDotGrid(spacing: Float, offsetX: Float, offsetY: Float, radius: Float, color: Color) {
    var y = offsetY
    while (y < size.height) {
        var x = offsetX
        while (x < size.width) {
            drawCircle(color, radius = radius, center = Offset(x, y))
            x += spacing
        }
        y += spacing
    }
}
