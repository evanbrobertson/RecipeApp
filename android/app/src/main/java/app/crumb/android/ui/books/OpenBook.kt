package app.crumb.android.ui.books

import android.os.Build
import android.view.View
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.snap
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
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import androidx.compose.ui.zIndex
import app.crumb.android.data.Cookbook
import app.crumb.android.data.CookbookListItem
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.Skeleton
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.DmSerif
import app.crumb.android.ui.theme.NunitoSans
import app.crumb.core.SpineStyle
import com.composables.icons.lucide.ArrowRight
import com.composables.icons.lucide.ChefHat
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.X
import kotlin.coroutines.resume
import kotlin.math.cos
import kotlin.math.min
import kotlin.math.sin
import kotlinx.coroutines.Job
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine

// A cookbook taken off the shelf (web/src/components/OpenBook.svelte): it leaves its place spine
// first, turns to show its cover as it comes to the middle, and the cover swings open on its
// table of contents. Closing puts it back.
//
// Compose has no shared 3D space, so the closed book is two faces placed by hand: the cover (with
// the page under it) and the spine, square to it at the hinge, each turned about its own middle
// and moved to where the solid book's face would be. The book is as deep as its spine on the
// shelf is wide, so at the start of the flight its spine sits exactly over the one on the shelf.

private val MoveIn = CubicBezierEasing(0.25f, 0.8f, 0.25f, 1f)
private val TurnIn = CubicBezierEasing(0.5f, 0f, 0.2f, 1f)
private val MoveOut = CubicBezierEasing(0.6f, 0f, 0.6f, 1f)
private val TurnOut = CubicBezierEasing(0.6f, 0f, 0.3f, 1f)
private val Swing = CubicBezierEasing(0.35f, 0.65f, 0.2f, 1f)
private val SwingAway = CubicBezierEasing(0.3f, 0.7f, 0.2f, 1f)
private val Shut = CubicBezierEasing(0.5f, 0f, 0.3f, 1f)
private val EaseIn = CubicBezierEasing(0.42f, 0f, 1f, 1f)
private val EaseOutCurve = CubicBezierEasing(0f, 0f, 0.58f, 1f)

/** The web's perspective, dp. */
private const val Perspective = 2200f

/** Waits until this view's next frame is on the screen. */
private suspend fun View.shown() {
    if (Build.VERSION.SDK_INT >= 29) {
        suspendCancellableCoroutine { done ->
            viewTreeObserver.registerFrameCommitCallback { done.resume(Unit) }
            invalidate()
        }
    }
    // The compositor shows it a frame later
    repeat(if (Build.VERSION.SDK_INT >= 29) 1 else 3) { withFrameNanos {} }
}

/** How much larger the perspective draws something [z] dp nearer than the page. */
private fun near(z: Float) = Perspective / (Perspective - z)

private val Scrim = Color(0xFF0D130F)
private val Ink = Color(0xFF1C2B22)

/**
 * The open book. [from] is where its spine was on the shelf; without it (or with animations
 * off) the book fades in and out instead of flying.
 */
