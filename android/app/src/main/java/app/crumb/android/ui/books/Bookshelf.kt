package app.crumb.android.ui.books

import android.provider.Settings
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.AnimationSpec
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.keyframes
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.absoluteOffset
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import app.crumb.android.data.CookbookListItem
import app.crumb.android.ui.theme.Crumb
import app.crumb.core.ShelfItem
import app.crumb.core.ShelfItemKind
import app.crumb.core.ShelfSpine
import app.crumb.core.SpinePivot
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Plus
import kotlin.math.roundToInt
import kotlinx.coroutines.delay

// The shelf from web/src/components/Bookshelf.svelte: books stand or lie on painted planks
// against a tiled wall, where crumb-core's `shelfLayout` puts them, with a pot of basil and a
// crock of spoons. A book lifts under a finger (or a pointer), and leaves its place empty while
// it's open; what lay on it settles, and lifts again as it comes back.

/** Room above the first shelf's tallest book. */
private const val Top = 14f

/** cubic-bezier(0.3, 1.5, 0.5, 1): a lift that overshoots a little. */
internal val Springy = CubicBezierEasing(0.3f, 1.5f, 0.5f, 1f)
internal val Glide = CubicBezierEasing(0.2f, 0.8f, 0.2f, 1f)
private val Drop = CubicBezierEasing(0.5f, 0f, 0.55f, 1.35f)
private val Ride = CubicBezierEasing(0.3f, 1.4f, 0.5f, 1f)
private val EaseOut = CubicBezierEasing(0f, 0f, 0.58f, 1f)

/**
 * Where an opened book was on the shelf, for its flight: its spine and tilt, and its centre on
 * the screen (px). The flight takes over the book with [taken] (its place goes empty) and hands
 * it back with [home] once it has landed over its place; [back] lifts what lay on it again as it
 * nears its place.
 */
class BookOrigin(
    val spine: ShelfSpine,
    val tilt: Float,
    val center: Offset,
    val taken: () -> Unit,
    val home: () -> Unit,
    val back: () -> Unit,
)

/**
 * The system's animation speed, which Compose already applies to animations: waits between them
 * are stretched by it too, so they keep in step. 0 is "Remove animations".
 */
@Composable
fun rememberMotionScale(): Float {
    val context = LocalContext.current
    return remember { Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) }
}

/** "Remove animations": no flight and no springs. */
@Composable
fun rememberReducedMotion(): Boolean = rememberMotionScale() == 0f

/**
 * The whole shelf, as wide as it's given: [single] is Home's one row that scrolls sideways.
 * [pulledId] is the open book, whose place stays empty until it's back. [onAdd] ends the last
 * shelf with the dashed outline of a new book.
 */
