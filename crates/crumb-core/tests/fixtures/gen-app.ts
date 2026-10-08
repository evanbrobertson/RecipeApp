/**
 * Generates app.json, the parity fixtures for the app-screen helpers in crumb-core:
 * books.rs (cover colours, old shelf geometry), shelf.rs (the shelf's layout), prep.rs (bowl
 * colours) and add.rs (the video queue wording).
 *
 * Regenerate from the repo root with:
 *
 *   bun crates/crumb-core/tests/fixtures/gen-app.ts
 *
 * The TypeScript in web/src/lib is the source of truth: this script imports the real
 * functions and records their output. Do not edit the JSON by hand.
 */

import {
  BOOK_COLORS,
  bookColor,
  bookLean,
  bookPalette,
  bookSize,
  seeded,
  spineBands,
  stackBooks,
} from "../../../../web/src/lib/books.ts"
import { layoutShelf, spineFor, splitTitle, titleWidth } from "../../../../web/src/lib/shelf.ts"
import { ingredientColor } from "../../../../web/src/lib/ingredientColor.ts"
import { jobProgress } from "../../../../web/src/lib/importLink.ts"

type Entry = { input: unknown; output: unknown }
const out = {
  bookColor: [] as Entry[],
  bookPalette: [] as Entry[],
  seeded: [] as Entry[],
  bookSize: [] as Entry[],
  bookLean: [] as Entry[],
  spineBands: [] as Entry[],
  stackBooks: [] as Entry[],
  titleWidth: [] as Entry[],
  splitTitle: [] as Entry[],
  spineFor: [] as Entry[],
  layoutShelf: [] as Entry[],
  ingredientColor: [] as Entry[],
  jobProgress: [] as Entry[],
}
const add = (key: keyof typeof out, input: unknown, output: unknown) =>
  out[key].push({ input, output })

// ── Colours ────────────────────────────────────────────────────────────────
for (const c of [...BOOK_COLORS, "Forest", "tomato", "olive", "purple", "", null]) {
  add("bookColor", c, bookColor(c as string))
  add("bookPalette", c, bookPalette(c as string))
}

// ── Geometry ───────────────────────────────────────────────────────────────
for (const id of [1, 2, 3, 7, 42, 99, 1234, 98765]) {
  for (const salt of [0, 3, 7, 11]) add("seeded", { id, salt }, seeded(id, salt))
}
const NAMES = ["Weeknights", "Nan's", "Baking Bible of Sunday Afternoons", "Crème brûlée & co", "🍝"]
const books = [1, 2, 3, 7, 42, 99, 1234, 98765].flatMap((id, i) =>
  [0, 1, 9, 40].map((recipeCount, j) => ({
    id: id * 10 + j,
    name: NAMES[(i + j) % NAMES.length]!,
    color: BOOK_COLORS[(i + j) % BOOK_COLORS.length]!,
    recipeCount,
  })),
)
for (const b of books) {
  add("bookSize", b, bookSize(b))
  add("spineBands", b, spineBands(b) ?? null)
  for (const atFoot of [false, true]) add("bookLean", { book: b, atFoot }, bookLean(b, atFoot))
}
for (const n of [0, 1, 2, 5, 12, 32]) {
  for (const towers of [1, 2, 3, 4]) {
    const list = books.slice(0, n)
    const stacks = stackBooks(list, towers).map((t) => t.map((b) => list.indexOf(b)))
    add("stackBooks", { books: list, towers }, stacks)
  }
}