@Composable
fun OpenBook(
    book: CookbookListItem,
    from: BookOrigin?,
    load: suspend (Long) -> Cookbook?,
    onOpenRecipe: (Long) -> Unit,
    onOpenCookbook: (Long) -> Unit,
    onClose: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    val motion = rememberMotionScale()
    val reduced = motion == 0f
    suspend fun wait(ms: Int) = delay((ms * motion).toLong())
    val c = Crumb.colors
    val paint = rememberShelfPaint(c)
    val look = bookLook(book.color)
    val pale = bookIsPale(book.color)
    val spine = from?.spine ?: remember(book) { spineOf(book) }
    var details by remember(book.id) { mutableStateOf<Cookbook?>(null) }
    var loading by remember(book.id) { mutableStateOf(true) }

    var open by remember { mutableStateOf(false) }
    var closing by remember { mutableStateOf(false) }
    // Where this window sits on the screen, px, to fly from the shelf's spine
    var root by remember { mutableStateOf<Pair<Offset, Size>?>(null) }
    // The dialog's window moves once or twice as it comes up: fly only once it has stopped
    var settled by remember { mutableStateOf(false) }
    var window by remember { mutableStateOf<View?>(null) }
    val fly = from != null && !reduced
    val scrim = remember { Animatable(0f) }
    val fade = remember { Animatable(if (fly) 1f else 0f) }
    // Linear time through each part of the flight, 0 on the shelf and 1 in the middle; eased below
    val move = remember { Animatable(if (fly) 0f else 1f) }
    val turn = remember { Animatable(if (fly) 0f else 1f) }
    var back by remember { mutableStateOf(false) }
    var flying by remember { mutableStateOf<Job?>(null) }

    suspend fun flight(inbound: Boolean) = coroutineScope {
        if (!fly) {
            fade.animateTo(if (inbound) 1f else 0f, tween(if (reduced) 1 else 260, easing = EaseOutCurve))
            return@coroutineScope
        }
        back = !inbound
        // The turn lags the move going out, and leads it coming back, as a hand would
        if (inbound) {
            launch { move.animateTo(1f, tween(720, easing = LinearEasing)) }
            launch { turn.animateTo(1f, tween(620, 110, LinearEasing)) }
        } else {
            launch { move.animateTo(0f, tween(620, 60, LinearEasing)) }
            launch { turn.animateTo(0f, tween(560, easing = LinearEasing)) }
        }
    }

    LaunchedEffect(book.id) {
        var seen: Pair<Offset, Size>? = null
        while (root == null || root != seen) {
            seen = root
            withFrameNanos {}
            withFrameNanos {}
        }
        settled = true
        // Shown over its place now (the window takes a frame or two to reach the screen): the
        // shelf can let it go, and the flight starts from exactly there
        snapshotFlow { window }.filterNotNull().first().shown()
        from?.taken?.invoke()
        launch { details = runCatching { load(book.id) }.getOrNull(); loading = false }
        launch { scrim.animateTo(1f, if (reduced) snap() else tween(400, easing = EaseOutCurve)) }
        // The cover starts to swing as the book settles, not after
        launch {
            wait(640)
            if (!closing) open = true
        }
        val job = launch { flight(true) }
        flying = job
        job.join()
        flying = null
    }

    fun close() {
        if (closing) return
        closing = true
        scope.launch {
            flying?.join()
            if (open) {
                open = false
                wait(500)
            }
            launch { scrim.animateTo(0f, if (reduced) snap() else tween(400, easing = EaseOutCurve)) }
            // What sat on the book lifts as it nears its place
            val lift = launch {
                wait(260)
                from?.back?.invoke()
            }
            flight(false)
            lift.cancel()
            from?.back?.invoke()
            // Back in its place under the flight before the flight goes
            from?.home?.invoke()
            withFrameNanos {}
            withFrameNanos {}
            onClose()
        }
    }

    Dialog(onDismissRequest = ::close, properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        // The scrim is ours: no window dim, no window animation
        val view = LocalView.current
        SideEffect {
            window = view
            (view.parent as? DialogWindowProvider)?.window?.apply {
                setDimAmount(0f)
                setWindowAnimations(0)
            }
        }
        BoxWithConstraints(
            Modifier
                .fillMaxSize()
                .onGloballyPositioned { root = it.localToScreen(Offset.Zero) to Size(it.size.width.toFloat(), it.size.height.toFloat()) }
                .background(Scrim.copy(alpha = 0.6f * scrim.value))
                .clickable(remember { MutableInteractionSource() }, indication = null, onClick = ::close),
            contentAlignment = Alignment.Center,
        ) {
            // Phones get one page; wider screens a spread, the cover opening to the left
            val spread = maxWidth > 640.dp
            val pageW = if (spread) minOf(380.dp, maxWidth * 0.44f) else minOf(maxWidth * 0.86f, 380.dp)
            val pageH = minOf(560.dp, maxHeight * 0.8f)
            val coverW = pageW + 6.dp
            val coverH = pageH + 12.dp
            // The closed book's depth: its spine face has the shelf spine's proportions
            val along = (if (spine.standing) spine.h else spine.w).toFloat()
            val across = (if (spine.standing) spine.w else spine.h).toFloat()
            val depth = coverH.value * across / along

            val cover = remember { Animatable(0f) }
            val coverAlpha = remember { Animatable(1f) }
            val frontShade = remember { Animatable(0f) }
            val insideShade = remember { Animatable(0.35f) }
            val pageShade = remember { Animatable(1f) }
            val closeAlpha = remember { Animatable(0f) }
            val shift = remember { Animatable(0f) }
            LaunchedEffect(open) {
                fun <T> spec(ms: Int, delay: Int = 0, easing: androidx.compose.animation.core.Easing = LinearEasing) =
                    if (reduced) snap<T>() else tween(ms, delay, easing)
                if (open) {
                    launch { cover.animateTo(-180f, if (spread) spec(950, easing = Swing) else spec(900, easing = SwingAway)) }
                    if (!spread) launch { coverAlpha.animateTo(0f, spec(300, 600)) }
                    launch { frontShade.animateTo(0.4f, spec(450, easing = EaseIn)) }
                    launch { insideShade.animateTo(0f, spec(500, 450, EaseOutCurve)) }
                    launch { pageShade.animateTo(0f, spec(600, 250, EaseOutCurve)) }
                    launch { closeAlpha.animateTo(1f, spec(300, easing = EaseOutCurve)) }
                    if (spread) launch { shift.animateTo(pageW.value / 2, spec(700, easing = Glide)) }
                } else if (closing) {
                    coverAlpha.snapTo(1f)
                    launch { cover.animateTo(0f, spec(500, easing = Shut)) }
                    launch { frontShade.animateTo(0f, spec(450, easing = EaseIn)) }
                    launch { insideShade.animateTo(0.35f, spec(500, easing = EaseOutCurve)) }
                    launch { pageShade.animateTo(1f, spec(600, 250, EaseOutCurve)) }
                    launch { closeAlpha.animateTo(0f, spec(300, easing = EaseOutCurve)) }
                    launch { shift.animateTo(0f, spec(500, easing = Glide)) }
                }
            }

            // Where the flight starts: the shelf spine's centre, from this box's centre, dp
            val start = remember(root, from) {
                val r = root
                if (from == null || r == null) Offset.Zero
                else (from.center - (r.first + Offset(r.second.width / 2, r.second.height / 2))) / view.resources.displayMetrics.density
            }
            // Turned to face you, the spine is nearer than the book's middle, so the perspective
            // draws it larger: start that much smaller to sit exactly over the shelf's
            val startScale = along / coverH.value / near(coverW.value / 2 - depth / 2)
            val lean = (from?.tilt ?: 0f) + (if (spine.standing) 0f else -90f)

            Box(
                Modifier
                    .size(coverW, coverH)
                    .graphicsLayer {
                        val p = if (back) MoveOut.transform(move.value) else MoveIn.transform(move.value)
                        val q = if (back) TurnOut.transform(turn.value) else TurnIn.transform(turn.value)
                        // Off the shelf on a slight arc towards you
                        val (at, z) = if (p < 0.4f) {
                            start * (1 - 0.45f * p / 0.4f) to 120f * p / 0.4f
                        } else {
                            start * (0.55f * (1 - (p - 0.4f) / 0.6f)) to 120f * (1 - (p - 0.4f) / 0.6f)
                        }
                        val k = near(z)
                        translationX = at.x * k * density
                        translationY = at.y * k * density
                        val s = (startScale + (1 - startScale) * p) * k * (0.96f + 0.04f * fade.value)
                        scaleX = s
                        scaleY = s
                        rotationZ = lean * (1 - q)
                        alpha = if (settled) fade.value else 0f
                    }
                    .clickable(remember { MutableInteractionSource() }, indication = null) {},
            ) {
                val q = if (back) TurnOut.transform(turn.value) else TurnIn.transform(turn.value)
                val theta = Math.toRadians(90.0 * (1 - q)).toFloat()
                val camera = Perspective / 72f
                // Each face turns about its own middle, so set it at its depth in the solid book
                // (from the book's middle, dp) and let the perspective scale it to match
                val spineZ = coverW.value / 2 * sin(theta) - depth / 2
                val coverZ = depth / 2 * cos(theta) - depth / 2
                // The spine, square to the cover at the hinge: only seen while the book turns
                if (sin(theta) > 0.01f) {
                    Box(
                        Modifier
                            .align(Alignment.Center)
                            .size(depth.dp, coverH)
                            .zIndex(spineZ)
                            .graphicsLayer {
                                cameraDistance = camera * density
                                val f = near(spineZ)
                                translationX = -(coverW.toPx() / 2) * cos(theta) * f
                                scaleX = f
                                scaleY = f
                                rotationY = Math.toDegrees(theta.toDouble()).toFloat() - 90f
                            }
                            .drawBehind {
                                val k = size.height / (along * density)
                                scale(k, pivot = Offset.Zero) {
                                    drawSpine(spine, look, pale, paint, turned = !spine.standing, box = Size(across * density, along * density))
                                }
                            },
                    )
                }
                // The cover, with the page under it
                Box(
                    Modifier
                        .fillMaxSize()
                        .zIndex(coverZ)
                        .graphicsLayer {
                            cameraDistance = camera * density
                            val f = near(coverZ)
                            translationX = depth.dp.toPx() / 2 * sin(theta) * f
                            scaleX = f
                            scaleY = f
                            rotationY = Math.toDegrees(theta.toDouble()).toFloat()
                            alpha = if (cos(theta) > 0.01f) 1f else 0f
                        },
                ) {
                    Box(Modifier.fillMaxSize().graphicsLayer { translationX = shift.value.dp.toPx() }) {
                        ContentsPage(
                            book, details, loading, open, spread, pageShade.value, onOpenRecipe, onOpenCookbook,
                            Modifier.offset(y = 6.dp).size(pageW, pageH),
                        )
                        Box(
                            Modifier
                                .fillMaxSize()
                                .graphicsLayer {
                                    transformOrigin = TransformOrigin(0f, 0.5f)
                                    cameraDistance = camera * density
                                    rotationY = cover.value
                                    translationX = if (spread) 0f else -8.dp.toPx() * (-cover.value / 180f)
                                    alpha = coverAlpha.value
                                },
                        ) {
                            if (cover.value > -90f) CoverFront(book, look, pale, spine.style, paint, frontShade.value)
                            else CoverInside(book, insideShade.value)
                        }
                        // Close, just above the page
                        Box(
                            Modifier
                                .align(Alignment.TopEnd)
                                .offset(x = (-6).dp, y = (-46).dp)
                                .graphicsLayer { alpha = closeAlpha.value }
                                .size(44.dp)
                                .clip(CircleShape)
                                .background(Color(0xFFFFFDF8).copy(alpha = 0.16f))
                                .clickable(enabled = open, onClick = ::close),
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(Lucide.X, "Close book", tint = Color(0xFFFFFDF8), modifier = Modifier.size(22.dp))
                        }
                    }
                }
            }
        }
    }
}

