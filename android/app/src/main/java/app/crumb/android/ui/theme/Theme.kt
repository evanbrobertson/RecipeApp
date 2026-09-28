package app.crumb.android.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.crumb.android.R

/** The Green Tile tokens from web/src/styles/app.css, light and "evening kitchen". */
@Immutable
data class CrumbColors(
    val canvas: Color,
    val sunk: Color,
    val paper: Color,
    val tint: Color,
    val line: Color,
    val lineStrong: Color,
    val ink: Color,
    val inkMuted: Color,
    val primary: Color,
    val tile: Color,
    val onTile: Color,
    val butter: Color,
    val onButter: Color,
    val nav: Color,
    val navInk: Color,
    val error: Color,
    /** Tint for pressed/selected fills (web --bg-accented). */
    val accented: Color,
    val inkDimmed: Color,
    val butterHover: Color,
    /** The glossy backsplash: base fill, grout lines and the gloss highlight's alpha. */
    val tileBase: Color,
    val tileGrout: Color,
    val tileGloss: Float,
    val shelfWood: Color,
    val shelfWoodDark: Color,
    val dark: Boolean,
)

val LightColors = CrumbColors(
    canvas = Color(0xFFF5F1E6),
    sunk = Color(0xFFEFEADD),
    paper = Color(0xFFFFFDF8),
    tint = Color(0xFFE4ECE3),
    line = Color(0xFFE3DFD0),
    lineStrong = Color(0xFFD3CEBD),
    ink = Color(0xFF1C2B22),
    inkMuted = Color(0xFF56635A),
    primary = Color(0xFF2F6B4F),
    tile = Color(0xFF2F6B4F),
    onTile = Color(0xFFFFFDF8),
    butter = Color(0xFFF3DA8B),
    onButter = Color(0xFF1C2B22),
    nav = Color(0xFF1C2B22),
    navInk = Color(0xFFB7C4BB),
    error = Color(0xFFA8432C),
    accented = Color(0xFFD6E2D5),
    inkDimmed = Color(0xFF5F6B63),
    butterHover = Color(0xFFEDCF6F),
    tileBase = Color(0xFF2F6B4F),
    tileGrout = Color(0xFF265A42),
    tileGloss = 0.1f,
    shelfWood = Color(0xFF2F6B4F),
    shelfWoodDark = Color(0xFF265A42),
    dark = false,
)

val DarkColors = CrumbColors(
    canvas = Color(0xFF141C17),
    sunk = Color(0xFF18211B),
    paper = Color(0xFF1D2721),
    tint = Color(0xFF24322A),
    line = Color(0xFF2C3830),
    lineStrong = Color(0xFF36453B),
    ink = Color(0xFFEFE9DA),
    inkMuted = Color(0xFFA8B3AA),
    primary = Color(0xFF93C4A3),
    tile = Color(0xFF3A7859),
    onTile = Color(0xFFF5F0E2),
    butter = Color(0xFFF0D582),
    onButter = Color(0xFF141C17),
    nav = Color(0xFF0D130F),
    navInk = Color(0xFF8E9C92),
    error = Color(0xFFE59A83),
    accented = Color(0xFF2C3D33),
    inkDimmed = Color(0xFF9BA69D),
    butterHover = Color(0xFFF5DF9C),
    tileBase = Color(0xFF1F3D2E),
    tileGrout = Color(0xFF17301F),
    tileGloss = 0.06f,
    shelfWood = Color(0xFF3B7A5A),
    shelfWoodDark = Color(0xFF2D5F45),
    dark = true,
)

val LocalCrumbColors = staticCompositionLocalOf { LightColors }

object Crumb {
    val colors: CrumbColors
        @Composable get() = LocalCrumbColors.current
}

private fun nunito(weight: Int) = Font(
    R.font.nunito_sans,
    weight = FontWeight(weight),
    variationSettings = FontVariation.Settings(FontVariation.weight(weight)),
)

val NunitoSans = FontFamily(nunito(400), nunito(600), nunito(700), nunito(800))
val DmSerif = FontFamily(Font(R.font.dm_serif_display))
val Caveat = FontFamily(Font(R.font.caveat))

private val Body = TextStyle(fontFamily = NunitoSans)
private val Serif = TextStyle(fontFamily = DmSerif, fontWeight = FontWeight.Normal)

