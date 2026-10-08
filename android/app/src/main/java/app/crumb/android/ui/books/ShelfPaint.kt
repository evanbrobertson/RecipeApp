package app.crumb.android.ui.books

import android.graphics.BlurMaskFilter
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.ImageShader
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.ShaderBrush
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.isSupported
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.sp
import androidx.core.graphics.createBitmap
import app.crumb.android.ui.theme.CrumbColors
import app.crumb.android.ui.theme.DmSerif
import app.crumb.core.ShelfSpine
import app.crumb.core.SpineStyle
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.roundToInt

// The shelf's paint, from BookSpine.svelte, Bookshelf.svelte and app.css: cloth with a mottled
// grain and a fine weave, a spine's rounded back lit from the upper left, its dressing and title,
// the tiled wall, the planks and their brackets, and the two props. Everything is drawn in dp
// (core's px), so a DrawScope here is scaled by the density first.

private const val Forest = 0xFF1C2B22
private val LabelPaper = Color(0xFFF3EAD2)
private val ShadowInk = Color(0xFF142019)

/** The bitmaps and text measurer the shelf draws with, made once per density and theme. */
class ShelfPaint(
    val density: Float,
    /** The web's `--cloth-grain`: grey noise, drawn soft-light over cloth. */
    val grain: ShaderBrush,
    /** 1px light lines one way, 1px dark lines the other, every 3px. */
    val weave: ShaderBrush,
    /** One wall tile, grout and gloss, repeated. */
    val wall: ShaderBrush,
    val text: TextMeasurer,
) {
    /** Older Android has no soft-light: the grain goes on faintly instead. */
    val softLight = BlendMode.Softlight.isSupported()
    internal val blur = android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG)
}

@Composable
fun rememberShelfPaint(colors: CrumbColors): ShelfPaint {
    val density = LocalDensity.current.density
    val text = rememberTextMeasurer(cacheSize = 64)
    val grain = remember(density) { ShaderBrush(ImageShader(grainBitmap(density), TileMode.Repeated, TileMode.Repeated)) }
    val weave = remember(density) { ShaderBrush(ImageShader(weaveBitmap(density), TileMode.Repeated, TileMode.Repeated)) }
    val wall = remember(density, colors) {
        ShaderBrush(ImageShader(tileBitmap(density, colors), TileMode.Repeated, TileMode.Repeated))
    }
    return remember(density, colors, text) { ShelfPaint(density, grain, weave, wall, text) }
}

/** Seeded value noise in three octaves, 160dp square and seamless, like feTurbulence's. */
private fun grainBitmap(density: Float): ImageBitmap {
    val n = (160 * density).roundToInt()
    val pixels = IntArray(n * n)
    val octaves = listOf(136, 272, 544).map { cells -> cells to FloatArray(cells * cells) { hash(it, cells) } }
    for (y in 0 until n) for (x in 0 until n) {
        var v = 0f
        var amp = 1f
        var total = 0f
        for ((cells, lattice) in octaves) {
            val fx = x.toFloat() / n * cells
            val fy = y.toFloat() / n * cells
            val x0 = floor(fx).toInt()
            val y0 = floor(fy).toInt()
            val tx = smooth(fx - x0)
            val ty = smooth(fy - y0)
            fun at(i: Int, j: Int) = lattice[(j % cells) * cells + (i % cells)]
            val top = at(x0, y0) + (at(x0 + 1, y0) - at(x0, y0)) * tx
            val bottom = at(x0, y0 + 1) + (at(x0 + 1, y0 + 1) - at(x0, y0 + 1)) * tx
            v += (top + (bottom - top) * ty) * amp
            total += amp
            amp *= 0.5f
        }
        // Around mid grey, which soft-light leaves alone, so only the mottling shows
        val g = (128 + (v / total - 0.5f) * 130).roundToInt().coerceIn(0, 255)
        pixels[y * n + x] = (150 shl 24) or (g shl 16) or (g shl 8) or g
    }
    return android.graphics.Bitmap.createBitmap(pixels, n, n, android.graphics.Bitmap.Config.ARGB_8888).asImageBitmap()
}

private fun smooth(t: Float) = t * t * (3 - 2 * t)

private fun hash(i: Int, salt: Int): Float {
    var h = i * 374761393 + salt * 668265263
    h = (h xor (h ushr 13)) * 1274126177
    return ((h xor (h ushr 16)) and 0xFFFF) / 65535f
}

