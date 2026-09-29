/**
 * Runs on every http(s) page, and only reads it: nothing leaves the page unless the cook
 * clicks. On a recipe page (see detect.ts) it asks "Read this recipe in Crumb?"; on a Crumb
 * page, before any Crumb is set, it offers to use that one.
 *
 * When the cook clicks (the background asks), it reads the recipe on the page for Crumb (see
 * page.ts): only the recipe, never anything else. On YouTube it reads the video's page for
 * Crumb instead (see youtube.ts), and asks when a video says it's a recipe. On the Crumb tab
 * opened for a video or a recipe, it hands that page what was read.
 */
import { load, save, siteOf } from "./crumb"
import { isRecipeItemType, recipeInJsonLd, type Found } from "./detect"
import {
  send,
  type PageForCrumb,
  type ReadPage,
  type ReadVideo,
  type VideoForCrumb,
  type Waiting,
} from "./messages"
import { readRecipe } from "./page"
import { termsNote } from "./terms"
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
    // A site whose terms forbid automated fetching: say why Crumb reads it from here
    note: termsNote(location.hostname) ?? undefined,
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

/**
 * Answers the background's request for the recipe on this page, when the cook clicks. Only the
 * top frame, only the extension's own background, and only the recipe (see page.ts).
 */
function answerReadPage() {
  chrome.runtime.onMessage.addListener((message: ReadPage, sender, reply) => {
    if (message?.type !== "readPage" || sender.id !== chrome.runtime.id) return false
    // The toolbar button was clicked: on a site whose terms forbid automated fetching, say
    // why Crumb reads the recipe from this page (the prompt may not have been shown)
    const note = termsNote(location.hostname)
    if (note)
      showToast({
        title: "Reading this recipe for Crumb",
        note,
        timeoutMs: 6000,
        actions: [{ label: "OK", run: () => {} }],
      })
    reply(readRecipe(document))
    return false
  })
}

/**
 * On the Crumb tab opened for a video or a recipe: hands the page what was read of it. The
 * Add page takes a video (`crumb:video`), the preview page a recipe (`crumb:page`).
 */
async function handOver() {
  if (new URLSearchParams(location.search).get("via") !== "extension") return
  const until = Date.now() + VIDEO_WAIT_MS
  let waiting: Waiting | null = null
  while (Date.now() < until) {
    const got = await send<Waiting | null | boolean>({ type: "takeVideo" })
    if (!got || typeof got !== "object") return
    if (got.status !== "reading") {
      waiting = got
      break
    }
    await new Promise((r) => setTimeout(r, VIDEO_ASK_MS))
  }
  // Still reading after all that: the page is told it couldn't be read, not left waiting
  const url = waiting?.url ?? new URLSearchParams(location.search).get("url") ?? ""
  const isPage = (waiting?.kind ?? (location.pathname === "/preview" ? "page" : "video")) === "page"
  const message: VideoForCrumb | PageForCrumb = isPage
    ? { type: "crumb:page", url, page: waiting?.page ?? null }
    : {
        type: "crumb:video",
        url,
        video: waiting?.video ?? null,
        captions: waiting?.captions ?? "none",
      }
  const post = () => window.postMessage(message, location.origin)
  post()
  // The page may not be listening yet: it asks when it is
  const want = isPage ? "crumb:want-page" : "crumb:want-video"
  window.addEventListener("message", (e) => {
    if (e.source === window && e.origin === location.origin && e.data?.type === want) post()
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
    readVideo(id).then(
      (read) => {
        console.info("[Crumb] read the video", id, read ? `(captions: ${read.captions})` : "")
        reply(read)
      },
      (err) => {
        console.info("[Crumb] couldn't read the video", id, err)
        reply(null)
      },
    )
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
  // Tells a Crumb page the extension is here (web/src/lib/extension.ts), so it can nudge
  if (isCrumbApp())
    document.documentElement.dataset.crumbExtension = chrome.runtime.getManifest().version
  const settings = await load()
  if (/(^|\.)youtube\.com$/i.test(location.hostname)) return onYouTube(settings)
  if (!settings.crumb) {
    if (isCrumbApp()) offerConnect()
    // Until a Crumb is set, recipe pages still offer (and the click opens the settings)
  } else if (location.origin === settings.crumb) {
    return handOver()
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

if (window.top === window) answerReadPage()
void main()
