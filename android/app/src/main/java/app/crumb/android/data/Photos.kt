package app.crumb.android.data

/** Recipe photo URLs, matching `web/src/lib/img.ts` and `src/images.rs`. */
object Photos {
    /** Widths the server makes. Must match `WIDTHS` in src/images.rs. */
    val WIDTHS = intArrayOf(160, 320, 480, 768, 1200)

    /** FNV-1a 32-bit over the image string's UTF-8 bytes, as 8 hex digits. */
    fun imageKey(image: String): String {
        var hash = 0x811c9dc5.toInt()
        for (byte in image.encodeToByteArray()) {
            hash = hash xor (byte.toInt() and 0xff)
            hash *= 0x01000193
        }
        return hash.toUInt().toString(16).padStart(8, '0')
    }

    /** The smallest server width at least [px] wide, or the largest. */
    fun snapWidth(px: Int): Int = WIDTHS.firstOrNull { it >= px } ?: WIDTHS.last()
}
