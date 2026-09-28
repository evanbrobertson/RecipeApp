/**
 * YouTube turns servers away ("Sign in to confirm you're not a bot"), so for a YouTube video
 * the extension reads the video's page here, in the cook's own browser, and sends Crumb the
 * text: its title, description and the words of its captions. Nothing of the cook's YouTube
 * sign-in is read or sent; the requests go to YouTube as the page's own would.
 *
 * The parsing is pure (strings and JSON in, text out) and tested without a browser;
 * `readVideo` at the bottom does the fetching, from the content script on youtube.com.
 */

/** What Crumb's import takes as `video` (src/video.rs `FromBrowser`). */
export interface VideoRead {
  title?: string
  description?: string
  author?: string
  thumbnail?: string
  /** Seconds. */
  duration?: number
  transcript?: string
}

/** Crumb keeps this much of a transcript; the rest isn't sent. */
export const MAX_TRANSCRIPT = 30_000

/** The video's id when `url` is a YouTube video (a watch page, a Short, a live or youtu.be). */
export function videoId(url: string | undefined): string | null {
  if (!url) return null
  let u: URL
  try {
    u = new URL(url)
  } catch {
    return null
  }
  const host = u.hostname.toLowerCase().replace(/^(www|m)\./, "")
  const id = /^[\w-]{6,20}$/
  if (host === "youtu.be") {
    const v = u.pathname.slice(1)
    return id.test(v) ? v : null
  }
  if (host !== "youtube.com") return null
  if (u.pathname === "/watch") {
    const v = u.searchParams.get("v") ?? ""
    return id.test(v) ? v : null
  }
  const m = /^\/(shorts|live)\/([\w-]+)/.exec(u.pathname)
  return m && id.test(m[2]!) ? m[2]! : null
}

/**
 * The JSON value that starts right after `marker` in a page's HTML, e.g.
 * `var ytInitialPlayerResponse = {...};`. Walks the braces (minding strings), so what
 * follows the value doesn't matter.
 */
export function jsonAfter(html: string, marker: string): unknown {
  const at = html.indexOf(marker)
  if (at < 0) return null
  const start = html.indexOf("{", at + marker.length)
  if (start < 0) return null
  let depth = 0
  let inString = false
  for (let i = start; i < html.length; i++) {
    const c = html[i]
    if (inString) {
      if (c === "\\") i++
      else if (c === '"') inString = false
    } else if (c === '"') inString = true
    else if (c === "{") depth++
    else if (c === "}" && --depth === 0) {
      try {
        return JSON.parse(html.slice(start, i + 1))
      } catch {
        return null
      }
    }
  }
  return null
}

type Json = Record<string, unknown>

function obj(v: unknown): Json {
  return v && typeof v === "object" && !Array.isArray(v) ? (v as Json) : {}
}

function str(v: unknown): string | undefined {
  return typeof v === "string" && v.trim() ? v.trim() : undefined
}

/** A caption track as the player lists it. */
export interface Track {
  baseUrl: string
  languageCode: string
  /** "asr" for YouTube's automatic captions. */
  kind?: string
}

/** The video's details and caption tracks from `ytInitialPlayerResponse`. */
export function readPlayer(player: unknown): { video: VideoRead; tracks: Track[] } {
  const details = obj(obj(player).videoDetails)
  const thumbs = obj(details.thumbnail).thumbnails
  const last = Array.isArray(thumbs) ? obj(thumbs[thumbs.length - 1]) : {}
  const seconds = Number(details.lengthSeconds)
  const video: VideoRead = {
    title: str(details.title),
    description: str(details.shortDescription),
    author: str(details.author),
    thumbnail: str(last.url),
    duration: Number.isFinite(seconds) && seconds > 0 ? seconds : undefined,
  }
  const list = obj(obj(obj(player).captions).playerCaptionsTracklistRenderer).captionTracks
  const tracks: Track[] = (Array.isArray(list) ? list : []).flatMap((t) => {
    const track = obj(t)
    const baseUrl = str(track.baseUrl)
    const languageCode = str(track.languageCode)
    return baseUrl && languageCode ? [{ baseUrl, languageCode, kind: str(track.kind) }] : []
  })
  return { video, tracks }
}

/** The English track to read: the cook's own captions before YouTube's automatic ones. */
export function pickTrack(tracks: readonly Track[]): Track | null {
  const english = tracks.filter((t) => /^en\b/i.test(t.languageCode))
  return english.find((t) => t.kind !== "asr") ?? english[0] ?? null
}

