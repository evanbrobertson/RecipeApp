/** What the content script asks the background for (it can't open tabs itself). */
export type Message =
  /** Read `url` in Crumb, in a new tab beside this one. */
  | { type: "read"; url: string }
  /** Use the Crumb at `origin` (the page the cook is on). */
  | { type: "connect"; origin: string }
  /** Open the extension's settings. */
  | { type: "settings" }

export function send(message: Message): Promise<unknown> {
  return chrome.runtime.sendMessage(message)
}
