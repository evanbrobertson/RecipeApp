package app.crumb.android.timers

import android.Manifest
import android.app.AlarmManager
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.media.AudioAttributes
import android.media.RingtoneManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import app.crumb.android.CrumbApplication
import app.crumb.android.MainActivity
import app.crumb.android.R
import app.crumb.android.data.CrumbJson
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer

/**
 * A kitchen timer, kept with its absolute end time (web/src/lib/timers.svelte.ts), so it
 * stays right across app restarts and reboots.
 */
@Serializable
data class KitchenTimer(
    val id: Long,
    val label: String,
    /** Seconds it was set for. */
    val total: Int,
    /** Epoch ms when it rings. */
    val endsAt: Long,
) {
    fun remainingSeconds(now: Long): Long = ((endsAt - now + 999) / 1000).coerceAtLeast(0)
    fun done(now: Long) = now >= endsAt
}

/** "M:SS", or "H:MM:SS" from an hour up (formatClock in timers.svelte.ts). */
fun formatClock(seconds: Long): String {
    val s = seconds.coerceAtLeast(0)
    val h = s / 3600
    val m = (s % 3600) / 60
    val sec = s % 60
    return if (h > 0) "%d:%02d:%02d".format(h, m, sec) else "%d:%02d".format(m, sec)
}

private val Context.timerData: DataStore<Preferences> by preferencesDataStore(name = "timers")

/**
 * Every running kitchen timer. Each one is an exact alarm, so it rings with the screen off
 * or the app closed, plus an ongoing notification counting down. Reboots reschedule them
 * ([TimerReceiver]). Dismissing a timer (running or done) removes it everywhere.
 */
class KitchenTimers(private val context: Context, private val scope: CoroutineScope) {
    private val list = MutableStateFlow<List<KitchenTimer>>(emptyList())
    val timers: StateFlow<List<KitchenTimer>> = list.asStateFlow()

    private val alarms = context.getSystemService(AlarmManager::class.java)
    private val notifications = NotificationManagerCompat.from(context)

    private var loaded = false

    /** Reads the saved timers once; later calls keep the (newer) in-memory list. */
    suspend fun load() {
        if (loaded) return
        loaded = true
        list.value = context.timerData.data.first()[KEY]?.let {
            runCatching { CrumbJson.decodeFromString(ListSerializer(KitchenTimer.serializer()), it) }.getOrNull()
        }.orEmpty()
    }

    fun start(label: String, seconds: Int): KitchenTimer {
        val now = System.currentTimeMillis()
        val timer = KitchenTimer(id = now, label = label, total = seconds, endsAt = now + seconds * 1000L)
        list.value = list.value + timer
        save()
        schedule(timer)
        showRunning(timer)
        return timer
    }

    fun remove(id: Long) {
        val timer = list.value.firstOrNull { it.id == id } ?: return
        list.value = list.value - timer
        save()
        alarms.cancel(alarmIntent(timer))
        notifications.cancel(notificationId(timer))
    }

    /** False when Android will only ring roughly on time (exact alarms not allowed). */
    fun canRingOnTime(): Boolean = Build.VERSION.SDK_INT < Build.VERSION_CODES.S || alarms.canScheduleExactAlarms()

    /** After a reboot alarms are gone: set them again, and ring any that ran out meanwhile. */
    fun restore() {
        val now = System.currentTimeMillis()
        list.value.forEach { timer ->
            if (timer.done(now)) ring(timer) else {
                schedule(timer)
                showRunning(timer)
            }
        }
    }

    internal fun ring(id: Long) {
        list.value.firstOrNull { it.id == id }?.let(::ring)
    }

