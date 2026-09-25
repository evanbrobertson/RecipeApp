/** `window.crumbTheme`, set up by the inlined theme-boot.js in every page's head. */
export type ThemeMode = "light" | "dark" | "system" | "sun"

export interface SunState {
  /** Whether it's dark outside right now. */
  dark: boolean
  /** When that next changes (ms). */
  next: number
  /** Today's sunrise and sunset (ms). Missing in polar day or night. */
  rise?: number
  set?: number
}

export interface CrumbTheme {
  /** Re-read the stored mode (and location) and apply it to the page. */
  apply(): void
  /** The stored mode, `"sun"` when nothing valid is stored. */
  mode(): ThemeMode
  sun(now?: number): SunState
  /** The location guessed from the time zone. */
  estimate(): { lat: number; lng: number }
  /** localStorage key for the mode (a bare string). */
  KEY: string
  /** localStorage key for a saved location, JSON `{"lat":n,"lng":n}`. */
  LOC: string
}

declare global {
  interface Window {
    crumbTheme: CrumbTheme
  }
}
