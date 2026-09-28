package app.crumb.android.ui.books

import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.crumb.android.data.CookbookListItem
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.DmSerif
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Plus
import kotlin.math.ceil
import kotlin.math.max
import kotlin.math.min

// The shelf from web/src/components/Bookshelf.svelte and CookbookSpine.svelte: cookbooks lie
// flat in towers, spines to the reader, on tile-green planks, with a herb pot on the shortest
// tower. Sits on a tint backdrop; the planks run to its edges.

private val Settle = CubicBezierEasing(0.2f, 0.8f, 0.2f, 1f)

/**
 * The whole shelf. On a phone each shelf holds one tall, centred tower of up to ten books;
 * [single] is Home's compact row of short towers (four books each) that scrolls sideways.
 * [pulledId] slides that book out, as it flies off to open. [onAdd] shows the dashed
 * new-book slot on top of the last tower.
 */
@Composable
fun Bookshelf(
    books: List<CookbookListItem>,
    onOpen: (CookbookListItem) -> Unit,
    modifier: Modifier = Modifier,
    pulledId: Long? = null,
    single: Boolean = false,
    onAdd: (() -> Unit)? = null,
) {
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val full = maxWidth
        val width = full.value
        val towerLen = if (single) 172f else min(300f, width - 88f)
        val shelves = remember(books, single, onAdd != null) {
            val n = books.size
            when {
                n == 0 -> if (onAdd != null) listOf(listOf(emptyList())) else emptyList()
                single -> listOf(stackBooks(books, ceil(n / 4.0).toInt()))
                else -> stackBooks(books, ceil(n / 10.0).toInt()).map { listOf(it) }
            }
        }
        val lastShelf = shelves.lastIndex
        val lastTower = (shelves.lastOrNull()?.size ?: 1) - 1
        // The pot sits on the first shelf's shortest tower, not the one with the new-book slot
        // (unless that's the only one)
        val potTower = remember(shelves) {
            val first = shelves.firstOrNull().orEmpty()
            val skip = if (onAdd != null && lastShelf == 0 && first.size > 1) lastTower else -1
            first.indices.filter { it != skip }
                .minByOrNull { i -> first[i].sumOf { bookSize(it).thickness } } ?: -1
        }

        Column(verticalArrangement = Arrangement.spacedBy(20.dp)) {
            shelves.forEachIndexed { si, shelf ->
                Column(if (single) Modifier.horizontalScroll(rememberScrollState()) else Modifier.fillMaxWidth()) {
                    Row(
                        Modifier
                            .then(if (single) Modifier.widthIn(min = full) else Modifier.fillMaxWidth())
                            .padding(horizontal = if (single) 20.dp else 16.dp),
                        horizontalArrangement = if (single) Arrangement.spacedBy(28.dp) else Arrangement.Center,
                        verticalAlignment = Alignment.Bottom,
                    ) {
                        shelf.forEachIndexed { ti, tower ->
                            Tower(
                                tower = tower,
                                towerLen = towerLen.dp,
                                pot = si == 0 && ti == potTower,
                                newBook = if (onAdd != null && si == lastShelf && ti == lastTower) onAdd else null,
                                pulledId = pulledId,
                                onOpen = onOpen,
                            )
                        }
                    }
                    // A flat tile-green plank
                    Box(
                        Modifier
                            .then(if (single) Modifier.widthIn(min = full).fillMaxWidth() else Modifier.fillMaxWidth())
                            .height(12.dp)
                            .background(Crumb.colors.tile),
                    )
                }
            }
        }
    }
}