// ── Shelf layout ───────────────────────────────────────────────────────────
const TITLES = [
  "Weeknight dinners",
  "Christmas baking",
  "Soups",
  "Grandma's Sunday roasts and other family favourites",
  "Bread",
  "Things to make when it's raining",
  "Curries",
  "Summer salads",
  "Ottolenghi-ish vegetable mains",
  "Breakfast",
  "Pasta",
  "Recipes from the trip to Lisbon",
  "Cakes",
  "Pickles & preserves",
  "Meal prep for busy weeks",
  "Desserts",
  "Dinner party showstoppers",
  "Mum's",
  "Crème brûlée & co",
  "🍝",
  "  ",
  "Supercalifragilisticexpialidociouslylongsingleword",
  "A very long cookbook title that keeps going and going well past two lines",
  "\uFEFFSunday  roast\u0085pie",
  ...Array<string>(10).fill("Pasta"),
]
for (const t of TITLES) {
  add("titleWidth", { text: t, size: 15 }, titleWidth(t, 15))
  add("splitTitle", { text: t, size: 14 }, splitTitle(t, 14))
}
const shelfBooks = TITLES.map((name, i) => ({
  id: 101 + i * 7,
  name,
  color: BOOK_COLORS[i % BOOK_COLORS.length]!,
  recipeCount: [0, 3, 12, 24, 40][i % 5]!,
}))
for (const b of shelfBooks) {
  const { book: _, ...spine } = spineFor(b)
  add("spineFor", b, { id: b.id, ...spine })
}
/** The TypeScript shapes as crumb-core's `ShelfRow`/`ShelfItem` serialise them. */
function rowsAsCore(list: typeof shelfBooks, rows: ReturnType<typeof layoutShelf>) {
  return rows.map((row) => ({
    width: row.width,
    items: row.items.map((it) => {
      if (it.kind !== "book") {
        const { kind, x, y, w, h, z } = it
        return { kind, x, y, w, h, z, book: null, stack: it.stack ?? null, on: it.on ?? null }
      }
      const { kind, x, y, w, h, z, stack, book, tilt, pivot, top, standing, style, lines, font } = it
      const spine = { index: list.indexOf(book as never), id: book.id, standing, w, h, style, lines, font, tilt, pivot, top }
      return { kind, x, y, w, h, z, book: spine, stack, on: null }
    }),
  }))
}
// Cases name their books as a slice of the `spineFor` inputs, to keep the file small
for (const [start, n] of [[0, 0], [0, 1], [0, 3], [5, 8], [0, 14], [TITLES.length - 10, 10], [0, TITLES.length]] as const) {
  const list = shelfBooks.slice(start, start + n)
  for (const [width, addable] of [[320, true], [343, true], [375, true], [700, true], [1000, false], [1100, true]] as const) {
    const opts = { width, single: false, addable }
    add("layoutShelf", { start, n, ...opts }, rowsAsCore(list, layoutShelf(list, opts)))
  }
  const single = { width: 0, single: true, addable: false }
  add("layoutShelf", { start, n, ...single }, rowsAsCore(list, layoutShelf(list, single)))
}

// ── Prep ───────────────────────────────────────────────────────────────────
for (const name of [
  "plain flour",
  "Unsalted Butter",
  "olive oil",
  "red pepper",
  "black peppercorns",
  "peppers",
  "fresh basil",
  "dark chocolate",
  "sweet potato",
  "garlic",
  "blueberries",
  "ice water",
  "brown sugar",
  "soy sauce",
  "chilli flakes",
  "chili",
  "saffron",
  "",
]) {
  add("ingredientColor", name, ingredientColor(name))
}

// ── Add box ────────────────────────────────────────────────────────────────
for (const [status, position] of [
  ["queued", undefined],
  ["queued", 1],
  ["queued", 2],
  ["queued", 3],
  ["queued", 11],
  ["queued", 22],
  ["queued", 103],
  ["running", undefined],
] as const) {
  add("jobProgress", { status, position: position ?? null }, jobProgress({ status, position } as never))
}

const path = new URL("./app.json", import.meta.url)
await Bun.write(path, `${JSON.stringify(out, null, 2)}\n`)
console.log(`wrote ${path.pathname}`)
for (const [key, entries] of Object.entries(out)) console.log(`  ${key}: ${entries.length}`)
