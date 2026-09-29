package app.crumb.android.ui.recipe

import app.crumb.android.data.Flag
import app.crumb.android.data.RecipeChecks
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import app.crumb.core.CheckToast
import app.crumb.core.checkDoneToast
import app.crumb.core.fixWeight
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

/** What Wee Chef did to a line on import (web `fixText`). */
fun fixText(flag: Flag): String {
    val detail = flag.detail as? JsonObject
    return app.crumb.core.fixText(detail.str("fix"), flag.itemText.orEmpty(), detail.str("category"), detail.str("was"))
}

/** The recipe's fixed flags, in check order. */
fun fixedFlags(checks: RecipeChecks?): List<Flag> = checks?.flags?.filter { it.state == "fixed" }.orEmpty()

/** The recipe's review flags for lines of text, in check order (a dead photo link has its own row). */
fun reviewFlags(checks: RecipeChecks?): List<Flag> = reviewFlags(checks?.flags.orEmpty())

fun reviewFlags(flags: List<Flag>): List<Flag> = flags.filter { it.state == "review" && it.field != "image" }

/** The review flag on a photo link the site refuses, shown as its own row. */
fun photoFlag(checks: RecipeChecks?): Flag? = checks?.flags?.find { it.state == "review" && it.field == "image" }

/** "Wee Chef tidied 2 things" (crumb-core `tidiedTitle`). */
fun tidiedTitle(count: Int): String = app.crumb.core.tidiedTitle(count.toUInt())

/** "3 lines might need a look" (crumb-core `lookTitle`). */
fun mightNeedALook(count: Int): String = app.crumb.core.lookTitle(count.toUInt())

/**
 * The fixes this check made that weren't there before: each flag counts once, except a "tidy"
 * flag which counts the small things it cleaned up (crumb-core `fixWeight`).
 */
fun newlyFixedCount(flags: List<Flag>, before: Set<Long>): Int =
    flags.filter { it.state == "fixed" && it.id !in before }
        .sumOf { flag ->
            val detail = (flag.detail as? JsonObject) ?: JsonObject(emptyMap())
            runCatching { fixWeight(detail.toString()).toInt() }.getOrDefault(1)
        }

/**
 * The toast when a check someone asked for is done. As the web, only this check's fixes count,
 * and the photo's flag isn't a line to look at.
 */
fun checkToast(checks: RecipeChecks, fixedBefore: Set<Long>): CheckToast {
    val review = reviewFlags(checks.flags).size
    return checkDoneToast(checks.status, newlyFixedCount(checks.flags, fixedBefore).toUInt(), review.toUInt())
}

/** A nutrition value: the raw string, a number printed plainly, or null when empty. */
fun nutritionValue(value: kotlinx.serialization.json.JsonElement?): String? {
    val primitive = value as? JsonPrimitive ?: return value?.toString()
    return primitive.content.takeIf { it.isNotEmpty() }
}

private fun JsonObject?.str(key: String): String? =
    (this?.get(key) as? JsonPrimitive)?.takeIf { it.isString }?.content