private fun weaveBitmap(density: Float): ImageBitmap {
    val n = max(3, (3 * density).roundToInt())
    val line = max(1, density.roundToInt())
    val bitmap = createBitmap(n, n)
    val canvas = android.graphics.Canvas(bitmap)
    val paint = android.graphics.Paint()
    paint.color = Color.White.copy(alpha = 0.06f).toArgb()
    canvas.drawRect(0f, (n - line).toFloat(), n.toFloat(), n.toFloat(), paint)
    paint.color = Color.Black.copy(alpha = 0.05f).toArgb()
    canvas.drawRect(0f, 0f, line.toFloat(), n.toFloat(), paint)
    return bitmap.asImageBitmap()
}

/** A glossy wall tile: grout along its top and left, a soft highlight in its top-left corner. */
private fun tileBitmap(density: Float, c: CrumbColors): ImageBitmap {
    val n = (58 * density).roundToInt()
    val bitmap = createBitmap(n, n)
    val canvas = android.graphics.Canvas(bitmap)
    val paint = android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG)
    canvas.drawColor(c.tint.toArgb())
    val gloss = if (c.dark) 0.07f else 0.45f
    paint.shader = android.graphics.RadialGradient(
        0f, 0f, n * 1.3f * 0.42f,
        Color.White.copy(alpha = gloss).toArgb(), Color.White.copy(alpha = 0f).toArgb(),
        android.graphics.Shader.TileMode.CLAMP,
    )
    canvas.drawRect(0f, 0f, n.toFloat(), n.toFloat(), paint)
    paint.shader = android.graphics.LinearGradient(
        0f, 0f, n.toFloat(), n.toFloat(),
        intArrayOf(0, 0, Color.Black.copy(alpha = if (c.dark) 0.08f else 0.03f).toArgb()),
        floatArrayOf(0f, 0.7f, 1f),
        android.graphics.Shader.TileMode.CLAMP,
    )
    canvas.drawRect(0f, 0f, n.toFloat(), n.toFloat(), paint)
    paint.shader = null
    paint.color = (if (c.dark) lerp(c.tint, Color.Black, 0.3f) else lerp(c.tint, c.ink, 0.16f)).toArgb()
    val g = 2 * density
    canvas.drawRect(0f, 0f, n.toFloat(), g, paint)
    canvas.drawRect(0f, 0f, g, n.toFloat(), paint)
    return bitmap.asImageBitmap()
}

/** Draws [block] in dp. */
internal inline fun DrawScope.scaleDp(density: Float, block: DrawScope.() -> Unit) = scale(density, pivot = Offset.Zero, block)

/** Cloth: its colour, the grain blended in, then the weave. Call inside a clip. */
fun DrawScope.drawCloth(color: Color, paint: ShelfPaint, topLeft: Offset = Offset.Zero, size: Size = this.size) {
    drawRect(color, topLeft, size)
    if (paint.softLight) drawRect(paint.grain, topLeft, size, blendMode = BlendMode.Softlight)
    else drawRect(paint.grain, topLeft, size, alpha = 0.12f)
    drawRect(paint.weave, topLeft, size)
}

/**
 * A blurred shadow of a rounded box, offset like CSS's drop-shadow, drawn natively so it can
 * spill past the box. Everything is in the scope's units (dp: the callers draw scaled).
 */
fun DrawScope.drawSoftShadow(
    paint: ShelfPaint,
    left: Float, top: Float, right: Float, bottom: Float,
    radius: Float, dx: Float, dy: Float, blur: Float, alpha: Float,
) {
    if (alpha <= 0f) return
    val p = paint.blur
    // CSS blur is twice the deviation; the mask filter's radius is about 1.7 deviations
    p.maskFilter = if (blur > 0.2f) BlurMaskFilter(blur / 2 / 0.57735f, BlurMaskFilter.Blur.NORMAL) else null
    p.color = ShadowInk.copy(alpha = alpha).toArgb()
    drawIntoCanvas { it.nativeCanvas.drawRoundRect(left + dx, top + dy, right + dx, bottom + dy, radius, radius, p) }
}

/** Measures one line of a title: never squeezed, cut short with an ellipsis if it can't fit. */
private fun ShelfPaint.line(text: String, font: Double, color: Color, shadow: Shadow?, max: Float): TextLayoutResult =
    this.text.measure(
        AnnotatedString(text),
        TextStyle(
            fontFamily = DmSerif,
            // Core sized it in px, so a large system font mustn't outgrow the spine
            fontSize = (font / Density(density, 1f).fontScale).sp,
            color = color,
            shadow = shadow,
        ),
        overflow = TextOverflow.Ellipsis,
        softWrap = false,
        maxLines = 1,
        constraints = Constraints(maxWidth = max(1f, max * density).toInt()),
        density = Density(density, 1f),
    )

