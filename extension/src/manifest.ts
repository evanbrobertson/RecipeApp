/**
 * The manifest, per browser: Chrome (and Edge) run the background as a service worker,
 * Firefox as an event page, and Firefox needs its add-on id and data declaration.
 */

export type Target = "chrome" | "firefox"
export const TARGETS: readonly Target[] = ["chrome", "firefox"]

/** Firefox's id for the add-on; changing it makes it a different add-on on AMO. */
export const GECKO_ID = "crumb@evanbrobertson.github.io"

/** Browser extension versions: one to four dot-separated numbers, no pre-release tags. */
export function validVersion(version: string): boolean {
  return /^(0|[1-9]\d{0,8})(\.(0|[1-9]\d{0,8})){0,3}$/.test(version)
}

export function manifest(target: Target, version: string): Record<string, unknown> {
  if (!validVersion(version)) throw new Error(`Not an extension version: ${version}`)
  const icons = {
    16: "icons/icon-16.png",
    32: "icons/icon-32.png",
    48: "icons/icon-48.png",
    128: "icons/icon-128.png",
  }
  return {
    manifest_version: 3,
    name: "Crumb",
    description: "Read any recipe in your Crumb recipe box, then add it in one click.",
    version,
    homepage_url: "https://github.com/evanbrobertson/RecipeApp",
    icons,
    action: {
      default_title: "Read this page in Crumb",
      default_icon: { 16: icons[16], 32: icons[32] },
    },
    // storage: the Crumb's address; activeTab: the page's address when the button's clicked;
    // scripting: on that click, reading a YouTube tab opened before the extension was updated
    permissions: ["storage", "activeTab", "scripting"],
    background:
      target === "chrome" ? { service_worker: "background.js" } : { scripts: ["background.js"] },
    // Every page, to notice recipes as they open (read in the page, nothing sent)
    content_scripts: [
      {
        matches: ["http://*/*", "https://*/*"],
        js: ["content.js"],
        run_at: "document_idle",
      },
    ],
    options_ui: { page: "options.html", open_in_tab: true },
    ...(target === "firefox" && {
      browser_specific_settings: {
        gecko: {
          id: GECKO_ID,
          strict_min_version: "140.0",
          // Nothing goes to the developer or anyone else: only to the cook's own Crumb
          data_collection_permissions: { required: ["none"] },
        },
        gecko_android: { strict_min_version: "142.0" },
      },
    }),
  }
}
