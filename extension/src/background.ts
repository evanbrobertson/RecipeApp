/**
 * Opens tabs for the content script and the toolbar button. Clicking the button reads the
 * current page in Crumb whether or not it looked like a recipe (the prompt may have been
 * closed, or the site muted); Crumb says so when there's no recipe on it.
 *
 * A YouTube video goes to Crumb's Add page instead, with what this browser read of it (see
 * youtube.ts): held in session storage for that one tab until its content script takes it.
 */
import { addVideoUrl, crumbOrigin, load, previewUrl, readable, save } from "./crumb"
import type { Message, ReadVideo, Waiting } from "./messages"
import { videoId, type VideoRead } from "./youtube"

const waitingKey = (tabId: number) => `video:${tabId}`

/** Asks a YouTube tab's content script to read its video; null if it can't. */
async function readInTab(tab: chrome.tabs.Tab | undefined): Promise<VideoRead | null> {
  if (tab?.id === undefined) return null
  try {
    const ask: ReadVideo = { type: "readVideo" }
    return ((await chrome.tabs.sendMessage(tab.id, ask)) as VideoRead | null) ?? null
  } catch {
    // No content script in that tab (opened before the extension was installed)
    return null
  }
}

async function read(url: string | undefined, from?: chrome.tabs.Tab, video?: VideoRead) {
  const { crumb } = await load()
  if (!crumb) return chrome.runtime.openOptionsPage()
  if (!readable(url, crumb)) return
  const where = { index: from ? from.index + 1 : undefined, openerTabId: from?.id }
  if (videoId(url)) {
    // The tab opens at once; the video is read meanwhile (a second or two) and handed over
    const tab = await chrome.tabs.create({ url: addVideoUrl(crumb, url!), ...where })
    if (tab.id === undefined) return
    const key = waitingKey(tab.id)
    const reading: Waiting = { crumb, url: url!, video: null }
    await chrome.storage.session.set({ [key]: reading })
    const got = video ?? (await readInTab(from))
    if (got) await chrome.storage.session.set({ [key]: { ...reading, video: got } })
    else await chrome.storage.session.remove(key)
    return
  }
  await chrome.tabs.create({ url: previewUrl(crumb, url!), ...where })
}

/**
 * Hands the Crumb tab opened for a video what was read of it, once. `video: null` means it's
 * still being read (ask again); null means there's nothing for this tab.
 */
async function takeVideo(sender: chrome.runtime.MessageSender): Promise<Waiting | null> {
  const id = sender.tab?.id
  if (id === undefined || !sender.tab?.url) return null
  const key = waitingKey(id)
  const waiting = (await chrome.storage.session.get(key))[key] as Waiting | undefined
  // Only that tab, and only while it's still on that Crumb
  if (!waiting || new URL(sender.tab.url).origin !== waiting.crumb) return null
  if (waiting.video) await chrome.storage.session.remove(key)
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
      if (origin && origin === from) await save({ crumb: origin })
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
