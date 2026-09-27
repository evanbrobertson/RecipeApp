package app.crumb.android.ui.importer

// The Import page's wording that isn't already shared Rust logic (web/src/islands/ImportTools.svelte).

/** "3 links found", or the empty hint when no links are typed yet. */
fun linksFoundText(count: Int): String = when (count) {
    0 -> "One per line, or any text with links in it"
    1 -> "1 link found"
    else -> "$count links found"
}

/** "2 added, 1 already saved, 3 skipped" (web's per-file result line). */
fun fileResultText(created: Int, duplicates: Int, skipped: Int): String {
    val parts = mutableListOf("$created added")
    if (duplicates > 0) parts += "$duplicates already saved"
    if (skipped > 0) parts += "$skipped skipped"
    return parts.joinToString(", ")
}

/** "photo.jpg" for one page, or "3 photos" for several read together. */
fun photoJobName(count: Int, firstName: String): String = if (count == 1) firstName else "$count photos"

/** The web's guard when more than six pages are chosen at once. */
fun tooManyPhotosText(max: Int): String = "Up to $max photos at a time, all pages of one recipe"
