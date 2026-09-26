package app.crumb.android.ui.books

import androidx.compose.ui.graphics.Color
import app.crumb.android.data.CookbookListItem
import kotlin.math.floor
import kotlin.math.min
import kotlin.math.sin

// The shelf's arithmetic, ported line for line from web/src/lib/books.ts so every book has the
// same size, lean and bands on the phone as in the browser. BookGeometryTest pins the values.

/** Cover cloth, the darker board edge, the title foil, and the two spine-band colours. */
data class BookLook(
    val cloth: Color,
    val shade: Color,
    val foil: Color,
    /** Cream needs an inset edge or it vanishes against a light page. */
    val outlined: Boolean,
    val bands: Pair<Color, Color>,
)

private val Forest = Color(0xFF1C2B22)
private val Tile = Color(0xFF2F6B4F)
private val Sage = Color(0xFFA9C4AE)
private val Butter = Color(0xFFF3DA8B)
private val Clay = Color(0xFFA55A40)
private val Cream = Color(0xFFF8F3E6)

/** The six cover colours, in picker order (the same in light and dark). */
val BookColors = listOf("forest", "tile", "sage", "butter", "clay", "cream")

private val Palette = mapOf(
    "forest" to BookLook(Forest, Color(0xFF111A15), Butter, false, Butter to Sage),
    "tile" to BookLook(Tile, Color(0xFF224F3A), Color(0xFFFFFDF8), false, Butter to Cream),
    "sage" to BookLook(Sage, Color(0xFF8AA890), Forest, false, Forest to Tile),
    "butter" to BookLook(Butter, Color(0xFFD9BD67), Forest, false, Forest to Clay),
    "clay" to BookLook(Clay, Color(0xFF7E412D), Color(0xFFFFF8EA), false, Butter to Cream),
    "cream" to BookLook(Cream, Color(0xFFDDD5C1), Forest, true, Tile to Clay),
)

private val Legacy = mapOf(
    "tomato" to "clay", "terracotta" to "clay", "mustard" to "butter", "ocean" to "tile",
    "plum" to "forest", "navy" to "forest", "charcoal" to "forest", "rose" to "cream",
)

/** Any stored colour name as one of the six (old ten-colour names included). */
fun bookColor(color: String?): String {
    val c = color.orEmpty().lowercase()
    return if (c in BookColors) c else Legacy[c] ?: "tile"
}

fun bookLook(color: String?): BookLook = Palette.getValue(bookColor(color))

fun randomBookColor(): String = BookColors.random()

/** Deterministic pseudo-random number in [0, 1) from an id, so books keep their size. */
fun seeded(id: Long, salt: Int = 0): Double {
    val x = sin(id * 9301.0 + salt * 49297.0) * 233280
    return x - floor(x)
}

/** JavaScript's Math.round: halves go up, also for negatives (-5.5 → -5). */
private fun jsRound(x: Double): Long = floor(x + 0.5).toLong()

/**
 * A book lying flat: [thickness] in dp grows with its recipes (never thinner than a comfortable
 * tap), [length] is a fraction of the tower's length, and [title] estimates the dp its spine
 * needs for the whole title, so a short book can stretch to fit.
 */
data class BookSize(val thickness: Int, val length: Double, val title: Int)

fun bookSize(book: CookbookListItem): BookSize = BookSize(
    thickness = jsRound(min(50.0, 38 + book.recipeCount * 0.8)).toInt(),
    length = 0.84 + seeded(book.id) * 0.16,
    title = jsRound(book.name.length * 7.6 + if (spineBand(book) != null) 52 else 28).toInt(),
)

/** How untidily a book sits: a small tilt (degrees) and a sideways nudge (dp). Feet sit flatter. */
data class BookLean(val tilt: Float, val nudge: Int)

fun bookLean(book: CookbookListItem, atFoot: Boolean = false): BookLean {
    val tilt = (seeded(book.id, 7) * 2 - 1) * if (atFoot) 0.6 else 2.5
    val nudge = (seeded(book.id, 11) * 2 - 1) * 7
    return BookLean((jsRound(tilt * 10) / 10.0).toFloat(), jsRound(nudge).toInt())
}

/** About half the books get two spine bands, in one of their two contrasting colours. */
fun spineBand(book: CookbookListItem): Color? {
    val r = seeded(book.id, 3)
    if (r < 0.45) return null
    val bands = bookLook(book.color).bands
    return if (r < 0.75) bands.first else bands.second
}

/**
 * Split books into [towers] stacks of roughly equal height, keeping their order. Each tower is
 * listed top to bottom in cookbook order, so the stack reads the way it looks.
 */
fun stackBooks(books: List<CookbookListItem>, towers: Int): List<List<CookbookListItem>> {
    val n = towers.coerceAtMost(books.size).coerceAtLeast(1)
    val total = books.sumOf { bookSize(it).thickness }
    val out = mutableListOf(mutableListOf<CookbookListItem>())
    var height = 0
    books.forEachIndexed { i, book ->
        val left = books.size - i
        val towersLeft = n - out.size
        // Start the next tower once this one reaches its share, but leave a book for every tower
        if (out.last().isNotEmpty() && towersLeft > 0 &&
            (height >= total.toDouble() * out.size / n || left <= towersLeft)
        ) {
            out.add(mutableListOf())
        }
        out.last().add(book)
        height += bookSize(book).thickness
    }
    return out
}
