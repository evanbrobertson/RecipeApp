package app.crumb.android.ui.edit

import app.crumb.android.data.CrumbJson
import app.crumb.android.data.Flag
import app.crumb.android.data.Recipe
import app.crumb.core.DraftSection
import app.crumb.core.RecipeDraft
import app.crumb.core.SectionKind
import app.crumb.core.flagIsHere
import app.crumb.core.photoFlagIsHere
import app.crumb.core.recipeDraftFromRecipe
import app.crumb.core.recipeDraftProblem
import app.crumb.core.recipeDraftToJson

// The draft conversions behind RecipeEditor (web/src/components/RecipeEditor.svelte) live in
// crumb-core's editor module: the form as text, saving it, and Wee Chef's one-tap fixes. This
// file only hands the app's models across.

/** A recipe as a form, with its nutrition typed back into "key: value" lines. */
fun recipeDraft(recipe: Recipe): RecipeDraft =
    recipeDraftFromRecipe(CrumbJson.encodeToString(Recipe.serializer(), recipe))

/** The value of one of core's time, yield and label fields (`metaFields` keys). */
fun RecipeDraft.meta(key: String): String = when (key) {
    "prepTime" -> prepTime
    "cookTime" -> cookTime
    "freezeTime" -> freezeTime
    "totalTime" -> totalTime
    "recipeYield" -> recipeYield
    "recipeCategory" -> recipeCategory
    "recipeCuisine" -> recipeCuisine
    "author" -> author
    else -> ""
}

/** The draft with one of core's time, yield and label fields set. */
fun RecipeDraft.withMeta(key: String, value: String): RecipeDraft = when (key) {
    "prepTime" -> copy(prepTime = value)
    "cookTime" -> copy(cookTime = value)
    "freezeTime" -> copy(freezeTime = value)
    "totalTime" -> copy(totalTime = value)
    "recipeYield" -> copy(recipeYield = value)
    "recipeCategory" -> copy(recipeCategory = value)
    "recipeCuisine" -> copy(recipeCuisine = value)
    "author" -> copy(author = value)
    else -> this
}

/** The review flag on a photo link the site refuses, while the draft still has that link. */
fun photoFlag(flags: List<Flag>, draft: RecipeDraft): Flag? =
    flags.find { it.itemText != null && photoFlagIsHere(draft, it.field, it.state, it.itemText) }

/** The review flags that belong under [section] of [kind]: its line is still there. */
fun flagsHere(flags: List<Flag>, kind: SectionKind, section: DraftSection): List<Flag> =
    flags.filter { it.itemText != null && flagIsHere(kind, section, it.field, it.state, it.itemText) }

/** The form's create/update body, or why it can't be saved yet. */
fun saveBody(draft: RecipeDraft): Result<String> =
    recipeDraftProblem(draft)?.let { Result.failure(IllegalStateException(it)) }
        ?: runCatching { recipeDraftToJson(draft) }
