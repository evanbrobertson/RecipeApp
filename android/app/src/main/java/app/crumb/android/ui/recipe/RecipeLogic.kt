package app.crumb.android.ui.recipe

import app.crumb.android.data.Flag
import app.crumb.android.data.RecipeChecks
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import java.time.Instant

// Pure wording and arithmetic for the recipe page, ported from web/src/lib/history.ts and
// web/src/lib/checks.ts. RecipeLogicTest pins the strings.

/** The cook log behind "Cooked 3 times · last 2 weeks ago" (web `CookStats`, lib/recipe.ts). */
data class CookStats(val count: Int, val lastCookedAt: String?)

/**
 * "Cooked once", "Cooked 3 times", with " · last Tuesday"/" · last 2 weeks ago" when a date
 * is known. Ported from cookedLine() in web/src/lib/history.ts. [nowMs] is for tests.
 */
fun cookedLine(stats: CookStats?, nowMs: Long = System.currentTimeMillis()): String? {
    if (stats == null || stats.count <= 0) return null
    val times = if (stats.count == 1) "Cooked once" else "Cooked ${stats.count} times"
    val at = stats.lastCookedAt?.let { runCatching { Instant.parse(it).toEpochMilli() }.getOrNull() }
        ?: return times
    val days = Math.floorDiv(nowMs - at, 86_400_000L)
    val ago = when {
        days < 1 -> "today"
        days < 2 -> "yesterday"
        days < 14 -> "$days days ago"
        days < 60 -> "${Math.round(days / 7.0)} weeks ago"
        days < 365 -> "${Math.round(days / 30.0)} months ago"
        else -> "${Math.round(days / 365.0)} years ago"
    }
    return "$times · last $ago"
}

/** "1 line" / "2 lines" (web `plural`, lib/checks.ts). */
fun plural(n: Int, one: String, many: String = "${one}s"): String = "$n ${if (n == 1) one else many}"

/** A line quoted in a sentence, shortened when long (web `quote`). */
fun quote(text: String, max: Int = 48): String {
    val t = text.trim()
    val shown = if (t.length > max) t.take(max - 1).trimEnd() + "…" else t
    return "“$shown”"
}

/** What Wee Chef did to a line on import (web `fixText`). */
fun fixText(flag: Flag): String {
    val detail = flag.detail as? JsonObject
    val text = flag.itemText.orEmpty()
    return when (detail.str("fix")) {
        "heading" -> "Made ${quote(text)} a section heading"
        "removed" -> "Removed ${quote(text)}"
        "notes" -> "Moved a tip to the notes: ${quote(text)}"
        "joined" -> "Joined a step that was split in two: ${quote(text)}"
        "tidy" -> "Cleaned up stray checkboxes, web codes, repeated lines, quantities or times"
        "category" -> {
            val category = detail.str("category")
            val was = detail.str("was")
            when {
                category != null && was != null -> "Changed the category from ${quote(was)} to $category"
                category != null -> "Set the category to $category"
                else -> "Cleared the category ${quote(was.orEmpty())}: it isn't one of Crumb's"
            }
        }
        else -> "Tidied ${quote(text)}"
    }
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