val CrumbTypography = Typography(
    displaySmall = Serif.copy(fontSize = 36.sp, lineHeight = 42.sp),
    headlineLarge = Serif.copy(fontSize = 32.sp, lineHeight = 38.sp),
    headlineMedium = Serif.copy(fontSize = 28.sp, lineHeight = 34.sp),
    headlineSmall = Serif.copy(fontSize = 24.sp, lineHeight = 30.sp),
    titleLarge = Serif.copy(fontSize = 22.sp, lineHeight = 28.sp),
    titleMedium = Body.copy(fontSize = 17.sp, lineHeight = 24.sp, fontWeight = FontWeight.W700),
    titleSmall = Body.copy(fontSize = 15.sp, lineHeight = 20.sp, fontWeight = FontWeight.W700),
    bodyLarge = Body.copy(fontSize = 17.sp, lineHeight = 26.sp),
    bodyMedium = Body.copy(fontSize = 15.sp, lineHeight = 22.sp),
    bodySmall = Body.copy(fontSize = 13.sp, lineHeight = 18.sp),
    labelLarge = Body.copy(fontSize = 15.sp, lineHeight = 20.sp, fontWeight = FontWeight.W700),
    labelMedium = Body.copy(fontSize = 13.sp, lineHeight = 18.sp, fontWeight = FontWeight.W600),
    labelSmall = Body.copy(fontSize = 13.sp, lineHeight = 16.sp, fontWeight = FontWeight.W600),
)

/** Four radii only: 16 cards, 12 controls and photos, full pills, and book spines. */
val CrumbShapes = Shapes(
    extraSmall = RoundedCornerShape(12.dp),
    small = RoundedCornerShape(12.dp),
    medium = RoundedCornerShape(16.dp),
    large = RoundedCornerShape(16.dp),
    extraLarge = RoundedCornerShape(16.dp),
)

/**
 * Light or dark for [settings], as the web's theme-boot.js decides it. In "Sunrise & sunset"
 * the answer is re-checked at the next sunrise or sunset (and at least hourly).
 */
@Composable
fun rememberDark(settings: ThemeSettings): Boolean {
    val system = isSystemInDarkTheme()
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    return when (settings.mode) {
        ThemeMode.Light -> false
        ThemeMode.Dark -> true
        ThemeMode.System -> system
        ThemeMode.Sun -> {
            // Back from the background: a timer may have been missed (the web's visibilitychange)
            LifecycleResumeEffect(Unit) {
                now = System.currentTimeMillis()
                onPauseOrDispose {}
            }
            val sun = SunClock.sun(now, settings.location ?: SunClock.estimate())
            LaunchedEffect(sun.next) {
                kotlinx.coroutines.delay((sun.next - System.currentTimeMillis() + 1000).coerceIn(60_000, 3_600_000))
                now = System.currentTimeMillis()
            }
            sun.dark
        }
    }
}

@Composable
fun CrumbTheme(dark: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    val c = if (dark) DarkColors else LightColors
    val scheme = if (dark) {
        darkColorScheme(
            primary = c.primary, onPrimary = c.canvas,
            primaryContainer = c.tile, onPrimaryContainer = c.onTile,
            secondaryContainer = c.butter, onSecondaryContainer = c.onButter,
            background = c.canvas, onBackground = c.ink,
            surface = c.canvas, onSurface = c.ink, onSurfaceVariant = c.inkMuted,
            surfaceContainerLowest = c.paper, surfaceContainerLow = c.paper,
            surfaceContainer = c.paper, surfaceContainerHigh = c.tint, surfaceContainerHighest = c.tint,
            surfaceVariant = c.tint, outline = c.lineStrong, outlineVariant = c.line,
            error = c.error,
        )
    } else {
        lightColorScheme(
            primary = c.primary, onPrimary = c.onTile,
            primaryContainer = c.tile, onPrimaryContainer = c.onTile,
            secondaryContainer = c.butter, onSecondaryContainer = c.onButter,
            background = c.canvas, onBackground = c.ink,
            surface = c.canvas, onSurface = c.ink, onSurfaceVariant = c.inkMuted,
            surfaceContainerLowest = c.paper, surfaceContainerLow = c.paper,
            surfaceContainer = c.paper, surfaceContainerHigh = c.tint, surfaceContainerHighest = c.tint,
            surfaceVariant = c.tint, outline = c.lineStrong, outlineVariant = c.line,
            error = c.error,
        )
    }
    CompositionLocalProvider(LocalCrumbColors provides c) {
        MaterialTheme(colorScheme = scheme, typography = CrumbTypography, shapes = CrumbShapes, content = content)
    }
}
