package app.crumb.android.ui.preview

import app.crumb.android.data.ApiException
import app.crumb.android.data.OfflineException
import app.crumb.android.data.Preview
import app.crumb.android.data.SITE_TERMS
import app.crumb.core.hostOf

// The preview page's wording and decisions (web/src/islands/PreviewActions.svelte, src/preview.rs).

/** Where a preview stands. */
sealed interface PreviewState {
    /** "Reading the recipe…": the server is fetching the page. */
    data object Reading : PreviewState

    /** Read and tidied as an import would, not saved yet. */
    data class Ready(val recipe: app.crumb.android.data.Recipe) : PreviewState

    /** Already in the box: go straight to it. */
    data class Saved(val id: Long, val title: String) : PreviewState

    /** A cooking video or another Crumb's shared cookbook: nothing to read first, import it. */
    data object Import : PreviewState

    /** Couldn't be read. [code] says why when the server does (`site_terms`, `site_blocked`). */
    data class Failed(val message: String, val code: String? = null, val signedOut: Boolean = false) : PreviewState
}

fun stateOf(preview: Preview): PreviewState = when (preview) {
    is Preview.Saved -> PreviewState.Saved(preview.id, preview.title)
    Preview.Import -> PreviewState.Import
    is Preview.Ready -> PreviewState.Ready(preview.recipe)
}

fun failedState(error: Throwable, friendly: String): PreviewState.Failed =
    PreviewState.Failed(friendly, (error as? ApiException)?.code, (error as? ApiException)?.isSignedOut == true)

/** "From seriouseats.com", or "From the site" when the link has no host. */
fun fromHost(url: String?): String = "From ${hostOf(url) ?: "the site"}"

/** "Reading seriouseats.com…" for the waiting button and title. */
fun readingHost(url: String?): String = "Reading ${hostOf(url) ?: "the site"}…"

/** Trying again can't help a site whose terms forbid fetching it. */
fun canTryAgain(code: String?): Boolean = code != SITE_TERMS

/** Only a recipe read from the site can be added from its preview; an offline miss can be retried. */
fun isOffline(error: Throwable): Boolean = error is OfflineException
