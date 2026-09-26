package app.crumb.android.data

/** What someone pasted or shared into Add: a link to a recipe, or the recipe itself. */
sealed interface ImportInput {
    data class Link(val url: String) : ImportInput
    data class Text(val text: String) : ImportInput

    companion object {
        private val URL = Regex("""https?://[^\s<>"]+""", RegexOption.IGNORE_CASE)

        /** Shares from browsers are often "Page title https://…"; that's still a link. */
        private const val SHARED_LINK_MAX = 500

        fun classify(raw: String): ImportInput? {
            val text = raw.trim()
            if (text.isEmpty()) return null
            val urls = URL.findAll(text).map { it.value.trimEnd('.', ',', ')', ']') }.toList()
            return if (urls.size == 1 && text.length <= SHARED_LINK_MAX) Link(urls.single()) else Text(text)
        }
    }
}
