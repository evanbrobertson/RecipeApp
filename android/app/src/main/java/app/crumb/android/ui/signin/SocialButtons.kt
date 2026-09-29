package app.crumb.android.ui.signin

import androidx.compose.foundation.Image
import androidx.compose.foundation.border
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.crumb.android.data.Provider
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.NunitoSans

/**
 * A divider, then "Continue with Google / Apple" for each provider this Crumb has keys for, as
 * the web's SocialButtons.svelte has them: outline buttons, Google's four-colour G and Apple's logo.
 * Nothing at all without providers. [going] is the one whose browser sign-in is being started.
 */
@Composable
fun SocialButtons(providers: List<Provider>, going: Provider?, onGo: (Provider) -> Unit) {
    if (providers.isEmpty()) return
    val colors = Crumb.colors
    Row(Modifier.fillMaxWidth().padding(vertical = 2.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(Modifier.weight(1f).height(1.dp).background(colors.line))
        Text("or", color = colors.inkMuted, fontFamily = NunitoSans, fontSize = 14.sp)
        Box(Modifier.weight(1f).height(1.dp).background(colors.line))
    }
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        for (p in providers) {
            val label = if (going == p) "Opening ${p.label}…" else "Continue with ${p.label}"
            val enabled = going == null
            Row(
                Modifier
                    .fillMaxWidth()
                    .height(48.dp)
                    .clip(ControlShape)
                    .background(colors.paper)
                    .border(1.dp, colors.lineStrong, ControlShape)
                    .clickable(enabled = enabled, role = Role.Button) { onGo(p) }
                    .padding(horizontal = 20.dp)
                    .testTag("social-${p.id}"),
                horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Image(
                    rememberVectorPainter(remember(p) { if (p == Provider.Google) GoogleG else AppleLogo }),
                    contentDescription = null,
                    colorFilter = if (p == Provider.Apple) ColorFilter.tint(colors.ink) else null,
                    modifier = Modifier.size(20.dp).clearAndSetSemantics {},
                )
                Text(
                    label,
                    color = if (enabled || going == p) colors.ink else colors.inkMuted,
                    fontFamily = NunitoSans,
                    fontWeight = FontWeight.Bold,
                    fontSize = 16.sp,
                    maxLines = 1,
                )
            }
        }
    }
}

// The web's own paths (SocialButtons.svelte), so the marks match.
private val GoogleG: ImageVector = ImageVector.Builder("google", 20.dp, 20.dp, 48f, 48f).apply {
    addPath(
        addPathNodes("M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z"),
        fill = SolidColor(Color(0xFFEA4335)),
    )
    addPath(
        addPathNodes("M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z"),
        fill = SolidColor(Color(0xFF4285F4)),
    )
    addPath(
        addPathNodes("M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z"),
        fill = SolidColor(Color(0xFFFBBC05)),
    )
    addPath(
        addPathNodes("M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.15 1.45-4.92 2.3-8.16 2.3-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z"),
        fill = SolidColor(Color(0xFF34A853)),
    )
}.build()

/** Apple's mark is one solid shape, tinted to the ink at the button (the web's currentColor). */
private val AppleLogo: ImageVector = ImageVector.Builder("apple", 20.dp, 20.dp, 24f, 24f).apply {
    addPath(
        addPathNodes(
            "M12.152 6.896c-.948 0-2.415-1.078-3.96-1.04-2.04.027-3.91 1.183-4.961 3.014-2.117 3.675-.546 9.103 1.519 12.09 1.013 1.454 2.208 3.09 3.792 3.039 1.52-.065 2.09-.987 3.935-.987 1.831 0 2.35.987 3.96.948 1.637-.026 2.676-1.48 3.676-2.948 1.156-1.688 1.636-3.325 1.662-3.415-.039-.013-3.182-1.221-3.22-4.857-.026-3.04 2.48-4.494 2.597-4.559-1.429-2.09-3.623-2.324-4.39-2.376-2-.156-3.675 1.09-4.61 1.09zM15.53 3.83c.843-1.012 1.4-2.427 1.245-3.83-1.207.052-2.662.805-3.532 1.818-.78.896-1.454 2.338-1.273 3.714 1.338.104 2.715-.688 3.559-1.701",
        ),
        fill = SolidColor(Color.Black),
    )
}.build()
