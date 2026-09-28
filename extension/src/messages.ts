import type { VideoRead } from "./youtube"

/** What the content script asks the background for (it can't open tabs itself). */
export type Message =
  /** Read `url` in Crumb, in a new tab beside this one; `video` is what was read of a YouTube video. */
  | { type: "read"; url: string; video?: VideoRead }
  /** Use the Crumb at `origin` (the page the cook is on). */
  | { type: "connect"; origin: string }
  /** Open the extension's settings. */
  | { type: "settings" }
  /** From the Crumb tab the extension opened for a video: what was read of it (or null). */
  | { type: "takeVideo" }

/** What the background asks a YouTube tab's content script: read the video it's showing. */
export interface ReadVideo {
  type: "readVideo"
}

/** What was read of a video, waiting for the Crumb tab opened for it; `video` null while reading. */
export interface Waiting {
  crumb: string
  url: string
  video: VideoRead | null
}

/** What reaches the Add page (web/src/islands/TopBox.svelte) by `window.postMessage`. */
export interface VideoForCrumb {
  type: "crumb:video"
  url: string
  video: VideoRead
}

export function send<T = unknown>(message: Message): Promise<T> {
  return chrome.runtime.sendMessage(message)
}