    private fun ring(timer: KitchenTimer) {
        val dismiss = PendingIntent.getBroadcast(
            context, notificationId(timer),
            Intent(context, TimerReceiver::class.java).setAction(TimerReceiver.DISMISS).putExtra(TimerReceiver.ID, timer.id),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val notification = NotificationCompat.Builder(context, RINGING)
            .setSmallIcon(R.drawable.ic_timer)
            .setContentTitle("⏰ ${timer.label} is up!")
            .setContentText("Tap to open Crumb")
            .setCategory(NotificationCompat.CATEGORY_ALARM)
            .setPriority(NotificationCompat.PRIORITY_MAX)
            .setContentIntent(openApp(timer))
            .setDeleteIntent(dismiss)
            .addAction(0, "Dismiss", dismiss)
            .setAutoCancel(true)
            .setOnlyAlertOnce(false)
            .build()
        post(timer, notification)
    }

    private fun showRunning(timer: KitchenTimer) {
        val stop = PendingIntent.getBroadcast(
            context, notificationId(timer),
            Intent(context, TimerReceiver::class.java).setAction(TimerReceiver.DISMISS).putExtra(TimerReceiver.ID, timer.id),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val notification = NotificationCompat.Builder(context, RUNNING)
            .setSmallIcon(R.drawable.ic_timer)
            .setContentTitle(timer.label)
            .setWhen(timer.endsAt)
            .setUsesChronometer(true)
            .setChronometerCountDown(true)
            .setShowWhen(true)
            .setOngoing(true)
            .setSilent(true)
            .setCategory(NotificationCompat.CATEGORY_STOPWATCH)
            .setContentIntent(openApp(timer))
            .addAction(0, "Cancel", stop)
            .build()
        post(timer, notification)
    }

    private fun post(timer: KitchenTimer, notification: android.app.Notification) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) return
        notifications.notify(notificationId(timer), notification)
    }

    private fun schedule(timer: KitchenTimer) {
        val intent = alarmIntent(timer)
        if (canRingOnTime()) {
            // An alarm clock: exact, allowed in Doze, and shown to the system as a user alarm
            alarms.setAlarmClock(AlarmManager.AlarmClockInfo(timer.endsAt, openApp(timer)), intent)
        } else {
            alarms.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, timer.endsAt, intent)
        }
    }

    private fun alarmIntent(timer: KitchenTimer) = PendingIntent.getBroadcast(
        context, notificationId(timer),
        Intent(context, TimerReceiver::class.java).setAction(TimerReceiver.RING).putExtra(TimerReceiver.ID, timer.id),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    private fun openApp(timer: KitchenTimer) = PendingIntent.getActivity(
        context, notificationId(timer),
        Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    private fun notificationId(timer: KitchenTimer) = (timer.id % Int.MAX_VALUE).toInt()

    private fun save() {
        val json = CrumbJson.encodeToString(ListSerializer(KitchenTimer.serializer()), list.value)
        scope.launch { context.timerData.edit { it[KEY] = json } }
    }

    companion object {
        private val KEY = stringPreferencesKey("timers")
        const val RUNNING = "timers-running"
        const val RINGING = "timers-ringing"

        /** Two channels: a silent countdown, and a loud one that rings like an alarm. */
        fun createChannels(context: Context) {
            val manager = context.getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(
                NotificationChannel(RUNNING, "Running timers", NotificationManager.IMPORTANCE_LOW).apply {
                    description = "The countdown while a kitchen timer runs"
                    setShowBadge(false)
                },
            )
            manager.createNotificationChannel(
                NotificationChannel(RINGING, "Timer alarms", NotificationManager.IMPORTANCE_HIGH).apply {
                    description = "Rings when a kitchen timer is up"
                    setSound(
                        RingtoneManager.getDefaultUri(RingtoneManager.TYPE_ALARM),
                        AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_ALARM)
                            .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION).build(),
                    )
                    enableVibration(true)
                    vibrationPattern = longArrayOf(0, 200, 100, 200, 100, 400)
                },
            )
        }
    }
}

/** Rings a timer when its alarm fires, dismisses one from its notification, and restores after a reboot. */
class TimerReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val app = context.applicationContext as CrumbApplication
        val pending = goAsync()
        app.scope.launch {
            try {
                val timers = app.container.timers
                timers.load()
                when (intent.action) {
                    RING -> timers.ring(intent.getLongExtra(ID, 0))
                    DISMISS -> timers.remove(intent.getLongExtra(ID, 0))
                    Intent.ACTION_BOOT_COMPLETED, Intent.ACTION_MY_PACKAGE_REPLACED,
                    AlarmManager.ACTION_SCHEDULE_EXACT_ALARM_PERMISSION_STATE_CHANGED -> timers.restore()
                }
            } finally {
                pending.finish()
            }
        }
    }

    companion object {
        const val RING = "app.crumb.android.timer.RING"
        const val DISMISS = "app.crumb.android.timer.DISMISS"
        const val ID = "id"
    }
}
