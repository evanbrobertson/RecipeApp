package app.crumb.android.ui.edit

import app.crumb.android.data.Flag
import app.crumb.android.data.Recipe
import app.crumb.android.data.RecipeFields
import app.crumb.android.data.Section
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

// The draft conversions behind RecipeEditor (web/src/components/RecipeEditor.svelte): the form
// as text, and the Wee Chef one-tap fixes. EditorLogicTest pins them.

/** One section as the editor holds it: a name and one item per line. */
data class DraftSection(val name: String = "", val text: String = "")

/** The whole form as text, before [toFields] turns it into the server's [RecipeFields]. */
data class RecipeDraft(
    val title: String = "",
    val description: String = "",
    val author: String = "",
    val prepTime: String = "",
    val cookTime: String = "",
    val freezeTime: String = "",
    val totalTime: String = "",
    val recipeYield: String = "",
    val recipeCategory: String = "",
    val recipeCuisine: String = "",
    val url: String = "",
    val image: String = "",
    val notes: String = "",
    val nutrition: String = "",
    val ingredients: List<DraftSection> = listOf(DraftSection()),
    val instructions: List<DraftSection> = listOf(DraftSection()),
)

/** A leading bullet ("- ", "• ") or numbering ("1. ", "2) "), as the web strips on save. */
private val ItemMarker = Regex("""^\s*(?:[-*•]|\d+[.)])\s+""")

/** The line without its leading bullet or number. */
fun stripItemMarker(line: String): String = ItemMarker.replace(line, "")

/** A recipe's sections as drafts, or one blank section when it has none. */
fun draftSections(sections: List<Section>?): List<DraftSection> =
    if (sections.isNullOrEmpty()) listOf(DraftSection())
    else sections.map { DraftSection(it.name.orEmpty(), it.items.joinToString("\n")) }

/** Drafts as server sections: bullet/number markers gone, blank lines dropped, empty sections gone. */
fun sectionList(sections: List<DraftSection>): List<Section> = sections.mapNotNull { section ->
    val items = section.text.split("\n").map { stripItemMarker(it).trim() }.filter { it.isNotEmpty() }
    if (items.isEmpty()) null else Section(section.name.trim().ifEmpty { null }, items)
}

/** A text field's value, or null when it's blank, so the server clears it. */
fun nullableField(value: String): String? = value.trim().ifEmpty { null }

/** "calories: 320" lines as a nutrition map; lines without a "key: value" are ignored. */
fun parseNutrition(text: String): Map<String, String> = buildMap {
    text.split("\n").forEach { line ->
        val at = line.indexOf(':')
        if (at >= 0) {
            val key = line.substring(0, at).trim()
            val value = line.substring(at + 1).trim()
            if (key.isNotEmpty() && value.isNotEmpty()) put(key, value)
        }
    }
}

/** The draft as the server's create/update body. Empty fields go as null, never "". */
fun RecipeDraft.toFields(): RecipeFields {
    val nutrition = parseNutrition(nutrition)
    return RecipeFields(
        title = title.trim(),
        description = nullableField(description),
        url = nullableField(url),
        image = nullableField(image),
        author = nullableField(author),
        prepTime = nullableField(prepTime),
        cookTime = nullableField(cookTime),
        totalTime = nullableField(totalTime),
        freezeTime = nullableField(freezeTime),
        recipeYield = nullableField(recipeYield),
        recipeCategory = nullableField(recipeCategory),
        recipeCuisine = nullableField(recipeCuisine),
        ingredients = sectionList(ingredients),
        instructions = sectionList(instructions),
        nutrition = if (nutrition.isEmpty()) null else nutrition,
        notes = nullableField(notes),
    )
}

/** A recipe as a draft, with its nutrition typed back into "key: value" lines. */
fun recipeDraft(recipe: Recipe): RecipeDraft = RecipeDraft(
    title = recipe.title,
    description = recipe.description.orEmpty(),
    author = recipe.author.orEmpty(),
    prepTime = recipe.prepTime.orEmpty(),
    cookTime = recipe.cookTime.orEmpty(),
    freezeTime = recipe.freezeTime.orEmpty(),
    totalTime = recipe.totalTime.orEmpty(),
    recipeYield = recipe.recipeYield.orEmpty(),
    recipeCategory = recipe.recipeCategory.orEmpty(),
    recipeCuisine = recipe.recipeCuisine.orEmpty(),
    url = recipe.url.orEmpty(),
    image = recipe.image.orEmpty(),
    notes = recipe.notes.orEmpty(),
    nutrition = nutritionText(recipe.nutrition),
    ingredients = draftSections(recipe.ingredients),
    instructions = draftSections(recipe.instructions),
)

