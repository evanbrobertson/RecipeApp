/**
 * Generates ingredients.json, the parity fixtures for
 * crates/crumb-core/src/ingredients.rs.
 *
 * Regenerate from the repo root with:
 *
 *   bun crates/crumb-core/tests/fixtures/gen-ingredients.ts
 *
 * The TypeScript in web/src/lib/ingredients.ts is the source of truth: this
 * script imports the real functions and records their output, so the Rust port
 * can be checked against them. Do not edit the JSON by hand.
 */

import {
  findTimers,
  formatQuantity,
  ingredientsForStep,
  miseEnPlace,
  parseIngredient,
  parseNumber,
  pluralUnit,
  scaleIngredient,
} from "../../../../web/src/lib/ingredients.ts"

type Entry = { input: unknown; output: unknown }

const out = {
  parseNumber: [] as Entry[],
  parseIngredient: [] as Entry[],
  formatQuantity: [] as Entry[],
  scaleIngredient: [] as Entry[],
  pluralUnit: [] as Entry[],
  miseEnPlace: [] as Entry[],
  findTimers: [] as Entry[],
  ingredientsForStep: [] as Entry[],
}

function add(key: keyof typeof out, input: unknown, output: unknown) {
  out[key].push({ input, output })
}

// ── parseNumber ────────────────────────────────────────────────────────────
for (const input of [
  "1",
  "12",
  "1/2",
  "1 1/2",
  "1½",
  "½",
  "⅓",
  ".5",
  "0.75",
  "2 ¾",
  "",
  "abc",
  "1/0",
  "3-4",
]) {
  add("parseNumber", input, parseNumber(input))
}

// ── Units table (order and aliases mirror web/src/lib/ingredients.ts) ───────
const UNITS: [string, string[]][] = [
  ["tsp", ["teaspoons", "teaspoon", "tsps", "tsp", "t"]],
  ["tbsp", ["tablespoons", "tablespoon", "tbsps", "tbsp", "tbs", "tbl", "T"]],
  ["cup", ["cups", "cup", "c"]],
  ["ml", ["milliliters", "millilitres", "milliliter", "millilitre", "ml", "mL"]],
  ["l", ["liters", "litres", "liter", "litre", "l", "L"]],
  ["fl oz", ["fluid ounces", "fluid ounce", "fl oz", "fl. oz."]],
  ["oz", ["ounces", "ounce", "oz"]],
  ["lb", ["pounds", "pound", "lbs", "lb"]],
  ["g", ["grams", "gram", "g", "gr"]],
  ["kg", ["kilograms", "kilogram", "kg"]],
  ["pinch", ["pinches", "pinch"]],
  ["dash", ["dashes", "dash"]],
  ["clove", ["cloves", "clove"]],
  ["can", ["cans", "can", "tins", "tin"]],
  ["stick", ["sticks", "stick"]],
  ["slice", ["slices", "slice"]],
  ["bunch", ["bunches", "bunch"]],
  ["sprig", ["sprigs", "sprig"]],
  ["handful", ["handfuls", "handful"]],
  ["piece", ["pieces", "piece"]],
  ["package", ["packages", "package", "packets", "packet", "pkg"]],
]
const CANONICAL_UNITS = UNITS.map(([unit]) => unit)
const ALL_ALIASES = UNITS.flatMap(([, aliases]) => aliases)

// ── parseIngredient ────────────────────────────────────────────────────────
const PARSE_INPUTS = [
  // plain
  "2 cups flour",
  "1 1/2 cups flour, sifted",
  "1½ tsp salt",
  "½ cup sugar",
  "3 eggs",
  "1 onion, finely chopped",
  // ranges
  "2-3 cloves garlic",
  "2 to 3 tbsp oil",
  "1–2 cups stock",
  // unit case sensitivity
  "1 t salt",
  "1 T butter",
  "2 Tbsp honey",
  "1 TSP vanilla",
  // word quantities
  "a pinch of salt",
  "an onion",
  "one lemon",
  "a few sprigs thyme",
  "a couple of eggs",
  "some parsley",
  // bullets and glyphs
  "▢ 1 cup rice",
  "☐ 2 eggs",
  "• 1 tsp cumin",
  "* 3 carrots",
  "✔ 1 lime",
  // no quantity
  "salt and pepper to taste",
  "Fresh basil, for garnish",
  "oil for frying",
  // parentheses and extras
  "1 (14 oz) can diced tomatoes",
  "2 cups (250 g) flour",
  "1 lb. chicken thighs",
  "200g butter, softened",
  "1 kg potatoes",
  // unicode/accents
  "2 jalapeños, seeded",
  "1 cup crème fraîche",
  "3 chiles",
  // edge
  "",
  "   ",
  "1",
  "cups",
  "1/2",
  "2 large eggs, beaten",
]
for (const input of PARSE_INPUTS) {
  add("parseIngredient", input, parseIngredient(input))
}
// every unit alias at least once
for (const alias of ALL_ALIASES) {
  const input = `2 ${alias} water`
  add("parseIngredient", input, parseIngredient(input))
}

