/**
 * Starts browser error reporting, but only when the server asks for it.
 *
 * The Rust server names the Sentry DSN, environment and release in a `Server-Timing` header on
 * page loads (see src/telemetry.rs), so one build serves every environment and a copy without
 * `SENTRY_DSN` loads no Sentry code at all. This file is the only part on the critical path
 * (under 1 KB); the SDK is a separate chunk fetched once the page has loaded and gone idle.
 */
export interface SentryBoot {
  dsn: string
  environment: string
  release?: string
}

function fromServer(): SentryBoot | undefined {
  try {
    const [nav] = performance.getEntriesByType("navigation") as PerformanceNavigationTiming[]
    const hint = new Map((nav?.serverTiming ?? []).map((t) => [t.name, t.description]))
    const dsn = hint.get("sentry")
    if (!dsn) return undefined
    return {
      dsn,
      environment: hint.get("sentry-env") || "production",
      release: hint.get("sentry-release") || undefined,
    }
  } catch {
    return undefined
  }
}

/** `astro dev` has no Rust server in front; PUBLIC_SENTRY_DSN opts a local session in. */
function fromDevEnv(): SentryBoot | undefined {
  const dsn = import.meta.env.PUBLIC_SENTRY_DSN
  if (!import.meta.env.DEV || !dsn) return undefined
  return { dsn, environment: "local" }
}

const boot = fromServer() ?? fromDevEnv()
if (boot) {
  // The SDK waits for the page to load and settle; errors thrown before then are kept and
  // reported once it starts
  const early: unknown[] = []
  const keep = (error: unknown) => early.length < 10 && early.push(error)
  const onError = (e: ErrorEvent) => keep(e.error ?? e.message)
  const onRejection = (e: PromiseRejectionEvent) => keep(e.reason)
  addEventListener("error", onError)
  addEventListener("unhandledrejection", onRejection)
  const start = () =>
    void import("./sentry").then((m) => {
      removeEventListener("error", onError)
      removeEventListener("unhandledrejection", onRejection)
      m.start(boot, early)
    })
  const whenIdle = () =>
    "requestIdleCallback" in window
      ? requestIdleCallback(start, { timeout: 3000 })
      : setTimeout(start, 1000)
  const whenLoaded = () =>
    document.readyState === "complete"
      ? whenIdle()
      : addEventListener("load", whenIdle, { once: true })
  // A speculatively prerendered page may never be seen: wait until it is
  if ((document as Document & { prerendering?: boolean }).prerendering) {
    document.addEventListener("prerenderingchange", whenLoaded, { once: true })
  } else {
    whenLoaded()
  }
}
