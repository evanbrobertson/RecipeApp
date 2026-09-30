package app.crumb.android.ui.components

import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.translate
import kotlin.math.sqrt
import androidx.compose.ui.unit.dp
import app.crumb.android.ui.theme.CrumbColors

/**
 * The glossy green backsplash (web `.tile-surface`): 32dp tiles with 2dp grout on the top
 * and left of each, and a soft highlight at 30%/30% of every tile. Brand moments only (the
 * home header, cook mode chrome), never behind body text.
 */
fun Modifier.tileSurface(colors: CrumbColors): Modifier = drawBehind {
    val tile = 32.dp.toPx()
    val grout = 2.dp.toPx()
    val shift = 1.dp.toPx()
    drawRect(colors.tileBase)
    // CSS "circle at 30% 30%" sizes to the farthest corner, and fades out at 55% of that
    val gloss = Brush.radialGradient(
        0f to Color.White.copy(alpha = colors.tileGloss),
        0.55f to Color.Transparent,
        center = Offset(tile * 0.3f, tile * 0.3f),
        radius = tile * 0.7f * sqrt(2f),
    )
    var y = -shift
    while (y < size.height) {
        var x = -shift
        while (x < size.width) {
            translate(x, y) {
                drawRect(gloss, size = Size(tile, tile))
                drawRect(colors.tileGrout, size = Size(tile, grout))
                drawRect(colors.tileGrout, size = Size(grout, tile))
            }
            x += tile
        }
        y += tile
    }
}
