package app.crumb.android.ui.components

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.NunitoSans
import kotlinx.coroutines.delay

enum class ToastTone { Default, Success, Error, Primary }

/** A button on a toast, e.g. Undo. */
data class ToastAction(val label: String, val onSelect: () -> Unit)

/** web/src/lib/toast.ts: errors and toasts with an action stay 8s, the rest 4s. */
data class Toast(
    val title: String,
    val description: String? = null,
    val tone: ToastTone = ToastTone.Default,
    val action: ToastAction? = null,
    val durationMs: Long = if (tone == ToastTone.Error || action != null) 8000 else 4000,
)

/**
 * The app's toasts, shown at the top of the screen like the web's #toaster. There's no page
 * load between screens here, so the web's `flash()` (toast after navigating) is just [show].
 */
object Toaster {
    internal val toasts = mutableStateListOf<Pair<Long, Toast>>()
    private var next = 0L

    fun show(toast: Toast) {
        toasts.add(next++ to toast)
    }

    fun show(title: String, description: String? = null, tone: ToastTone = ToastTone.Default, action: ToastAction? = null) =
        show(Toast(title, description, tone, action))

    internal fun dismiss(id: Long) {
        toasts.removeAll { it.first == id }
    }
}

private val Settle = CubicBezierEasing(0.2f, 0.8f, 0.2f, 1f)

/** Put once over the whole app. */
@Composable
fun ToastHost(modifier: Modifier = Modifier) {
    Box(modifier.fillMaxSize().windowInsetsPadding(WindowInsets.statusBars), contentAlignment = Alignment.TopCenter) {
        Column(
            Modifier.padding(top = 12.dp, start = 16.dp, end = 16.dp).widthIn(max = 384.dp).fillMaxWidth(),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Toaster.toasts.forEach { (id, toast) -> ToastCard(id, toast) }
        }
    }
}

@Composable
private fun ToastCard(id: Long, toast: Toast) {
    val c = Crumb.colors
    val visible = remember(id) { MutableTransitionState(false).apply { targetState = true } }
    LaunchedEffect(id) {
        delay(toast.durationMs)
        visible.targetState = false
    }
    LaunchedEffect(visible.isIdle, visible.currentState) {
        if (visible.isIdle && !visible.currentState) Toaster.dismiss(id)
    }
    val tone = when (toast.tone) {
        // --success is the primary green in both themes
        ToastTone.Success -> c.primary
        ToastTone.Error -> c.error
        ToastTone.Primary -> c.tile
        ToastTone.Default -> null
    }
    AnimatedVisibility(
        visible,
        enter = fadeIn(tween(250, easing = Settle)) + slideInVertically(tween(250, easing = Settle)) { -it / 3 },
        exit = fadeOut(tween(200)) + slideOutVertically(tween(200)) { -6 },
    ) {
        Row(
            Modifier
                .fillMaxWidth()
                .shadow(20.dp, CardShape, ambientColor = c.ink, spotColor = c.ink)
                .clip(CardShape)
                .background(c.paper)
                .border(1.dp, c.line, CardShape)
                .clickable { visible.targetState = false }
                .semantics { liveRegion = LiveRegionMode.Polite }
                .then(
                    if (tone != null) {
                        // Toned toasts get a pill-shaped bar inside the left edge
                        Modifier.drawBehind {
                            drawRoundRect(
                                tone,
                                topLeft = Offset(10.dp.toPx(), 12.dp.toPx()),
                                size = Size(4.dp.toPx(), size.height - 24.dp.toPx()),
                                cornerRadius = CornerRadius(2.dp.toPx()),
                            )
                        }
                    } else Modifier,
                )
                .padding(start = if (tone != null) 24.dp else 16.dp, end = if (toast.action != null) 8.dp else 16.dp, top = 12.dp, bottom = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Column(Modifier.weight(1f)) {
                Text(toast.title, color = c.ink, fontFamily = NunitoSans, fontSize = 15.sp, fontWeight = FontWeight.Bold)
                toast.description?.let {
                    Text(it, color = c.inkMuted, fontFamily = NunitoSans, fontSize = 14.sp, modifier = Modifier.padding(top = 2.dp))
                }
            }
            toast.action?.let { action ->
                Box(
                    Modifier
                        .heightIn(min = 44.dp)
                        .clip(ControlShape)
                        .clickable {
                            visible.targetState = false
                            action.onSelect()
                        }
                        .padding(horizontal = 14.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(action.label, color = c.primary, fontFamily = NunitoSans, fontSize = 15.sp, fontWeight = FontWeight.Bold)
                }
            }
        }
    }
}
