package app.crumb.android.data

import android.content.ContentResolver
import android.net.Uri

/** What an import produced; the screen picks the toast and where to go. */
sealed interface ImportOutcome {
    /** One recipe; [onDevice] when the phone read photos itself (worth checking the amounts). */
    data class Saved(
        val id: Long,
        val title: String,
        val isNew: Boolean,
        val onDevice: Boolean = false,
        /** Wee Chef watched a cooking video for it. */
        val fromVideo: Boolean = false,
        /** The page's photo link was dead, so it was saved without one. */
        val droppedPhoto: Boolean = false,
    ) : ImportOutcome

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
    private val videoJobs: VideoJobs,
) {
    /** A link; [progress] gets "Queued (2nd)…"-style lines while a cooking video waits its turn. */
    suspend fun link(url: String, progress: (String) -> Unit = {}): ImportOutcome {
        val link = url.trim()
        return outcome(
            when (val started = api.startImport(link)) {
                is ImportStart.Done -> started.result
                // Kept watching after the screen is left, and after the app is
                is ImportStart.Queued -> videoJobs.track(link, started.job, progress)
            },
        )
    }

    suspend fun text(text: String): ImportOutcome = outcome(api.importText(text))

    suspend fun photos(pages: List<Uri>, note: String? = null, status: (String) -> Unit = {}): ImportOutcome.Saved {
        val r = photos.import(pages, note, status)
        return ImportOutcome.Saved(r.id, r.title, r.isNew, r.onDevice)
    }

    /** Whether Wee Chef reads photos here; when not, a note with them has no use. */
    suspend fun readsPhotos(): Boolean = photos.visionAvailable()

    suspend fun files(uris: List<Uri>): ImportOutcome.Files =
        ImportOutcome.Files(api.importFiles(uris.map { Incoming.upload(resolver, it) }))

    private fun outcome(r: ImportResult): ImportOutcome =
        r.cookbook?.let { ImportOutcome.Book(it) }
            ?: ImportOutcome.Saved(r.id, r.title, r.isNew, fromVideo = r.fromVideo, droppedPhoto = r.droppedPhoto)

    companion object {
        /** File types the server can't read, refused before uploading (TopBox's check). */
        fun unreadable(mimeType: String?, name: String?): Boolean =
            mimeType?.startsWith("video/") == true || mimeType?.startsWith("audio/") == true ||
                name?.let { Regex("\\.(docx?|pages|rtf)$", RegexOption.IGNORE_CASE).containsMatchIn(it) } == true
    }
}
