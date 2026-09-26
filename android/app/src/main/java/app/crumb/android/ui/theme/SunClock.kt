package app.crumb.android.ui.theme

import app.crumb.core.estimateLocation
import app.crumb.core.sunState
import java.time.ZoneId
import java.time.ZonedDateTime

/** Where the sun is worked out for: a saved location, or an estimate from the time zone. */
data class SunLocation(val lat: Double, val lng: Double)

/** Whether it's dark outside at [now] and when that next changes (epoch ms). */
data class SunState(val dark: Boolean, val next: Long)

/**
 * The "Sunrise & sunset" theme: dark from today's sunset to tomorrow's sunrise. The maths is
 * crumb-core's port of web/src/lib/theme-boot.js, so the phone flips when the web app does.
 */
object SunClock {
    fun sun(now: Long, loc: SunLocation): SunState {
        val state = sunState(now.toDouble(), app.crumb.core.SunLocation(loc.lat, loc.lng))
        return SunState(state.dark, state.nextChangeMs.toLong())
    }

    /** A location from the time zone alone; usually within half an hour of the real times. */
    fun estimate(zone: ZoneId = ZoneId.systemDefault(), year: Int = ZonedDateTime.now(zone).year): SunLocation {
        val jan = ZonedDateTime.of(year, 1, 1, 12, 0, 0, 0, zone).offset.totalSeconds
        val jul = ZonedDateTime.of(year, 7, 1, 12, 0, 0, 0, zone).offset.totalSeconds
        val estimate = estimateLocation(zone.id, minOf(jan, jul))
        return SunLocation(estimate.lat, estimate.lng)
    }
}
