package app.crumb.android.ui.suggestions

import app.crumb.android.data.ChecksStatus
import app.crumb.android.data.CrumbApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

// The Suggestions page's wording and shared state (web/src/lib/checks.ts). The status line is
// crumb-core's `checksStatusText`, so every app says it the same way.

/**
 * One line on where "Check all" stands (web `checksStatusText`), e.g.
 * `"5 recipes to check (2 restored) · suggests fixes, tidies only stray symbols"`.
 * [run] is how many this Check all queued, so progress counts failures as done too.
 */
fun checksStatusText(status: ChecksStatus, run: Int? = null): String =
    app.crumb.core.checksStatusText(
        app.crumb.core.ChecksCounts(
            eligible = status.eligible.toUInt(),
            checked = status.checked.toUInt(),
            pending = status.pending.toUInt(),
            tidied = status.tidied.toUInt(),
            toCheck = status.toCheck.toUInt(),
            due = status.due.toUInt(),
            restored = status.restored.toUInt(),
            edited = status.edited.toUInt(),
        ),
        run?.takeIf { it >= 0 }?.toUInt(),
    )

/** How each counted field reads in a summary, singular then plural (web `NAMES`). */
private val FieldNames = mapOf(
    "ingredients" to ("ingredient" to "ingredients"),
    "instructions" to ("step" to "steps"),
    "notes" to ("note" to "notes"),
    "totalTime" to ("time" to "times"),
)

/** "2 steps · 1 ingredient" (web `summary`), the field with the most flags first. */
fun summary(fields: Map<String, Int>): String =
    fields.entries
        .sortedByDescending { it.value }
        .joinToString(" · ") { (field, n) ->
            val (one, many) = FieldNames[field] ?: ("thing" to "things")
            "$n ${if (n == 1) one else many}"
        }

/** How long to wait before the first Check all poll (web `watch`'s default). */
internal const val PollStartMs = 1_500L
private const val PollMaxMs = 8_000L
private const val PollMaxFailedMs = 10_000L

/**
 * The next poll delay (web `poll`): quick again at 1.5s whenever a recipe finishes, ×1.5 up
 * to 8s while nothing moves, and ×2 up to 10s after a failed request.
 */
fun nextPollDelay(currentMs: Long, moved: Boolean, failed: Boolean): Long = when {
    failed -> minOf(currentMs * 2, PollMaxFailedMs)
    moved -> PollStartMs
    else -> minOf((currentMs * 1.5).toLong(), PollMaxMs)
}

/** "Check all" progress as `(run - pending) / run`, 0..1 (web `progress`). */
fun checksProgress(run: Int, pending: Int): Float =
    if (run > 0) ((run - pending).toFloat() / run).coerceIn(0f, 1f) else 0f

/**
 * The count on the Review tab's badge (web `--review-count`), kept in step by the Suggestions
 * screen while it loads and polls, and by the app when it starts or comes back to the front.
 */
object ReviewCount {
    private val _count = MutableStateFlow(0)
    val count: StateFlow<Int> = _count.asStateFlow()

    /** What the badge shows: null when nothing waits, else the number, "99+" past 99. */
    fun label(n: Int = _count.value): String? = when {
        n <= 0 -> null
        n > 99 -> "99+"
        else -> n.toString()
    }

    fun set(n: Int) {
        _count.value = n.coerceAtLeast(0)
    }

    /** Re-read the list length; a failed request keeps the last known count. */
    suspend fun refresh(api: CrumbApi) {
        runCatching { api.reviewList().size }.getOrNull()?.let(::set)
    }
}
