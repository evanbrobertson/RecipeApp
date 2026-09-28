package app.crumb.android.ui

import app.crumb.android.data.CookbookListItem
import app.crumb.android.ui.books.BookLean
import app.crumb.android.ui.books.bookColor
import app.crumb.android.ui.books.bookLean
import app.crumb.android.ui.books.bookSize
import app.crumb.android.ui.books.seeded
import app.crumb.android.ui.books.spineBand
import app.crumb.android.ui.books.stackBooks
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Expected values come from running web/src/lib/books.ts on the same books. */
class BookGeometryTest {
    private val books = (1L..7L).map { CookbookListItem(id = it, name = "Book $it", recipeCount = it * 3) }

    @Test fun seededMatchesTheWeb() {
        assertEquals(0.461531434383, seeded(1), 1e-9)
        assertEquals(0.520363897638, seeded(2), 1e-9)
        assertEquals(0.001548065542, seeded(3), 1e-9)
    }

    @Test fun sizesAndLeansMatchTheWeb() {
        assertEquals(40, bookSize(books[0]).thickness)
        assertEquals(0.9138450295012444, bookSize(books[0]).length, 1e-9)
        assertEquals(74, bookSize(books[0]).title)
        assertEquals(45, bookSize(books[2]).thickness)
        assertEquals(BookLean(-2.2f, -6), bookLean(books[0]))
        assertEquals(BookLean(-0.5f, -6), bookLean(books[0], atFoot = true))
        assertEquals(BookLean(1.5f, 1), bookLean(books[1]))
        assertNull(spineBand(books[0]))
    }

    @Test fun stacksTowersLikeTheWeb() {
        assertEquals(listOf(listOf(1L, 2, 3, 4), listOf(5L, 6, 7)), stackBooks(books, 2).map { t -> t.map { it.id } })
        assertEquals(listOf(listOf(1L, 2, 3), listOf(4L, 5), listOf(6L, 7)), stackBooks(books, 3).map { t -> t.map { it.id } })
    }

    @Test fun legacyColoursMapToTheSix() {
        assertEquals("clay", bookColor("Tomato"))
        assertEquals("tile", bookColor(null))
    }
}