@Composable
fun Bookshelf(
    books: List<CookbookListItem>,
    onOpen: (CookbookListItem, BookOrigin?) -> Unit,
    modifier: Modifier = Modifier,
    pulledId: Long? = null,
    single: Boolean = false,
    onAdd: (() -> Unit)? = null,
) {
    val c = Crumb.colors
    val paint = rememberShelfPaint(c)
    val motion = rememberMotionScale()
    val reduced = motion == 0f
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val width = maxWidth.value
        val rows = remember(books, width, single, onAdd != null) { layoutShelf(books, width, single, onAdd != null) }
        val rowHeight = Shelf.rowHeight.toFloat()
        val height = Top + rows.size * rowHeight
        val inside = if (single) maxOf(width, rows.maxOfOrNull { it.width.toFloat() } ?: 0f) else width

        // Books glide to new places on a resize or a new book, but not into their first ones
        var ready by remember { mutableStateOf(false) }
        LaunchedEffect(rows.isNotEmpty()) {
            if (rows.isEmpty()) return@LaunchedEffect
            withFrameNanos {}
            withFrameNanos {}
            ready = true
        }

        // A book back from being open settles into its place
        var landed by remember { mutableStateOf<Long?>(null) }
        var last by remember { mutableStateOf<Long?>(null) }
        LaunchedEffect(pulledId) {
            val was = last
            last = pulledId
            if (was != null && pulledId == null) {
                landed = was
                delay((600 * motion).toLong())
                landed = null
            }
        }

        // The book whose place in a stack is empty: what lay on it settles onto the book below
        // once it has gone, and lifts again as it comes back (OpenBook calls `back`)
        var gap by remember { mutableStateOf<Long?>(null) }
        // A book that's flying stays in its place until the flight is drawn over it, and is back in
        // it before the flight goes, so it's never missing or doubled for a frame
        var flying by remember { mutableStateOf<Long?>(null) }
        var away by remember { mutableStateOf<Long?>(null) }
        val gone = pulledId?.takeIf { flying != it || away == it }
        LaunchedEffect(gone) {
            if (gone == null) {
                gap = null
            } else {
                delay((180 * motion).toLong())
                gap = gone
            }
        }
        LaunchedEffect(pulledId) {
            if (pulledId == null) {
                flying = null
                away = null
            }
        }
        val drops = remember(rows, gap) {
            buildMap {
                rows.forEachIndexed { ri, row ->
                    val g = row.items.firstOrNull { it.kind == ShelfItemKind.BOOK && it.spine?.id == gap } ?: return@forEachIndexed
                    val stack = g.stack ?: return@forEachIndexed
                    row.items.filter { it.stack == stack && it.y > g.y }.forEach { put(itemKey(it, ri), g.h.toFloat()) }
                }
            }
        }

        /** The book under a finger, pointer or focus: its neighbours give way, a pot on it rides along. */
        var active by remember { mutableStateOf<Long?>(null) }

        Box(if (single) Modifier.horizontalScroll(rememberScrollState()) else Modifier) {
            Box(
                Modifier
                    .size(inside.dp, height.dp)
                    .drawBehind {
                        drawWall(paint)
                        rows.indices.forEach { ri -> drawPlank(Top + ri * rowHeight + Shelf.clearance.toFloat(), c.shelfWood, paint) }
                    },
            ) {
                rows.forEachIndexed { ri, row ->
                    row.items.forEachIndexed { i, item ->
                        /** Top of an item's box, from the shelves' top; feet sink into the plank's face */
                        val top = Top + ri * rowHeight + Shelf.clearance.toFloat() + 4f - item.y.toFloat() - item.h.toFloat()
                        key(itemKey(item, ri)) {
                            val x by animateFloatAsState(item.x.toFloat(), placeSpec(ready, reduced), label = "x")
                            val y by animateFloatAsState(top, placeSpec(ready, reduced), label = "y")
                            val place = Modifier
                                .absoluteOffset { IntOffset((x * density).roundToInt(), (y * density).roundToInt()) }
                                .size(item.w.dp, item.h.dp)
                                .zIndex(item.z.toFloat() + 1)
                            val drop = drops[itemKey(item, ri)] ?: 0f
                            when (item.kind) {
                                ShelfItemKind.BOOK -> {
                                    val book = books[item.bookIndex!!.toInt()]
                                    // Its immediate standing neighbours tip away from it
                                    fun standingId(at: Int) = row.items.getOrNull(at)
                                        ?.takeIf { it.kind == ShelfItemKind.BOOK && it.spine?.standing == true }?.spine?.id
                                    val standing = item.spine!!.standing
                                    val tip = when {
                                        !standing || active == null -> 0f
                                        standingId(i + 1) == active -> -1.2f
                                        standingId(i - 1) == active -> 1.2f
                                        else -> 0f
                                    }
                                    ShelfBookItem(
                                        item, book, paint, place,
                                        drop = drop,
                                        tip = tip,
                                        pulled = gone == book.id,
                                        landed = landed == book.id,
                                        reduced = reduced,
                                        onActive = { on -> if (on) active = book.id else if (active == book.id) active = null },
                                        onOpen = { center ->
                                            val from = center?.let {
                                                BookOrigin(
                                                    item.spine!!, item.tilt.toFloat(), it,
                                                    taken = { away = book.id },
                                                    home = { away = null },
                                                    back = { gap = null },
                                                )
                                            }
                                            flying = from?.let { book.id }
                                            away = null
                                            onOpen(book, from)
                                        },
                                    )
                                }
                                ShelfItemKind.ADD -> NewBookOutline(place, reduced) { onAdd?.invoke() }
                                ShelfItemKind.POT -> Pot(
                                    place,
                                    paint,
                                    ride = item.on != null && item.on == active && item.on != pulledId,
                                    drop = drop,
                                    reduced = reduced,
                                )
                                ShelfItemKind.CROCK -> Box(place.drawBehind { drawCrock(paint) })
                            }
                        }
                    }
                }
            }
        }
    }
}

