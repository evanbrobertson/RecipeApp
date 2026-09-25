import type { BookColor } from "#shared/utils/recipe"

/** Cloth, darker shade (for depth) and foil colour per cookbook colour. */
export const BOOK_PALETTE: Record<BookColor, { cloth: string; shade: string; foil: string }> = {
  tomato: { cloth: "#c8442c", shade: "#8f2c1b", foil: "#f6d38b" },
  sage: { cloth: "#7d9471", shade: "#56684c", foil: "#f3ecd2" },
  mustard: { cloth: "#d4a233", shade: "#9b7219", foil: "#3b2a12" },
  plum: { cloth: "#7b4a6b", shade: "#52304a", foil: "#f1cfa0" },
  ocean: { cloth: "#2f6f8f", shade: "#1e4a60", foil: "#f3dca5" },
  terracotta: { cloth: "#b8643f", shade: "#84432a", foil: "#fbe6c4" },
  forest: { cloth: "#2f5d46", shade: "#1d3d2d", foil: "#e9cf86" },
  navy: { cloth: "#2b3a5c", shade: "#1a2440", foil: "#e8c979" },
  rose: { cloth: "#c77d8a", shade: "#955562", foil: "#fff1e0" },
  charcoal: { cloth: "#3b3a38", shade: "#222120", foil: "#d9b86c" },
}

export function bookPalette(color: string | null | undefined) {
  return BOOK_PALETTE[(color as BookColor) ?? "tomato"] ?? BOOK_PALETTE.tomato
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

/** Spine size: thicker books hold more recipes; heights vary a little like a real shelf. */
export function bookSize(book: ShelfBook) {
  const width = Math.round(Math.min(74, 34 + book.recipeCount * 2.2))
  const height = Math.round(178 + seeded(book.id) * 52)
  return { width, height, depth: 132 }
}
