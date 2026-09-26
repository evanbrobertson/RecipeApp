package app.crumb.android.data

/**
 * Recipe times arrive as whatever the source had: ISO 8601 ("PT1H30M") from most sites,
 * or text someone typed ("about 40 minutes"). ISO is shown as "1 hr 30 min"; anything
 * else is shown as written.
 */
object Durations {
    private val ISO = Regex("""^P(?:(\d+)D)?(?:T(?:(\d+)H)?(?:(\d+)M)?(?:(\d+(?:\.\d+)?)S)?)?$""", RegexOption.IGNORE_CASE)

    fun display(raw: String?): String? {
        val text = raw?.trim()?.takeIf { it.isNotEmpty() } ?: return null
        val match = ISO.matchEntire(text) ?: return text
        val (d, h, m, s) = match.destructured
        var minutes = (d.toLongOrNull() ?: 0) * 24 * 60 +
            (h.toLongOrNull() ?: 0) * 60 +
            (m.toLongOrNull() ?: 0)
        if (minutes == 0L && (s.toDoubleOrNull() ?: 0.0) > 0) minutes = 1
        if (minutes == 0L) return null
        val hours = minutes / 60
        val rest = minutes % 60
        return when {
            hours == 0L -> "$rest min"
            rest == 0L -> "$hours hr"
            else -> "$hours hr $rest min"
        }
    }
}
