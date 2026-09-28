package app.crumb.android.ui.prep

import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.crumb.android.ui.theme.Crumb
import app.crumb.core.Vessel

// Hand-drawn-ish bowls, boards and jars for mise en place (web/src/components/PrepBowl.svelte).
// Bigger quantity, bigger bowl. The SVG's 100-wide viewBox is drawn at a uniform scale, so the
// geometry below uses the same numbers as the component. Colours come from the component's CSS
// vars for light and "evening kitchen".

private fun vesselSize(vessel: Vessel): Dp = when (vessel) {
    Vessel.PINCH -> 44.dp
    Vessel.RAMEKIN -> 58.dp
    Vessel.SMALL -> 74.dp
    Vessel.MEDIUM -> 94.dp
    Vessel.LARGE -> 116.dp
    Vessel.BOARD -> 112.dp
    Vessel.JAR -> 46.dp
}

private fun heightFactor(vessel: Vessel): Float = when (vessel) {
    Vessel.JAR -> 1.3f
    Vessel.BOARD -> 0.62f
    else -> 0.72f
}

/** The themable vessel parts. Fills are the ingredient's own colour, not from here. */
private data class VesselColors(
    val bowl: Color,
    val bowlInside: Color,
    val bowlEdge: Color,
    val gloss: Color,
    val board: Color,
    val boardEdge: Color,
    val boardHole: Color,
    val jarGlass: Color,
    val jarEdge: Color,
    val jarCap: Color,
    val jarHoles: Color,
    val shadow: Color,
)

private fun vesselColors(dark: Boolean): VesselColors = if (dark) {
    VesselColors(
        bowl = Color(0xFF2C3D33),
        bowlInside = Color(0xFF1A241E),
        bowlEdge = Color(0xFF43584A),
        gloss = Color(0x24FFFFFF),
        board = Color(0xFFB3915F),
        boardEdge = Color(0xFF94744A),
        boardHole = Color(0xFF7C603C),
        jarGlass = Color(0xFF24322A),
        jarEdge = Color(0xFF4A5E50),
        jarCap = Color(0xFF3B7A5A),
        jarHoles = Color(0xFF1A241E),
        shadow = Color(0x59000000),
    )
} else {
    VesselColors(
        bowl = Color(0xFFFBF8EF),
        bowlInside = Color(0xFFE4ECE3),
        bowlEdge = Color(0xFFC9D6CA),
        gloss = Color(0xCCFFFFFF),
        board = Color(0xFFE6C792),
        boardEdge = Color(0xFFCFA96F),
        boardHole = Color(0xFFB98F58),
        jarGlass = Color(0xFFEEF3EC),
        jarEdge = Color(0xFFB8C8BB),
        jarCap = Color(0xFF2F6B4F),
        jarHoles = Color(0xFFE4ECE3),
        shadow = Color(0x1F1C2B22),
    )
}

/**
 * One vessel for a mise en place item. When [filled] it animates its contents in: boards and
 * bowls scale up with the web's overshoot bounce, the jar's level rises.
 */
@Composable
fun PrepBowl(vessel: Vessel, filled: Boolean, color: Color, modifier: Modifier = Modifier) {
    val dark = Crumb.colors.dark
    // Boards and bowls grow their contents with the web's overshoot bounce...
    val progress by animateFloatAsState(
        targetValue = if (filled) 1f else 0f,
        animationSpec = tween(durationMillis = 350, easing = CubicBezierEasing(0.3f, 1.6f, 0.5f, 1f)),
        label = "prep-fill",
    )
    // ...while the jar's level just rises (web rect.bowl-fill: y/height 0.4s).
    val level by animateFloatAsState(
        targetValue = if (filled) 1f else 0f,
        animationSpec = tween(durationMillis = 400),
        label = "prep-level",
    )
    val width = vesselSize(vessel)
    Canvas(modifier.size(width, width * heightFactor(vessel))) {
        val s = size.width / 100f
        scale(s, s, pivot = Offset.Zero) {
            val theme = vesselColors(dark)
            when (vessel) {
                Vessel.BOARD -> drawBoard(color, progress, theme)
                Vessel.JAR -> drawJar(color, level, theme)
                else -> drawBowl(color, progress, theme)
            }
        }
    }
}

