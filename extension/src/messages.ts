import type { PageReading } from "./page"
import type { CaptionSource, VideoRead, VideoReading } from "./youtube"

/** What the content script asks the background for (it can't open tabs itself). */
export type Message =
  /** Read `url` in Crumb: a recipe in this tab, a video in a new one beside it (`video` is what was read of it). */
  | { type: "read"; url: string; video?: VideoReading }
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

/** What the background asks any page's content script: the recipe on it (see page.ts). */
export interface ReadPage {
  type: "readPage"
}

/**
 * A page or video being read for the Crumb tab opened for it: `reading` until the page's tab
 * answers, then `read` (with what was read) or `failed`. `kind` says which of `video` and
 * `page` to look at.
 */
export interface Waiting {
  crumb: string
  url: string
  kind: "video" | "page"
  status: "reading" | "read" | "failed"
  video: VideoRead | null
  captions: CaptionSource
  page: PageReading | null
}

/**
 * What reaches the Crumb preview page (web/src/islands/PreviewActions.svelte) by
 * `window.postMessage`: the recipe as read from the page, or `page: null` when there was none
 * (then Crumb reads the link itself).
 */
export interface PageForCrumb {
  type: "crumb:page"
  url: string
  page: PageReading | null
}

/**
 * What reaches the Add page (web/src/islands/TopBox.svelte) by `window.postMessage`: the
 * video as read, or `video: null` when it couldn't be read in the browser.
 */
export interface VideoForCrumb {
  type: "crumb:video"
  url: string
  video: VideoRead | null
  captions: CaptionSource
}

export function send<T = unknown>(message: Message): Promise<T> {
  return chrome.runtime.sendMessage(message)
}
