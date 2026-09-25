/**
 * Runs `fn` once the page is actually being looked at. Speculation rules may
 * prerender a page on hover, and a prerendered page shouldn't claim it was viewed
 * or grab a wake lock until it's shown.
 */
export function whenActive(fn: () => void) {
  const doc = document as Document & { prerendering?: boolean }
  if (doc.prerendering) {
    document.addEventListener("prerenderingchange", fn, { once: true })
  } else fn()
}
