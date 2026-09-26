package app.crumb.android.ui.books

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.crumb.android.data.Cookbook
import app.crumb.android.data.CookbookListItem
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.Skeleton
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.DmSerif
import app.crumb.android.ui.theme.NunitoSans
import com.composables.icons.lucide.ArrowRight
import com.composables.icons.lucide.ChefHat
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.X
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

private val Land = CubicBezierEasing(0.2f, 0.8f, 0.2f, 1f)
private val Swing = CubicBezierEasing(0.3f, 0.7f, 0.2f, 1f)

/**
 * A cookbook pulled off the shelf (web/src/components/OpenBook.svelte, phone layout): it flies
 * up to the centre, the cover swings open and away, and the page is its table of contents.
 */
@Composable
fun OpenBook(
    book: CookbookListItem,
    load: suspend (Long) -> Cookbook?,
    onOpenRecipe: (Long) -> Unit,
    onOpenCookbook: (Long) -> Unit,
    onClose: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    val look = bookLook(book.color)
    var details by remember(book.id) { mutableStateOf<Cookbook?>(null) }
    var loading by remember(book.id) { mutableStateOf(true) }
    // 0 = off-screen below, 1 = landed (tilted back, a little small), 2 = open and flat
    val stage = remember { Animatable(0f) }
    val cover = remember { Animatable(0f) }
    val coverFade = remember { Animatable(1f) }

    LaunchedEffect(book.id) {
        launch { details = runCatching { load(book.id) }.getOrNull(); loading = false }
        launch { stage.animateTo(1f, tween(450, easing = Land)) }
        // Let the closed book land before the cover swings open
        delay(380)
        launch { stage.animateTo(2f, tween(600, easing = Land)) }
        launch { cover.animateTo(1f, tween(900, easing = Swing)) }
        delay(600)
        coverFade.animateTo(0f, tween(300))
    }

    fun close() {
        scope.launch {
            launch { coverFade.snapTo(1f) }
            launch { cover.animateTo(0f, tween(450, easing = Swing)) }
            stage.animateTo(1f, tween(450, easing = Land))
            onClose()
        }
    }

    Dialog(onDismissRequest = ::close, properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        BoxWithConstraints(
            Modifier
                .fillMaxSize()
                .background(Color(0xFF0D130F).copy(alpha = 0.6f * stage.value.coerceAtMost(1f)))
                .clickable(remember { MutableInteractionSource() }, indication = null, onClick = ::close)
                .padding(16.dp),
            contentAlignment = Alignment.Center,
        ) {
            // min(86vw, 380px); maxWidth is inside the 16dp padding
            val pageW = minOf((maxWidth + 32.dp) * 0.86f, 380.dp, maxWidth)
            val pageH = minOf(560.dp, maxHeight * 0.8f)
            val drop = maxHeight.value * 0.4f
            Box(
                Modifier
                    .size(pageW, pageH)
                    .graphicsLayer {
                        val s = stage.value
                        cameraDistance = 22 * density
                        if (s <= 1f) {
                            // Flying in: from below, steeply tilted and small, to landed
                            translationY = (1 - s) * drop * density
                            rotationX = 40f + (8f - 40f) * s
                            val k = 0.4f + (0.92f - 0.4f) * s
                            scaleX = k; scaleY = k
                            alpha = s
                        } else {
                            val t = s - 1f
                            rotationX = 8f * (1 - t)
                            val k = 0.92f + 0.08f * t
                            scaleX = k; scaleY = k
                        }
                    }
                    .clickable(remember { MutableInteractionSource() }, indication = null) {},
            ) {
                ContentsPage(book, details, loading, onOpenRecipe, onOpenCookbook)
                Cover(book, look, cover.value, coverFade.value)
                // Close, just above the book
                Box(
                    Modifier
                        .align(Alignment.TopEnd)
                        .offset(y = (-52).dp)
                        .size(44.dp)
                        .clip(CircleShape)
                        .background(Color(0xFFFFFDF8).copy(alpha = 0.16f))
                        .clickable(onClick = ::close),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(Lucide.X, "Close book", tint = Color(0xFFFFFDF8), modifier = Modifier.size(22.dp))
                }
            }
        }
    }
}

