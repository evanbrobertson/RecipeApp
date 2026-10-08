/** Cover colours for cookbooks on the shelf (the server stores these six names). */
export const BOOK_COLORS = ["forest", "tile", "sage", "butter", "clay", "cream"] as const

export type BookColor = (typeof BOOK_COLORS)[number]

export interface BookLook {
  /** Cover cloth */
  cloth: string
  /** Darker cloth for the hidden faces, for depth */
  shade: string
  /** Title text on the cloth */
  foil: string
  /** Inset edge for covers that would vanish against a light page ("none" otherwise) */
  border: string
  /** Contrasting palette colours for the optional spine bands */
  bands: [string, string]
}

const FOREST = "#1C2B22"
const TILE = "#2F6B4F"
const SAGE = "#A9C4AE"
const BUTTER = "#F3DA8B"
const CLAY = "#A55A40"
const CREAM = "#F8F3E6"

/** The same in light and dark mode. */
export const BOOK_PALETTE: Record<BookColor, BookLook> = {
  forest: { cloth: FOREST, shade: "#111a15", foil: BUTTER, border: "none", bands: [BUTTER, SAGE] },
  tile: { cloth: TILE, shade: "#224f3a", foil: "#FFFDF8", border: "none", bands: [BUTTER, CREAM] },
  sage: { cloth: SAGE, shade: "#8aa890", foil: FOREST, border: "none", bands: [FOREST, TILE] },
  butter: { cloth: BUTTER, shade: "#d9bd67", foil: FOREST, border: "none", bands: [FOREST, CLAY] },
  clay: { cloth: CLAY, shade: "#7e412d", foil: "#FFF8EA", border: "none", bands: [BUTTER, CREAM] },
  cream: {
    cloth: CREAM,
    shade: "#ddd5c1",
    foil: FOREST,
    border: "inset 0 0 0 1px rgb(28 43 34 / 0.22)",
    bands: [TILE, CLAY],
  },
}

/** Names from the old ten-colour palette, in case one slips through. */
const LEGACY: Record<string, BookColor> = {
  tomato: "clay",
  terracotta: "clay",
  mustard: "butter",
  ocean: "tile",
  plum: "forest",
  navy: "forest",
  charcoal: "forest",
  rose: "cream",
}

/** Normalise any stored colour name to one of the six. */
export function bookColor(color: string | null | undefined): BookColor {
  const c = (color ?? "").toLowerCase()
  if ((BOOK_COLORS as readonly string[]).includes(c)) return c as BookColor
  return LEGACY[c] ?? "tile"
}

export function bookPalette(color: string | null | undefined): BookLook {
  return BOOK_PALETTE[bookColor(color)]
}

/** The cover edge as one entry of a `box-shadow` list, where "none" would void the whole list. */
export function edgeShadow(look: BookLook) {
  return look.border === "none" ? "0 0 #0000" : look.border
}

export function randomBookColor(): BookColor {
  return BOOK_COLORS[Math.floor(Math.random() * BOOK_COLORS.length)]!
}

/** Deterministic pseudo-random number in [0, 1) from an id, so books keep their size. */
export function seeded(id: number, salt = 0) {
  const x = Math.sin(id * 9301 + salt * 49297) * 233280
  return x - Math.floor(x)
}

export interface ShelfBook {
  id: number
  name: string
  description?: string | null
  color: string | null
  recipeCount: number
}
