package app.crumb.android.ui.recipe

import app.crumb.android.data.Recipe
import app.crumb.android.data.VideoEmbed
import app.crumb.android.data.VideoWin
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

// The recipe video's arithmetic and wording (web/src/components/RecipeVideo.svelte): which embed to
// play, the floating window's placement, and the page the WebView loads. VideoLogicTest pins them.

/** The recipe's embed: the server's, else core's read of the link (an offline cached copy may lack it). */
fun videoEmbedFor(recipe: Recipe): VideoEmbed? {
    recipe.videoEmbed?.let { return it }
    val link = recipe.video?.takeIf { it.isNotBlank() } ?: return null
    return app.crumb.core.videoEmbed(link)?.let {
        VideoEmbed(it.provider, it.label, it.embedUrl, it.watchUrl, it.thumbnail, it.vertical)
    }
}

/** The label on the "Watch on …" link: the site, or just the host for a file or a JW Player embed. */
fun watchLabel(embed: VideoEmbed?, host: String?): String =
    if (embed != null && embed.provider != "file" && embed.provider != "jwplayer") "Watch on ${embed.label}"
    else host ?: "Watch"

/** Sites that start playing when asked; the rest wait for a second tap (web `autoplay`). */
fun autoplayUrl(embed: VideoEmbed): String =
    if (embed.provider == "file" || embed.provider == "tiktok" || embed.provider == "instagram") embed.embedUrl
    else embed.embedUrl + (if ("?" in embed.embedUrl) "&" else "?") + "autoplay=1"

private fun escapeHtml(text: String) = text
    .replace("&", "&amp;").replace("\"", "&quot;").replace("<", "&lt;").replace(">", "&gt;")

/**
 * The page the WebView loads: one full-size iframe, as on the recipe page, or null when the
 * embed isn't an http(s) address (nothing else is ever loaded).
 */
fun playerHtml(embed: VideoEmbed): String? {
    val url = autoplayUrl(embed)
    if (!url.startsWith("https://") && !url.startsWith("http://")) return null
    return """<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1">""" +
        """<style>html,body{margin:0;height:100%;background:#000}iframe{border:0;width:100%;height:100%}</style></head><body>""" +
        """<iframe src="${escapeHtml(url)}" allow="autoplay; encrypted-media; picture-in-picture; fullscreen" """ +
        """allowfullscreen referrerpolicy="strict-origin-when-cross-origin"></iframe></body></html>"""
}

/** The screen the floating player lives on, in dp: its size and the system bars it stays clear of. */
data class VideoScreen(val width: Float, val height: Float, val top: Float = 0f, val bottom: Float = 0f)

/** Which part of the floating window a drag moves: the whole window, or a bottom corner's handle. */
enum class VideoDrag { Move, Left, Right }

/** The floating window: its bar (40dp) above a 16:9 or 9:16 picture, kept 8dp inside the screen. */
object VideoWindow {
    const val BAR = 40f
    const val EDGE = 8f

    /** Width at which the bar's "Back to video" button fits beside the grip. */
    private const val BACK_FITS = 216f

    /** Width at which "Back to video" gets its label as well as its arrow. */
    private const val BACK_LABEL = 300f

    /** Height over width of the picture. */
    fun ratio(tall: Boolean) = if (tall) 16f / 9f else 9f / 16f

    fun height(w: Float, tall: Boolean) = (w * ratio(tall)).roundToInt() + BAR

    private fun clamp(v: Float, lo: Float, hi: Float) = min(max(v, lo), max(lo, hi))

    /** [w] made a width the screen can hold: at least the minimum (260dp wide videos, 150dp tall) and no more than fits. */
    fun fitWidth(w: Float, tall: Boolean, s: VideoScreen): Float {
        val min = if (tall) 150f else 260f
        val most = min(s.width - 2 * EDGE, (s.height - s.top - s.bottom - 2 * EDGE - BAR) / ratio(tall))
        return clamp(w, min(min, most), most).roundToInt().toFloat()
    }

    /** [r] whole on screen, at a size the screen can hold. */
    fun fit(r: VideoWin, tall: Boolean, s: VideoScreen): VideoWin {
        val w = fitWidth(if (r.w.isFinite()) r.w else 0f, tall, s)
        val x = if (r.x.isFinite()) r.x else s.width
        val y = if (r.y.isFinite()) r.y else 0f
        return VideoWin(
            x = clamp(x, EDGE, s.width - w - EDGE).roundToInt().toFloat(),
            y = clamp(y, EDGE + s.top, s.height - s.bottom - height(w, tall) - EDGE).roundToInt().toFloat(),
            w = w,
        )
    }

    /** Where it first floats: the top of a phone (tall videos top right at 45% width), bottom right on a wide screen. */
    fun firstPlace(tall: Boolean, s: VideoScreen): VideoWin {
        val wide = s.width >= 768f
        val w = fitWidth(
            if (tall) (if (wide) 240f else s.width * 0.45f) else if (wide) max(448f, s.width * 0.36f) else s.width,
            tall,
            s,
        )
        return fit(VideoWin(s.width, if (wide) s.height - s.bottom - height(w, tall) - 80f else s.top + EDGE, w), tall, s)
    }

    /** Moves or resizes [from] by (dx, dy); a corner keeps the opposite bottom corner where it is. */
    fun apply(kind: VideoDrag, from: VideoWin, dx: Float, dy: Float, tall: Boolean, s: VideoScreen): VideoWin {
        if (kind == VideoDrag.Move) return fit(from.copy(x = from.x + dx, y = from.y + dy), tall, s)
        val w = fitWidth(if (kind == VideoDrag.Left) from.w - dx else from.w + dx, tall, s)
        val bottom = from.y + height(from.w, tall)
        return fit(
            VideoWin(if (kind == VideoDrag.Left) from.x + from.w - w else from.x, bottom - height(w, tall), w),
            tall,
            s,
        )
    }

    /** Four 40dp buttons and the grip need 200dp: below that "Back to video" gives way to the grip. */
    fun showBack(w: Float) = w >= BACK_FITS

    fun showBackLabel(w: Float) = w >= BACK_LABEL
}

/**
 * Whether the slot is scrolled past, above the screen: under a quarter of it still visible (the web's
 * observer). A short page can't scroll far enough for that, so once the list is [atEnd] any slot
 * that has started to leave the top counts too; the web's page always has room to scroll on.
 */
fun slotAway(top: Float, height: Float, viewport: Float, atEnd: Boolean = false): Boolean {
    if (top >= 0f || height <= 0f) return false
    if (atEnd) return true
    val visible = min(top + height, viewport) - max(top, 0f)
    return visible / height < 0.25f
}