/** Nutrition as "key: value" lines (web Object.entries), preserving order. */
fun nutritionText(nutrition: JsonObject?): String =
    nutrition?.entries?.joinToString("\n") { (key, value) ->
        "$key: ${(value as? JsonPrimitive)?.content ?: value.toString()}"
    }.orEmpty()

/** A draft for a brand-new recipe, optionally pre-filled with the title from "From scratch". */
fun newDraft(title: String): RecipeDraft = RecipeDraft(title = title)

/** A recipe's category when it isn't one of [categories] any more, else null. */
fun legacyCategory(value: String, categories: List<String>): String? =
    value.takeIf { it.isNotBlank() && it !in categories }

/** A line quoted in a sentence, shortened when long (web `quote`, lib/checks.ts). */
fun quote(text: String, max: Int = 48): String {
    val trimmed = text.trim()
    return if (trimmed.length > max) "“${trimmed.take(max - 1).trimEnd()}…”" else "“$trimmed”"
}

/** A one-tap fix for a flagged line: its button label and the draft it produces. */
data class EditorFix(val label: String, val apply: () -> RecipeDraft)

/**
 * The fix offered for [flag] in the section at [index] of [draft]'s ingredients or instructions,
 * or null when the line is gone or no fix applies (web `fixFor`). The returned fix is bound to
 * [draft]; call [EditorFix.apply] to get the changed draft.
 */
fun fixFor(kind: String, index: Int, flag: Flag, draft: RecipeDraft): EditorFix? {
    val sections = if (kind == "ingredients") draft.ingredients else draft.instructions
    val section = sections.getOrNull(index) ?: return null
    val itemText = flag.itemText ?: return null
    val lines = section.text.split("\n")
    val at = lines.indexOfFirst { it.trim() == itemText }
    if (at < 0) return null

    fun without(drop: Int): List<String> = lines.filterIndexed { i, _ -> i != drop }

    fun withSections(list: List<DraftSection>): RecipeDraft =
        if (kind == "ingredients") draft.copy(ingredients = list) else draft.copy(instructions = list)

    fun replaceText(text: String): RecipeDraft {
        val list = sections.toMutableList()
        list[index] = section.copy(text = text)
        return withSections(list)
    }

    return when (flag.kind) {
        "heading" -> {
            // A heading needs lines under it, or the empty section would vanish on save
            if (lines.drop(at + 1).none { it.trim().isNotEmpty() }) return null
            EditorFix("Make it a heading") {
                val name = Regex("""[:.\s]+$""").replace(itemText, "")
                val before = lines.take(at).joinToString("\n").trim()
                val after = lines.drop(at + 1).joinToString("\n")
                if (before.isEmpty() && section.name.trim().isEmpty()) {
                    val list = sections.toMutableList()
                    list[index] = DraftSection(name, after)
                    withSections(list)
                } else {
                    val list = sections.toMutableList()
                    list[index] = section.copy(text = before)
                    list.add(index + 1, DraftSection(name, after))
                    withSections(list)
                }
            }
        }
        "junk" -> EditorFix("Remove it") { replaceText(without(at).joinToString("\n")) }
        "not_instruction" -> if (kind == "instructions") {
            EditorFix("Move to notes") {
                val notes = listOf(draft.notes.trim(), itemText).filter { it.isNotEmpty() }.joinToString("\n\n")
                replaceText(without(at).joinToString("\n")).copy(notes = notes)
            }
        } else null
        "fragment" -> if (at > 0) {
            EditorFix("Join with the step above") {
                val next = lines.toMutableList()
                next[at - 1] = next[at - 1].trimEnd() + " " + next[at].trim()
                next.removeAt(at)
                replaceText(next.joinToString("\n"))
            }
        } else null
        else -> null
    }
}