/**
 * A cloth spine filling this scope (dp): [turned] is a lying spine drawn standing, to be turned a
 * quarter left (the open book's flight), so its light still comes from above once turned.
 */
fun DrawScope.drawSpine(
    spine: ShelfSpine,
    look: BookLook,
    pale: Boolean,
    paint: ShelfPaint,
    turned: Boolean = false,
    box: Size = size,
) {
    val up = spine.standing || turned
    val w = box.width / paint.density
    val h = box.height / paint.density
    scale(paint.density, pivot = Offset.Zero) {
        val shape = Path().apply {
            addRoundRect(
                if (up) RoundRect(0f, 0f, w, h, CornerRadius(3f), CornerRadius(3f), CornerRadius(2f), CornerRadius(2f))
                else RoundRect(0f, 0f, w, h, CornerRadius(3f)),
            )
        }
        clipPath(shape) {
            scale(1 / paint.density, pivot = Offset.Zero) {
                drawCloth(look.cloth, paint, size = Size(w * paint.density, h * paint.density))
            }
            val orn = look.bands.first.copy(alpha = 0.9f)
            when (spine.style) {
                SpineStyle.RULES -> {
                    // Two fine gilt rules at each end
                    for (at in listOf(12f, 16.5f, h - 18f, h - 13.5f).takeIf { up } ?: listOf(12f, 16.5f, w - 18f, w - 13.5f)) {
                        if (up) drawRect(orn, Offset(3f, at), Size(w - 6f, 1.5f))
                        else drawRect(orn, Offset(at, 3f), Size(1.5f, h - 6f))
                    }
                }
                SpineStyle.ENDS -> {
                    // Contrasting cloth at head and foot, edged with a foil line
                    val ends = if (up) listOf(Offset(0f, 0f), Offset(0f, h - 14f)) else listOf(Offset(0f, 0f), Offset(w - 14f, 0f))
                    val endSize = if (up) Size(w, 14f) else Size(14f, h)
                    ends.forEach { o ->
                        scale(1 / paint.density, pivot = Offset.Zero) {
                            drawCloth(look.bands.first, paint, o * paint.density, endSize * paint.density)
                        }
                    }
                    if (up) {
                        drawRect(look.foil, Offset(0.5f, 14f), Size(w - 1f, 1.5f))
                        drawRect(look.foil, Offset(0.5f, h - 15.5f), Size(w - 1f, 1.5f))
                    } else {
                        drawRect(look.foil, Offset(14f, 0.5f), Size(1.5f, h - 1f))
                        drawRect(look.foil, Offset(w - 15.5f, 0.5f), Size(1.5f, h - 1f))
                    }
                }
                else -> Unit
            }
            drawTitle(spine, look, pale, paint, up, w, h)
            drawShine(up, turned, w, h)
        }
    }
}

private fun DrawScope.drawTitle(spine: ShelfSpine, look: BookLook, pale: Boolean, paint: ShelfPaint, up: Boolean, w: Float, h: Float) {
    val label = spine.style == SpineStyle.LABEL
    val color = if (label) Color(Forest) else look.foil
    val lineH = (spine.font * 1.12).toFloat()
    // Foil stamped into dark cloth; dark type on pale cloth catches the light below it. The
    // offsets are the text's own, so a standing title's (turned a quarter) points up the page
    val lift = if (pale) 1f else -1f
    val shadow = when {
        label -> null
        up -> Shadow(if (pale) Color.White.copy(alpha = 0.35f) else Color.Black.copy(alpha = 0.28f), Offset(-lift * paint.density, 0f))
        else -> Shadow(if (pale) Color.White.copy(alpha = 0.35f) else Color.Black.copy(alpha = 0.28f), Offset(0f, lift * paint.density))
    }
    // Room along the spine: the dressing's ends, and a label's padding
    val room = if (up) h - 26f - (if (label) 20f else 0f) else w - 28f - (if (label) 20f else 0f)
    val lines = spine.lines.map { paint.line(it, spine.font, color, shadow, room) }
    val textW = lines.maxOf { it.size.width } / paint.density
    val blockH = lineH * lines.size
    val angle = if (up) 90f + (if (label) 0.6f else 0f) else if (label) -0.6f else 0f
    rotate(angle, pivot = Offset(w / 2, h / 2)) {
        // In the text's own frame, centred on the spine
        val cx = w / 2
        val cy = h / 2
        if (label) {
            val lw = textW + 20f
            val lh = if (up) lineH + 8f else blockH + 7f
            val l = cx - lw / 2
            val t = cy - lh / 2
            drawLabel(l, t, lw, lh, paint)
        }
        lines.forEachIndexed { i, layout ->
            val lw = layout.size.width / paint.density
            val lh = layout.size.height / paint.density
            val slot = cy - blockH / 2 + i * lineH - (if (label && !up) 0.5f else 0f)
            scale(1 / paint.density, pivot = Offset.Zero) {
                drawText(layout, topLeft = Offset((cx - lw / 2) * paint.density, (slot + (lineH - lh) / 2) * paint.density))
            }
        }
    }
}

