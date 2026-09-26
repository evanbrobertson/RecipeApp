package app.crumb.android.data

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.ImageDecoder
import android.graphics.Paint
import android.net.Uri
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.text.TextRecognition
import com.google.mlkit.vision.text.latin.TextRecognizerOptions
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import java.io.ByteArrayOutputStream
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlin.math.max
import kotlin.math.roundToInt

/** A recipe saved from photos; [onDevice] when the phone read them, so amounts are worth a check. */
data class PhotoResult(val id: Long, val title: String, val isNew: Boolean, val onDevice: Boolean)

/**
 * Recipes from photos (web/src/lib/photos.ts). With Wee Chef reading photos on the server,
 * the pages go to `/api/recipes/import/photos`; without it, ML Kit reads them on the phone and
 * the text goes through the ordinary text import.
 */
class PhotoImport(private val context: Context, private val api: CrumbApi) {
    private var vision: Boolean? = null

    /**
     * Reads [pages] (in order) as one recipe and saves it. [status] gets short progress lines
     * for the UI. Throws with a readable message.
     */
    suspend fun import(pages: List<Uri>, note: String? = null, status: (String) -> Unit = {}): PhotoResult {
        require(pages.size in 1..MAX_PHOTOS) { "Up to $MAX_PHOTOS photos, all pages of one recipe." }
        val jpegs = withContext(Dispatchers.IO) { pages.map { shrink(it) } }
        val n = pages.size
        if (visionAvailable()) {
            status("Reading $n photo${if (n == 1) "" else "s"}…")
            try {
                val r = api.importPhotos(jpegs, note?.trim()?.takeIf { it.isNotEmpty() })
                return PhotoResult(r.id, r.title, r.isNew, onDevice = false)
            } catch (e: ApiException) {
                // Wee Chef was switched off since we asked: read them here instead
                if (!(e.status == 400 && "Wee Chef" in e.message)) throw e
                vision = false
            }
        }
        val text = readOnDevice(jpegs, status)
        if (text.replace(Regex("\\s"), "").length < 10) {
            throw IllegalStateException("Couldn't make out any writing. Try a closer, sharper photo in good light.")
        }
        status("Sorting out the recipe…")
        val r = api.importText(text)
        return PhotoResult(r.id, r.title, r.isNew, onDevice = true)
    }

    private suspend fun visionAvailable(): Boolean =
        vision ?: runCatching { api.connector().vision }.getOrDefault(false).also { vision = it }

    /**
     * An upright JPEG no bigger than 1568 px on its long edge, on white (for transparent
     * PNGs). Re-encoding also leaves the location and other EXIF behind.
     */
    private fun shrink(uri: Uri): ByteArray {
        val source = ImageDecoder.createSource(context.contentResolver, uri)
        val bitmap = ImageDecoder.decodeBitmap(source) { decoder, info, _ ->
            val scale = minOf(1f, LONG_EDGE.toFloat() / max(info.size.width, info.size.height))
            decoder.setTargetSize(
                max(1, (info.size.width * scale).roundToInt()),
                max(1, (info.size.height * scale).roundToInt()),
            )
            decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
        }
        val flat = Bitmap.createBitmap(bitmap.width, bitmap.height, Bitmap.Config.ARGB_8888)
        Canvas(flat).apply {
            drawColor(android.graphics.Color.WHITE)
            drawBitmap(bitmap, 0f, 0f, Paint(Paint.FILTER_BITMAP_FLAG))
        }
        bitmap.recycle()
        return ByteArrayOutputStream().use { out ->
            flat.compress(Bitmap.CompressFormat.JPEG, QUALITY, out)
            flat.recycle()
            out.toByteArray()
        }
    }

    /** OCR on the phone, one page after another, joined into one text. */
    private suspend fun readOnDevice(pages: List<ByteArray>, status: (String) -> Unit): String {
        status("Getting the reader ready…")
        val reader = TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS)
        try {
            val texts = pages.mapIndexed { i, jpeg ->
                status(if (pages.size > 1) "Reading page ${i + 1} of ${pages.size}…" else "Reading the photo…")
                val bitmap = android.graphics.BitmapFactory.decodeByteArray(jpeg, 0, jpeg.size)
                val result = suspendCancellableCoroutine { cont ->
                    reader.process(InputImage.fromBitmap(bitmap, 0))
                        .addOnSuccessListener { cont.resume(it.text) }
                        .addOnFailureListener { cont.resumeWithException(it) }
                }
                bitmap.recycle()
                result.trim()
            }
            return texts.filter { it.isNotEmpty() }.joinToString("\n\n")
        } finally {
            reader.close()
        }
    }

    companion object {
        const val MAX_PHOTOS = 6
        private const val LONG_EDGE = 1568
        private const val QUALITY = 85
    }
}
