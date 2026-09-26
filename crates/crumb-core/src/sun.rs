//! The "Sunrise & sunset" theme from `web/src/lib/theme-boot.js`: dark from today's sunset
//! to tomorrow's sunrise, at a saved location or one estimated from the time zone. Every
//! app flips at the same moment the web does.

use std::f64::consts::PI;
use std::sync::LazyLock;

use regex::Regex;

const DAY: f64 = 86_400_000.0;
const RAD: f64 = PI / 180.0;
const SIX_HOURS: f64 = 6.0 * 3_600_000.0;

/// Sunrise and sunset for a day, or the sun never setting or rising (polar day or night).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SunTimes {
    /// Unix milliseconds.
    RiseSet {
        rise: f64,
        set: f64,
    },
    PolarDay,
    PolarNight,
}

/// Sunrise and sunset (sun 0.833° below the horizon) for the day containing `now_ms`.
/// After suncalc (BSD-2, Vladimir Agafonkin).
pub fn sun_times(now_ms: f64, lat: f64, lng: f64) -> SunTimes {
    let e = RAD * 23.4397;
    let d = now_ms / DAY - 0.5 + 2_440_588.0 - 2_451_545.0;
    let lw = RAD * -lng;
    let phi = RAD * lat;
    // JavaScript's Math.round: halves go up
    let n = (d - 0.0009 - lw / (2.0 * PI) + 0.5).floor();
    let ds = 0.0009 + lw / (2.0 * PI) + n;
    let m = RAD * (357.5291 + 0.985_600_28 * ds);
    let c = RAD * (1.9148 * m.sin() + 0.02 * (2.0 * m).sin() + 0.0003 * (3.0 * m).sin());
    let l = m + c + RAD * 102.9372 + PI;
    let dec = (e.sin() * l.sin()).asin();
    let noon = 2_451_545.0 + ds + 0.0053 * m.sin() - 0.0069 * (2.0 * l).sin();
    let cos_w = ((RAD * -0.833).sin() - phi.sin() * dec.sin()) / (phi.cos() * dec.cos());
    if cos_w < -1.0 {
        return SunTimes::PolarDay;
    }
    if cos_w > 1.0 {
        return SunTimes::PolarNight;
    }
    let w = cos_w.acos();
    let a = 0.0009 + (w + lw) / (2.0 * PI) + n;
    let set = 2_451_545.0 + a + 0.0053 * m.sin() - 0.0069 * (2.0 * l).sin();
    let rise = noon - (set - noon);
    let ms = |j: f64| (j + 0.5 - 2_440_588.0) * DAY;
    SunTimes::RiseSet {
        rise: ms(rise),
        set: ms(set),
    }
}

/// Whether it's dark outside at a moment, and when that next changes (Unix ms).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SunState {
    pub dark: bool,
    pub next_change_ms: f64,
}

pub fn sun_state(now_ms: f64, lat: f64, lng: f64) -> SunState {
    match sun_times(now_ms, lat, lng) {
        SunTimes::PolarDay | SunTimes::PolarNight => SunState {
            dark: sun_times(now_ms, lat, lng) == SunTimes::PolarNight,
            next_change_ms: now_ms + SIX_HOURS,
        },
        SunTimes::RiseSet { rise, .. } if now_ms < rise => SunState {
            dark: true,
            next_change_ms: rise,
        },
        SunTimes::RiseSet { set, .. } if now_ms < set => SunState {
            dark: false,
            next_change_ms: set,
        },
        SunTimes::RiseSet { .. } => SunState {
            dark: true,
            next_change_ms: match sun_times(now_ms + DAY, lat, lng) {
                SunTimes::RiseSet { rise, .. } => rise,
                _ => now_ms + SIX_HOURS,
            },
        },
    }
}

static SOUTH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^(Australia|Antarctica|Pacific/(Auckland|Chatham|Fiji|Tongatapu|Noumea)|",
        r"America/(Santiago|Argentina|Sao_Paulo|Montevideo|Asuncion|La_Paz|Lima|Punta_Arenas)|",
        r"Africa/(Johannesburg|Maputo|Harare|Windhoek|Gaborone|Maseru|Mbabane|Lusaka|Blantyre|Lubumbashi)|",
        r"Indian/(Mauritius|Reunion|Antananarivo)|Atlantic/Stanley)"
    ))
    .unwrap()
});

/// A location good enough for sunrise and sunset, from the time zone alone: longitude from
/// its standard (non-DST) offset, `min(january, july)` seconds east of UTC, and latitude
/// from its region. Usually within half an hour of the real times.
pub fn estimate_location(zone_id: &str, standard_offset_east_secs: i32) -> (f64, f64) {
    let lat = if SOUTH.is_match(zone_id) {
        -34.0
    } else if zone_id.starts_with("Europe") {
        50.0
    } else if zone_id.starts_with("Asia") || zone_id.starts_with("Africa") {
        30.0
    } else {
        40.0
    };
    (lat, f64::from(standard_offset_east_secs) / 60.0 / 4.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-06-21T12:00:00Z
    const MIDSUMMER: f64 = 1_782_043_200_000.0;

    #[test]
    fn london_midsummer() {
        let SunTimes::RiseSet { rise, set } = sun_times(MIDSUMMER, 51.5, -0.13) else {
            panic!("London has a sunrise");
        };
        // About 03:43 and 20:21 UTC+1
        let hour = |ms: f64| (ms - (MIDSUMMER - 12.0 * 3_600_000.0)) / 3_600_000.0;
        assert!((hour(rise) - 3.73).abs() < 0.1, "rise {}", hour(rise));
        assert!((hour(set) - 20.35).abs() < 0.1, "set {}", hour(set));
        let noon = sun_state(MIDSUMMER, 51.5, -0.13);
        assert!(!noon.dark);
        assert_eq!(noon.next_change_ms, set);
        let late = sun_state(set + 60_000.0, 51.5, -0.13);
        assert!(late.dark);
        assert!(late.next_change_ms > set + 6.0 * 3_600_000.0);
        let early = sun_state(rise - 60_000.0, 51.5, -0.13);
        assert!(early.dark);
        assert_eq!(early.next_change_ms, rise);
    }

    #[test]
    fn polar_day_and_night() {
        assert_eq!(sun_times(MIDSUMMER, 80.0, 15.0), SunTimes::PolarDay);
        assert_eq!(sun_times(MIDSUMMER, -80.0, 15.0), SunTimes::PolarNight);
        let s = sun_state(MIDSUMMER, 80.0, 15.0);
        assert!(!s.dark);
        assert_eq!(s.next_change_ms, MIDSUMMER + SIX_HOURS);
        assert!(sun_state(MIDSUMMER, -80.0, 15.0).dark);
    }

    #[test]
    fn estimates_from_the_zone() {
        assert_eq!(estimate_location("Europe/London", 0), (50.0, 0.0));
        assert_eq!(
            estimate_location("America/New_York", -5 * 3600),
            (40.0, -75.0)
        );
        assert_eq!(
            estimate_location("Australia/Sydney", 10 * 3600),
            (-34.0, 150.0)
        );
        assert_eq!(estimate_location("Asia/Tokyo", 9 * 3600), (30.0, 135.0));
        assert_eq!(estimate_location("", 0), (40.0, 0.0));
    }
}