private fun DrawScope.drawBoard(fill: Color, p: Float, t: VesselColors) {
    drawOval(t.shadow, topLeft = Offset(4f, 51f), size = Size(92f, 10f))
    drawRoundRect(t.boardEdge, topLeft = Offset(4f, 10f), size = Size(86f, 44f), cornerRadius = CornerRadius(10f, 10f))
    drawRoundRect(t.board, topLeft = Offset(4f, 10f), size = Size(86f, 40f), cornerRadius = CornerRadius(10f, 10f))
    drawCircle(t.boardHole, radius = 3.5f, center = Offset(84f, 20f))
    drawPath(
        Path().apply {
            moveTo(14f, 22f); quadraticTo(34f, 19f, 54f, 22f)
            moveTo(18f, 34f); quadraticTo(42f, 37f, 64f, 33f)
            moveTo(12f, 44f); quadraticTo(32f, 42f, 46f, 45f)
        },
        t.boardEdge,
        style = Stroke(width = 1.5f),
    )
    val alpha = p.coerceIn(0f, 1f)
    val grow = 0.6f + 0.4f * p
    translate(0f, 6f * (1f - p)) {
        scale(grow, grow, pivot = Offset(47.5f, 32.5f)) {
            boardChunk(fill, alpha, 24f, 24f, -8f, Offset(28f, 28f))
            boardChunk(fill, alpha, 37f, 28f, 12f, Offset(41f, 32f))
            boardChunk(fill, alpha, 30f, 36f, 0f, Offset.Zero)
            boardChunk(fill, alpha, 46f, 20f, 20f, Offset(50f, 24f))
            boardChunk(fill, alpha, 50f, 33f, -15f, Offset(54f, 37f))
            boardChunk(fill, alpha, 62f, 26f, 5f, Offset(66f, 30f))
        }
    }
}

private fun DrawScope.boardChunk(fill: Color, alpha: Float, x: Float, y: Float, degrees: Float, pivot: Offset) {
    rotate(degrees, pivot) {
        drawRoundRect(
            fill.copy(alpha = alpha),
            topLeft = Offset(x, y),
            size = Size(9f, 9f),
            cornerRadius = CornerRadius(2f, 2f),
        )
    }
}

private fun DrawScope.drawBowl(fill: Color, p: Float, t: VesselColors) {
    drawOval(t.shadow, topLeft = Offset(20f, 64f), size = Size(60f, 8f))
    val body = Path().apply {
        moveTo(4f, 26f)
        quadraticTo(6f, 66f, 50f, 66f)
        quadraticTo(94f, 66f, 96f, 26f)
        close()
    }
    drawPath(body, t.bowl)
    drawPath(body, t.bowlEdge, style = Stroke(width = 2.5f))
    drawOval(t.bowlInside, topLeft = Offset(4f, 14f), size = Size(92f, 24f))
    drawOval(t.bowlEdge, topLeft = Offset(4f, 14f), size = Size(92f, 24f), style = Stroke(width = 2.5f))
    val alpha = p.coerceIn(0f, 1f)
    val grow = 0.6f + 0.4f * p
    translate(0f, 6f * (1f - p)) {
        scale(grow, grow, pivot = Offset(50f, 26f)) {
            drawOval(fill.copy(alpha = alpha), topLeft = Offset(12f, 19f), size = Size(76f, 16f))
            drawOval(
                lerp(fill, Color.White, 0.08f).copy(alpha = alpha),
                topLeft = Offset(24f, 19f),
                size = Size(52f, 10f),
            )
        }
    }
    drawPath(
        Path().apply { moveTo(14f, 36f); quadraticTo(20f, 54f, 40f, 58f) },
        t.gloss,
        style = Stroke(width = 3f, cap = StrokeCap.Round),
    )
}

private fun DrawScope.drawJar(fill: Color, p: Float, t: VesselColors) {
    drawOval(t.shadow, topLeft = Offset(16f, 119f), size = Size(68f, 10f))
    drawRoundRect(t.jarGlass, topLeft = Offset(22f, 30f), size = Size(56f, 92f), cornerRadius = CornerRadius(14f, 14f))
    drawRoundRect(
        t.jarEdge,
        topLeft = Offset(22f, 30f),
        size = Size(56f, 92f),
        cornerRadius = CornerRadius(14f, 14f),
        style = Stroke(width = 3f),
    )
    val y = 120f - 64f * p
    val level = 62f * p
    if (level > 0.5f) {
        drawRoundRect(fill, topLeft = Offset(26f, y), size = Size(48f, level), cornerRadius = CornerRadius(10f, 10f))
    }
    drawRoundRect(t.jarCap, topLeft = Offset(26f, 12f), size = Size(48f, 22f), cornerRadius = CornerRadius(6f, 6f))
    drawCircle(t.jarHoles, radius = 2.5f, center = Offset(40f, 22f))
    drawCircle(t.jarHoles, radius = 2.5f, center = Offset(50f, 22f))
    drawCircle(t.jarHoles, radius = 2.5f, center = Offset(60f, 22f))
}
