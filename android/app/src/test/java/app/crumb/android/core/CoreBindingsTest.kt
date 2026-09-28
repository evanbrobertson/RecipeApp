package app.crumb.android.core

import app.crumb.core.IngredientLine
import app.crumb.core.Vessel
import app.crumb.core.findTimers
import app.crumb.core.ingredientsForStep
import app.crumb.core.isoDurationMinutes
import app.crumb.core.kicker
import app.crumb.core.miseEnPlace
import app.crumb.core.scaleIngredient
import org.junit.Assert.assertEquals
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
}
