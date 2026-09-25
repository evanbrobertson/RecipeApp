// Inlined into every page's <head> (pagereveal can fire before deferred scripts run).
//
// Named cross-document view transitions: a recipe card's photo morphs into the recipe
// page's hero, and back. Only one element per page may carry the name, and only while
// it's on screen, so it's named at the last moment (pageswap on the old page, pagereveal
// on the new one) and cleared when the transition finishes. Elsewhere pages crossfade.
;(() => {
  if (!("onpageswap" in window)) return
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
    if (!el || !vt) return
    el.style.viewTransitionName = "photo"
    vt.finished.finally(() => (el.style.viewTransitionName = ""))
  }
  addEventListener("pageswap", (e) =>
    name(pick(recipeId(e.activation?.entry?.url)), e.viewTransition),
  )
  addEventListener("pagereveal", (e) =>
    name(pick(recipeId(window.navigation?.activation?.from?.url)), e.viewTransition),
  )
})()
