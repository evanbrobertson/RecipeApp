// Inlined into every page's <head> (see Layout.astro), so the right theme is on the first
// paint. It also exposes `window.crumbTheme` for the settings page. Keep it small and
// dependency-free: it runs before anything else.
//
// Modes: "light", "dark", "system" (prefers-color-scheme) and "sun" (dark between today's
// sunset and tomorrow's sunrise). "sun" uses a saved location if the cook allowed one,
// otherwise an estimate from the time zone: longitude from the standard UTC offset,
// latitude from the zone's region. That's usually within half an hour.
;(() => {
  const KEY = "crumb:theme"
  const LOC = "crumb:sun-loc"
  const root = document.documentElement
  const media = matchMedia("(prefers-color-scheme: dark)")
  const read = (k) => {
    try {
      return localStorage.getItem(k)
    } catch {
      return null
    }
  }
  let timer = 0

  const rad = Math.PI / 180
  const DAY = 86400000
  // Sunrise and sunset (sun 0.833° below the horizon) for the day containing `now`.
  // After suncalc (BSD-2, Vladimir Agafonkin). Returns [rise, set] in ms, or a boolean
  // for polar day (true) / polar night (false).
  const sunTimes = (now, lat, lng) => {
    const e = rad * 23.4397
    const d = now / DAY - 0.5 + 2440588 - 2451545
    const lw = rad * -lng
    const phi = rad * lat
    const n = Math.round(d - 0.0009 - lw / (2 * Math.PI))
    const ds = 0.0009 + lw / (2 * Math.PI) + n
    const M = rad * (357.5291 + 0.98560028 * ds)
    const C = rad * (1.9148 * Math.sin(M) + 0.02 * Math.sin(2 * M) + 0.0003 * Math.sin(3 * M))
    const L = M + C + rad * 102.9372 + Math.PI
    const dec = Math.asin(Math.sin(e) * Math.sin(L))
    const noon = 2451545 + ds + 0.0053 * Math.sin(M) - 0.0069 * Math.sin(2 * L)
    const cosW =
      (Math.sin(rad * -0.833) - Math.sin(phi) * Math.sin(dec)) / (Math.cos(phi) * Math.cos(dec))
    if (cosW < -1) return true
    if (cosW > 1) return false
    const w = Math.acos(cosW)
    const a = 0.0009 + (w + lw) / (2 * Math.PI) + n
    const set = 2451545 + a + 0.0053 * Math.sin(M) - 0.0069 * Math.sin(2 * L)
    const rise = noon - (set - noon)
    const ms = (j) => (j + 0.5 - 2440588) * DAY
    return [ms(rise), ms(set)]
  }

  const estimate = () => {
    const y = new Date().getFullYear()
    // Minutes west of UTC outside daylight saving time
    const std = Math.max(
      new Date(y, 0, 1).getTimezoneOffset(),
      new Date(y, 6, 1).getTimezoneOffset(),
    )
    let zone = ""
    try {
      zone = Intl.DateTimeFormat().resolvedOptions().timeZone || ""
    } catch {}
    const south =
      /^(Australia|Antarctica|Pacific\/(Auckland|Chatham|Fiji|Tongatapu|Noumea)|America\/(Santiago|Argentina|Sao_Paulo|Montevideo|Asuncion|La_Paz|Lima|Punta_Arenas)|Africa\/(Johannesburg|Maputo|Harare|Windhoek|Gaborone|Maseru|Mbabane|Lusaka|Blantyre|Lubumbashi)|Indian\/(Mauritius|Reunion|Antananarivo)|Atlantic\/Stanley)/.test(
        zone,
      )
    const lat = south ? -34 : zone.startsWith("Europe") ? 50 : /^(Asia|Africa)/.test(zone) ? 30 : 40
    return { lat, lng: -std / 4 }
  }

  const location = () => {
    try {
      const saved = JSON.parse(read(LOC) || "null")
      if (saved && isFinite(saved.lat) && isFinite(saved.lng)) return saved
    } catch {}
    return estimate()
  }

  // Is it dark outside now, and when does that next change?
  const sun = (now = Date.now()) => {
    const { lat, lng } = location()
    const t = sunTimes(now, lat, lng)
    if (typeof t === "boolean") return { dark: !t, next: now + 6 * 3600000 }
    const [rise, set] = t
    if (now < rise) return { dark: true, next: rise, rise, set }
    if (now < set) return { dark: false, next: set, rise, set }
    const tomorrow = sunTimes(now + DAY, lat, lng)
    const next = typeof tomorrow === "boolean" ? now + 6 * 3600000 : tomorrow[0]
    return { dark: true, next, rise, set }
  }

  const mode = () => {
    const m = read(KEY)
    return m === "light" || m === "dark" || m === "system" || m === "sun" ? m : "sun"
  }

  const apply = () => {
    clearTimeout(timer)
    const m = mode()
    let dark = m === "dark"
    if (m === "system") dark = media.matches
    if (m === "sun") {
      const s = sun()
      dark = s.dark
      // Flip at sunrise/sunset while the page is open (re-checked at least hourly)
      timer = setTimeout(apply, Math.max(60000, Math.min(s.next - Date.now() + 1000, 3600000)))
    }
    root.classList.toggle("dark", dark)
    const meta = document.querySelector('meta[name="theme-color"]')
    if (meta) meta.content = (dark ? meta.dataset.dark : meta.dataset.light) || meta.content
  }

  media.addEventListener("change", () => mode() === "system" && apply())
  addEventListener("storage", (e) => (e.key === KEY || e.key === LOC) && apply())
  // A tab left in the background may have missed its timer
  document.addEventListener("visibilitychange", () => document.hidden || apply())

  window.crumbTheme = { apply, mode, sun, estimate, KEY, LOC }
  apply()

  // Local time for "Try next": the UTC offset, and the zone for the hemisphere
  const age = "; path=/; max-age=31536000; samesite=lax"
  document.cookie = "crumb_tz=" + -new Date().getTimezoneOffset() + age
  try {
    const zone = Intl.DateTimeFormat().resolvedOptions().timeZone
    if (zone && /^[\w+/-]+$/.test(zone)) document.cookie = "crumb_zone=" + zone + age
  } catch {}
})()
