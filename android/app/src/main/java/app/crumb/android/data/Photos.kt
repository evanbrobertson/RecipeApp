package app.crumb.android.data

/** Recipe photo URLs, matching `web/src/lib/img.ts` and `src/images.rs` through crumb-core. */
object Photos {
    /** FNV-1a 32-bit over the image string's UTF-8 bytes, as 8 hex digits. */
    fun imageKey(image: String): String = app.crumb.core.imageKey(image)

    /** The smallest server width at least [px] wide, or the largest. */
    fun snapWidth(px: Int): Int = app.crumb.core.snapWidth(px.coerceAtLeast(0).toUInt()).toInt()
}
