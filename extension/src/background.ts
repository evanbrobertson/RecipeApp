/**
 * Opens tabs for the content script and the toolbar button. Clicking the button reads the
 * current page in Crumb whether or not it looked like a recipe (the prompt may have been
 * closed, or the site muted); Crumb says so when there's no recipe on it.
 */
import { crumbOrigin, load, previewUrl, readable, save } from "./crumb"
import type { Message } from "./messages"

async function read(url: string | undefined, from?: chrome.tabs.Tab) {
  const { crumb } = await load()
  if (!crumb) return chrome.runtime.openOptionsPage()
  if (!readable(url, crumb)) return
  await chrome.tabs.create({
    url: previewUrl(crumb, url!),
    index: from ? from.index + 1 : undefined,
    openerTabId: from?.id,
  })
}

async function handle(message: Message, sender: chrome.runtime.MessageSender) {
  switch (message.type) {
    case "read":
      return read(message.url, sender.tab)
    case "connect": {
      // Only the page the cook is on can name itself, and only its own origin
      const origin = crumbOrigin(message.origin)
      const from = sender.tab?.url ? new URL(sender.tab.url).origin : null
      if (origin && origin === from) await save({ crumb: origin })
      return
    }
    case "settings":
      return chrome.runtime.openOptionsPage()
  }
}

chrome.runtime.onMessage.addListener((message: Message, sender, reply) => {
  handle(message, sender).then(
    () => reply(true),
    () => reply(false),
  )
  return true
})

chrome.action.onClicked.addListener((tab) => void read(tab.url, tab))

chrome.runtime.onInstalled.addListener(async ({ reason }) => {
  if (reason === "install" && !(await load()).crumb) await chrome.runtime.openOptionsPage()
})
