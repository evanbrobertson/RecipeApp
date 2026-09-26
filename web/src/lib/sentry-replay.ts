/** Session Replay in its own chunk (the largest part of the SDK), added after the page is idle. */
import { replayIntegration } from "@sentry/astro"
import { redact } from "./sentry"

type Frame = { data?: { tag?: string; payload?: { description?: unknown } } }

export const replay = () =>
  replayIntegration({
    maskAllText: true,
    maskAllInputs: true,
    blockAllMedia: true,
    // Network and navigation entries name the URL, which on a share page holds its token
    beforeAddRecordingEvent(event) {
      const payload = (event as Frame).data?.payload
      if (typeof payload?.description === "string")
        payload.description = redact(payload.description)
      return event
    },
  })
