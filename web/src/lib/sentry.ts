/**
 * The browser SDK, loaded by sentry-boot.ts only when the server has Sentry on.
 *
 * Recipes are private, so nothing identifying is collected: no user info, cookies, headers,
 * bodies or query strings, and Session Replay masks all text and blocks all media.
 */
import { addIntegration, browserTracingIntegration, captureException, init } from "@sentry/astro"
import type { SentryBoot } from "./sentry-boot"

type Crumb = { category?: string; message?: string; data?: Record<string, unknown> }

// A share link's token is the share itself: `/s/{token}` (and the old `/api/shares/{token}`)
// never goes into a breadcrumb, a transaction or a replay's network list
export const redact = (url: string) => url.replace(/(^|\/)(s|shares)\/[^/?#\s]+/g, "$1$2/[token]")

const bare = (url: unknown) => (typeof url === "string" ? redact(url.replace(/[?#].*$/s, "")) : url)

// Click and input breadcrumbs name the element they hit, which can be a recipe or cookbook
// title, so they keep only the category; URLs (fetch, xhr, navigation) lose their query
// strings, fragments and share tokens
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
    beforeSendTransaction(event) {
      if (event.transaction) event.transaction = redact(event.transaction)
      if (event.request?.url) event.request.url = redact(event.request.url)
      for (const span of event.spans ?? [])
        if (span.description) span.description = redact(span.description)
      return event
    },
    beforeSend(event) {
      if (event.request?.url) event.request.url = redact(event.request.url)
      if (event.transaction) event.transaction = redact(event.transaction)
      return event
    },
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