/** The menu shadow (0 16px 28px -16px) under a page or cover. */
private fun Modifier.menuShadow(paint: ShelfPaint, dark: Boolean, radius: Dp = 16.dp) = drawBehind {
    scaleDp(paint.density) {
        val w = size.width / paint.density
        val h = size.height / paint.density
        drawSoftShadow(paint, 16f, 16f, w - 16f, h - 16f, radius.value, 0f, 16f, 28f, if (dark) 0.7f else 0.45f)
    }
}

/** The front of the cover: cloth over board, the hinge's groove, a foil frame round the title. */
@Composable
private fun CoverFront(book: CookbookListItem, look: BookLook, pale: Boolean, style: SpineStyle, paint: ShelfPaint, shade: Float) {
    val c = Crumb.colors
    val shape = RoundedCornerShape(topEnd = 16.dp, bottomEnd = 16.dp)
    val frame = if (style == SpineStyle.RULES) look.bands.first else look.foil
    val shadow = if (pale) Shadow(Color.White.copy(alpha = 0.4f), Offset(0f, 2f)) else Shadow(Color.Black.copy(alpha = 0.3f), Offset(0f, -2f))
    Box(
        Modifier
            .fillMaxSize()
            .menuShadow(paint, c.dark)
            .clip(shape)
            .drawBehind {
                drawCloth(look.cloth, paint)
                if (style == SpineStyle.ENDS) {
                    // Two-tone books get contrasting corners
                    val half = 46.dp.toPx()
                    for (y in listOf(0f, size.height)) {
                        rotate(45f, pivot = Offset(size.width, y)) {
                            drawRect(look.bands.first, Offset(size.width - half, y - half), Size(half * 2, half * 2))
                        }
                    }
                }
                drawRect(
                    Brush.horizontalGradient(
                        0f to Color.Black.copy(alpha = 0.3f), 0.03f to Color.White.copy(alpha = 0.08f),
                        0.05f to Color.Black.copy(alpha = 0.14f), 0.09f to Color.Transparent,
                    ),
                )
                drawRect(
                    Brush.radialGradient(
                        0f to Color.White.copy(alpha = 0.12f), 0.6f to Color.Transparent,
                        center = Offset(size.width * 0.2f, 0f), radius = size.width * 1.2f,
                    ),
                )
                drawRect(Brush.verticalGradient(0.85f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.12f)))
            }
            .drawWithContent {
                drawContent()
                val px = 1.dp.toPx()
                drawRect(Color.Black.copy(alpha = 0.12f), Offset(px / 2, px / 2), Size(size.width - px, size.height - px), style = Stroke(px))
                look.edge?.let { drawRect(it.copy(alpha = EdgeAlpha), Offset(px / 2, px / 2), Size(size.width - px, size.height - px), style = Stroke(px)) }
                drawRect(Color(0xFF0A100C).copy(alpha = shade))
            }
            .padding(22.dp)
            .border(1.dp, frame, RoundedCornerShape(5.dp))
            .padding(5.dp)
            .border(1.5.dp, frame, RoundedCornerShape(3.dp))
            .padding(20.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Icon(Lucide.ChefHat, null, tint = look.foil, modifier = Modifier.size(32.dp))
            val title = TextStyle(fontFamily = DmSerif, fontSize = 26.sp, lineHeight = 32.sp, textAlign = TextAlign.Center)
            if (style == SpineStyle.LABEL) {
                // Labelled books carry the title on a paper label on the cover too
                Text(
                    book.name, style = title, color = Ink,
                    modifier = Modifier
                        .graphicsLayer { rotationZ = -0.8f }
                        .clip(RoundedCornerShape(3.dp))
                        .background(Color(0xFFF3EAD2))
                        .border(1.dp, Ink.copy(alpha = 0.18f), RoundedCornerShape(3.dp))
                        .padding(4.dp)
                        .border(1.dp, Ink.copy(alpha = 0.28f))
                        .padding(horizontal = 14.dp, vertical = 10.dp),
                )
            } else {
                Text(book.name, style = title.copy(shadow = shadow), color = look.foil)
            }
            Box(Modifier.fillMaxWidth(0.4f).height(1.dp).background(look.foil))
            Text(
                "RECIPES", color = look.foil, fontFamily = NunitoSans, fontSize = 13.sp,
                fontWeight = FontWeight.Bold, letterSpacing = 0.2.em,
            )
        }
    }
}

