/** Thin fetch wrapper for the Crumb API. Errors carry the server's human message. */

export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message)
  }
}

interface Options {
  method?: "GET" | "POST" | "PATCH" | "DELETE"
  body?: unknown
  form?: FormData
}

export async function api<T = unknown>(path: string, opts: Options = {}): Promise<T> {
  const headers: Record<string, string> = { accept: "application/json" }
  let body: BodyInit | undefined
  if (opts.form) body = opts.form
  else if (opts.body !== undefined) {
    headers["content-type"] = "application/json"
    body = JSON.stringify(opts.body)
  }

  let res: Response
  try {
    res = await fetch(path, { method: opts.method ?? "GET", headers, body })
  } catch {
    throw new ApiError("Couldn't reach the server. Check your connection.", 0)
  }

  // Signed out (session expired or password changed): go sign in, then come back
  if (res.status === 401 && !path.startsWith("/api/auth/")) {
    location.href = `/login?next=${encodeURIComponent(location.pathname + location.search)}`
  }

  const type = res.headers.get("content-type") ?? ""
  const data = type.includes("json") ? await res.json().catch(() => null) : await res.text()
  if (!res.ok) {
    const err = data as { message?: string; statusMessage?: string } | null
    throw new ApiError(err?.message || err?.statusMessage || res.statusText || "Error", res.status)
  }
  return data as T
}

/** A readable message from anything thrown by api(). */
export function errorMessage(e: unknown, fallback = "Something went wrong"): string {
  return e instanceof Error && e.message ? e.message : fallback
}

/** The data the server inlined into this page (see the page-data table in the Rust server). */
export function inlineData<T>(): T | null {
  const el = document.getElementById("page-data")
  try {
    return el?.textContent ? (JSON.parse(el.textContent) as T | null) : null
  } catch {
    return null
  }
}

/** Numeric id from the current path, e.g. /recipes/12/cook → 12. */
export function pathId(): number {
  return Number(location.pathname.split("/")[2])
}
