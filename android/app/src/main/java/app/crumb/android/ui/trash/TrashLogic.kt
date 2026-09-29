package app.crumb.android.ui.trash

import app.crumb.android.data.Restored
import app.crumb.android.data.Trashed
import java.time.Instant
import kotlin.math.ceil

// The trash's wording and small sums, from web/src/islands/TrashPage.svelte and the delete
// confirmations in RecipePage.svelte and RecipesPage.svelte. TrashLogicTest pins them.

private const val DAY_MS = 86_400_000.0

/**
 * "12 days left", counted to the day it goes; null when [purgeAt] isn't a date.
 * TODO(core): trash_left_label (the web computes this inline in TrashPage.svelte)
 */
fun daysLeftLabel(purgeAt: String, nowMs: Long): String? {
    val at = runCatching { Instant.parse(purgeAt).toEpochMilli() }.getOrNull() ?: return null
    val days = maxOf(0, ceil((at - nowMs) / DAY_MS).toInt())
    return if (days <= 1) "Goes for good today" else "$days days left"
}

/** The "Empty the trash?" body. */
fun emptyTrashDescription(count: Int): String =
    "$count recipe${if (count == 1) "" else "s"} will be deleted for good. This can't be undone."

/** The single delete's confirmation body. */
const val DELETE_ONE_DESCRIPTION = "It goes to the trash, where you can put it back for 30 days."

/** The bulk delete's confirmation body. */
fun deleteManyDescription(count: Int): String =
    "$count recipe${if (count == 1) "" else "s"} will go to the trash, where you can put ${if (count == 1) "it" else "them"} back for 30 days."

/** The toast after deleting: "Moved 3 recipes to the trash". */
fun movedToTrashTitle(count: Int): String = "Moved $count recipe${if (count == 1) "" else "s"} to the trash"

/** The toast after Undo: "Put back" or "Put back 3 recipes". */
fun putBackTitle(count: Int): String = if (count == 1) "Put back" else "Put back $count recipes"

/** What a restore from the trash page says: the recipe is back, or its link was saved again since. */
fun restoredToast(restored: Restored, title: String): Pair<String, String> =
    if (restored.isNew) "Put back" to title else "It's already in your box" to "Its link was saved again since"

/** The trash list without [id]. */
fun List<Trashed>.without(id: Long): List<Trashed> = filter { it.id != id }
