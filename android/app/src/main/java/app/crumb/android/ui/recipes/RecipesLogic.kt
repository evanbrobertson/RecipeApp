package app.crumb.android.ui.recipes

import app.crumb.android.data.Durations
import app.crumb.android.data.RecipeSummary
import app.crumb.core.kicker

// Pure logic for the recipes list, ported from web/src/islands/RecipesPage.svelte and
// web/src/lib/categories.ts. RecipesLogicTest pins the strings and the selection rules.

/** "1 recipe" / "2 recipes". */
fun recipeCountLabel(count: Int): String = "$count recipe" + if (count == 1) "" else "s"

/** The "Delete recipes?" body. */
fun deleteDescription(count: Int): String = "${recipeCountLabel(count)} will be permanently deleted."

/** "Deleted 3 recipes". */
fun deletedTitle(count: Int): String = "Deleted ${recipeCountLabel(count)}"

/** "Added 2 to Weeknight dinners". */
fun addedTitle(count: Int, book: String): String = "Added $count to $book"

/** The search placeholder: "Search 12 recipes" until a query is applied. */
fun searchPlaceholder(recipeCount: Int, hasQuery: Boolean): String =
    if (recipeCount > 0 && !hasQuery) "Search $recipeCount recipes" else "Search by name or ingredient…"

/** The "Nothing matches" line: the query in curly quotes, or a nudge to another category. */
fun noMatchesDescription(query: String): String =
    if (query.isNotEmpty()) "No recipes found for “$query”." else "Try another category."

/** The card's muted line: "Breakfast · British · 30m" (web RecipeCard's `sub`). */
fun cardSubtitle(recipe: RecipeSummary): String {
    val category = kicker(recipe.recipeCategory, recipe.recipeCuisine).takeIf { it.isNotBlank() }
    return listOfNotNull(category, Durations.display(recipe.totalTime)).joinToString(" · ")
}

/**
 * The categories present among [recipes], in [order] (app.crumb.core.categories()).
 * A recipe with no category is only ever found under "Everything".
 */
fun categoriesIn(recipes: List<RecipeSummary>, order: List<String>): List<String> {
    val present = recipes.mapNotNull { it.recipeCategory?.takeIf { c -> c.isNotBlank() } }.toHashSet()
    return order.filter { it in present }
}

/** Flip one recipe's membership (web's toggle). */
fun toggleSelection(selected: Set<Long>, id: Long): Set<Long> =
    if (id in selected) selected - id else selected + id

/** True when there is something to select and all of it already is. */
fun allSelected(visible: List<Long>, selected: Set<Long>): Boolean =
    visible.isNotEmpty() && visible.all { it in selected }

/** Select every visible recipe, or clear when they all already are (web's toggleAll). */
fun toggleAllSelection(visible: List<Long>, selected: Set<Long>): Set<Long> =
    if (allSelected(visible, selected)) emptySet() else visible.toSet()

/**
 * The web's `requestId` guard: only the newest search may write its results. Each request
 * takes a token from [begin]; [isCurrent] is true only for the most recent one.
 */
class SearchGate {
    private var latest = 0

    fun begin(): Int = ++latest

    fun isCurrent(token: Int): Boolean = token == latest
}
