/**
 * The cook's Crumb: where it is, and the settings kept in `storage.sync` (so they follow the
 * browser profile). Pure helpers first, tested without a browser; storage below.
 */

export interface Settings {
  /** The Crumb's origin, e.g. "https://crumb.example.com"; empty until it's set. */
  crumb: string
  /** Offer "Read this recipe in Crumb?" on recipe pages. */
  prompt: boolean
  /** Sites (hostnames, without "www.") the offer was turned off for. */
  muted: string[]
}

export const DEFAULTS: Settings = { crumb: "", prompt: true, muted: [] }

/** "crumb.example.com" or a full URL, as the origin of that Crumb; null if it isn't one. */
export function crumbOrigin(raw: string): string | null {
  let value = raw.trim()
  if (!value) return null
  if (!/^https?:\/\//i.test(value)) value = `https://${value}`
  try {
    const url = new URL(value)
    if (!/^https?:$/.test(url.protocol)) return null
    const host = url.hostname
    // A domain, an IP address or localhost; not a bare word
    return host.includes(".") || host.includes(":") || host === "localhost" ? url.origin : null
  } catch {
    return null
  }
}

/** The Crumb page that reads `page` and offers to add it (src/preview.rs). */
export function previewUrl(crumb: string, page: string): string {
  return `${crumb}/preview?url=${encodeURIComponent(page)}`
}

/**
 * Crumb's Add page with a video's link filled in. `via=extension` tells it the extension has
 * what it read of the video waiting (web/src/islands/TopBox.svelte).
 */
export function addVideoUrl(crumb: string, video: string): string {
  return `${crumb}/add?url=${encodeURIComponent(video)}&via=extension`
}

/** A page the extension can hand to Crumb: http(s), and not the Crumb itself. */
export function readable(page: string | undefined, crumb: string): boolean {
  if (!page) return false
  try {
    const url = new URL(page)
    return /^https?:$/.test(url.protocol) && url.origin !== crumb
  } catch {
    return false
  }
}

/** A hostname as the muted list keeps it. */
export function siteOf(hostname: string): string {
  return hostname.toLowerCase().replace(/^www\./, "")
}

/** Settings as stored, with anything missing or malformed at its default. */
export function normalize(stored: Record<string, unknown>): Settings {
  return {
    crumb: typeof stored.crumb === "string" ? (crumbOrigin(stored.crumb) ?? "") : "",
    prompt: typeof stored.prompt === "boolean" ? stored.prompt : DEFAULTS.prompt,
    muted: Array.isArray(stored.muted)
      ? stored.muted.filter((s): s is string => typeof s === "string")
      : [],
  }
}

export async function load(): Promise<Settings> {
  return normalize(await chrome.storage.sync.get(DEFAULTS as unknown as Record<string, unknown>))
}

export async function save(patch: Partial<Settings>): Promise<void> {
  await chrome.storage.sync.set(patch)
}
