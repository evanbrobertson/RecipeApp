import { whenActive } from "./active"

/**
 * Keeps the screen on while a page is open (cook and prep modes).
 * Browsers may want a tap first, so `active` tells the UI whether to offer one.
 */
export function screenAwake() {
  const state = $state({ supported: "wakeLock" in navigator, active: false })
  let sentinel: WakeLockSentinel | null = null

  async function request() {
    if (!state.supported || document.visibilityState !== "visible") return
    try {
      sentinel = await navigator.wakeLock.request("screen")
      state.active = true
      sentinel.addEventListener("release", () => (state.active = false))
    } catch {
      state.active = false
    }
  }

  whenActive(() => void request())
  // The lock is dropped whenever the tab is hidden; take it back on return
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible" && !state.active) void request()
  })

  return {
    get supported() {
      return state.supported
    },
    get active() {
      return state.active
    },
    request,
  }
}
