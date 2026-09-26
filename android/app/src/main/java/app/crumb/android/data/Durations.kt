package app.crumb.android.data

import app.crumb.core.isoDurationMinutes
import kotlin.math.roundToLong

/**
 * Recipe times arrive as whatever the source had: ISO 8601 ("PT1H30M"), the server's own
 * "1h 30m", or text someone typed ("about 40 minutes"). ISO is parsed by crumb-core and shown
 * the way the server writes times ("1h 30m"); anything else is shown as written.
 */
object Durations {
    fun display(raw: String?): String? {
        val text = raw?.trim()?.takeIf { it.isNotEmpty() } ?: return null
        val exact = isoDurationMinutes(text) ?: return text
        if (exact <= 0.0) return null
        val minutes = exact.roundToLong().coerceAtLeast(1)
        val (h, m) = minutes / 60 to minutes % 60
        return when {
            h == 0L -> "${m}m"
            m == 0L -> "${h}h"
            else -> "${h}h ${m}m"
        }
    }
}
