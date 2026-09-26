/**
 * Generates format.json, the parity fixtures for crates/crumb-core/src/format.rs
 * (and the display helpers already in crumb-core: source::is_share_link,
 * source::source_url and model::count_items).
 *
 * Regenerate from the repo root with:
 *
 *   bun crates/crumb-core/tests/fixtures/gen-format.ts
 *
 * The TypeScript in web/src/lib/format.ts and web/src/lib/recipe.ts is the source
 * of truth: this script imports the real functions and records their output, so the
 * Rust port can be checked against them. Do not edit the JSON by hand.
 */

import {
  bookImportedTitle,
  bookToText,
  isShareLink,
  publicSource,
  recipeToMarkdown,
  recipeToText,
} from "../../../../web/src/lib/format.ts"
import { countItems, hostOf, kicker, webLink } from "../../../../web/src/lib/recipe.ts"

type Entry = { input: unknown; output: unknown }

const out = {
  isShareLink: [] as Entry[],
  publicSource: [] as Entry[],
  recipeToMarkdown: [] as Entry[],
  recipeToText: [] as Entry[],
  bookToText: [] as Entry[],
  bookImportedTitle: [] as Entry[],
  countItems: [] as Entry[],
  kicker: [] as Entry[],
  webLink: [] as Entry[],
  hostOf: [] as Entry[],
}

function add(key: keyof typeof out, input: unknown, output: unknown) {
  out[key].push({ input, output })
}

// ── isShareLink ────────────────────────────────────────────────────────────
const SHARE = "https://crumb.example/s/abcdefghijklmnop"
const SHARE_OTHER = "https://other.example/s/abcdefghijklmnop/42"
for (const input of [
  SHARE,
  `${SHARE}/42`,
  `${SHARE}/`,
  "https://crumb.example/s/abcdefghijklmno", // 15-char token, not a share
  "https://crumb.example/s/abcdefghijklmnopq/7", // 17-char token, still a share
  "https://crumb.example/x/s/abcdefghijklmnop", // /s/ not at the root
  `${SHARE}?box=1`,
  "https://crumb.example/s/abcdefghijklmnop/42/?x=1",
  "https://crumb.example/recipes/1",
  "not a url",
  "https://",
  "",
  null,
] as unknown[]) {
  add("isShareLink", input, isShareLink(input as string))
}

// ── publicSource ───────────────────────────────────────────────────────────
const NORMAL = "https://origin.example/recipe"
for (const originalUrl of [null, SHARE, NORMAL]) {
  for (const url of [null, SHARE, NORMAL]) {
    const r = { url, originalUrl }
    add("publicSource", r, publicSource(r))
  }
}

// ── recipeToMarkdown / recipeToText ─────────────────────────────────────────
interface RecipeLike {
  title: string
  description?: string | null
  url?: string | null
  originalUrl?: string | null
  author?: string | null
  prepTime?: string | null
  cookTime?: string | null
  freezeTime?: string | null
  totalTime?: string | null
  recipeYield?: string | null
  recipeCategory?: string | null
  recipeCuisine?: string | null
  ingredients: { name: string | null; items: string[] }[]
  instructions: { name: string | null; items: string[] }[]
  notes?: string | null
}

function recipe(over: Partial<RecipeLike> & { title: string }): RecipeLike {
  return {
    title: over.title,
    description: null,
    url: null,
    originalUrl: null,
    author: null,
    prepTime: null,
    cookTime: null,
    freezeTime: null,
    totalTime: null,
    recipeYield: null,
    recipeCategory: null,
    recipeCuisine: null,
    ingredients: [],
    instructions: [],
    notes: null,
    ...over,
  }
}

const minimal = recipe({
  title: "Toast",
  ingredients: [{ name: null, items: ["1 slice bread"] }],
})

const full = recipe({
  title: "Roast Chicken",
  description: "A weeknight roast.",
  url: NORMAL,
  author: "Evan",
  prepTime: "10 min",
  cookTime: "1 hr",
  freezeTime: "3 months",
  totalTime: "1 hr 10 min",
  recipeYield: "4 servings",
  recipeCategory: "Dinner",
  recipeCuisine: "British",
  ingredients: [
    { name: null, items: ["1 chicken", "2 tbsp oil"] },
    { name: "Stuffing", items: ["1 lemon", "2 sprigs thyme"] },
  ],
  instructions: [
    { name: null, items: ["Heat the oven.", "Roast the chicken."] },
    { name: "Rest", items: ["Rest for 15 minutes."] },
  ],
  notes: "Save the fat.",
})

const emptySections = recipe({
  title: "Empty",
  ingredients: [
    { name: null, items: [] },
    { name: "Sauce", items: [] },
  ],
  instructions: [{ name: null, items: [] }],
})

const shareOnly = recipe({
  title: "Shared",
  url: SHARE,
  ingredients: [{ name: null, items: ["1 egg"] }],
})

const sourcedFromShare = recipe({
  title: "Shared from somewhere",
  url: SHARE,
  originalUrl: NORMAL,
  ingredients: [{ name: null, items: ["1 egg"] }],
})

const shareAsOriginal = recipe({
  title: "Odd one",
  url: NORMAL,
  originalUrl: SHARE,
  ingredients: [{ name: null, items: ["1 egg"] }],
})

const unicode = recipe({
  title: "Crème brûlée 🍮",
  description: "Dessert",
  ingredients: [{ name: "Custard", items: ["500 ml double cream", "1 vanilla pod"] }],
  instructions: [{ name: null, items: ["Heat the cream.", "Whisk the yolks."] }],
})