/** A stack of books lying flat, listed top to bottom. */
@Composable
private fun Tower(
    tower: List<CookbookListItem>,
    towerLen: Dp,
    pot: Boolean,
    newBook: (() -> Unit)?,
    pulledId: Long?,
    onOpen: (CookbookListItem) -> Unit,
) {
    Column(Modifier.width(towerLen + 16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        if (pot) {
            // Off-centre, as if someone put it down in passing
            Row(Modifier.fillMaxWidth()) {
                Box(Modifier.fillMaxWidth(0.28f))
                HerbPot(Modifier.size(44.dp, 66.dp).graphicsLayer { translationY = 1.dp.toPx() })
            }
        }
        if (newBook != null) NewBookSlot(towerLen, newBook)
        tower.forEachIndexed { i, book ->
            Spine(book, towerLen, atFoot = i == tower.lastIndex, pulled = pulledId == book.id) { onOpen(book) }
        }
    }
}

/** A book lying flat with its spine to the reader: the title reads left to right. */
@Composable
fun Spine(book: CookbookListItem, towerLen: Dp, atFoot: Boolean, pulled: Boolean, onOpen: () -> Unit) {
    val look = bookLook(book.color)
    val size = bookSize(book)
    val lean = bookLean(book, atFoot)
    val band = spineBand(book)
    val c = Crumb.colors
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    // Pressed slides it out a little towards the reader's hand; pulled slides it right out
    val shift by animateFloatAsState(
        when { pulled -> lean.nudge + 40f; pressed -> lean.nudge + 14f; else -> lean.nudge.toFloat() },
        tween(350, easing = Settle), label = "shift",
    )
    val tilt by animateFloatAsState(
        when { pulled -> 0f; pressed -> lean.tilt * 0.4f; else -> lean.tilt },
        tween(350, easing = Settle), label = "tilt",
    )
    // Its own length, stretched to fit the title, but never much past the tower's longest
    val width = min(towerLen.value * 1.06f, max(towerLen.value * size.length.toFloat(), size.title.toFloat())).dp
    val hairline = c.ink.copy(alpha = 0.09f)
    Box(
        Modifier
            .graphicsLayer {
                translationX = shift.dp.toPx()
                rotationZ = tilt
            }
            .size(width, size.thickness.dp)
            .clip(RoundedCornerShape(3.dp))
            .drawBehind {
                drawRect(look.cloth)
                if (band != null) {
                    val inset = 10.dp.toPx()
                    val w = 2.dp.toPx()
                    drawRect(band, Offset(inset, 0f), Size(w, this.size.height))
                    drawRect(band, Offset(this.size.width - inset - w, 0f), Size(w, this.size.height))
                }
                // The board edge, a shade darker than the spine cloth
                val edge = 3.dp.toPx()
                drawRect(look.shade, Offset(0f, this.size.height - edge), Size(this.size.width, edge))
                // A faint hairline keeps dark covers visible on a dark shelf; cream gets a firmer edge
                val px = 1.dp.toPx()
                drawRoundRect(
                    if (look.outlined) Color(0x381C2B22) else hairline,
                    topLeft = Offset(px / 2, px / 2),
                    size = Size(this.size.width - px, this.size.height - px),
                    cornerRadius = CornerRadius(3.dp.toPx()),
                    style = Stroke(px),
                )
            }
            .clickable(interaction, indication = null, role = Role.Button, onClick = onOpen)
            .semantics { contentDescription = "Open ${book.name}, ${book.recipeCount} recipes" }
            .padding(start = if (band != null) 26.dp else 14.dp, end = if (band != null) 26.dp else 14.dp, bottom = 2.dp),
        contentAlignment = Alignment.CenterStart,
    ) {
        Text(
            book.name,
            color = look.foil,
            fontFamily = DmSerif,
            fontSize = 14.sp,
            lineHeight = 16.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/** A dashed outline of a book, waiting on top of the last stack. */
@Composable
private fun NewBookSlot(towerLen: Dp, onAdd: () -> Unit) {
    val c = Crumb.colors
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val lift by animateFloatAsState(if (pressed) -3f else 0f, tween(180), label = "lift")
    val ink = if (pressed) c.primary else c.inkMuted
    val dash = if (pressed) c.primary else c.lineStrong
    Box(
        Modifier
            .padding(bottom = 4.dp)
            .graphicsLayer {
                translationX = (-4).dp.toPx()
                translationY = lift.dp.toPx()
                rotationZ = -1.5f
            }
            .size(towerLen * 0.82f, 44.dp)
            .drawBehind {
                val w = 2.dp.toPx()
                drawRoundRect(
                    dash,
                    topLeft = Offset(w / 2, w / 2),
                    size = Size(size.width - w, size.height - w),
                    cornerRadius = CornerRadius(3.dp.toPx()),
                    style = Stroke(w, pathEffect = PathEffect.dashPathEffect(floatArrayOf(6.dp.toPx(), 4.dp.toPx()))),
                )
            }
            .clickable(interaction, indication = null, role = Role.Button, onClick = onAdd)
            .semantics { contentDescription = "New cookbook" },
        contentAlignment = Alignment.Center,
    ) {
        Icon(Lucide.Plus, null, tint = ink, modifier = Modifier.size(20.dp))
    }
}

/** A little herb pot, because why not (the web's 60×90 SVG, path for path). */
@Composable
fun HerbPot(modifier: Modifier = Modifier) {
    val paths = remember {
        listOf(
            Color(0xFFA9C4AE) to Path().apply {
                moveTo(30f, 52f); cubicTo(18f, 38f, 8f, 36f, 4f, 24f); cubicTo(16f, 24f, 26f, 32f, 30f, 46f); close()
            },
            Color(0xFF2F6B4F) to Path().apply {
                moveTo(30f, 52f); cubicTo(42f, 36f, 52f, 32f, 57f, 18f); cubicTo(44f, 20f, 34f, 30f, 30f, 46f); close()
            },
            Color(0xFF3B7A5A) to Path().apply {
                moveTo(30f, 54f); cubicTo(26f, 36f, 28f, 20f, 34f, 8f); cubicTo(38f, 22f, 36f, 38f, 31f, 52f); close()
            },
            Color(0xFFA55A40) to Path().apply {
                moveTo(14f, 58f); relativeLineTo(32f, 0f); relativeLineTo(-4f, 32f); relativeLineTo(-24f, 0f); close()
            },
        )
    }
    Canvas(modifier) {
        scale(size.width / 60f, size.height / 90f, pivot = Offset.Zero) {
            paths.forEach { (color, path) -> drawPath(path, color) }
            drawRect(Color(0xFF8F4C35), Offset(11f, 52f), Size(38f, 8f))
        }
    }
}
