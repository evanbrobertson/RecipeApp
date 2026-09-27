/**
 * The "Export from Just the Recipe" bookmarklet on the Import page.
 *
 * Just the Recipe draws its web app onto a <canvas> (Flutter), so there's nothing to scrape.
 * Run on justtherecipe.com while signed in, the bookmarklet reads the cook's own recipes and
 * collections through the Firebase SDK the page has already loaded, with their session, and
 * downloads them as `just-the-recipe.json` (the format `src/importers.rs` reads). Nothing is
 * sent anywhere but Just the Recipe's own backend: the file goes to Crumb by upload.
 *
 * Both functions are stringified into the bookmarklet (`bookmarklet()`), so neither may use
 * anything from outside itself. Port of github.com/evanbrobertson/justtherecipe-export.
 */

/**
 * Just the Recipe's saved data, as read in the page, into Crumb's import format.
 * `raw.recipes[].photoUrl` is the photo already resolved to a link, if any.
 * @param {{ collections: any[], recipes: any[] }} raw
 */
export function toLibrary(raw) {
  const clean = (/** @type {unknown} */ s) => (typeof s === "string" ? s.trim() : "")
  const link = (/** @type {unknown} */ s) => (/^https?:\/\//i.test(clean(s)) ? clean(s) : null)
  /* Durations are kept in microseconds */
  const duration = (/** @type {unknown} */ us) => {
    const m = typeof us === "number" && us > 0 ? Math.round(us / 60_000_000) : 0
    if (!m) return null
    const h = Math.floor(m / 60)
    return [h && `${h}h`, m % 60 && `${m % 60}m`].filter(Boolean).join(" ")
  }
  /* A list with "group" entries (a header and its steps) into [{ name, items }] sections */
  const sections = (/** @type {any[]} */ list, /** @type {(e: any) => string} */ textOf) => {
    /** @type {{ name: string | null, items: string[] }[]} */
    const out = [{ name: null, items: [] }]
    for (const entry of list ?? []) {
      if (entry?.type === "group") {
        const items = (entry.steps ?? []).map(textOf).filter(Boolean)
        out.push({ name: clean(entry.name).replace(/:$/, "") || null, items })
        out.push({ name: null, items: [] })
      } else {
        const text = textOf(entry)
        if (text) out.at(-1)?.items.push(text)
      }
    }
    return out.filter((s) => s.items.length)
  }
  const joined = (/** @type {unknown} */ v) =>
    Array.isArray(v) ? v.map(clean).filter(Boolean).join(", ") || null : null

  /** @type {Map<string, string[]>} */
  const booksOf = new Map()
  for (const c of raw.collections) {
    const name = clean(c.name)
    if (!name) continue
    for (const id of c.recipes ?? []) booksOf.set(id, [...(booksOf.get(id) ?? []), name])
  }
  return {
    format: "just-the-recipe",
    version: 1,
    exportedAt: new Date().toISOString(),
    cookbooks: raw.collections
      .map((c) => clean(c.name))
      .filter(Boolean)
      .map((name) => ({ name, description: null })),
    recipes: raw.recipes.map((r) => ({
      title: clean(r.name) || "Untitled recipe",
      url: link(r.sourceUrl),
      image: link(r.photoUrl),
      prepTime: duration(r.prepTime),
      cookTime: duration(r.cookTime),
      totalTime: duration(r.totalTime),
      recipeYield: r.servings ? String(r.servings) : null,
      recipeCategory: joined(r.categories),
      recipeCuisine: joined(r.cuisines),
      ingredients: sections(r.ingredients, (i) => clean(i?.name ?? i?.text)),
      instructions: sections(r.instructions, (s) => clean(s?.text ?? s?.name)),
      notes: clean(r.notes) || null,
      cookbooks: booksOf.get(r.docId) ?? [],
    })),
  }
}

/**
 * Runs on justtherecipe.com: reads, converts with `convert` (`toLibrary`) and downloads.
 * `importUrl` is this Crumb's Import page, linked from the finished message.
 * @param {typeof toLibrary} convert
 * @param {string} importUrl
 */
export async function exportLibrary(convert, importUrl) {
  const w = /** @type {any} */ (window)
  if (!/(^|\.)justtherecipe\.com$/.test(location.hostname)) {
    alert("Open justtherecipe.com and sign in, then tap this bookmark again.")
    return
  }
  const app = w.firebase_core?.getApps?.()[0]
  const uid = app && w.firebase_auth?.getAuth(app).currentUser?.uid
  if (!uid) {
    alert(
      "Sign in to Just the Recipe first (wait for your recipes to show), then tap this bookmark again.",
    )
    return
  }

  document.getElementById("crumb-jtr")?.remove()
  const box = document.createElement("div")
  box.id = "crumb-jtr"
  box.style.cssText =
    "position:fixed;z-index:2147483647;top:16px;left:50%;transform:translateX(-50%);" +
    "width:min(420px,calc(100vw - 32px));box-sizing:border-box;padding:16px 20px;" +
    "border-radius:16px;background:#fffdf8;color:#1c2b22;border:1px solid #e3dfd0;" +
    "box-shadow:0 8px 24px rgba(0,0,0,.18);font:15px/1.45 system-ui,sans-serif"
  const text = document.createElement("p")
  text.style.margin = "0"
  box.append(text)
  document.body.append(box)
  const say = (/** @type {string} */ msg) => (text.textContent = msg)
  const finish = (/** @type {string} */ msg, /** @type {boolean} */ linkBack) => {
    say(msg)
    const row = document.createElement("p")
    row.style.cssText = "margin:12px 0 0;display:flex;gap:16px;justify-content:flex-end"
    if (linkBack) {
      const a = document.createElement("a")
      a.href = importUrl
      a.textContent = "Open Crumb"
      a.style.cssText = "color:#2f6b4f;font-weight:700"
      row.append(a)
    }
    const close = document.createElement("button")
    close.textContent = "Close"
    close.style.cssText = "all:unset;cursor:pointer;font-weight:700;color:#2f6b4f"
    close.addEventListener("click", () => box.remove())
    row.append(close)
    box.append(row)
  }

  try {
    say("Reading your recipes…")
    const F = w.firebase_firestore
    const db = F.getFirestore(app)
    const plain = (/** @type {any} */ v) =>
      v && typeof v.toDate === "function"
        ? v.toDate().toISOString()
        : Array.isArray(v)
          ? v.map(plain)
          : v && typeof v === "object"
            ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, plain(x)]))
            : v
    const list = async (/** @type {string} */ name) =>
      (await F.getDocs(F.collection(db, "saved-recipes", uid, name))).docs.map(
        (/** @type {any} */ d) => ({ docId: d.id, ...plain(d.data()) }),
      )
    const bookmarks = await list("bookmarks")
    const collections = await list("collections")
    /* The rules only allow reading recipes one at a time, as the app does */
    const ids = [
      ...new Set([
        ...bookmarks.map((/** @type {any} */ b) => b.recipeId ?? b.docId),
        ...collections.flatMap((/** @type {any} */ c) => c.recipes ?? []),
      ]),
    ]
    const storage = w.firebase_storage
    const recipes = []
    let missing = 0
    for (const [i, id] of ids.entries()) {
      say(`Reading your recipes… ${i + 1} of ${ids.length}`)
      try {
        const snap = await F.getDoc(F.doc(db, "saved-recipes", uid, "recipes", id))
        if (!snap.exists()) {
          missing++
          continue
        }
        const r = { docId: id, ...plain(snap.data()) }
        /* A photo the cook uploaded may be a Storage path rather than a link */
        let photo = r.customImageLocation
        if (photo && !/^https?:/i.test(photo)) {
          try {
            photo = await storage.getDownloadURL(storage.ref(storage.getStorage(app), photo))
          } catch {
            photo = null
          }
        }
        r.photoUrl = photo || r.imageUrls?.[0] || null
        recipes.push(r)
      } catch {
        missing++
      }
    }
    if (!recipes.length) {
      finish("No saved recipes found in this Just the Recipe account.", false)
      return
    }

    const library = convert({ collections, recipes })
    const file = new Blob([JSON.stringify(library, null, 2)], { type: "application/json" })
    const a = document.createElement("a")
    a.href = URL.createObjectURL(file)
    a.download = "just-the-recipe.json"
    document.body.append(a)
    a.click()
    a.remove()
    setTimeout(() => URL.revokeObjectURL(a.href), 60_000)
    const n = recipes.length
    finish(
      `Saved ${n} recipe${n === 1 ? "" : "s"} as just-the-recipe.json` +
        (missing ? ` (${missing} couldn't be read)` : "") +
        ". Upload it on Crumb's Import page.",
      true,
    )
  } catch (e) {
    finish(`Couldn't read your recipes: ${e instanceof Error ? e.message : e}`, false)
  }
}

/** Placeholder for the Import page's own address, filled in on the page (it varies by host). */
export const IMPORT_URL = "__CRUMB_IMPORT_URL__"

/** The bookmarklet, as a `javascript:` link. */
export function bookmarklet() {
  const source = `void (${exportLibrary})(${toLibrary}, "${IMPORT_URL}")`
  return `javascript:${encodeURIComponent(source)}`
}
