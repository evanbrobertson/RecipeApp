/**
 * Runs on every http(s) page, and only reads it: nothing leaves the page unless the cook
 * clicks. On a recipe page (see detect.ts) it asks "Read this recipe in Crumb?"; on a Crumb
 * page, before any Crumb is set, it offers to use that one.
 *
 * On YouTube it reads the video's page for Crumb (see youtube.ts), and asks when a video says
 * it's a recipe. On the Crumb tab opened for a video, it hands that page what was read.
 */
import { load, save, siteOf } from "./crumb"
import { isRecipeItemType, recipeInJsonLd, type Found } from "./detect"
import { send, type ReadVideo, type VideoForCrumb, type Waiting } from "./messages"
import { showToast } from "./toast"
import { looksLikeRecipe, peekVideo, readVideo, videoId } from "./youtube"

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

function offerRead(found: Found, title = "Read this recipe in Crumb?") {
  const site = siteOf(location.hostname)
  showToast({
    title,
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

/** How long the Crumb tab waits for the video to be read, and how often it asks. */
const VIDEO_WAIT_MS = 20_000
const VIDEO_ASK_MS = 400

/** On the Crumb tab opened for a video: hands the Add page what was read of it. */
async function handOverVideo() {
  if (new URLSearchParams(location.search).get("via") !== "extension") return
  const until = Date.now() + VIDEO_WAIT_MS
  let waiting: Waiting | null = null
  while (Date.now() < until) {
    const got = await send<Waiting | null | boolean>({ type: "takeVideo" })
    if (!got || typeof got !== "object") return
    if (got.video) {
      waiting = got
      break
    }
    await new Promise((r) => setTimeout(r, VIDEO_ASK_MS))
  }
  if (!waiting?.video) return
  const message: VideoForCrumb = { type: "crumb:video", url: waiting.url, video: waiting.video }
  const post = () => window.postMessage(message, location.origin)
  post()
  // The page may not be listening yet: it asks when it is
  window.addEventListener("message", (e) => {
    if (e.source === window && e.origin === location.origin && e.data?.type === "crumb:want-video")
      post()
  })
}

/** On YouTube: reads the video when the toolbar button asks, and offers on recipe videos. */
function onYouTube(settings: Awaited<ReturnType<typeof load>>) {
  chrome.runtime.onMessage.addListener((message: ReadVideo, _sender, reply) => {
    if (message?.type !== "readVideo") return false
    const id = videoId(location.href)
    if (!id) {
      reply(null)
      return false
    }
    readVideo(id).then(reply, () => reply(null))
    return true
  })
  if (!settings.prompt || settings.muted.includes(siteOf(location.hostname))) return
  let offered = ""
  const look = async () => {
    const id = videoId(location.href)
    if (!id || id === offered) return
    const video = await peekVideo(id).catch(() => null)
    // Still on that video, and it says it's a recipe
    if (!video || videoId(location.href) !== id || !looksLikeRecipe(video)) return
    offered = id
    offerRead({ name: video.title ?? "" }, "Read this recipe video in Crumb?")
  }
  void look()
  // YouTube changes videos without loading a new page
  document.addEventListener("yt-navigate-finish", () => void look())
}

async function main() {
  if (window.top !== window) return
  const settings = await load()
  if (/(^|\.)youtube\.com$/i.test(location.hostname)) return onYouTube(settings)
  if (!settings.crumb) {
    if (isCrumbApp()) offerConnect()
    // Until a Crumb is set, recipe pages still offer (and the click opens the settings)
  } else if (location.origin === settings.crumb) {
    return handOverVideo()
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
