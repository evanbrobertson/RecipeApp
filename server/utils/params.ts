import type { H3Event } from "h3"

export function idParam(event: H3Event, name = "id"): number {
  const id = Number(getRouterParam(event, name))
  if (!Number.isInteger(id) || id <= 0) {
    throw createError({ statusCode: 400, statusMessage: `Invalid ${name}` })
  }
  return id
}