/** A paper label, gummed on by hand: a hairline, a paper margin and a second hairline inside. */
private fun DrawScope.drawLabel(l: Float, t: Float, w: Float, h: Float, paint: ShelfPaint) {
    drawSoftShadow(paint, l, t, l + w, t + h, 2f, 0f, 1f, 1f, 0.18f)
    val r = CornerRadius(2f)
    drawRoundRect(LabelPaper, Offset(l, t), Size(w, h), r)
    drawRoundRect(
        Brush.linearGradient(
            0f to Color.White.copy(alpha = 0.35f), 0.4f to Color.Transparent, 1f to Color(150, 110, 40).copy(alpha = 0.1f),
            start = Offset(l, t), end = Offset(l + w * 0.34f, t + h),
        ),
        Offset(l, t), Size(w, h), r,
    )
    drawRoundRect(Color(Forest).copy(alpha = 0.18f), Offset(l + 0.5f, t + 0.5f), Size(w - 1f, h - 1f), r, style = Stroke(1f))
    drawRect(Color(Forest).copy(alpha = 0.28f), Offset(l + 3f, t + 3f), Size(w - 6f, h - 6f), style = Stroke(1f))
}

/** The rounded back of the spine, lit from the upper left, darker at head and foot. */
private fun DrawScope.drawShine(up: Boolean, turned: Boolean, w: Float, h: Float) {
    if (up) {
        val across = listOf(
            0f to Color.Black.copy(alpha = 0.3f), 0.13f to Color.White.copy(alpha = 0.16f),
            0.36f to Color.White.copy(alpha = 0.05f), 0.52f to Color.Transparent,
            0.76f to Color.Black.copy(alpha = 0.1f), 1f to Color.Black.copy(alpha = 0.36f),
        ).toTypedArray()
        drawRect(
            if (turned) Brush.horizontalGradient(*across, startX = w, endX = 0f) else Brush.horizontalGradient(*across, startX = 0f, endX = w),
            size = Size(w, h),
        )
        val len = if (turned) w else h
        val along = arrayOf(
            0f to Color.White.copy(alpha = 0.14f), 8f / len to Color.Transparent,
            (len - 7f) / len to Color.Transparent, 1f to Color.Black.copy(alpha = 0.26f),
        )
        drawRect(
            if (turned) Brush.horizontalGradient(*along, startX = 0f, endX = w) else Brush.verticalGradient(*along, startY = 0f, endY = h),
            size = Size(w, h),
        )
    } else {
        drawRect(
            Brush.verticalGradient(
                0f to Color.White.copy(alpha = 0.2f), 0.26f to Color.White.copy(alpha = 0.07f), 0.48f to Color.Transparent,
                0.74f to Color.Black.copy(alpha = 0.1f), 1f to Color.Black.copy(alpha = 0.34f),
                startY = 0f, endY = h,
            ),
            size = Size(w, h),
        )
        drawRect(
            Brush.horizontalGradient(
                0f to Color.Black.copy(alpha = 0.2f), 6f / w to Color.Transparent,
                (w - 6f) / w to Color.Transparent, 1f to Color.Black.copy(alpha = 0.22f),
                startX = 0f, endX = w,
            ),
            size = Size(w, h),
        )
    }
}

/**
 * The top edge of a book seen from a little above, in the 5dp over a box [w] wide (dp): pages
 * between dark board edges on a standing book, the cover on a lying one.
 */
