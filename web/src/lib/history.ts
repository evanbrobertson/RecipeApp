/** The cook and view log behind "Try next" (stored on the server, so it follows you). */
import { api, errorMessage } from "./api"
import type { CookStats } from "./recipe"
import { toast } from "./toast"

/** Fire-and-forget: a view never gets in the way of the page. */
export function logView(id: number) {
  void fetch(`/api/recipes/${id}/viewed`, { method: "POST", keepalive: true }).catch(() => {})
}

/**
 * Marks a recipe as cooked and offers Undo. `onchange` gets the new stats after either.
 */
export async function markCooked(id: number, onchange?: (stats: CookStats) => void) {
  try {
    const { eventId, ...stats } = await api<CookStats & { eventId: number | null }>(
      `/api/recipes/${id}/cooked`,
      { method: "POST" },
    )
    onchange?.(stats)
    // Already logged a moment ago: no Undo, which would take away that earlier cook
    if (eventId == null) {
      toast({ title: "Already marked as cooked", description: "Logged in the last few hours." })
      return
    }
    toast({
      title: "Marked as cooked",
      description: "It'll sit out of Try next for a couple of weeks.",
      tone: "success",
      action: {
        label: "Undo",
        onselect: () => {
          api<CookStats>(`/api/recipes/${id}/cooked?event=${eventId}`, { method: "DELETE" })
            .then((s) => onchange?.(s))
            .catch((e) =>
              toast({ title: "Couldn't undo", description: errorMessage(e), tone: "error" }),
            )
        },
      },
    })
  } catch (e) {
    toast({ title: "Couldn't mark as cooked", description: errorMessage(e), tone: "error" })
  }
}

/** "Cooked 3 times · last 2 weeks ago" */
export function cookedLine(stats: CookStats | undefined, now = Date.now()): string | null {
  if (!stats?.count) return null
  const times = stats.count === 1 ? "Cooked once" : `Cooked ${stats.count} times`
  if (!stats.lastCookedAt) return times
  const days = Math.floor((now - Date.parse(stats.lastCookedAt)) / 86_400_000)
  const ago =
    days < 1
      ? "today"
      : days < 2
        ? "yesterday"
        : days < 14
          ? `${days} days ago`
          : days < 60
            ? `${Math.round(days / 7)} weeks ago`
            : days < 365
              ? `${Math.round(days / 30)} months ago`
              : `${Math.round(days / 365)} years ago`
  return `${times} · last ${ago}`
}
