package app.crumb.android.data

import android.content.ContentResolver
import android.net.Uri

/** What an import produced; the screen picks the toast and where to go. */
sealed interface ImportOutcome {
    /** One recipe; [onDevice] when the phone read photos itself (worth checking the amounts). */
    data class Saved(val id: Long, val title: String, val isNew: Boolean, val onDevice: Boolean = false) : ImportOutcome

    /** A shared cookbook link from another Crumb. */
    data class Book(val cookbook: ImportedCookbook) : ImportOutcome

    /** Files through the importer: what each one added. */
    data class Files(val results: List<ImportSummary>) : ImportOutcome {
        val created get() = results.flatMap { it.created }
    }
}

/**
 * The web's TopBox/ImportTools saving paths in one place: a link, pasted text, photos of one
 * recipe, or files. Several links go through the Import screen instead (one at a time).
 */
class Importer(
    private val api: CrumbApi,
    private val photos: PhotoImport,
    private val resolver: ContentResolver,
) {
    suspend fun link(url: String): ImportOutcome = outcome(api.importUrl(url.trim()))

    suspend fun text(text: String): ImportOutcome = outcome(api.importText(text))

    suspend fun photos(pages: List<Uri>, note: String? = null, status: (String) -> Unit = {}): ImportOutcome.Saved {
        val r = photos.import(pages, note, status)
        return ImportOutcome.Saved(r.id, r.title, r.isNew, r.onDevice)
    }

    suspend fun files(uris: List<Uri>): ImportOutcome.Files =
        ImportOutcome.Files(api.importFiles(uris.map { Incoming.upload(resolver, it) }))

    private fun outcome(r: ImportResult): ImportOutcome =
        r.cookbook?.let { ImportOutcome.Book(it) } ?: ImportOutcome.Saved(r.id, r.title, r.isNew)

    companion object {
        private val URL = Regex("""https?://\S+""", RegexOption.IGNORE_CASE)

        /** Every distinct link in some text, in order (ImportTools' link scan). */
        fun links(text: String): List<String> = URL.findAll(text).map { it.value.trimEnd('.', ',', ')', ']') }.distinct().toList()

        /** File types the server can't read, refused before uploading (TopBox's check). */
        fun unreadable(mimeType: String?, name: String?): Boolean =
            mimeType?.startsWith("video/") == true || mimeType?.startsWith("audio/") == true ||
                name?.let { Regex("\\.(docx?|pages|rtf)$", RegexOption.IGNORE_CASE).containsMatchIn(it) } == true
    }
}
