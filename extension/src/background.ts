/**
 * Opens tabs for the content script and the toolbar button. Clicking the button reads the
 * current page in Crumb whether or not it looked like a recipe (the prompt may have been
 * closed, or the site muted); Crumb says so when there's no recipe on it.
 *
 * A YouTube video goes to Crumb's Add page instead, with what this browser read of it (see
 * youtube.ts). A recipe page goes to Crumb's preview with the recipe this browser read from it
 * (see page.ts), because the site may turn Crumb's server away but not the cook. Either way it's
 * held in session storage for that one tab until the Crumb page's content script takes it.
 */
import { addVideoUrl, crumbOrigin, load, mayOfferItself, previewUrl, readable, save } from "./crumb"
import type { Message, ReadPage, ReadVideo, Waiting } from "./messages"
import type { PageReading } from "./page"
import { videoId, type VideoReading } from "./youtube"

const waitingKey = (tabId: number) => `video:${tabId}`

async function askTab<T>(tabId: number, ask: ReadVideo | ReadPage): Promise<T | null> {
  return ((await chrome.tabs.sendMessage(tabId, ask)) as T | null) ?? null
}

/**
 * Asks a tab's content script to read what's on it (a YouTube video, or a recipe); null if
 * it can't. A tab opened before the extension was installed or updated has no live content
 * script, so it's added (the toolbar click grants that tab) and asked again.
 */
async function readInTab<T>(
  tab: chrome.tabs.Tab | undefined,
  ask: ReadVideo | ReadPage,
): Promise<T | null> {
  if (tab?.id === undefined) return null
  try {
    return await askTab<T>(tab.id, ask)
  } catch {
    try {
      await chrome.scripting.executeScript({ target: { tabId: tab.id }, files: ["content.js"] })
      return await askTab<T>(tab.id, ask)
    } catch (err) {
      console.info("[Crumb] couldn't read the page in its tab", err)
      return null
    }
  }
}

async function read(url: string | undefined, from?: chrome.tabs.Tab, video?: VideoReading) {
  const { crumb } = await load()
  if (!crumb) return chrome.runtime.openOptionsPage()
  if (!readable(url, crumb)) return
  const where = { index: from ? from.index + 1 : undefined, openerTabId: from?.id }
  if (videoId(url)) {
    // The tab opens at once; the video is read meanwhile (a second or two) and handed over
    const tab = await chrome.tabs.create({ url: addVideoUrl(crumb, url!), ...where })
    if (tab.id === undefined) return
    const key = waitingKey(tab.id)
    const reading: Waiting = {
      crumb,
      url: url!,
      kind: "video",
      status: "reading",
      video: null,
      captions: "none",
      page: null,
    }
    await chrome.storage.session.set({ [key]: reading })
    const got = video ?? (await readInTab<VideoReading>(from, { type: "readVideo" }))
    const done: Waiting = got
      ? { ...reading, status: "read", video: got.video, captions: got.captions }
      : { ...reading, status: "failed" }
    await chrome.storage.session.set({ [key]: done })
    return
  }
  // A recipe page: the recipe as this browser reads it goes to Crumb with the link, since the
  // site may turn Crumb's server away but not the cook. Nothing else of the page is read.
  const tab = await chrome.tabs.create({ url: previewUrl(crumb, url!, true), ...where })
  if (tab.id === undefined) return
  const key = waitingKey(tab.id)
  const reading: Waiting = {
    crumb,
    url: url!,
    kind: "page",
    status: "reading",
    video: null,
    captions: "none",
    page: null,
  }
  await chrome.storage.session.set({ [key]: reading })
  const got = await readInTab<PageReading>(from, { type: "readPage" })
  const done: Waiting = got
    ? { ...reading, status: "read", page: got }
    : { ...reading, status: "failed" }
  await chrome.storage.session.set({ [key]: done })
}

/**
 * Hands the Crumb tab opened for a video or a recipe what was read of it, once it's read (or failed).
 * Status `reading` means ask again; null means there's nothing for this tab.
 */
async function takeVideo(sender: chrome.runtime.MessageSender): Promise<Waiting | null> {
  const id = sender.tab?.id
  if (id === undefined || !sender.tab?.url) return null
  const key = waitingKey(id)
  const waiting = (await chrome.storage.session.get(key))[key] as Waiting | undefined
  // Only that tab, and only while it's still on that Crumb
  if (!waiting || new URL(sender.tab.url).origin !== waiting.crumb) return null
  if (waiting.status !== "reading") await chrome.storage.session.remove(key)
  return waiting
}

async function handle(message: Message, sender: chrome.runtime.MessageSender): Promise<unknown> {
  switch (message.type) {
    case "read":
      return read(message.url, sender.tab, message.video)
    case "connect": {
      // Only the page the cook is on can name itself, and only its own origin
      const origin = crumbOrigin(message.origin)
      const from = sender.tab?.url ? new URL(sender.tab.url).origin : null
      // Never replaces a Crumb that's already set: changing it is for the settings page
      if (origin && origin === from && mayOfferItself(origin) && !(await load()).crumb) {
        await save({ crumb: origin })
      }
      return
    }
    case "settings":
      return chrome.runtime.openOptionsPage()
    case "takeVideo":
      return takeVideo(sender)
  }
}

chrome.runtime.onMessage.addListener((message: Message, sender, reply) => {
  handle(message, sender).then(
    (result) => reply(result === undefined ? true : result),
    () => reply(false),
  )
  return true
})

chrome.action.onClicked.addListener((tab) => void read(tab.url, tab))

chrome.tabs.onRemoved.addListener((tabId) => void chrome.storage.session.remove(waitingKey(tabId)))

chrome.runtime.onInstalled.addListener(async ({ reason }) => {
  if (reason === "install" && !(await load()).crumb) await chrome.runtime.openOptionsPage()
})