fun DrawScope.drawTopEdge(standing: Boolean, cloth: Color, w: Float) {
    val r = CornerRadius(2f)
    fun shape(l: Float, rt: Float) = Path().apply {
        addRoundRect(RoundRect(l, -5f, rt, 0f, r, r, CornerRadius.Zero, CornerRadius.Zero))
    }
    if (standing) {
        clipPath(shape(1f, w - 1f)) {
            drawRect(Brush.verticalGradient(listOf(Color(0xFFD9CFB4), Color(0xFFF4EEDD)), -5f, 0f), Offset(1f, -5f), Size(w - 2f, 5f))
            drawRect(Color.Black.copy(alpha = 0.3f), Offset(1f, -5f), Size(2f, 5f))
            drawRect(Color.Black.copy(alpha = 0.3f), Offset(w - 3f, -5f), Size(2f, 5f))
        }
    } else {
        clipPath(shape(2f, w - 2f)) {
            drawRect(lerp(cloth, Color.White, 0.2f), Offset(2f, -5f), Size(w - 4f, 5f))
            drawRect(
                Brush.verticalGradient(0f to Color.Black.copy(alpha = 0.25f), 0.7f to Color.Transparent, startY = -5f, endY = 0f),
                Offset(2f, -5f), Size(w - 4f, 5f),
            )
        }
    }
}

/** The tiled wall over this whole scope. */
fun DrawScope.drawWall(paint: ShelfPaint) {
    val g = paint.density
    translate(-g, -g) { drawRect(paint.wall, size = Size(size.width + g, size.height + g)) }
}

/**
 * One plank across this scope's width with its top at [top] (dp): an 8dp top face catching the
 * light toward its front lip, then the painted front, a shadow on the tiles, two brackets.
 */
fun DrawScope.drawPlank(top: Float, wood: Color, paint: ShelfPaint) {
    val d = paint.density
    val w = size.width / d
    scale(d, pivot = Offset.Zero) {
        // Its shadow on the tiles
        drawRect(
            Brush.verticalGradient(
                0f to ShadowInk.copy(alpha = 0.26f), 0.4f to ShadowInk.copy(alpha = 0.1f), 1f to Color.Transparent,
                startY = top + 22f, endY = top + 38f,
            ),
            Offset(0f, top + 22f), Size(w, 16f),
        )
        // Brackets, 9% in from each end
        for (x in listOf(w * 0.09f, w * 0.91f - 16f)) corbel(x, top + 22f, wood, paint)
        val face = Size(w, 22f)
        translate(0f, top) {
            scale(1 / d, pivot = Offset.Zero) { drawCloth(wood, paint, size = face * d) }
            val dark = lerp(wood, Color.Black, 0.3f)
            drawRect(
                Brush.verticalGradient(
                    0f to dark, 7f / 22 to lerp(wood, Color.White, 0.22f), 8f / 22 to lerp(wood, Color.White, 0.1f),
                    8f / 22 to lerp(wood, Color.White, 0.3f), 9f / 22 to wood, 19f / 22 to wood,
                    1f to lerp(wood, Color.Black, 0.25f),
                    startY = 0f, endY = 22f,
                ),
                size = face,
                alpha = 0.92f,
            )
        }
    }
}

private val CorbelPath by lazy { svg("M0 0 h16 v7 h-2 v5 h-2 v5 a4 4 0 0 1 -8 0 v-5 h-2 v-5 h-2 z") }

private fun DrawScope.corbel(x: Float, y: Float, wood: Color, paint: ShelfPaint) {
    translate(x, y) {
        drawSoftShadow(paint, 2f, 0f, 14f, 21f, 4f, 3f, 4f, 3f, 0.25f)
        drawPath(CorbelPath, lerp(wood, Color.Black, 0.18f))
        drawRect(Color.White.copy(alpha = 0.12f), size = Size(16f, 2f))
    }
}

private fun svg(d: String): Path = PathParser().parsePathString(d).toPath()

private class Shape(val path: Path, val color: Color, val alpha: Float = 1f)

private val PotShapes by lazy {
    listOf(
        Shape(svg("M23 41 C 16 35, 9 34, 2 36 C 8 43, 16 43, 23 41Z"), Color(0xFFA9C4AE)),
        Shape(svg("M23 41 C 20 29, 13 22, 5 20 C 7 31, 14 37, 23 41Z"), Color(0xFF6F9F7E)),
        Shape(svg("M23 41 C 26 27, 33 19, 42 17 C 40 29, 32 37, 23 41Z"), Color(0xFF3B7A5A)),
        Shape(svg("M23 41 C 19 25, 21 11, 27 2 C 31 15, 29 29, 23 41Z"), Color(0xFF2F6B4F)),
        Shape(svg("M23 41 C 30 34, 37 33, 44 35 C 38 42, 30 43, 23 41Z"), Color(0xFF5E9473)),
    )
}
private val PotVein by lazy { svg("M27 4 C 25 16, 24 28, 23 41") }
private val PotBody by lazy { svg("M9 46 h28 l-3 22 h-22 z") }
private val PotSide by lazy { svg("M27 46 h10 l-3 22 h-8 z") }

