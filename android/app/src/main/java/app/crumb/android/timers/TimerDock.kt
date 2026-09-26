package app.crumb.android.timers

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import androidx.core.content.ContextCompat
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.components.Toast
import app.crumb.android.ui.components.ToastAction
import app.crumb.android.ui.components.ToastTone
import app.crumb.android.ui.components.Toaster
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.NunitoSans
import com.composables.icons.lucide.AlarmClock
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.X
import kotlinx.coroutines.delay

/**
 * The running timers as pills (web/src/islands/TimerDock.svelte): a countdown and label while
 * running, a solid tile "Done!" with a pulsing ring once up. Tap a pill to dismiss it. Shown
 * over every screen; while the app is open a finished timer also gets the web's toast.
 */
@Composable
fun TimerDock(modifier: Modifier = Modifier) {
    val timers = AppContainerProvider.timers
    val list by timers.timers.collectAsStateWithLifecycle()
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    val announced = remember { mutableSetOf<Long>() }

    LaunchedEffect(list.isNotEmpty()) {
        while (list.isNotEmpty()) {
            now = System.currentTimeMillis()
            delay(500)
        }
    }
    LaunchedEffect(now, list) {
        list.filter { it.done(now) && it.id !in announced }.forEach { t ->
            announced += t.id
            // Only fresh ones: a timer that ran out while the app was away just shows as done
            if (now - t.endsAt < 5000) Toaster.show(Toast("⏰ ${t.label} is up!", tone = ToastTone.Primary, durationMs = 15_000))
        }
    }

    FlowRow(
        modifier.fillMaxWidth().padding(horizontal = 16.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        list.forEach { t ->
            key(t.id) {
                val shown = remember { MutableTransitionState(false).apply { targetState = true } }
                AnimatedVisibility(shown, enter = scaleIn(tween(150), initialScale = 0.75f), exit = scaleOut(tween(150)) + fadeOut(tween(150))) {
                    TimerPill(t, now) { timers.remove(t.id) }
                }
            }
        }
    }
}

@Composable
private fun TimerPill(t: KitchenTimer, now: Long, onDismiss: () -> Unit) {
    val c = Crumb.colors
    val done = t.done(now)
    val pill = RoundedCornerShape(50)
    Box(contentAlignment = Alignment.Center) {
        if (done) {
            // A ring that keeps calling; the pill itself stays solid so its text stays readable
            val ping = rememberInfiniteTransition(label = "ping")
            val grow by ping.animateFloat(1f, 1.35f, infiniteRepeatable(tween(1000), RepeatMode.Restart), label = "grow")
            Box(
                Modifier
                    .matchParentSize()
                    .graphicsLayer {
                        scaleX = grow
                        scaleY = grow
                        alpha = 0.4f * (1.35f - grow) / 0.35f
                    }
                    .clip(pill)
                    .background(c.tile),
            )
        }
        Row(
            Modifier
                .height(44.dp)
                .shadow(if (done) 12.dp else 10.dp, pill, ambientColor = c.ink, spotColor = c.ink)
                .clip(pill)
                .background(if (done) c.tile else c.paper)
                .then(if (done) Modifier else Modifier.border(1.dp, c.line, pill))
                .clickable(onClick = onDismiss)
                .semantics { contentDescription = "Timer ${t.label}. Tap to dismiss" }
                .padding(start = 14.dp, end = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            val ink = if (done) c.onTile else c.ink
            val muted = if (done) c.onTile else c.inkMuted
            Icon(Lucide.AlarmClock, null, tint = if (done) c.onTile else c.primary, modifier = Modifier.size(18.dp))
            Text(
                if (done) "Done!" else formatClock(t.remainingSeconds(now)),
                color = ink, fontFamily = NunitoSans, fontSize = 15.sp, fontWeight = FontWeight.Bold,
                style = androidx.compose.ui.text.TextStyle(fontFeatureSettings = "tnum"),
            )
            Text(
                t.label, color = muted, fontFamily = NunitoSans, fontSize = 13.sp, fontWeight = FontWeight.SemiBold,
                maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.widthIn(max = 128.dp),
            )
            Icon(Lucide.X, null, tint = muted, modifier = Modifier.size(16.dp))
        }
    }
}

/** Once per run of the app is enough to mention late alarms. */
private var nudgedAboutAlarms = false

/**
 * Starts a kitchen timer from a tap (cook mode's "Start 10 minutes timer"). The first time,
 * asks to post notifications (so it can ring with the app closed), and if Android won't
 * allow exact alarms, offers the setting that lets it ring on time.
 */
@Composable
fun rememberTimerStarter(): (label: String, seconds: Int) -> Unit {
    val context = LocalContext.current
    val timers = AppContainerProvider.timers
    val askNotifications = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {}
    return remember(timers) {
        { label, seconds ->
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
                ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
            ) {
                askNotifications.launch(Manifest.permission.POST_NOTIFICATIONS)
            }
            timers.start(label, seconds)
            if (!timers.canRingOnTime() && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S && !nudgedAboutAlarms) {
                nudgedAboutAlarms = true
                Toaster.show(
                    "Timers may ring a little late",
                    "Let Crumb use alarms so they ring right on time.",
                    ToastTone.Default,
                    ToastAction("Allow") {
                        context.startActivity(
                            Intent(Settings.ACTION_REQUEST_SCHEDULE_EXACT_ALARM, Uri.parse("package:${context.packageName}"))
                                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                        )
                    },
                )
            }
        }
    }
}
