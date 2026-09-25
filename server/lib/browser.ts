import { existsSync } from "node:fs"
import type { Browser } from "playwright-core"

/**
 * Headless Chromium fallback for sites that block plain HTTP fetches.
 * Chromium is installed in the Docker image; locally set CHROMIUM_PATH or it's skipped.
 * Only one page loads at a time to keep memory in check on a small Railway instance.
 */

const CANDIDATES = [
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
  "/usr/bin/google-chrome",
  "/opt/pw-browsers/chromium",
]

const USER_AGENT =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"

let executable: string | null | undefined
let queue: Promise<unknown> = Promise.resolve()

export function browserExecutable(): string | null {
  if (process.env.BROWSER_SCRAPING === "off") return null
  if (executable !== undefined) return executable
  const configured = process.env.CHROMIUM_PATH
  executable =
    configured && existsSync(configured) ? configured : (CANDIDATES.find(existsSync) ?? null)
  return executable
}

export function browserAvailable(): boolean {
  return browserExecutable() !== null
}

/** Serializes browser work so two imports never spawn two Chromiums. */
function exclusive<T>(task: () => Promise<T>): Promise<T> {
  const run = queue.then(task, task)
  queue = run.catch(() => {})
  return run
}

// Runs in the page (a string so server code doesn't need DOM types)
const RECIPE_READY = `[...document.querySelectorAll('script[type="application/ld+json"]')]
  .some((s) => /Recipe/.test(s.textContent || ""))
  || !!document.querySelector('[itemprop="recipeIngredient"], [class*="ingredient"]')`

const CHALLENGE_TITLE = /just a moment|attention required|access denied|verify you are human|robot/i

/**
 * Loads a page in real Chromium and returns the rendered HTML (after scripts run),
 * or throws if Chromium isn't available or the page can't be loaded.
 */
export function fetchWithBrowser(url: string, timeoutMs = 40_000): Promise<string> {
  const path = browserExecutable()
  if (!path) return Promise.reject(new Error("Chromium is not installed"))

  return exclusive(async () => {
    const { chromium } = await import("playwright-core")
    let browser: Browser | null = null
    try {
      browser = await chromium.launch({
        executablePath: path,
        headless: true,
        timeout: 20_000,
        args: [
          "--no-sandbox",
          "--disable-dev-shm-usage",
          "--disable-gpu",
          "--disable-blink-features=AutomationControlled",
        ],
      })
      const context = await browser.newContext({
        userAgent: USER_AGENT,
        locale: "en-US",
        viewport: { width: 1366, height: 900 },
        extraHTTPHeaders: { "Accept-Language": "en-US,en;q=0.9" },
      })
      // Look less like automation
      await context.addInitScript(() => {
        Object.defineProperty(navigator, "webdriver", { get: () => undefined })
      })
      const page = await context.newPage()
      // Skip heavy assets: we only need the DOM
      await page.route("**/*", (route) => {
        const type = route.request().resourceType()
        if (type === "image" || type === "media" || type === "font") return route.abort()
        return route.continue()
      })

      const deadline = Date.now() + timeoutMs
      const response = await page.goto(url, { waitUntil: "domcontentloaded", timeout: timeoutMs })

      // Bot challenges usually redirect or reload once solved; give them a moment
      for (let i = 0; i < 8 && CHALLENGE_TITLE.test(await page.title()); i++) {
        await page.waitForTimeout(1500)
      }

      // Wait briefly for client-rendered recipe data (JSON-LD or ingredient lists)
      const remaining = Math.max(1000, Math.min(8000, deadline - Date.now()))
      await page.waitForFunction(RECIPE_READY, undefined, { timeout: remaining }).catch(() => {})

      const status = response?.status() ?? 0
      if (status >= 400 && CHALLENGE_TITLE.test(await page.title())) {
        throw new Error(`Blocked by the site (${status})`)
      }
      return await page.content()
    } finally {
      await browser?.close().catch(() => {})
    }
  })
}
