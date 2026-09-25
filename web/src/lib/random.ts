/**
 * Surprise me: links to /random that skip what this session has already served and
 * what was opened recently. The server also skips anything cooked lately.
 */
import { read, recentlyViewed, write } from "./storage"

const SEEN = "crumb:random:seen"

function seen(): number[] {
  return read<number[]>(SEEN, [], "session")
}

/** Remember a recipe the random link landed on. */
export function rememberRandom(id: number) {
  write(SEEN, [id, ...seen().filter((s) => s !== id)].slice(0, 40), "session")
}

/** The /random URL for right now, avoiding `current` whenever there's an alternative. */
export function randomHref(current?: number): string {
  const exclude = [...new Set([...seen(), ...recentlyViewed().map((r) => r.id)])]
  const params = new URLSearchParams()
  if (exclude.length) params.set("exclude", exclude.join(","))
  if (current) params.set("current", String(current))
  const query = params.toString()
  return query ? `/random?${query}` : "/random"
}

/**
 * Keeps every `a[data-random]` link's exclusions current at the moment it's used.
 * The plain /random link still works without JavaScript.
 */
export function wireRandomLinks(root: ParentNode = document) {
  for (const a of root.querySelectorAll<HTMLAnchorElement>("a[data-random]")) {
    const update = () => {
      const current = Number(a.dataset.current) || undefined
      a.href = randomHref(current)
    }
    a.addEventListener("pointerdown", update)
    a.addEventListener("focus", update)
    a.addEventListener("click", update)
  }
}
