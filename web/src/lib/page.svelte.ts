import { inlineData } from "./api"

/**
 * Page data as reactive state: the server-inlined JSON when present (the normal case,
 * no request at all), otherwise loaded with `load` (e.g. under `astro dev`).
 */
export function pageState<T>(load: () => Promise<T>) {
  const state = $state({ data: inlineData<T>(), loading: false, missing: false })
  if (state.data === null) {
    state.loading = true
    load()
      .then((d) => (state.data = d))
      .catch(() => (state.missing = true))
      .finally(() => (state.loading = false))
  }
  return state
}
