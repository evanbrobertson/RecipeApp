package app.crumb.android.ui.recipe

import app.crumb.android.data.Flag
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class RecipeLogicTest {
    private val day = 86_400_000L
    private val start = 1_767_225_600_000L // 2026-01-01T00:00:00Z

    @Test
    fun cookedLineReadsLikeTheWeb() {
        val at = "2026-01-01T00:00:00Z"
        assertNull(cookedLine(null))
        assertNull(cookedLine(CookStats(0, at)))
        assertEquals("Cooked once", cookedLine(CookStats(1, null)))
        assertEquals("Cooked 2 times", cookedLine(CookStats(2, null)))
        assertEquals("Cooked once · last today", cookedLine(CookStats(1, at), start + day / 2))
        assertEquals("Cooked 3 times · last yesterday", cookedLine(CookStats(3, at), start + day + day / 2))
        assertEquals("Cooked 3 times · last 5 days ago", cookedLine(CookStats(3, at), start + 5 * day))
        assertEquals("Cooked 2 times · last 2 weeks ago", cookedLine(CookStats(2, at), start + 14 * day))
        assertEquals("Cooked 2 times · last 3 months ago", cookedLine(CookStats(2, at), start + 90 * day))
        assertEquals("Cooked 2 times · last 2 years ago", cookedLine(CookStats(2, at), start + 730 * day))
    }

    @Test
    fun plural() {
        assertEquals("1 line", plural(1, "line"))
        assertEquals("2 lines", plural(2, "line"))
        assertEquals("1 thing", plural(1, "thing"))
        assertEquals("3 things", plural(3, "thing"))
    }

    @Test
    fun fixTextMatchesEachFix() {
        assertEquals("Made “Sauce:” a section heading", fixText(flag(fix = "heading", text = "Sauce:")))
        assertEquals("Removed “Share this”", fixText(flag(fix = "removed", text = "Share this")))
        assertEquals("Moved a tip to the notes: “Use less salt”", fixText(flag(fix = "notes", text = "Use less salt")))
        assertEquals("Joined a step that was split in two: “and stir”", fixText(flag(fix = "joined", text = "and stir")))
        assertEquals(
            "Cleaned up stray checkboxes, web codes, repeated lines, quantities or times",
            fixText(flag(fix = "tidy")),
        )
        assertEquals(
            "Changed the category from “Puddings” to Dessert",
            fixText(flag(fix = "category", extras = mapOf("category" to "Dessert", "was" to "Puddings"))),
        )
        assertEquals("Set the category to Dessert", fixText(flag(fix = "category", extras = mapOf("category" to "Dessert"))))
        assertEquals(
            "Cleared the category “Puddings”: it isn't one of Crumb's",
            fixText(flag(fix = "category", extras = mapOf("was" to "Puddings"))),
        )
        assertEquals("Tidied “a stray line”", fixText(flag(text = "a stray line")))
    }

    @Test
    fun nutritionLabelsAndValues() {
        assertEquals("Calories", nutritionLabel("calories"))
        assertEquals("Saturated fat", nutritionLabel("saturatedFatContent"))
        assertEquals("Sugar", nutritionLabel("sugarContent"))
        assertEquals("fiber", nutritionLabel("fiber"))
        assertEquals("1g", nutritionValue(JsonPrimitive("1g")))
        assertEquals("12", nutritionValue(JsonPrimitive(12)))
        assertNull(nutritionValue(null))
    }

    @Test
    fun newlyFixedCountSkipsEarlierFixesAndCountsTidy() {
        val flags = listOf(
            Flag(1, "recipe", state = "fixed", kind = "tidy", detail = buildJsonObject { put("fix", "tidy"); put("count", 3) }),
            Flag(2, "ingredients", state = "fixed", kind = "junk", detail = buildJsonObject { put("fix", "removed") }),
            Flag(3, "instructions", state = "fixed", kind = "tidy", detail = buildJsonObject { put("fix", "tidy") }),
            Flag(4, "ingredients", state = "review", kind = "heading"),
        )
        assertEquals(2, newlyFixedCount(flags, setOf(1L)))
        assertEquals(5, newlyFixedCount(flags, emptySet()))
    }

    private fun flag(
        fix: String? = null,
        text: String? = "line",
        extras: Map<String, String> = emptyMap(),
    ): Flag = Flag(
        id = 1,
        field = "recipe",
        itemText = text,
        kind = "tidy",
        state = "fixed",
        detail = if (fix == null && extras.isEmpty()) null else buildJsonObject {
            fix?.let { put("fix", it) }
            extras.forEach { (k, v) -> put(k, v) }
        },
    )
}
