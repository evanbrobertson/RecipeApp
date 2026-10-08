package app.crumb.android.ui.books

import androidx.compose.ui.graphics.Color
import app.crumb.android.data.CookbookListItem
import app.crumb.core.ShelfBook
import app.crumb.core.ShelfMetrics
import app.crumb.core.ShelfRow
import app.crumb.core.ShelfSpine
import app.crumb.core.shelfLayout
import app.crumb.core.shelfMetrics
import app.crumb.core.spineFor
import app.crumb.core.bookEdgeAlpha
import app.crumb.core.bookColors

// The shelf's layout and cover colours are crumb-core's (`shelfLayout`, `spineFor`, `bookLook`),
// so every book stands or lies, and sits, where it does in the browser. These turn its hex
// strings into Compose colours.

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

/** Dark type on light cloth: its title catches the light below instead of being stamped in. */
fun bookIsPale(color: String?): Boolean = bookColor(color) in setOf("sage", "butter", "cream")

private fun CookbookListItem.shelf() = ShelfBook(id, name, color, recipeCount)

/** The shelf's fixed sizes, dp (core's `shelf::METRICS`). */
val Shelf: ShelfMetrics by lazy { shelfMetrics() }

/**
 * Where every book, prop and the new-book outline sits, shelf by shelf, for a shelf [width] dp
 * wide; [single] is one row as long as it needs. Items name their book by index into [books].
 */
fun layoutShelf(books: List<CookbookListItem>, width: Float, single: Boolean, addable: Boolean): List<ShelfRow> =
    shelfLayout(books.map { it.shelf() }, width.toDouble(), single, addable)

/** The spine a book gets on the shelf, for drawing it where the shelf didn't (the open book). */
fun spineOf(book: CookbookListItem): ShelfSpine = spineFor(book.shelf())
