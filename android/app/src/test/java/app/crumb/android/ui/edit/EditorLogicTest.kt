package app.crumb.android.ui.edit

import app.crumb.android.data.CrumbJson
import app.crumb.android.data.Flag
import app.crumb.android.data.Recipe
import app.crumb.android.data.Section
import app.crumb.core.DraftSection
import app.crumb.core.SectionKind
import app.crumb.core.fixFor
import app.crumb.core.metaFields
import app.crumb.core.recipeDraftNew
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The app's side of the editor: models handed to crumb-core's editor and its answers back. */
class EditorLogicTest {
    private val soup = Recipe(
        id = 1,
        title = "Soup",
        video = "https://youtu.be/abc",
        ingredients = listOf(Section("Broth", listOf("stock", "salt"))),
        nutrition = CrumbJson.parseToJsonElement("""{"calories":320,"fat":"12 g"}""") as JsonObject,
    )

    private fun body(result: Result<String>): JsonObject = CrumbJson.parseToJsonElement(result.getOrThrow()).jsonObject

    @Test fun aSavedRecipeBecomesAFormAndSavesBackWithItsVideo() {
        val draft = recipeDraft(soup)
        assertEquals("https://youtu.be/abc", draft.video)
        assertEquals("Broth", draft.ingredients.single().name)
        assertEquals("stock\nsalt", draft.ingredients.single().text)
        assertEquals("calories: 320\nfat: 12 g", draft.nutrition)

        val json = body(saveBody(draft))
        assertEquals("https://youtu.be/abc", json["video"]!!.jsonPrimitive.content)
        assertEquals("320", json["nutrition"]!!.jsonObject["calories"]!!.jsonPrimitive.content)
        // Blank fields go as explicit nulls: the server reads a missing field as "leave it"
        assertEquals(JsonNull, json["description"])
        assertEquals(JsonNull, body(saveBody(draft.copy(video = "  ")))["video"])
    }

    @Test fun aFormWithoutATitleSaysWhyItCannotBeSaved() {
        assertEquals("Give the recipe a title.", saveBody(recipeDraftNew("  ")).exceptionOrNull()?.message)
        assertTrue(saveBody(recipeDraftNew("Pie")).isSuccess)
    }

    @Test fun theMetaFieldsReadAndWriteTheDraft() {
        var draft = recipeDraftNew("Pie")
        val keys = metaFields().map { it.key }
        keys.forEachIndexed { i, key -> draft = draft.withMeta(key, "v$i") }
        keys.forEachIndexed { i, key -> assertEquals("v$i", draft.meta(key)) }
        assertEquals("v4", draft.recipeYield)
    }

    @Test fun thePhotoFlagShowsOnlyWhileTheLinkIsStillThere() {
        val flag = Flag(9, "image", "https://x.test/a.jpg", "dead_image", "review")
        val other = Flag(10, "ingredients", "https://x.test/a.jpg", "junk", "review")
        val draft = recipeDraftNew("Pie").copy(image = "https://x.test/a.jpg")
        assertEquals(flag, photoFlag(listOf(other, flag), draft))
        assertNull(photoFlag(listOf(flag), draft.copy(image = "https://x.test/b.jpg")))
        assertNull(photoFlag(listOf(flag), recipeDraftNew("Pie")))
        assertNull(photoFlag(listOf(flag.copy(state = "dismissed")), draft))
    }

    @Test fun aLineFlagSitsUnderItsSectionAndOffersACoreFix() {
        val flag = Flag(1, "instructions", "Enjoy!", "junk", "review")
        val draft = recipeDraftNew("Pie").copy(instructions = listOf(DraftSection("", "Bake.\nEnjoy!")))
        val section = draft.instructions.single()
        assertEquals(listOf(flag), flagsHere(listOf(flag), SectionKind.INSTRUCTIONS, section))
        assertTrue(flagsHere(listOf(flag), SectionKind.INGREDIENTS, section).isEmpty())
        assertTrue(flagsHere(listOf(flag), SectionKind.INSTRUCTIONS, DraftSection("", "Bake.")).isEmpty())

        val fix = fixFor(draft, SectionKind.INSTRUCTIONS, 0u, flag.kind, flag.itemText!!)
        assertNotNull(fix)
        assertEquals("Remove it", fix!!.label)
        assertEquals("Bake.", fix.draft.instructions.single().text)
    }
}
