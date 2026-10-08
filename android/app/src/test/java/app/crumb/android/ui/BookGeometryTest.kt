package app.crumb.android.ui

import app.crumb.android.data.CookbookListItem
import app.crumb.android.ui.books.Shelf
import app.crumb.android.ui.books.bookColor
import app.crumb.android.ui.books.bookIsPale
import app.crumb.android.ui.books.layoutShelf
import app.crumb.android.ui.books.spineOf
import app.crumb.core.ShelfItemKind
import app.crumb.core.seeded
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The layout is core's (parity-tested against the web there); these pin what the shelf relies on. */
class BookGeometryTest {
    private val names = listOf(
        "Weeknight dinners", "Christmas baking", "Soups", "Things to make when it's raining",
        "Ottolenghi-ish vegetable mains", "Grandma's Sunday roasts and other family favourites", "Bread",
        "Curries", "Summer salads", "Breakfast", "Pasta", "Meal prep for busy weeks", "Cakes",
    )
    private val books = names.mapIndexed { i, name -> CookbookListItem(id = i + 1L, name = name, recipeCount = (i * 7L) % 30) }

    @Test fun seededMatchesTheWeb() {
        assertEquals(0.461531434383, seeded(1, 0), 1e-9)
        assertEquals(0.520363897638, seeded(2, 0), 1e-9)
        assertEquals(0.001548065542, seeded(3, 0), 1e-9)
    }

    @Test fun everyBookIsPlacedOnceWithTheOutlineLast() {
        for (width in listOf(360f, 411f, 600f, 1000f)) {
            val rows = layoutShelf(books, width, single = false, addable = true)
            val placed = rows.flatMap { r -> r.items.mapNotNull { it.bookIndex?.toInt() } }
            assertEquals(books.indices.toList(), placed.sorted())
            // The outline comes after the last book (only a prop may follow it)
            val last = rows.last().items
            assertTrue(last.indexOfFirst { it.kind == ShelfItemKind.ADD } > last.indexOfLast { it.kind == ShelfItemKind.BOOK })
            assertEquals(1, rows.sumOf { r -> r.items.count { it.kind == ShelfItemKind.ADD } })
            // Nothing runs off a shelf
            rows.forEach { r -> r.items.forEach { assertTrue(it.x >= 0 && it.x + it.w <= width + 0.5) } }
        }
    }

    @Test fun homeIsOneRowThatScrolls() {
        val rows = layoutShelf(books, 360f, single = true, addable = false)
        assertEquals(1, rows.size)
        assertTrue(rows[0].width > 360)
        assertTrue(rows[0].items.none { it.kind == ShelfItemKind.ADD })
    }

    @Test fun longTitlesLieDownAndShortOnesStand() {
        assertTrue(spineOf(books[2]).standing)
        val long = spineOf(books[5])
        assertFalse(long.standing)
        assertEquals(2, long.lines.size)
        // Everything fits under the next shelf
        layoutShelf(books, 411f, single = false, addable = true).flatMap { it.items }
            .forEach { assertTrue(it.y + it.h <= Shelf.clearance) }
    }

    @Test fun legacyColoursMapToTheSix() {
        assertEquals("clay", bookColor("Tomato"))
        assertEquals("tile", bookColor(null))
        assertTrue(bookIsPale("butter"))
        assertFalse(bookIsPale("forest"))
    }
}