// ── formatQuantity ─────────────────────────────────────────────────────────
for (const input of [
  0,
  0.125,
  0.25,
  1 / 3,
  0.5,
  2 / 3,
  0.75,
  1,
  1.5,
  1.25,
  1.333333,
  2.5,
  3,
  0.1,
  0.05,
  1.005,
  2.675,
  10,
  12.5,
  100,
  0.3333333333,
  7 / 8,
  // extra cases that exercise the toFixed fallbacks and the 0.94 cut-off
  0.2,
  0.4,
  0.35,
  0.45,
  0.55,
  0.6,
  0.9,
  1.1,
  2.7,
  0.95,
  0.06,
  0.04,
  1.05,
  2.05,
  0.15,
]) {
  add("formatQuantity", input, formatQuantity(input))
}

// ── scaleIngredient ────────────────────────────────────────────────────────
const SCALE_FACTORS = [0.5, 1, 1.5, 2, 3, 0.25]
const SCALE_INPUTS = PARSE_INPUTS.filter((s) => s.trim().length > 0).slice(0, 24)
for (const input of SCALE_INPUTS) {
  for (const factor of SCALE_FACTORS) {
    add("scaleIngredient", { raw: input, factor }, scaleIngredient(input, factor))
  }
}

// ── pluralUnit ─────────────────────────────────────────────────────────────
for (const unit of CANONICAL_UNITS) {
  for (const quantity of [0, 0.5, 1, 1.5, 2]) {
    add("pluralUnit", { unit, quantity }, pluralUnit(unit, quantity))
  }
}

// ── miseEnPlace ────────────────────────────────────────────────────────────
const MISE_LINES: string[][] = [
  [
    "2 tbsp vegetable oil",
    "1 onion, finely chopped",
    "3 cloves garlic, minced",
    "1 tbsp ginger, grated",
    "2 tsp ground cumin",
    "1 tsp ground coriander",
    "1/2 tsp turmeric",
    "400 ml coconut milk",
    "2 cups chicken stock",
    "500 g chicken thighs, diced",
    "salt and pepper to taste",
  ],
  [
    "2 cups all-purpose flour",
    "1 1/2 tsp baking powder",
    "1/2 tsp salt",
    "1 cup unsalted butter, softened",
    "1 1/2 cups granulated sugar",
    "3 large eggs",
    "2 tsp vanilla extract",
    "3/4 cup whole milk",
  ],
  [
    "2 cups mixed greens",
    "1/4 cup fresh parsley, chopped",
    "2 tbsp fresh dill, chopped",
    "juice of 1 lemon",
    "zest and juice of 2 limes",
    "3 tbsp olive oil",
    "1 cup cherry tomatoes, halved",
    "1 cucumber, sliced",
  ],
  [
    "1 (28 oz) can crushed tomatoes",
    "2 (14 oz) cans diced tomatoes",
    "1 tbsp tomato paste",
    "2 tbsp olive oil",
    "1 onion, diced",
    "3 cloves garlic, sliced",
  ],
  [
    "1 tsp garlic powder",
    "2 tsp onion powder",
    "1 tsp smoked paprika",
    "1/2 tsp cayenne pepper",
    "1 tbsp chili powder",
    "1 tsp dried oregano",
    "1 bay leaf",
  ],
  [
    "2 onions, diced",
    "4 cloves garlic, minced",
    "3 carrots, peeled and sliced",
    "2 potatoes, cubed",
    "1 head broccoli, cut into florets",
  ],
  ["juice of 1 lemon", "juice of 2 oranges", "zest of 1 lime"],
  [
    "500 g chicken thighs",
    "1 lb beef, cubed",
    "200g butter, softened",
    "2 sticks celery, diced",
    "3 lbs onions, sliced",
  ],
  [
    "salt and pepper to taste",
    "oil for frying",
    "fresh basil, for garnish",
    "1 cup rice",
    "2 eggs",
  ],
  ["2-3 cloves garlic", "1-2 tbsp olive oil", "1–2 cups stock", "3 eggs, beaten"],
]
for (const lines of MISE_LINES) {
  add("miseEnPlace", lines, miseEnPlace(lines))
}