private fun itemKey(it: ShelfItem, row: Int): String =
    if (it.kind == ShelfItemKind.BOOK) "${it.spine?.id}" else "${it.kind}-$row"

private fun placeSpec(ready: Boolean, reduced: Boolean): AnimationSpec<Float> =
    if (ready && !reduced) tween(600, easing = Glide) else snap()

private fun pivotOf(pivot: SpinePivot) = when (pivot) {
    SpinePivot.LEFT -> TransformOrigin(0f, 1f)
    SpinePivot.RIGHT -> TransformOrigin(1f, 1f)
    SpinePivot.CENTER -> TransformOrigin.Center
}

/**
 * One book on the shelf. A tap opens it; pressing tips it up a little, and a long press (or a
 * pointer over it) lifts it right up, the way a finger tips a book out by its head.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ShelfBookItem(
    item: ShelfItem,
    book: CookbookListItem,
    paint: ShelfPaint,
    modifier: Modifier,
    drop: Float,
    tip: Float,
    pulled: Boolean,
    landed: Boolean,
    reduced: Boolean,
    onActive: (Boolean) -> Unit,
    onOpen: (Offset?) -> Unit,
) {
    val spine = item.spine!!
    val look = bookLook(book.color)
    val pale = bookIsPale(book.color)
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val hovered by interaction.collectIsHoveredAsState()
    val focused by interaction.collectIsFocusedAsState()
    // Held down: stays lifted until the finger comes up (the click's press ends at the long press)
    var held by remember { mutableStateOf(false) }
    val lifted = (hovered || focused || held) && !pulled
    LaunchedEffect(lifted) { onActive(lifted) }

    // Pressing is quick; the lift itself springs
    val pressing = pressed && !held
    val spec: AnimationSpec<Float> = when {
        reduced -> snap()
        pressing -> tween(120, easing = Springy)
        else -> tween(500, easing = Springy)
    }
    val standing = spine.standing
    val liftX by animateFloatAsState(
        when { standing -> 0f; pressing -> 10f; lifted -> 16f; else -> 0f }, spec, label = "liftX",
    )
    val liftY by animateFloatAsState(
        when { !standing -> if (lifted && !pressing) -1f else 0f; pressing -> -6f; lifted -> -14f; else -> 0f }, spec, label = "liftY",
    )
    val press by animateFloatAsState(if (pressing) 0.98f else 1f, spec, label = "press")
    val shadow by animateFloatAsState(
        if (lifted || pressing) 1f else 0f, if (reduced) snap() else tween(300, easing = EaseOut), label = "shadow",
    )
    val tipped by animateFloatAsState(tip, if (reduced) snap() else tween(500, easing = Springy), label = "tip")
    val dropped by animateFloatAsState(drop, if (reduced) snap() else tween(450, easing = Drop), label = "drop")
    val settle = remember { Animatable(0f) }
    LaunchedEffect(landed) {
        if (landed && !reduced) {
            settle.snapTo(-8f)
            settle.animateTo(0f, tween(500, easing = Springy))
        }
    }
    var coords by remember { mutableStateOf<LayoutCoordinates?>(null) }
    val w = item.w.toFloat()
    val h = item.h.toFloat()

    Box(
        modifier
            .graphicsLayer { alpha = if (pulled) 0f else 1f }
            .pointerInput(Unit) {
                awaitEachGesture {
                    awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                    do {
                        val event = awaitPointerEvent(PointerEventPass.Initial)
                    } while (event.changes.any { it.pressed })
                    held = false
                }
            }
            .hoverable(interaction, enabled = !pulled)
            .combinedClickable(
                interactionSource = interaction,
                indication = null,
                enabled = !pulled,
                role = Role.Button,
                onLongClick = { held = true },
                onClick = {
                    val center = coords?.takeIf { it.isAttached }?.let { it.localToScreen(Offset(it.size.width / 2f, it.size.height / 2f)) }
                    onOpen(center)
                },
            )
            .semantics { contentDescription = "Open ${book.name}, ${book.recipeCount} recipes" },
    ) {
        // Settles onto the book below when the one under it is taken; leans as the layout says
        Box(
            Modifier
                .fillMaxSize()
                .graphicsLayer {
                    translationY = dropped.dp.toPx()
                    rotationZ = item.tilt.toFloat()
                    transformOrigin = pivotOf(item.pivot)
                },
        ) {
            // Gives way to a lifted neighbour
            Box(
                Modifier
                    .fillMaxSize()
                    .graphicsLayer {
                        rotationZ = tipped
                        transformOrigin = if (tip < 0f || tipped < 0f) TransformOrigin(0f, 1f) else TransformOrigin(1f, 1f)
                    }
                    .graphicsLayer {
                        translationX = liftX.dp.toPx()
                        translationY = (liftY + settle.value).dp.toPx()
                        scaleX = press
                        scaleY = press
                    }
                    .onGloballyPositioned { coords = it }
                    .drawBehind {
                        val s = shadow
                        val lifting = if (standing) Triple(10f, 8f, 0.3f) else Triple(6f, 6f, 0.3f)
                        scaleDp(paint.density) {
                            drawSoftShadow(
                                paint, 0f, if (item.top) -5f else 0f, w, h, 3f,
                                dx = 3f + 3f * s, dy = 3f + (lifting.first - 3f) * s,
                                blur = 3f + (lifting.second - 3f) * s, alpha = 0.28f + (lifting.third - 0.28f) * s,
                            )
                            if (item.top) drawTopEdge(standing, look.cloth, w)
                        }
                        drawSpine(spine, look, pale, paint)
                    },
            )
        }
    }
}

/** A pot of basil. On a book, it slides out with it, and drops when the book under it is taken. */
@Composable
private fun Pot(modifier: Modifier, paint: ShelfPaint, ride: Boolean, drop: Float, reduced: Boolean) {
    val spec: AnimationSpec<Float> = if (reduced) snap() else tween(500, easing = Ride)
    val rideX by animateFloatAsState(if (ride) 16f else 0f, spec, label = "rideX")
    val rideY by animateFloatAsState(drop - (if (ride) 1f else 0f), spec, label = "rideY")
    // Moved, it rocks on its base before it settles
    val wobble = remember { Animatable(0f) }
    LaunchedEffect(ride, drop) {
        wobble.snapTo(0f)
        if ((ride || drop != 0f) && !reduced) {
            wobble.animateTo(
                0f,
                keyframes {
                    durationMillis = 800
                    0f at 0 using EaseOut
                    -5f at 200 using EaseOut
                    3f at 400 using EaseOut
                    -1.2f at 600 using EaseOut
                },
            )
        }
    }
    Box(
        modifier.graphicsLayer {
            translationX = rideX.dp.toPx()
            translationY = rideY.dp.toPx()
            rotationZ = wobble.value
            transformOrigin = TransformOrigin(0.5f, 1f)
        }.drawBehind { drawPot(paint) },
    )
}

/** A dashed outline of a book, waiting at the end of the last shelf. */
@Composable
private fun NewBookOutline(modifier: Modifier, reduced: Boolean, onAdd: () -> Unit) {
    val c = Crumb.colors
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val hovered by interaction.collectIsHoveredAsState()
    val focused by interaction.collectIsFocusedAsState()
    val on = pressed || hovered || focused
    val ink by animateColorAsState(if (on) c.primary else c.inkMuted, tween(180), label = "ink")
    val dash by animateColorAsState(if (on) c.primary else c.ink.copy(alpha = 0.3f), tween(180), label = "dash")
    val lift by animateFloatAsState(if (on) -8f else 0f, if (reduced) snap() else tween(400, easing = Springy), label = "lift")
    Box(
        modifier
            .graphicsLayer { translationY = lift.dp.toPx() }
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
            .hoverable(interaction)
            .clickable(interaction, indication = null, role = Role.Button, onClick = onAdd)
            .semantics { contentDescription = "New cookbook" },
        contentAlignment = Alignment.Center,
    ) {
        Icon(Lucide.Plus, null, tint = ink, modifier = Modifier.size(20.dp))
    }
}