/** The inside of the cover: endpaper, faintly striped with the cover colour, drawn mirrored so it reads. */
@Composable
private fun CoverInside(book: CookbookListItem, shade: Float) {
    val c = Crumb.colors
    val look = bookLook(book.color)
    val light = lerp(c.paper, look.cloth, 0.12f)
    val lighter = lerp(c.paper, look.cloth, 0.07f)
    val shape: Shape = RoundedCornerShape(topStart = 16.dp, bottomStart = 16.dp)
    Column(
        Modifier
            .fillMaxSize()
            .graphicsLayer { rotationY = 180f }
            .clip(shape)
            .drawBehind {
                drawRect(lighter)
                val band = 10.dp.toPx()
                var x = -size.height
                while (x < size.width + size.height) {
                    drawLine(light, Offset(x, 0f), Offset(x + size.height, size.height), strokeWidth = band)
                    x += 2 * band * 1.4142f
                }
            }
            .drawWithContent {
                drawContent()
                drawRect(Color(0xFF0A100C).copy(alpha = shade))
            }
            .border(1.dp, c.line, shape)
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

@Composable
private fun ContentsPage(
    book: CookbookListItem,
    details: Cookbook?,
    loading: Boolean,
    open: Boolean,
    spread: Boolean,
    shade: Float,
    onOpenRecipe: (Long) -> Unit,
    onOpenCookbook: (Long) -> Unit,
    modifier: Modifier,
) {
    val c = Crumb.colors
    val paint = rememberShelfPaint(c)
    val shape = if (spread) RoundedCornerShape(topEnd = 16.dp, bottomEnd = 16.dp) else RoundedCornerShape(16.dp)
    Column(
        modifier
            .menuShadow(paint, c.dark)
            .clip(shape)
            .background(c.paper)
            .drawBehind {
                // The gutter, on a spread
                if (spread) drawRect(Brush.horizontalGradient(0f to Color.Black.copy(alpha = 0.08f), 0.08f to Color.Transparent))
            }
            .drawWithContent {
                drawContent()
                // The cover's shadow, sweeping off the page as it opens
                drawRect(
                    Brush.horizontalGradient(0.3f to Color.Transparent, 1f to Color(20, 30, 24).copy(alpha = 0.28f)),
                    alpha = shade,
                )
            }
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
                            // Each line rises in as the cover opens, one after another
                            val rise = remember { Animatable(0f) }
                            val motion = rememberMotionScale()
                            LaunchedEffect(open) {
                                if (open) {
                                    rise.snapTo(0f)
                                    delay(((250 + min(i, 12) * 40) * motion).toLong())
                                    rise.animateTo(1f, tween(400, easing = Glide))
                                }
                            }
                            Row(
                                Modifier
                                    .fillMaxWidth()
                                    .graphicsLayer {
                                        val t = if (open) rise.value else 1f
                                        alpha = t
                                        translationY = (1 - t) * 6.dp.toPx()
                                    }
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
