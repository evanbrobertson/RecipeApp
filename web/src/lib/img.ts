/**
 * Sized recipe photos. The server resizes a recipe's stored image (a third-party URL
 * or an embedded data: URI) to a fixed width at `/img/{recipeId}/{width}?v={key}`.
 * `key` is a hash of the image string, so the response can be cached forever and a
 * new image gets a new URL. The same hash is computed in `src/images.rs`.
 *
 * If the resize fails the endpoint 404s; `<img onerror>` then falls back to the
 * original URL (see `fallbackSrc`).
 */

/** Widths the server will produce. Anything else is snapped to the nearest one. */
export const IMG_WIDTHS = [160, 320, 480, 768, 1200] as const

/** FNV-1a 32-bit over the image string's UTF-8 bytes, as 8 hex digits. */
export function imageKey(image: string): string {
  let h = 0x811c9dc5
  for (const byte of new TextEncoder().encode(image)) {
    h ^= byte
    h = Math.imul(h, 0x01000193) >>> 0
  }
  return h.toString(16).padStart(8, "0")
}

export interface Sized {
  src: string
  srcset: string
  /** The original image, for `onerror`. */
  fallback: string
}

/**
 * `src` and `srcset` for a recipe photo shown at about `width` CSS pixels.
 * `sizes` is left to the caller, since only it knows the layout.
 */
export function sized(recipe: { id: number; image?: string | null }, width: number): Sized | null {
  const image = recipe.image
  if (!image) return null
  const v = imageKey(image)
  const url = (w: number) => `/img/${recipe.id}/${w}?v=${v}`
  // 1x and 2x of the slot, from the widths the server makes
  const pick = (target: number) => IMG_WIDTHS.find((w) => w >= target) ?? IMG_WIDTHS.at(-1)!
  const one = pick(width)
  const two = pick(width * 2)
  const set = [...new Set([one, two])]
  return {
    src: url(one),
    srcset: set.map((w) => `${url(w)} ${w}w`).join(", "),
    // An embedded photo the resizer can't read (SVG, AVIF…) still shows as itself
    fallback: image,
  }
}

/** `onerror` handler: swap to the original URL once, then give up (the caller shows a placeholder). */
export function fallbackSrc(img: HTMLImageElement, fallback: string): boolean {
  if (fallback && img.dataset.fellBack !== "1") {
    img.dataset.fellBack = "1"
    img.removeAttribute("srcset")
    img.src = fallback
    return true
  }
  return false
}
