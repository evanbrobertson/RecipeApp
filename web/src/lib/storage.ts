/**
 * Browser storage that never throws (private windows, blocked site data) and
 * falls back to the given default.
 */

function store(kind: "local" | "session"): Storage | null {
  try {
    return kind === "local" ? window.localStorage : window.sessionStorage
  } catch {
    return null
  }
}

export function read<T>(key: string, fallback: T, kind: "local" | "session" = "local"): T {
  try {
    const raw = store(kind)?.getItem(key)
    return raw == null ? fallback : (JSON.parse(raw) as T)
  } catch {
    return fallback
  }
}

export function write(key: string, value: unknown, kind: "local" | "session" = "local") {
  try {
    store(kind)?.setItem(key, JSON.stringify(value))
  } catch {
    // storage full or unavailable: the feature just doesn't persist
  }
}

export function remove(key: string, kind: "local" | "session" = "local") {
  try {
    store(kind)?.removeItem(key)
  } catch {
    // ignore
  }
}

// ─── Per-recipe state shared by the recipe, cook and prep pages ─────────────

export const scaleKey = (id: number) => `crumb:scale:${id}`

export function getScale(id: number): number {
  return read(scaleKey(id), 1, "session")
}

export function setScale(id: number, scale: number) {
  write(scaleKey(id), scale, "session")
}

// ─── Recently viewed, for "pick up where you left off" ──────────────────────

export interface Viewed {
  id: number
  title: string
  image: string | null
  /** When it was last opened (ms). Missing on entries saved before this was recorded. */
  at?: number
  /** Number of steps, for "Step 4 of 9" while cooking. */
  steps?: number
}

const RECENT = "crumb:recent"

export function recentlyViewed(): Viewed[] {
  return read<Viewed[]>(RECENT, [])
}

export function rememberViewed(r: {
  id: number
  title: string
  image: string | null
  instructions?: { items: string[] }[]
}) {
  const steps = r.instructions?.reduce((n, s) => n + s.items.length, 0)
  write(
    RECENT,
    [
      { id: r.id, title: r.title, image: r.image, at: Date.now(), steps },
      ...recentlyViewed().filter((v) => v.id !== r.id),
    ].slice(0, 6),
  )
}

/** The step cook mode is on for this recipe in this tab (0-based), if it has been opened. */
export function cookStep(id: number): number | null {
  return read<number | null>(`crumb:cook:${id}`, null, "session")
}

export function forgetViewed(ids: number[]) {
  write(
    RECENT,
    recentlyViewed().filter((v) => !ids.includes(v.id)),
  )
}
