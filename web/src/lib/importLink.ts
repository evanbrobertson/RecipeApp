/** Saving a link or pasted text (`POST /api/recipes/import`), following a video's job. */

import { api, ApiError } from "./api"
import type { BookImported } from "./recipe"

export interface Imported {
  id: number
  title: string
  isNew: boolean
  /** Another Crumb's shared cookbook: where its recipes went. */
  cookbook?: BookImported
  /** A cooking video Wee Chef watched in the server's queue. */
  fromVideo?: boolean
  /** The page's photo link was dead, so it was saved without one. */
  droppedPhoto?: boolean
}

/** A cooking video's place in the server's queue (`GET /api/import/jobs/{id}`). */
interface Job {
  id: string
  jobId?: string
  status: "queued" | "running" | "done" | "failed"
  /** 1 = next. */
  position?: number
  recipe?: { id: number; title: string; isNew: boolean }
  statusCode?: number
  message?: string
}

const POLL_MS = 2000

function ordinal(n: number): string {
  const tens = n % 100
  const last = n % 10
  const suffix = tens >= 11 && tens <= 13 ? "th" : (["th", "st", "nd", "rd"][last] ?? "th")
  return `${n}${suffix}`
}

/** What the Add box says while a video waits or is watched. */
export function jobProgress(job: Pick<Job, "status" | "position">): string {
  if (job.status === "queued") {
    return job.position && job.position > 1 ? `Queued (${ordinal(job.position)})…` : "Up next…"
  }
  return "Watching the video… this can take a minute or two"
}

/**
 * Saves a link or text. A cooking video comes back as a job, which is polled until the
 * recipe is saved; `progress` gets short lines like "Queued (2nd)…".
 */
export async function importRecipe(
  body: { url: string; video?: Record<string, unknown> } | { text: string },
  progress: (s: string) => void = () => {},
): Promise<Imported> {
  const res = await api<Imported | Job>("/api/recipes/import", { method: "POST", body })
  if (!("jobId" in res) || !res.jobId) return res as Imported
  let job: Job = res
  for (;;) {
    if (job.status === "done" && job.recipe) return { ...job.recipe, fromVideo: true }
    if (job.status === "failed") {
      throw new ApiError(job.message || "Couldn't read that video", job.statusCode ?? 422)
    }
    progress(jobProgress(job))
    await new Promise((r) => setTimeout(r, POLL_MS))
    job = await api<Job>(`/api/import/jobs/${encodeURIComponent(res.jobId)}`)
  }
}
