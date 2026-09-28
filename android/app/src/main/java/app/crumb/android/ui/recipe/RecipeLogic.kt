package app.crumb.android.ui.recipe

import app.crumb.android.data.Flag
import app.crumb.android.data.RecipeChecks
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import java.time.Instant

// Wording and arithmetic for the recipe page (web/src/lib/history.ts and lib/checks.ts), mostly
// through crumb-core. RecipeLogicTest pins the strings.

/** The cook log behind "Cooked 3 times · last 2 weeks ago" (web `CookStats`, lib/recipe.ts). */
data class CookStats(val count: Int, val lastCookedAt: String?)

/** "Cooked 3 times · last 2 weeks ago", or null when never cooked. [nowMs] is for tests. */
fun cookedLine(stats: CookStats?, nowMs: Long = System.currentTimeMillis()): String? {
    if (stats == null || stats.count <= 0) return null
    val at = stats.lastCookedAt?.let { runCatching { Instant.parse(it).toEpochMilli() }.getOrNull() }
    return app.crumb.core.cookedLine(stats.count.toUInt(), at, nowMs)
}

/** "1 line" / "2 lines" (web `plural`, lib/checks.ts). */
fun plural(n: Int, one: String, many: String = "${one}s"): String = "$n ${if (n == 1) one else many}"

/** What Wee Chef did to a line on import (web `fixText`). */
fun fixText(flag: Flag): String {
    val detail = flag.detail as? JsonObject
    return app.crumb.core.fixText(detail.str("fix"), flag.itemText.orEmpty(), detail.str("category"), detail.str("was"))
}

/** The recipe's fixed flags, in check order. */
fun fixedFlags(checks: RecipeChecks?): List<Flag> = checks?.flags?.filter { it.state == "fixed" }.orEmpty()

/** The recipe's review flags, in check order. */
fun reviewFlags(checks: RecipeChecks?): List<Flag> = checks?.flags?.filter { it.state == "review" }.orEmpty()

/** "Wee Chef tidied 2 things". */
fun tidiedTitle(count: Int): String = "Wee Chef tidied ${plural(count, "thing")}"

/** "3 lines might need a look". */
fun mightNeedALook(count: Int): String = "${plural(count, "line")} might need a look"

/**
 * The fixes this check made that weren't there before: each flag counts once, except a "tidy"
 * flag which counts the small things it cleaned up.
 */
fun newlyFixedCount(flags: List<Flag>, before: Set<Long>): Int =
    flags.filter { it.state == "fixed" && it.id !in before }
        .sumOf { flag ->
            val detail = flag.detail as? JsonObject
            if (detail.str("fix") == "tidy") detail.int("count") ?: 1 else 1
        }

/** The web's nutritionLabels map (lib/recipe.ts:207-220). */
private val NutritionLabels = mapOf(
    "calories" to "Calories",
    "fatContent" to "Fat",
    "saturatedFatContent" to "Saturated fat",
    "unsaturatedFatContent" to "Unsaturated fat",
    "transFatContent" to "Trans fat",
    "carbohydrateContent" to "Carbs",
    "sugarContent" to "Sugar",
    "fiberContent" to "Fiber",
    "proteinContent" to "Protein",
    "cholesterolContent" to "Cholesterol",
    "sodiumContent" to "Sodium",
    "servingSize" to "Serving size",
)

/** A nutrition key as a label: the map's name, else the key with a trailing "Content" cut. */
fun nutritionLabel(key: String): String = NutritionLabels[key] ?: key.replace(Regex("Content$"), "")

/** A nutrition value: the raw string, a number printed plainly, or null when empty. */
fun nutritionValue(value: kotlinx.serialization.json.JsonElement?): String? {
    val primitive = value as? JsonPrimitive ?: return value?.toString()
    return primitive.content.takeIf { it.isNotEmpty() }
}

private fun JsonObject?.str(key: String): String? =
    (this?.get(key) as? JsonPrimitive)?.takeIf { it.isString }?.content

private fun JsonObject?.int(key: String): Int? =
    (this?.get(key) as? JsonPrimitive)?.content?.toIntOrNull()
