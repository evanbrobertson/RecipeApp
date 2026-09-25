/** Session Replay in its own chunk (the largest part of the SDK), added after the page is idle. */
import { replayIntegration } from "@sentry/astro"

export const replay = () =>
  replayIntegration({ maskAllText: true, maskAllInputs: true, blockAllMedia: true })