/** The front of the cover, hinged on its left edge; past 90° its endpaper faces the reader. */
@Composable
private fun Cover(book: CookbookListItem, look: BookLook, open: Float, fade: Float) {
    val c = Crumb.colors
    val angle = -180f * open
    Box(
        Modifier
            .fillMaxSize()
            .graphicsLayer {
                transformOrigin = TransformOrigin(0f, 0.5f)
                cameraDistance = 22 * density
                rotationY = angle
                translationX = -8.dp.toPx() * open
                alpha = fade
            },
    ) {
        if (angle > -90f) {
            Box(
                Modifier
                    .fillMaxSize()
                    .padding(vertical = 0.dp)
                    .shadow(16.dp, RoundedCornerShape(16.dp))
                    .clip(RoundedCornerShape(16.dp))
                    .background(look.cloth)
                    // A soft shadow along the hinge
                    .drawBehind {
                        drawRect(Brush.horizontalGradient(0f to Color.Black.copy(alpha = 0.22f), 0.07f to Color.Transparent))
                    }
                    .then(if (look.outlined) Modifier.border(1.dp, Color(0x381C2B22), RoundedCornerShape(16.dp)) else Modifier)
                    .padding(28.dp),
            ) {
                // A foil frame with a second hairline 5dp outside it
                Column(
                    Modifier
                        .fillMaxSize()
                        .border(1.dp, look.foil, RoundedCornerShape(3.dp))
                        .padding(5.dp)
                        .border(1.5.dp, look.foil, RoundedCornerShape(3.dp))
                        .padding(20.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(14.dp, Alignment.CenterVertically),
                ) {
                    Icon(Lucide.ChefHat, null, tint = look.foil, modifier = Modifier.size(32.dp))
                    Text(book.name, color = look.foil, fontFamily = DmSerif, fontSize = 26.sp, lineHeight = 30.sp, textAlign = TextAlign.Center)
                    Box(Modifier.fillMaxWidth(0.4f).height(1.dp).background(look.foil))
                    Text(
                        "RECIPES", color = look.foil, fontFamily = NunitoSans, fontSize = 13.sp,
                        fontWeight = FontWeight.Bold, letterSpacing = 0.2.em,
                    )
                }
            }
        } else {
            // Endpaper: paper faintly striped with the cover colour, drawn mirrored so it reads
            val light = lerp(c.paper, look.cloth, 0.12f)
            val lighter = lerp(c.paper, look.cloth, 0.07f)
            Column(
                Modifier
                    .fillMaxSize()
                    .graphicsLayer { rotationY = 180f }
                    .clip(RoundedCornerShape(16.dp))
                    .drawBehind {
                        drawRect(lighter)
                        val band = 10.dp.toPx()
                        var x = -size.height
                        while (x < size.width + size.height) {
                            drawLine(light, Offset(x, 0f), Offset(x + size.height, size.height), strokeWidth = band)
                            x += 2 * band * 1.4142f
                        }
                    }
                    .border(1.dp, c.line, RoundedCornerShape(16.dp))
                    .padding(horizontal = 30.dp, vertical = 34.dp),
            ) {
                Text("EX LIBRIS", style = CrumbText.meta.copy(fontWeight = FontWeight.Bold, letterSpacing = 0.3.em), color = c.inkMuted)
                Text(book.name, fontFamily = DmSerif, fontSize = 24.sp, lineHeight = 28.sp, color = c.ink, modifier = Modifier.padding(top = 12.dp))
                book.description?.takeIf { it.isNotBlank() }?.let {
                    Text(it, style = CrumbText.bodySmall, color = c.ink, modifier = Modifier.padding(top = 12.dp))
                }
                Box(Modifier.weight(1f))
                Text("A cookbook of ${book.recipeCount} recipes", style = CrumbText.meta, color = c.inkMuted)
            }
        }
    }
}

@Composable
private fun ContentsPage(
    book: CookbookListItem,
    details: Cookbook?,
    loading: Boolean,
    onOpenRecipe: (Long) -> Unit,
    onOpenCookbook: (Long) -> Unit,
) {
    val c = Crumb.colors
    Column(
        Modifier
            .fillMaxSize()
            .shadow(16.dp, RoundedCornerShape(16.dp))
            .clip(RoundedCornerShape(16.dp))
            .background(c.paper)
            .padding(start = 26.dp, end = 26.dp, top = 28.dp, bottom = 20.dp),
    ) {
        Row(Modifier.fillMaxWidth().padding(bottom = 16.dp), verticalAlignment = Alignment.Bottom) {
            Text("Contents", style = CrumbText.sectionTitle, color = c.ink, modifier = Modifier.weight(1f))
            Text("${book.recipeCount} recipes", style = CrumbText.meta, color = c.inkMuted)
        }
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState())) {
            when {
                loading -> repeat(5) {
                    Skeleton(Modifier.fillMaxWidth().height(16.dp))
                    Box(Modifier.height(12.dp))
                }
                details?.recipes.isNullOrEmpty() -> Text(
                    "Blank pages, for now. Add recipes from any recipe page or with “Select” on the recipes list.",
                    style = CrumbText.bodySmall, color = c.inkMuted,
                )
                else -> BoxWithConstraints {
                    // The title takes what it needs (leaving room for a leader and the number)
                    val titleMax = maxWidth - 48.dp
                    Column {
                        details!!.recipes.forEachIndexed { i, r ->
                            Row(
                                Modifier
                                    .fillMaxWidth()
                                    .heightIn(min = 44.dp)
                                    .clickable { onOpenRecipe(r.id) }
                                    .padding(vertical = 10.dp),
                                verticalAlignment = Alignment.Bottom,
                                horizontalArrangement = Arrangement.spacedBy(6.dp),
                            ) {
                                Text(
                                    r.title, fontFamily = DmSerif, fontSize = 16.sp, lineHeight = 20.sp, color = c.ink,
                                    modifier = Modifier.widthIn(max = titleMax), maxLines = 2, overflow = TextOverflow.Ellipsis,
                                )
                                // Dotted leader to the page number
                                Box(
                                    Modifier
                                        .weight(1f)
                                        .height(2.dp)
                                        .offset(y = (-4).dp)
                                        .drawBehind {
                                            drawLine(
                                                c.lineStrong, Offset(0f, 0f), Offset(size.width, 0f), strokeWidth = 2.dp.toPx(),
                                                pathEffect = PathEffect.dashPathEffect(floatArrayOf(2.dp.toPx(), 3.dp.toPx())),
                                            )
                                        },
                                )
                                Text("${i + 1}", style = CrumbText.bodySmall, color = c.inkMuted)
                            }
                        }
                    }
                }
            }
        }
        Row(Modifier.fillMaxWidth().padding(top = 16.dp), horizontalArrangement = Arrangement.End) {
            Btn("Open cookbook", { onOpenCookbook(book.id) }, style = BtnStyle.Soft, trailingIcon = Lucide.ArrowRight)
        }
    }
}
