package app.crumb.android.ui.home

import app.crumb.core.detectAdd
import app.crumb.core.linksIn
import app.crumb.core.AddMode as CoreAddMode

// What the Add field holds and how to save it (web/src/islands/TopBox.svelte). What a paste is
// comes from crumb-core; the modes' labels and what Add does with each are this screen's.

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
    "cooking\nvideos too",
)

/** The mode Crumb worked out, and a short summary for the footer. */
data class Detected(val mode: AddMode, val summary: String)

/** What was pasted, worked out by crumb-core (`detectAdd`) so every app reads it the same. */
fun detect(raw: String): Detected {
    val d = detectAdd(raw)
    val mode = when (d.mode) {
        CoreAddMode.AUTO -> AddMode.Auto
        CoreAddMode.LINK -> AddMode.Link
        CoreAddMode.TEXT -> AddMode.Text
        CoreAddMode.SCRATCH -> AddMode.Scratch
    }
    return Detected(mode, d.summary)
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
