package app.crumb.android.data

import app.crumb.core.displayDuration

/**
 * Recipe times arrive as whatever the source had: ISO 8601 ("PT1H30M"), the server's own
 * "1h 30m", or text someone typed ("about 40 minutes"). crumb-core shows ISO the way the
 * server writes times and anything else as written.
 */
object Durations {
    fun display(raw: String?): String? = displayDuration(raw)
}