/** Joins words into one paragraph, cut to what Crumb keeps. */
function paragraph(parts: readonly string[]): string {
  return parts.join(" ").replace(/\s+/g, " ").trim().slice(0, MAX_TRANSCRIPT)
}

/** The words of a caption track fetched with `fmt=json3`. */
export function json3Text(json: unknown): string {
  const events = obj(json).events
  if (!Array.isArray(events)) return ""
  return paragraph(
    events.flatMap((e) => {
      const segs = obj(e).segs
      return Array.isArray(segs) ? segs.map((s) => str(obj(s).utf8) ?? "") : []
    }),
  )
}

/** Every value under `key` anywhere in a JSON value (YouTube's nesting moves about). */
export function findAll(value: unknown, key: string, out: unknown[] = [], depth = 0): unknown[] {
  if (depth > 40 || !value || typeof value !== "object") return out
  if (Array.isArray(value)) {
    for (const v of value) findAll(v, key, out, depth + 1)
    return out
  }
  for (const [k, v] of Object.entries(value as Json)) {
    if (k === key) out.push(v)
    findAll(v, key, out, depth + 1)
  }
  return out
}

/** The words of the page's "Show transcript" panel (`youtubei/v1/get_transcript`'s reply). */
export function transcriptPanelText(json: unknown): string {
  return paragraph(
    findAll(json, "transcriptSegmentRenderer").map((seg) => {
      const runs = obj(obj(seg).snippet).runs
      return Array.isArray(runs) ? runs.map((r) => str(obj(r).text) ?? "").join("") : ""
    }),
  )
}

/** Whether a video says it's a recipe, so the extension offers to read it. */
export function looksLikeRecipe(video: VideoRead): boolean {
  const said = `${video.title ?? ""}\n${video.description ?? ""}`
  return /\b(recipes?|ingredients?)\b/i.test(said)
}

// In the browser: the content script on youtube.com, fetching from YouTube as its page does

async function text(url: string, init?: RequestInit): Promise<string> {
  const res = await fetch(url, { credentials: "include", ...init })
  return res.ok ? res.text() : ""
}

async function captionTrackText(track: Track): Promise<string> {
  const url = new URL(track.baseUrl, "https://www.youtube.com")
  url.searchParams.set("fmt", "json3")
  const body = await text(url.toString())
  if (!body.trim()) return ""
  try {
    return json3Text(JSON.parse(body))
  } catch {
    return ""
  }
}

/** The "Show transcript" panel's words, when the caption track itself comes back empty. */
async function transcriptPanel(html: string): Promise<string> {
  const params = findAll(jsonAfter(html, "ytInitialData = "), "getTranscriptEndpoint")
    .map((e) => str(obj(e).params))
    .find(Boolean)
  const key = /"INNERTUBE_API_KEY":"([^"]+)"/.exec(html)?.[1]
  const context = jsonAfter(html, '"INNERTUBE_CONTEXT":')
  if (!params || !key || !context) return ""
  const body = await text(
    `https://www.youtube.com/youtubei/v1/get_transcript?key=${encodeURIComponent(key)}&prettyPrint=false`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ context, params }),
    },
  )
  try {
    return body ? transcriptPanelText(JSON.parse(body)) : ""
  } catch {
    return ""
  }
}

async function watchPage(id: string): Promise<string> {
  return text(`https://www.youtube.com/watch?v=${encodeURIComponent(id)}`)
}

/** A video's title and description, to decide whether to offer (no captions fetched). */
export async function peekVideo(id: string): Promise<VideoRead | null> {
  const player = jsonAfter(await watchPage(id), "ytInitialPlayerResponse = ")
  return player ? readPlayer(player).video : null
}

/** Reads a YouTube video's page (fetched afresh: YouTube swaps pages without reloading). */
export async function readVideo(id: string): Promise<VideoRead | null> {
  const html = await watchPage(id)
  const player = jsonAfter(html, "ytInitialPlayerResponse = ")
  if (!player) return null
  const { video, tracks } = readPlayer(player)
  if (!video.title && !video.description) return null
  const track = pickTrack(tracks)
  let transcript = ""
  try {
    if (track) transcript = await captionTrackText(track)
    if (!transcript) transcript = await transcriptPanel(html)
  } catch {
    // The details are still worth sending without the words
  }
  if (transcript) video.transcript = transcript
  return video
}
