package app.crumb.android.ui.theme

import java.time.ZoneId
import java.time.ZonedDateTime
import kotlin.math.PI
import kotlin.math.acos
import kotlin.math.asin
import kotlin.math.cos
import kotlin.math.roundToLong
import kotlin.math.sin

/** Where the sun is worked out for: a saved location, or an estimate from the time zone. */
data class SunLocation(val lat: Double, val lng: Double)

/** Whether it's dark outside at [now] and when that next changes (epoch ms). */
data class SunState(val dark: Boolean, val next: Long)

/**
 * The "Sunrise & sunset" theme, ported from web/src/lib/theme-boot.js so the phone flips at
 * the same moment the web app does: dark from today's sunset to tomorrow's sunrise.
 */
object SunClock {
    private const val DAY = 86_400_000L
    private const val RAD = PI / 180

    /**
     * Sunrise and sunset (sun 0.833° below the horizon) for the day containing [now], in ms.
     * After suncalc (BSD-2, Vladimir Agafonkin). Null in polar day or night.
     */
    fun sunTimes(now: Long, lat: Double, lng: Double): Pair<Long, Long>? = times(now, lat, lng).first

    private fun times(now: Long, lat: Double, lng: Double): Pair<Pair<Long, Long>?, Boolean> {
        val e = RAD * 23.4397
        val d = now.toDouble() / DAY - 0.5 + 2440588 - 2451545
        val lw = RAD * -lng
        val phi = RAD * lat
        val n = Math.round(d - 0.0009 - lw / (2 * PI)).toDouble()
        val ds = 0.0009 + lw / (2 * PI) + n
        val m = RAD * (357.5291 + 0.98560028 * ds)
        val c = RAD * (1.9148 * sin(m) + 0.02 * sin(2 * m) + 0.0003 * sin(3 * m))
        val l = m + c + RAD * 102.9372 + PI
        val dec = asin(sin(e) * sin(l))
        val noon = 2451545 + ds + 0.0053 * sin(m) - 0.0069 * sin(2 * l)
        val cosW = (sin(RAD * -0.833) - sin(phi) * sin(dec)) / (cos(phi) * cos(dec))
        if (cosW < -1) return null to true
        if (cosW > 1) return null to false
        val w = acos(cosW)
        val a = 0.0009 + (w + lw) / (2 * PI) + n
        val set = 2451545 + a + 0.0053 * sin(m) - 0.0069 * sin(2 * l)
        val rise = noon - (set - noon)
        fun ms(j: Double) = ((j + 0.5 - 2440588) * DAY).roundToLong()
        return (ms(rise) to ms(set)) to false
    }

    fun sun(now: Long, loc: SunLocation): SunState {
        val (t, polarDay) = times(now, loc.lat, loc.lng)
        if (t == null) return SunState(dark = !polarDay, next = now + 6 * 3_600_000)
        val (rise, set) = t
        return when {
            now < rise -> SunState(true, rise)
            now < set -> SunState(false, set)
            else -> SunState(true, sunTimes(now + DAY, loc.lat, loc.lng)?.first ?: (now + 6 * 3_600_000))
        }
    }

    private val SOUTH = Regex(
        "^(Australia|Antarctica|Pacific/(Auckland|Chatham|Fiji|Tongatapu|Noumea)|" +
            "America/(Santiago|Argentina|Sao_Paulo|Montevideo|Asuncion|La_Paz|Lima|Punta_Arenas)|" +
            "Africa/(Johannesburg|Maputo|Harare|Windhoek|Gaborone|Maseru|Mbabane|Lusaka|Blantyre|Lubumbashi)|" +
            "Indian/(Mauritius|Reunion|Antananarivo)|Atlantic/Stanley)",
    )

    /**
     * Longitude from the zone's standard (non-DST) UTC offset, latitude from its region.
     * Usually within half an hour of the real times.
     */
    fun estimate(zone: ZoneId = ZoneId.systemDefault(), year: Int = ZonedDateTime.now(zone).year): SunLocation {
        val jan = ZonedDateTime.of(year, 1, 1, 12, 0, 0, 0, zone).offset.totalSeconds
        val jul = ZonedDateTime.of(year, 7, 1, 12, 0, 0, 0, zone).offset.totalSeconds
        val standardEastMinutes = minOf(jan, jul) / 60
        val id = zone.id
        val lat = when {
            SOUTH.containsMatchIn(id) -> -34.0
            id.startsWith("Europe") -> 50.0
            id.startsWith("Asia") || id.startsWith("Africa") -> 30.0
            else -> 40.0
        }
        return SunLocation(lat, standardEastMinutes / 4.0)
    }
}
