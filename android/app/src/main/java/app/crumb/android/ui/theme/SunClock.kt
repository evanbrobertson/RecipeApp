package app.crumb.android.ui.theme

import app.crumb.core.estimateLocation
import app.crumb.core.sunState
import java.time.ZoneId
import java.time.ZonedDateTime

/** Where the sun is worked out for: a saved location, or an estimate from the time zone. */
data class SunLocation(val lat: Double, val lng: Double)

/** Whether it's dark outside at [now] and when that next changes (epoch ms). */
data class SunState(val dark: Boolean, val next: Long)

/** Sunrise and sunset for a day, or the sun never setting or rising (polar day or night). */
sealed interface SunTimes {
    /** Unix milliseconds. */
    data class RiseSet(val rise: Long, val set: Long) : SunTimes
    data object PolarDay : SunTimes
    data object PolarNight : SunTimes
}

/**
 * The "Sunrise & sunset" theme: dark from today's sunset to tomorrow's sunrise. The maths is
 * crumb-core's port of web/src/lib/theme-boot.js, so the phone flips when the web app does.
 */
object SunClock {
    fun sun(now: Long, loc: SunLocation): SunState {
        val state = sunState(now.toDouble(), app.crumb.core.SunLocation(loc.lat, loc.lng))
        return SunState(state.dark, state.nextChangeMs.toLong())
    }

    /** Sunrise and sunset for the day containing [now] (crumb-core's suncalc port). */
    fun sunTimes(now: Long, loc: SunLocation): SunTimes =
        when (val t = app.crumb.core.sunTimes(now.toDouble(), app.crumb.core.SunLocation(loc.lat, loc.lng))) {
            is app.crumb.core.SunTimes.RiseSet -> SunTimes.RiseSet(t.rise.toLong(), t.set.toLong())
            app.crumb.core.SunTimes.PolarDay -> SunTimes.PolarDay
            app.crumb.core.SunTimes.PolarNight -> SunTimes.PolarNight
        }

    /** A location from the time zone alone; usually within half an hour of the real times. */
    fun estimate(zone: ZoneId = ZoneId.systemDefault(), year: Int = ZonedDateTime.now(zone).year): SunLocation {
        val jan = ZonedDateTime.of(year, 1, 1, 12, 0, 0, 0, zone).offset.totalSeconds
        val jul = ZonedDateTime.of(year, 7, 1, 12, 0, 0, 0, zone).offset.totalSeconds
        val estimate = estimateLocation(zone.id, minOf(jan, jul))
        return SunLocation(estimate.lat, estimate.lng)
    }
}
