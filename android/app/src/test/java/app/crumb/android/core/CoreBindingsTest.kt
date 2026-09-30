package app.crumb.android.core

import app.crumb.core.CookProgress
import app.crumb.core.FieldCount
import app.crumb.core.IngredientLine
import app.crumb.core.PrepGroupKind
import app.crumb.core.ShelfBook
import app.crumb.core.Vessel
import app.crumb.core.bookCoverColor
import app.crumb.core.bookSize
import app.crumb.core.cookProgress
import app.crumb.core.dateLabel
import app.crumb.core.findTimers
import app.crumb.core.fixWeight
import app.crumb.core.greeting
import app.crumb.core.ingredientColor
import app.crumb.core.ingredientsForStep
import app.crumb.core.isVideoUrl
import app.crumb.core.isoDurationMinutes
import app.crumb.core.jobProgress
import app.crumb.core.kicker
import app.crumb.core.linksInText
import app.crumb.core.lookTitle
import app.crumb.core.miseEnPlace
import app.crumb.core.nutritionLabel
import app.crumb.core.plural
import app.crumb.core.prepGroupTitle
import app.crumb.core.prepGroups
import app.crumb.core.reviewSummary
import app.crumb.core.scaleIngredient
import app.crumb.core.stackBooks
import app.crumb.core.tidiedTitle
import app.crumb.core.vesselCountLabel
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Calls the real Rust crumb-core (host build) through the generated bindings. */
class CoreBindingsTest {
    @Test fun scales_ingredients() {
        assertEquals("3 cups flour", scaleIngredient("2 cups flour", 1.5))
    }

    @Test fun finds_step_timers() {
        val timers = findTimers("Bake for 25–30 minutes, then rest 5 min")
        assertEquals(listOf(1800u, 300u), timers.map { it.seconds })
    }

    @Test fun matches_step_ingredients_by_index() {
        val lines = listOf("100g plain flour", "2 large eggs", "300ml milk", "caster sugar to serve")
            .map { IngredientLine(it, null) }
        assertEquals(listOf(0u, 1u, 2u), ingredientsForStep("Whisk the flour, eggs and milk", null, lines))
    }

    @Test fun plans_mise_en_place() {
        val items = miseEnPlace(listOf("300ml milk", "a pinch of salt"))
        assertEquals(Vessel.PINCH, items[1].vessel)
    }

    @Test fun reads_durations_and_kickers() {
        assertEquals(90.0, isoDurationMinutes("PT1H30M")!!, 0.0)
        assertEquals("Breakfast · British", kicker("Breakfast", "British"))
    }

    @Test fun jobProgressAndVideoLinks() {
        assertEquals("Queued (2nd)…", jobProgress("queued", 2u))
        assertEquals("Up next…", jobProgress("queued", 1u))
        assertTrue(isVideoUrl("https://www.tiktok.com/@cook/video/1"))
        assertFalse(isVideoUrl("https://example.com/pie"))
    }

    @Test fun findsEveryDistinctLinkInAnyText() {
        val text = "Try these: https://www.bbcgoodfood.com/recipes/easy-pancakes, and (https://example.com/soup). " +
            "Again: https://www.bbcgoodfood.com/recipes/easy-pancakes"
        assertEquals(
            listOf("https://www.bbcgoodfood.com/recipes/easy-pancakes", "https://example.com/soup"),
            linksInText(text),
        )
    }

    @Test fun groupsAPrepList() {
        val groups = prepGroups(listOf("2 onions, diced", "500 g flour"))
        assertEquals(listOf(PrepGroupKind.CHOP, PrepGroupKind.MEASURE), groups.map { it.kind })
        assertEquals(1u, groups[1].items[0].key)
        assertEquals("Chop & prep", prepGroupTitle(PrepGroupKind.CHOP))
        assertEquals("2 × small bowls", vesselCountLabel(Vessel.SMALL, 2u))
        assertEquals("#faf5e8", ingredientColor("Plain Flour"))
    }

    @Test fun shelfGeometryComesFromCore() {
        val book = ShelfBook(1, "Book 1", null, 3)
        assertEquals(40u, bookSize(book).thickness)
        assertEquals("tile", bookCoverColor("purple"))
        assertEquals(listOf(listOf(0u, 1u), listOf(2u)), stackBooks(listOf(book, book.copy(id = 2), book.copy(id = 3)), 2u))
    }

    @Test fun homeAndRecipePageWording() {
        assertEquals("what's for lunch?", greeting(12u))
        assertEquals("Jan 1, 1970", dateLabel(0, 0))
        assertEquals(CookProgress(3, 8), cookProgress(2, 8))
        assertEquals("Wee Chef tidied 2 things", tidiedTitle(2u))
        assertEquals("1 line might need a look", lookTitle(1u))
        assertEquals("Carbs", nutritionLabel("carbohydrateContent"))
        assertEquals(4u, fixWeight("""{"fix":"tidy","count":4}"""))
        assertEquals("2 steps · 1 ingredient", reviewSummary(listOf(FieldCount("ingredients", 1u), FieldCount("instructions", 2u))))
        assertEquals("1 broken photo link", reviewSummary(listOf(FieldCount("image", 1u))))
        assertEquals("3 lines", plural(3u, "line"))
    }
}