// ── findTimers ─────────────────────────────────────────────────────────────
for (const input of [
  "Simmer for 20 minutes.",
  "Bake 1 hour",
  "Rest 1-2 hours",
  "Cook 30 seconds, then 5 mins",
  "Roast for 1 hour 15 minutes",
  "Boil 10–12 min",
  "Chill overnight",
  "Stir well",
  "Bake 45 min at 180C",
  "Cook for an hour",
]) {
  add("findTimers", input, findTimers(input))
}

// ── ingredientsForStep ─────────────────────────────────────────────────────
type Ing = { raw: string; section: string | null }
const STEP_CASES: { step: { text: string; section: string | null }; ingredients: Ing[] }[] = [
  {
    step: { text: "Add the eggs and sugar", section: null },
    ingredients: [
      { raw: "3 eggs", section: null },
      { raw: "1 cup sugar", section: null },
      { raw: "2 cups flour", section: null },
    ],
  },
  {
    step: { text: "Fold in the sugar", section: "Make the Filling" },
    ingredients: [
      { raw: "1/2 cup sugar", section: "Make the Filling" },
      { raw: "1 cup sugar", section: "Make the Crust" },
      { raw: "2 cups flour", section: "Make the Crust" },
    ],
  },
  {
    step: { text: "Beat the eggs", section: null },
    ingredients: [
      { raw: "2 eggs", section: null },
      { raw: "1 egg white", section: null },
      { raw: "1 cup milk", section: null },
    ],
  },
  {
    step: { text: "Drizzle the olive oil", section: null },
    ingredients: [
      { raw: "2 tbsp olive oil", section: null },
      { raw: "1 tbsp oil", section: null },
      { raw: "1 tbsp vinegar", section: null },
    ],
  },
  {
    step: { text: "Add the pumpkin", section: null },
    ingredients: [
      { raw: "1 cup pumpkin puree", section: null },
      { raw: "1 tsp pumpkin pie spice", section: null },
    ],
  },
  {
    step: { text: "Add the garlic", section: null },
    ingredients: [
      { raw: "3 cloves garlic, minced", section: null },
      { raw: "1 tsp garlic powder", section: null },
    ],
  },
  {
    step: { text: "Preheat the oven to 180C", section: null },
    ingredients: [
      { raw: "2 cups flour", section: null },
      { raw: "1 cup sugar", section: null },
    ],
  },
  {
    step: { text: "Mix the flour", section: "For the Dough" },
    ingredients: [
      { raw: "2 cups flour", section: "Dough" },
      { raw: "1 cup flour", section: "Topping" },
      { raw: "1/2 cup sugar", section: "Topping" },
    ],
  },
  {
    step: { text: "Add tomatoes", section: null },
    ingredients: [
      { raw: "2 tomatoes", section: null },
      { raw: "1 tsp tomato paste", section: null },
      { raw: "1 tbsp olive oil", section: null },
    ],
  },
  {
    step: { text: "Stir in the tomatoes and basil", section: null },
    ingredients: [
      { raw: "2 tomatoes, chopped", section: null },
      { raw: "1/4 cup fresh basil", section: null },
      { raw: "1 tsp tomato paste", section: null },
      { raw: "2 cups rice", section: null },
    ],
  },
]
for (const c of STEP_CASES) {
  add("ingredientsForStep", c, ingredientsForStep(c.step, c.ingredients))
}

const path = new URL("./ingredients.json", import.meta.url)
await Bun.write(path, `${JSON.stringify(out, null, 2)}\n`)
console.log(`wrote ${path.pathname}`)
for (const [key, entries] of Object.entries(out)) {
  console.log(`  ${key}: ${entries.length}`)
}
