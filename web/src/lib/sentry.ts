/**
 * The browser SDK, loaded by sentry-boot.ts only when the server has Sentry on.
 *
 * Recipes are private, so nothing identifying is collected: no user info, cookies, headers,
 * bodies or query strings, and Session Replay masks all text and blocks all media.
 */
import { addIntegration, browserTracingIntegration, captureException, init } from "@sentry/astro"
import type { SentryBoot } from "./sentry-boot"

type Crumb = { category?: string; message?: string; data?: Record<string, unknown> }

const bare = (url: unknown) => (typeof url === "string" ? url.replace(/[?#].*$/s, "") : url)

// Click and input breadcrumbs name the element they hit, which can be a recipe or cookbook
// title, so they keep only the category; URLs lose their query strings and fragments
function scrub<T extends Crumb>(crumb: T): T {
  if (crumb.category?.startsWith("ui.")) delete crumb.message
  const data = crumb.data
  if (data) for (const key of ["url", "from", "to"]) if (key in data) data[key] = bare(data[key])
  return crumb
}

// Replay is the biggest part of the SDK: fetch it only after the page has gone idle again
const addReplay = () => void import("./sentry-replay").then((m) => addIntegration(m.replay()))

export function start({ dsn, environment, release }: SentryBoot, early: unknown[] = []) {
  const production = environment === "stable" || environment === "production"
  init({
    dsn,
    environment,
    release,
    integrations: [browserTracingIntegration()],
    beforeBreadcrumb: scrub,
    tracesSampleRate: production ? 0.2 : 1.0,
    replaysSessionSampleRate: 0.05,
    replaysOnErrorSampleRate: 1.0,
    dataCollection: {
      userInfo: false,
      cookies: false,
      httpHeaders: false,
      httpBodies: [],
      urlQueryParams: false,
      stackFrameVariables: false,
    },
  })

  for (const error of early) captureException(error)

  if ("requestIdleCallback" in window) requestIdleCallback(addReplay, { timeout: 5000 })
  else setTimeout(addReplay, 2000)
}