/** A pot of basil, 46×68 (the web's SVG, shape for shape), its foot at this scope's bottom. */
fun DrawScope.drawPot(paint: ShelfPaint) {
    scale(paint.density, pivot = Offset.Zero) {
        drawSoftShadow(paint, 9f, 46f, 37f, 68f, 2f, 3f, 3f, 3f, 0.22f)
        drawSoftShadow(paint, 6f, 18f, 40f, 47f, 12f, 3f, 3f, 3f, 0.14f)
        PotShapes.forEach { drawPath(it.path, it.color) }
        drawPath(PotVein, Color(0xFFA9C4AE), style = Stroke(0.8f))
        drawPath(PotBody, Color(0xFFB5654A))
        drawPath(PotSide, Color(0xFF8F4C35), alpha = 0.55f)
        drawRoundRect(Color(0xFFC4704F), Offset(6f, 39f), Size(34f, 8f), CornerRadius(1.5f))
        drawRect(Color(0xFF8F4C35).copy(alpha = 0.6f), Offset(6f, 45f), Size(34f, 2f))
        drawRoundRect(Color.White.copy(alpha = 0.16f), Offset(8.5f, 40.5f), Size(7f, 4f), CornerRadius(1f))
    }
}

private val CrockBody by lazy { svg("M7 42 h38 q3 0 3 3 v42 q0 9 -9 9 h-26 q-9 0 -9 -9 v-42 q0 -3 3 -3 z") }
private val CrockSide by lazy { svg("M38 42 h7 q3 0 3 3 v42 q0 9 -9 9 h-3 z") }

/** A stoneware crock of spoons, 52×96. */
fun DrawScope.drawCrock(paint: ShelfPaint) {
    scale(paint.density, pivot = Offset.Zero) {
        drawSoftShadow(paint, 4f, 40f, 48f, 96f, 9f, 3f, 3f, 3f, 0.22f)
        // A wooden spoon, a ladle-ish spoon and a whisk
        drawLine(Color(0xFF8A5A3A), Offset(27f, 54f), Offset(24f, 22f), 3.5f, StrokeCap.Round)
        rotate(-5f, pivot = Offset(23f, 14f)) {
            drawRoundRect(Color(0xFFA8744D), Offset(17f, 4f), Size(13f, 20f), CornerRadius(4f))
        }
        drawLine(Color(0xFFC08F5A), Offset(18f, 54f), Offset(11f, 16f), 4f, StrokeCap.Round)
        rotate(-11f, pivot = Offset(9f, 10f)) { drawOval(Color(0xFFD4A26B), Offset(3f, 1f), Size(12f, 18f)) }
        rotate(-11f, pivot = Offset(8f, 9f)) { drawOval(Color.White.copy(alpha = 0.18f), Offset(5.5f, 4f), Size(5f, 10f)) }
        drawLine(Color(0xFF2F6B4F), Offset(33f, 54f), Offset(37f, 28f), 3.5f, StrokeCap.Round)
        drawOval(Color(0xFF9AA59F), Offset(31.5f, 1f), Size(15f, 28f), style = Stroke(1.4f))
        drawOval(Color(0xFF9AA59F), Offset(35.5f, 2f), Size(7f, 26f), style = Stroke(1.2f))
        drawPath(CrockBody, Color(0xFFF1E9D6))
        drawPath(CrockSide, Color(0xFFC9BEA4), alpha = 0.5f)
        drawRect(Color(0xFF2F6B4F), Offset(4f, 54f), Size(44f, 5f))
        drawRect(Color(0xFF2F6B4F), Offset(4f, 62f), Size(44f, 1.6f))
        drawRoundRect(Color(0xFFE6DCC4), Offset(3f, 38f), Size(46f, 7f), CornerRadius(3.5f))
        drawRoundRect(Color.White.copy(alpha = 0.35f), Offset(7f, 47f), Size(5f, 40f), CornerRadius(2.5f))
    }
}