const blankLines = recipe({
  title: "Blank lines",
  ingredients: [{ name: null, items: ["2 eggs"] }],
  instructions: [
    { name: null, items: ["Beat the eggs.\n\nAdd salt.", "Fold in the flour.\n"] },
  ],
})

const notesOnly = recipe({
  title: "Plain",
  notes: "Made this twice.",
  ingredients: [{ name: null, items: ["1 cup rice"] }],
})

const blankNames = recipe({
  title: "Blank names",
  ingredients: [{ name: "", items: ["1 tsp salt"] }],
  instructions: [{ name: "", items: ["Season."] }],
})

const RECIPES: [string, RecipeLike][] = [
  ["minimal", minimal],
  ["full", full],
  ["emptySections", emptySections],
  ["shareOnly", shareOnly],
  ["sourcedFromShare", sourcedFromShare],
  ["shareAsOriginal", shareAsOriginal],
  ["unicode", unicode],
  ["blankLines", blankLines],
  ["notesOnly", notesOnly],
  ["blankNames", blankNames],
]

for (const [name, r] of RECIPES) {
  add("recipeToMarkdown", { name, recipe: r }, recipeToMarkdown(r))
}

const TEXT_LINKS: (string | null)[] = [null, SHARE]
for (const [name, r] of RECIPES) {
  for (const link of TEXT_LINKS) {
    add("recipeToText", { name, recipe: r, link }, recipeToText(r, link))
  }
}

// ── bookToText ─────────────────────────────────────────────────────────────
const BOOK_CASES: { name: string; titles: string[]; link?: string | null }[] = [
  { name: "Empty shelf", titles: [] },
  { name: "Weeknight dinners", titles: ["Roast Chicken"], link: NORMAL },
  { name: "Baking", titles: ["Scones", "Victoria sponge", "Shortbread"] },
  { name: "Baking with a link", titles: ["Scones", "Victoria sponge"], link: NORMAL },
  { name: "🍜 Noodles", titles: ["Ramen", "Pho"], link: SHARE },
  { name: "Titles with blank link", titles: ["One"], link: "" },
  { name: "Titles with null link", titles: ["One"], link: null },
]
for (const c of BOOK_CASES) {
  add("bookToText", c, bookToText(c.name, c.titles, c.link))
}

// ── bookImportedTitle ──────────────────────────────────────────────────────
const IMPORT_CASES: { name: string; added: number; duplicates: number; skipped?: number }[] = [
  { name: "Weeknight dinners", added: 12, duplicates: 0 },
  { name: "Weeknight dinners", added: 1, duplicates: 0 },
  { name: "Weeknight dinners", added: 0, duplicates: 0 },
  { name: "Weeknight dinners", added: 12, duplicates: 0, skipped: 1 },
  { name: "Weeknight dinners", added: 1, duplicates: 0, skipped: 0 },
  { name: "Weeknight dinners", added: 0, duplicates: 3 },
  { name: "Weeknight dinners", added: 0, duplicates: 3, skipped: 2 },
  { name: "Weeknight dinners", added: 5, duplicates: 3 },
  { name: "Weeknight dinners", added: 0, duplicates: 0, skipped: 4 },
  { name: "Weeknight dinners", added: 0, duplicates: 0, skipped: 0 },
]
for (const c of IMPORT_CASES) {
  add("bookImportedTitle", c, bookImportedTitle(c))
}

// ── countItems ─────────────────────────────────────────────────────────────
const COUNT_CASES: { name: string | null; items: string[] }[][] = [
  [],
  [{ name: null, items: [] }],
  [{ name: null, items: ["a"] }],
  [{ name: "Sauce", items: ["a", "b", "c"] }],
  [
    { name: null, items: ["a", "b"] },
    { name: "Topping", items: ["c"] },
    { name: null, items: [] },
  ],
]
for (const sections of COUNT_CASES) {
  add("countItems", sections, countItems(sections))
}

// ── kicker ─────────────────────────────────────────────────────────────────
const KICKER_CASES: { recipeCategory: string | null; recipeCuisine: string | null }[] = [
  { recipeCategory: "Dinner", recipeCuisine: "Italian" },
  { recipeCategory: "Dinner", recipeCuisine: null },
  { recipeCategory: null, recipeCuisine: "Italian" },
  { recipeCategory: null, recipeCuisine: null },
  { recipeCategory: "", recipeCuisine: "" },
  { recipeCategory: "", recipeCuisine: "Italian" },
  { recipeCategory: "Dinner", recipeCuisine: "" },
]
for (const c of KICKER_CASES) {
  add("kicker", c, kicker(c))
}

// ── webLink / hostOf ───────────────────────────────────────────────────────
const URL_CASES: (string | null)[] = [
  "https://www.example.com/a",
  "http://Example.COM:8080/x",
  "https://www.www.example.com",
  "javascript:alert(1)",
  "data:text/html,hi",
  "ftp://example.com",
  "mailto:a@b.c",
  "https://",
  "not a url",
  "https://bücher.example/",
  "https://example.com./",
  null,
  "",
  "  https://example.com  ",
]
for (const input of URL_CASES) {
  add("webLink", input, webLink(input))
  add("hostOf", input, hostOf(input))
}

const path = new URL("./format.json", import.meta.url)
await Bun.write(path, `${JSON.stringify(out, null, 2)}\n`)
console.log(`wrote ${path.pathname}`)
for (const [key, entries] of Object.entries(out)) {
  console.log(`  ${key}: ${entries.length}`)
}
