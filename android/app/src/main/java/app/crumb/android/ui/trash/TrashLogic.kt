package app.crumb.android.ui.trash

import app.crumb.android.data.Restored
import app.crumb.android.data.Trashed
import java.time.Instant

// The trash's wording, from crumb-core's `trash` module (the web's TrashPage.svelte and the
// delete confirmations in RecipePage.svelte and RecipesPage.svelte). TrashLogicTest pins them.

/** "12 days left", counted to the day it goes (crumb-core); null when [purgeAt] isn't a date. */
fun daysLeftLabel(purgeAt: String, nowMs: Long): String? =
    runCatching { Instant.parse(purgeAt).toEpochMilli() }.getOrNull()
        ?.let { app.crumb.core.trashDaysLeft(it, nowMs) }

/** The "Empty the trash?" body. */
fun emptyTrashDescription(count: Int): String = app.crumb.core.trashEmpty(count.toUInt())

/** The single delete's confirmation body. */
val DELETE_ONE_DESCRIPTION: String get() = app.crumb.core.trashDeleteOne()

/** The bulk delete's confirmation body. */
fun deleteManyDescription(count: Int): String = app.crumb.core.trashDeleteMany(count.toUInt())

/** The toast after deleting: "Moved 3 recipes to the trash". */
fun movedToTrashTitle(count: Int): String = app.crumb.core.trashMoved(count.toUInt())

/** The toast after Undo: "Put back" or "Put back 3 recipes". */
fun putBackTitle(count: Int): String = app.crumb.core.trashPutBack(count.toUInt())

/** What a restore from the trash page says: the recipe is back, or its link was saved again since. */
fun restoredToast(restored: Restored, title: String): Pair<String, String> =
    if (restored.isNew) "Put back" to title else "It's already in your box" to "Its link was saved again since"

/** The trash list without [id]. */
fun List<Trashed>.without(id: Long): List<Trashed> = filter { it.id != id }
