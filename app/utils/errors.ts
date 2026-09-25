/** Pulls a readable message out of a $fetch error. */
export function errorMessage(e: unknown, fallback = "Something went wrong"): string {
  const err = e as {
    data?: { statusMessage?: string; message?: string; data?: unknown }
    message?: string
  }
  const issues = (err?.data?.data as { issues?: { message: string }[] } | undefined)?.issues
  if (issues?.length) return issues[0]!.message
  return err?.data?.statusMessage || err?.data?.message || fallback
}
