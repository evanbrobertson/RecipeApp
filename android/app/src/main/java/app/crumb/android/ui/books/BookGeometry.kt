package app.crumb.android.ui.books

import androidx.compose.ui.graphics.Color
import app.crumb.android.data.CookbookListItem
import app.crumb.core.ShelfBook
import app.crumb.core.bookEdgeAlpha
import app.crumb.core.bookColors

// The shelf's arithmetic and cover colours are crumb-core's (`bookSize`, `bookLean`,
// `spineBand`, `stackBooks`, `bookLook`), so every book has the same size, lean and bands on
// the phone as in the browser. These turn its hex strings and unsigned numbers into Compose
// colours and Ints.

/** Cover cloth, the darker board edge, the title foil, and the two spine-band colours. */
data class BookLook(
    val cloth: Color,
    val shade: Color,
    val foil: Color,
    /** Cream needs an inset edge (drawn at [EdgeAlpha]) or it vanishes against a light page. */
    val edge: Color?,
    val bands: Pair<Color, Color>,
) {
    val outlined get() = edge != null
}

/** The opacity of a cover's inset edge. */
val EdgeAlpha: Float by lazy { bookEdgeAlpha().toFloat() }

/** The six cover colours, in picker order (the same in light and dark). */
val BookColors: List<String> by lazy { bookColors() }

/** A core hex colour ("#2F6B4F") as a Compose colour. */
private fun hex(value: String): Color = Color(0xFF000000L or value.removePrefix("#").toLong(16))

/** Any stored colour name as one of the six (old ten-colour names included). */
fun bookColor(color: String?): String = app.crumb.core.bookCoverColor(color)

fun bookLook(color: String?): BookLook {
    val l = app.crumb.core.bookLook(color)
    return BookLook(hex(l.cloth), hex(l.shade), hex(l.foil), l.edge?.let(::hex), hex(l.bands[0]) to hex(l.bands[1]))
}

fun randomBookColor(): String = BookColors.random()

private fun CookbookListItem.shelf() = ShelfBook(id, name, color, recipeCount)

/**
 * A book lying flat: [thickness] in dp grows with its recipes (never thinner than a comfortable
 * tap), [length] is a fraction of the tower's length, and [title] estimates the dp its spine
 * needs for the whole title, so a short book can stretch to fit.
 */
data class BookSize(val thickness: Int, val length: Double, val title: Int)

fun bookSize(book: CookbookListItem): BookSize =
    app.crumb.core.bookSize(book.shelf()).let { BookSize(it.thickness.toInt(), it.length, it.title.toInt()) }

/** How untidily a book sits: a small tilt (degrees) and a sideways nudge (dp). Feet sit flatter. */
data class BookLean(val tilt: Float, val nudge: Int)

fun bookLean(book: CookbookListItem, atFoot: Boolean = false): BookLean =
    app.crumb.core.bookLean(book.shelf(), atFoot).let { BookLean(it.tilt.toFloat(), it.nudge) }

/** About half the books get two spine bands, in one of their two contrasting colours. */
fun spineBand(book: CookbookListItem): Color? = app.crumb.core.spineBand(book.shelf())?.let(::hex)

/**
 * Split books into [towers] stacks of roughly equal height, keeping their order. Each tower is
 * listed top to bottom in cookbook order, so the stack reads the way it looks.
 */
fun stackBooks(books: List<CookbookListItem>, towers: Int): List<List<CookbookListItem>> =
    app.crumb.core.stackBooks(books.map { it.shelf() }, towers.toUInt())
        .map { tower -> tower.map { books[it.toInt()] } }
