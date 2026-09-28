package app.crumb.android.data

import android.content.ContentResolver
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns

/**
 * Something handed to Crumb from another app (Share → Crumb) or picked in the app: text or a
 * link, photos (the pages of one recipe), or files (PDF, saved web page, text, backups,
 * Paprika, Mealie, .zip). The Add screen decides what to do with it, as the web's TopBox does.
 */
data class Incoming(
    val text: String? = null,
    val photos: List<Uri> = emptyList(),
    val files: List<Uri> = emptyList(),
) {
    val isEmpty get() = text.isNullOrBlank() && photos.isEmpty() && files.isEmpty()

    companion object {
        /** SEND / SEND_MULTIPLE from another app, or null if there's nothing Crumb can use. */
        fun from(intent: Intent?, resolver: ContentResolver): Incoming? {
            if (intent == null) return null
            val uris = when (intent.action) {
                Intent.ACTION_SEND -> listOfNotNull(intent.streamUri())
                Intent.ACTION_SEND_MULTIPLE -> intent.streamUris()
                else -> return null
            }
            val text = intent.getStringExtra(Intent.EXTRA_TEXT)?.takeIf { it.isNotBlank() }?.let { text ->
                // Some apps put the page title in the subject and only the link in the text
                val subject = intent.getStringExtra(Intent.EXTRA_SUBJECT)
                if (subject != null && subject !in text && !text.trim().startsWith("http")) "$subject\n\n$text" else text
            }
            val (photos, files) = uris.partition { isPhoto(resolver.getType(it), displayName(resolver, it)) }
            return Incoming(text.takeIf { uris.isEmpty() }, photos, files).takeUnless { it.isEmpty }
        }

        fun isPhoto(mimeType: String?, name: String?): Boolean =
            mimeType?.startsWith("image/") == true ||
                name?.let { Regex("\\.(jpe?g|png|webp|gif|heic|heif|avif)$", RegexOption.IGNORE_CASE).containsMatchIn(it) } == true

        fun displayName(resolver: ContentResolver, uri: Uri): String? =
            runCatching {
                resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
                    if (c.moveToFirst()) c.getString(0) else null
                }
            }.getOrNull() ?: uri.lastPathSegment

        /** A picked or shared file as an upload for `/api/import/files`. */
        fun upload(resolver: ContentResolver, uri: Uri): UploadFile {
            val name = displayName(resolver, uri) ?: "recipe"
            val bytes = resolver.openInputStream(uri)?.use { it.readBytes() } ?: error("Couldn't open $name")
            return UploadFile(name, resolver.getType(uri) ?: "application/octet-stream", bytes)
        }

        @Suppress("DEPRECATION")
        private fun Intent.streamUri(): Uri? =
            if (android.os.Build.VERSION.SDK_INT >= 33) getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
            else getParcelableExtra(Intent.EXTRA_STREAM)

        @Suppress("DEPRECATION")
        private fun Intent.streamUris(): List<Uri> =
            (if (android.os.Build.VERSION.SDK_INT >= 33) getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri::class.java)
            else getParcelableArrayListExtra(Intent.EXTRA_STREAM)).orEmpty()
    }
}
