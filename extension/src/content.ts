/**
 * Runs on every http(s) page, and only reads it: nothing leaves the page unless the cook
 * clicks. On a recipe page (see detect.ts) it asks "Read this recipe in Crumb?"; on a Crumb
 * page, before any Crumb is set, it offers to use that one.
 */
import { load, save, siteOf } from "./crumb"
import { isRecipeItemType, recipeInJsonLd, type Found } from "./detect"
import { send } from "./messages"
import { showToast } from "./toast"

/** Some pages add their JSON-LD after load; one more look after this long. */
const LOOK_AGAIN_MS = 1500
const TOAST_MS = 20_000

function recipeOnPage(): Found | null {
  const blocks = Array.from(
    document.querySelectorAll('script[type="application/ld+json" i]'),
    (s) => s.textContent ?? "",
  )
  const found = recipeInJsonLd(blocks)
  if (found) return found
  for (const el of document.querySelectorAll("[itemtype]")) {
    if (isRecipeItemType(el.getAttribute("itemtype"))) {
      const name = el.querySelector('[itemprop="name"]')?.textContent?.trim() ?? ""
      return { name }
    }
  }
  return null
}

/** A signed-in Crumb page (its Layout names the app). */
function isCrumbApp(): boolean {
  return document.querySelector('meta[name="application-name"][content="Crumb"]') !== null
}

function offerConnect() {
  const origin = location.origin
  showToast({
    title: "Use this Crumb with the extension?",
    detail: location.host,
    timeoutMs: TOAST_MS,
    actions: [
      {
        label: "Use this Crumb",
        primary: true,
        run: () => void send({ type: "connect", origin }),
      },
      { label: "Not now", run: () => {} },
    ],
  })
}

function offerRead(found: Found) {
  const site = siteOf(location.hostname)
  showToast({
    title: "Read this recipe in Crumb?",
    detail: found.name || document.title,
    timeoutMs: TOAST_MS,
    actions: [
      {
        label: "Read in Crumb",
        primary: true,
        run: () => void send({ type: "read", url: location.href }),
      },
      {
        label: "Not on this site",
        run: async () => {
          const { muted } = await load()
          if (!muted.includes(site)) await save({ muted: [...muted, site] })
        },
      },
    ],
  })
}

async function main() {
  if (window.top !== window) return
  const settings = await load()
  if (!settings.crumb) {
    if (isCrumbApp()) offerConnect()
    // Until a Crumb is set, recipe pages still offer (and the click opens the settings)
  } else if (location.origin === settings.crumb) {
    return
  }
  if (!settings.prompt || settings.muted.includes(siteOf(location.hostname))) return
  if (isCrumbApp()) return
  const found = recipeOnPage()
  if (found) return offerRead(found)
  setTimeout(() => {
    const late = recipeOnPage()
    if (late) offerRead(late)
  }, LOOK_AGAIN_MS)
}

void main()
