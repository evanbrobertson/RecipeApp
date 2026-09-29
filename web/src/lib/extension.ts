/**
 * The Crumb browser extension, as the site sees it: where to get it, and whether this browser
 * has it. Every "get the extension" link reads `extensionUrl()`.
 */

/**
 * Where to get the extension. Until it's in the browser stores this is its releases page;
 * the Chrome Web Store, Edge Add-ons and Firefox Add-ons links go here (pick by browser).
 */
export function extensionUrl(): string {
  return "https://github.com/evanbrobertson/RecipeApp/releases?q=extension"
}

/** The extension is installed in this browser: its content script marks Crumb pages. */
export function extensionInstalled(): boolean {
  return typeof document !== "undefined" && !!document.documentElement.dataset.crumbExtension
}

/**
 * The extension can be had here: it's installed, or this isn't a phone or tablet (touch and
 * no hover), where browsers have none. Firefox for Android has it, and then it's installed.
 */
export function extensionPossible(): boolean {
  if (extensionInstalled()) return true
  return !matchMedia("(pointer: coarse) and (hover: none)").matches
}
