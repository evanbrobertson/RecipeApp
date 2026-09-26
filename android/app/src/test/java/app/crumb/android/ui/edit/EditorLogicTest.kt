package app.crumb.android.ui.edit

import app.crumb.android.data.Flag
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The RecipeEditor conversions and Wee Chef fixes (web/src/components/RecipeEditor.svelte). */
class EditorLogicTest {
    private fun flag(kind: String, text: String, field: String = "instructions") =
        Flag(id = 1, field = field, itemText = text, kind = kind, state = "review")

    @Test fun strips_bullets_and_numbering() {
        val sections = sectionList(listOf(DraftSection("", "- 2 cups flour\n1. 1 tsp salt\n• pinch of nutmeg")))
        assertEquals(listOf("2 cups flour", "1 tsp salt", "pinch of nutmeg"), sections.single().items)
    }

    @Test fun trims_lines_and_drops_blank_ones() {
        val sections = sectionList(listOf(DraftSection("", "  flour  \n\n \nsalt")))
        assertEquals(listOf("flour", "salt"), sections.single().items)
    }

    @Test fun drops_sections_left_empty() {
        val sections = sectionList(listOf(DraftSection("For the sauce", "  \n \n"), DraftSection("", "bake")))
        assertEquals(1, sections.size)
        assertEquals(listOf("bake"), sections.single().items)
        assertNull(sections.single().name)
    }

    @Test fun keeps_section_names_and_nulls_blank_ones() {
        val sections = sectionList(listOf(DraftSection(" For the sauce ", "flour"), DraftSection("  ", "bake")))
        assertEquals("For the sauce", sections[0].name)
        assertNull(sections[1].name)
    }

    @Test fun empty_fields_become_null_never_blank() {
        val fields = RecipeDraft(title = "  Pie  ", description = "  ", url = " https://x ").toFields()
        assertEquals("Pie", fields.title)
        assertNull(fields.description)
        assertEquals("https://x", fields.url)
        assertNull(fields.notes)
        assertNull(fields.nutrition)
    }

    @Test fun parses_nutrition_key_value_lines() {
        assertEquals(
            mapOf("calories" to "320", "protein" to "12 g"),
            parseNutrition("calories: 320\nno colon here\nprotein: 12 g\n: 9\nfat:"),
        )
    }

    @Test fun later_nutrition_keys_win() {
        assertEquals(mapOf("calories" to "2"), parseNutrition("calories: 1\ncalories: 2"))
    }

    @Test fun quotes_and_shortens() {
        assertEquals("“Short”", quote("Short"))
        assertEquals("“abc…”", quote("abcdef", 4))
        assertEquals("“Sauce:”", quote("  Sauce:  "))
    }

    @Test fun heading_fix_names_the_first_section_when_nothing_is_above() {
        val draft = RecipeDraft(instructions = listOf(DraftSection("", "Sauce:\nMix it")))
        val fix = fixFor("instructions", 0, flag("heading", "Sauce:"), draft)!!
        assertEquals("Make it a heading", fix.label)
        val after = fix.apply()
        assertEquals(listOf(DraftSection("Sauce", "Mix it")), after.instructions)
    }

    @Test fun heading_fix_splits_when_lines_are_above() {
        val draft = RecipeDraft(instructions = listOf(DraftSection("", "Prep\nSauce:\nMix it")))
        val after = fixFor("instructions", 0, flag("heading", "Sauce:"), draft)!!.apply()
        assertEquals(listOf(DraftSection("", "Prep"), DraftSection("Sauce", "Mix it")), after.instructions)
    }

    @Test fun heading_fix_needs_lines_under_it() {
        val draft = RecipeDraft(instructions = listOf(DraftSection("", "Sauce:")))
        assertNull(fixFor("instructions", 0, flag("heading", "Sauce:"), draft))
    }

    @Test fun junk_fix_removes_the_line() {
        val draft = RecipeDraft(ingredients = listOf(DraftSection("", "2 cups flour\nSubscribe to my channel")))
        val fix = fixFor("ingredients", 0, flag("junk", "Subscribe to my channel", field = "ingredients"), draft)!!
        assertEquals("Remove it", fix.label)
        assertEquals(listOf(DraftSection("", "2 cups flour")), fix.apply().ingredients)
    }

    @Test fun not_instruction_fix_moves_a_tip_to_the_notes() {
        val draft = RecipeDraft(notes = "Use the good pan.", instructions = listOf(DraftSection("", "Mix it\nTip: cold butter")))
        val fix = fixFor("instructions", 0, flag("not_instruction", "Tip: cold butter"), draft)!!
        assertEquals("Move to notes", fix.label)
        val after = fix.apply()
        assertEquals(listOf(DraftSection("", "Mix it")), after.instructions)
        assertEquals("Use the good pan.\n\nTip: cold butter", after.notes)
    }

    @Test fun fragment_fix_joins_with_the_step_above() {
        val draft = RecipeDraft(instructions = listOf(DraftSection("", "Mix the dry\nand the wet")))
        val fix = fixFor("instructions", 0, flag("fragment", "and the wet"), draft)!!
        assertEquals("Join with the step above", fix.label)
        assertEquals(listOf(DraftSection("", "Mix the dry and the wet")), fix.apply().instructions)
    }

    @Test fun no_fix_when_the_flagged_line_is_gone() {
        val draft = RecipeDraft(instructions = listOf(DraftSection("", "Mix it")))
        assertNull(fixFor("instructions", 0, flag("junk", "not there"), draft))
    }
}
