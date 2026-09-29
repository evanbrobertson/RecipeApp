/**
 * Sites that turn Crumb's server away (a bot check) don't turn the cook's browser away, so
 * the extension reads the recipe from the page here and hands it to Crumb with the link.
 *
 * Only the recipe is read, and only from the DOM: the page's JSON-LD blocks that hold a
 * Recipe or, if none does, the recipe card's HTML (scripts, forms and every attribute the
 * parser doesn't need stripped), plus the page's `og:image` and `og:title`. Never cookies,
 * storage, headers or the rest of the page. What is read is capped at [`MAX_READING_BYTES`];
 * if the recipe doesn't fit, nothing is sent.
 *
 * A pure function over a `Document`, so it's tested without a browser; the content script
 * passes it the page's own document when the cook clicks.
 */
import { recipeInJsonLd } from "./detect"

/** What Crumb's import takes as `page` (src/scraper/page.rs `FromPage`). */
export interface PageReading {
  /** The text of each JSON-LD script that holds a Recipe. */
  jsonLd: string[]
  /** The recipe card's HTML, only when no JSON-LD held the recipe. */
  card?: string
  image?: string
  title?: string
}

/** The most that is read (bytes of text); Crumb refuses more. */
export const MAX_READING_BYTES = 512 * 1024
const MAX_SCRIPTS = 32
const MAX_TITLE = 500
const MAX_IMAGE = 2_000

/** The recipe-card plugins' containers, in the order they're tried. */
export const CARD_SELECTORS = [
  ".wprm-recipe-container",
  ".tasty-recipes",
  ".mv-create-card",
  "#zlrecipe-container",
  ".wp-block-wpzoom-recipe-card-block-recipe-card",
  '[itemtype*="schema.org/Recipe" i]',
] as const

/** Elements dropped from a card: nothing in them is recipe, and forms and scripts can hold tokens. */
const DROPPED =
  "script, style, noscript, iframe, object, embed, form, input, textarea, select, button"

/** The attributes Crumb's parser reads (src/scraper.rs); every other one is stripped. */
const KEPT_ATTRIBUTES = new Set([
  "class",
  "id",
  "itemprop",
  "itemscope",
  "itemtype",
  "content",
  "datetime",
  "href",
  "src",
  "data-src",
  "data-lazy-src",
  "alt",
  "type",
  "data-id",
  "videoid",
  "style",
])

const encoder = new TextEncoder()
const bytes = (text: string) => encoder.encode(text).length

/** The card, cleaned: a copy of the container without scripts, forms or unneeded attributes. */
export function cardHtml(container: Element): string {
  const copy = container.cloneNode(true) as Element
  for (const el of Array.from(copy.querySelectorAll(DROPPED))) el.remove()
  for (const el of [copy, ...Array.from(copy.querySelectorAll("*"))]) {
    for (const name of el.getAttributeNames()) {
      if (!KEPT_ATTRIBUTES.has(name)) el.removeAttribute(name)
    }
  }
  return copy.outerHTML
}

function meta(doc: Document, property: string): string | undefined {
  const content = doc.querySelector(`meta[property="${property}" i]`)?.getAttribute("content")
  return content?.trim() || undefined
}

/**
 * The recipe on this page for Crumb, or null when there is none (or it doesn't fit under the
 * cap): then nothing is sent and Crumb reads the link itself.
 */
export function readRecipe(doc: Document): PageReading | null {
  const jsonLd: string[] = []
  let size = 0
  const room = (text: string) => size + bytes(text) <= MAX_READING_BYTES

  const image = meta(doc, "og:image")
  const title = meta(doc, "og:title")
  const safeImage =
    image && /^https?:\/\//i.test(image) && image.length <= MAX_IMAGE ? image : undefined
  const safeTitle = title?.slice(0, MAX_TITLE)
  size += bytes(safeImage ?? "") + bytes(safeTitle ?? "")

  for (const script of Array.from(doc.querySelectorAll('script[type="application/ld+json" i]'))) {
    const text = script.textContent ?? ""
    if (jsonLd.length >= MAX_SCRIPTS || !recipeInJsonLd([text])) continue
    if (!room(text)) continue
    jsonLd.push(text)
    size += bytes(text)
  }

  let card: string | undefined
  if (jsonLd.length === 0) {
    for (const selector of CARD_SELECTORS) {
      // Some sites put the microdata type on the whole page: that isn't a card
      const container = Array.from(doc.querySelectorAll(selector)).find(
        (el) => !["HTML", "BODY", "MAIN"].includes(el.tagName.toUpperCase()),
      )
      if (!container) continue
      const html = cardHtml(container)
      if (room(html)) {
        card = html
        break
      }
    }
    if (card === undefined) return null
  }

  const reading: PageReading = { jsonLd }
  if (card !== undefined) reading.card = card
  if (safeImage) reading.image = safeImage
  if (safeTitle) reading.title = safeTitle
  return reading
}
