package app.crumb.android.ui.home

import java.net.URI

// What the Add field holds and how to save it, ported from the module script in
// web/src/islands/TopBox.svelte. AddDetectTest pins the rules.

/** How the Add field will save what's in it. */
enum class AddMode(val label: String, val hint: String) {
    Auto("Auto-detect", "Crumb works out what you pasted"),
    Link("Link", "Import from a website"),
    Text("Recipe text", "Paste the whole recipe"),
    Photo("Photo", "Snap or upload the pages of a recipe"),
    File("File", "PDF, saved web page, text or backup"),
    Claude("Claude", "Send a recipe from a Claude chat"),
    Scratch("From scratch", "Write it out yourself"),
    Apps("Other apps", "Import from another recipe app"),
}

/** Handwritten hints beside Add on Home's tile; the web's short forms, which fit a phone. */
val AddTips = listOf(
    "paste the\nwhole recipe",
    "or snap a\nphoto of it",
    "PDFs\nwork too",
    "links from\nany site",
    "backups\nwork too",
    "a few links\nat once",
)

/** The mode Crumb worked out, and a short summary for the footer. */
data class Detected(val mode: AddMode, val summary: String)

private val UrlRe = Regex("""^https?://\S+$""", RegexOption.IGNORE_CASE)

// A line that reads like an ingredient: starts with an amount or a bullet, or names a unit
private val IngredientRe = Regex(
    """^\s*([-•*▢□]|\d|[½¼¾⅓⅔⅛]|a (pinch|handful|few))|\b(cups?|tbsp|tsp|tablespoons?|teaspoons?|grams?|g|kg|ml|l|oz|ounces?|lbs?|pounds?|cloves?|pinch)\b""",
    RegexOption.IGNORE_CASE,
)

/** Every token of [text] that is a web link, in order. */
fun linksIn(text: String): List<String> = text.trim().split(Regex("\\s+")).filter { UrlRe.matches(it) }

private fun host(url: String): String =
    runCatching { URI(url).host?.removePrefix("www.") }.getOrNull().orEmpty()

private fun plural(n: Int, one: String) = "$n $one" + if (n == 1) "" else "s"

/** What was pasted (web `detect`). */
fun detect(raw: String): Detected {
    val text = raw.trim()
    if (text.isEmpty()) return Detected(AddMode.Auto, "")
    val tokens = text.split(Regex("\\s+"))
    val urls = tokens.filter { UrlRe.matches(it) }
    if (urls.isNotEmpty() && urls.size == tokens.size) {
        return if (urls.size > 1) Detected(AddMode.Link, "${urls.size} links") else Detected(AddMode.Link, host(urls[0]))
    }
    val lines = text.split(Regex("\n+")).filter { it.isNotBlank() }
    // Shared text like "Best lasagna https://…": one link and a few words on one line
    if (urls.size == 1 && lines.size == 1) return Detected(AddMode.Link, host(urls[0]))
    if (lines.size >= 3) {
        val n = lines.count { IngredientRe.containsMatchIn(it) }
        return Detected(AddMode.Text, if (n > 0) plural(n, "ingredient") else "${lines.size} lines")
    }
    if (lines.size == 1 && text.length <= 80) return Detected(AddMode.Scratch, "New recipe")
    return Detected(AddMode.Text, plural(lines.size, "line"))
}

/** Whether Add can go ahead in [mode] (web `canAdd`, less the saving check). */
fun canAdd(mode: AddMode, input: String, photos: Int, photosReady: Boolean): Boolean = when (mode) {
    AddMode.Claude, AddMode.Apps, AddMode.File -> true
    AddMode.Photo -> photos == 0 || photosReady
    AddMode.Link -> linksIn(input).isNotEmpty()
    AddMode.Scratch -> input.isNotBlank()
    AddMode.Text, AddMode.Auto -> input.trim().length >= 10
}

/** The Add button's label. */
fun addLabel(mode: AddMode, input: String): String = when {
    mode == AddMode.Claude || mode == AddMode.Apps -> "Open"
    mode == AddMode.Link && linksIn(input).size > 1 -> "Import all"
    else -> "Add"
}
