/**
 * Generates app.json, the parity fixtures for the app-screen helpers in crumb-core:
 * books.rs (shelf geometry), prep.rs (bowl colours) and add.rs (the video queue wording).
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
