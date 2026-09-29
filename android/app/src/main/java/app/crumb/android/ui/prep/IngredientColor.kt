package app.crumb.android.ui.prep

import androidx.compose.ui.graphics.Color

/** A plausible colour for what's in the bowl: crumb-core's `ingredientColor` (a hex string) as a colour. */
fun ingredientColor(name: String): Color =
    Color(0xFF000000L or app.crumb.core.ingredientColor(name).removePrefix("#").toLong(16))
