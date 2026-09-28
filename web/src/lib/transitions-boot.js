// Inlined into every page's <head> (pagereveal can fire before deferred scripts run).
//
// Named cross-document view transitions: a recipe card's photo morphs into the recipe
// page's hero, and back. Only one element per page may carry the name, and only while
// it's on screen, so it's named at the last moment (pageswap on the old page, pagereveal
// on the new one) and cleared when the transition finishes. Elsewhere pages crossfade.
;(() => {
  if (!("onpageswap" in window)) return
  const noop = () => {}
  const recipeId = (url) => {
    if (!url) return null
    const m = /^\/recipes\/(\d+)\/?$/.exec(new URL(url, location.href).pathname)
    return m ? Number(m[1]) : null
  }
  const onScreen = (el) => {
    const r = el.getBoundingClientRect()
    return r.width > 0 && r.bottom > 0 && r.top < innerHeight && r.right > 0 && r.left < innerWidth
  }
  const pick = (other) => {
    const hero = document.querySelector("[data-photo-hero]")
    if (hero) return other == null && onScreen(hero) ? hero : null
    if (other == null) return null
    for (const el of document.querySelectorAll(`[data-photo="${other}"]`))
      if (onScreen(el)) return el
    return null
  }
  const name = (el, vt) => {
    if (!vt) return
    // A skipped transition (the other page opted out, the tab was hidden, a quick second
    // navigation) rejects `ready` and can reject `finished` with "AbortError: Transition was
    // skipped". That's routine, so handle both on every transition, photo or plain crossfade,
    // or they surface as unhandled rejections
    vt.ready.catch(noop)
    if (!el) return void vt.finished.catch(noop)
    el.style.viewTransitionName = "photo"
    const clear = () => (el.style.viewTransitionName = "")
    vt.finished.then(clear, clear)
  }
  addEventListener("pageswap", (e) =>
    name(pick(recipeId(e.activation?.entry?.url)), e.viewTransition),
  )
  addEventListener("pagereveal", (e) =>
    name(pick(recipeId(window.navigation?.activation?.from?.url)), e.viewTransition),
  )
})()
