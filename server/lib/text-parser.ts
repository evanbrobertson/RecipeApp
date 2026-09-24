import type { RecipeFields, RecipeSection } from "#shared/utils/recipe"
import { normalizeSections } from "#shared/utils/recipe"

/**
 * Heuristic parser that turns pasted recipe text (from a website, an email, notes,
 * a cookbook scan...) into structured fields. No network or LLM required.
 */

type Mode = "preamble" | "ingredients" | "instructions" | "notes" | "nutrition"

const HEADINGS: [Mode, RegExp][] = [
  [
    "ingredients",
    /^(?:ingredients?(?: list)?|what you(?:'|’)?ll need|you(?:'|’)?ll need|you will need|shopping list)$/i,
  ],
  [
    "instructions",
    /^(?:instructions?|directions?|method|steps|preparation|prep(?:aration)? steps|how to make(?: it)?|to make)$/i,
  ],
  [
    "notes",
    /^(?:notes?|recipe notes?|tips?(?: (?:&|and) (?:notes|tricks))?|cook(?:'|’)?s notes?)$/i,
  ],
  ["nutrition", /^(?:nutrition(?:al)?(?: (?:facts|info(?:rmation)?))?(?: per serving)?)$/i],
]

// Time labels need a separator or the word "time" so prose like "Cook the pasta..." isn't matched
const SEP = String.raw`(?:\s+time\s*[:\-–]?|\s*[:\-–])\s*(.+)$`
const META: [keyof RecipeFields, RegExp][] = [
  ["prepTime", new RegExp(`^prep(?:aration)?${SEP}`, "i")],
  ["cookTime", new RegExp(`^cook(?:ing)?${SEP}`, "i")],
  ["totalTime", new RegExp(`^(?:total|ready in)${SEP}`, "i")],
  [
    "freezeTime",
    new RegExp(`^(?:additional|chill(?:ing)?|rest(?:ing)?|inactive|freeze|freezing)${SEP}`, "i"),
  ],
  ["recipeYield", /^(?:serves|servings?|yield|makes)\s*[:\-–]?\s*(.+)$/i],
  ["recipeCuisine", /^cuisine\s*[:\-–]\s*(.+)$/i],
  ["recipeCategory", /^(?:course|category)\s*[:\-–]\s*(.+)$/i],
  ["author", /^(?:author|recipe by|by)\s*[:\-–]\s*(.+)$/i],
]

const BULLET = /^\s*(?:[-*•·▢☐□◦‣⁃–—]|\[\s?[x ]?\s?\])\s*/i
const NUMBERED = /^\s*(?:step\s*)?(\d{1,2})\s*(?:[.):]|-(?=\s))\s*|^\s*step\s*(\d{1,2})\s*$/i
const QUANTITY =
  /^(?:\d|[¼½¾⅓⅔⅛⅜⅝⅞]|a |an |one |two |three |four |five |six |half |pinch|dash|handful|splash|some |few )/i
const UNITS =
  /\b(?:cups?|c\.|tbsps?|tablespoons?|tsps?|teaspoons?|grams?|g|kg|ml|l|litres?|liters?|oz|ounces?|lbs?|pounds?|pinch|cloves?|cans?|sticks?|slices?|bunch|sprigs?|packages?|pkg)\b/i
const COOKING_VERB =
  /^(?:add|bake|beat|blend|boil|bring|combine|cook|cool|cover|cut|drain|fold|fry|grease|heat|let|line|melt|mix|place|pour|preheat|reduce|remove|roast|serve|simmer|slice|sprinkle|stir|strain|toss|transfer|whisk|season|chop|dice|garnish|allow|repeat|set|spread|top|turn|wash|rinse|put|make|using|in a|in the|once|meanwhile|when|while|after|then|finally)\b/i

function cleanLine(line: string): string {
  return line.replace(/ /g, " ").replace(/\s+/g, " ").trim()
}

function headingMode(line: string): Mode | null {
  const text = line
    .replace(/^#+\s*/, "")
    .replace(/[:：]\s*$/, "")
    .replace(/^\*\*(.+)\*\*$/, "$1")
    .trim()
  for (const [mode, re] of HEADINGS) if (re.test(text)) return mode
  return null
}

/** A short line ending with ":" (or a "For the ..." line) that names a sub-section. */
function subHeading(line: string): string | null {
  const text = line
    .replace(/^#+\s*/, "")
    .replace(/^\*\*(.+)\*\*$/, "$1")
    .trim()
  if (text.length > 60) return null
  if (/[:：]$/.test(text) && !/\d/.test(text)) return text.replace(/[:：]$/, "").trim()
  if (/^for (?:the )?[a-z][a-z\s&,'-]*$/i.test(text) && !QUANTITY.test(text)) return text
  if (/^#+\s/.test(line)) return text
  return null
}

function looksLikeIngredient(line: string): boolean {
  if (line.length > 120 || COOKING_VERB.test(line) || line.endsWith(".")) return false
  return QUANTITY.test(line) || (UNITS.test(line) && line.split(" ").length <= 10)
}

function looksLikeInstruction(line: string): boolean {
  return line.length > 60 || COOKING_VERB.test(line) || /[.!]$/.test(line)
}

function pushItem(sections: RecipeSection[], item: string) {
  if (sections.length === 0) sections.push({ name: null, items: [] })
  sections[sections.length - 1]!.items.push(item)
}

function startSection(sections: RecipeSection[], name: string) {
  const last = sections[sections.length - 1]
  if (last && last.items.length === 0) last.name = name
  else sections.push({ name, items: [] })
}

export interface ParsedText {
  recipe: RecipeFields
  /** True when the text had recognisable Ingredients/Instructions headings. */
  structured: boolean
}

export function parseRecipeText(input: string): ParsedText {
  const lines = input.replace(/\r\n?/g, "\n").split("\n").map(cleanLine)

  const recipe: RecipeFields = {
    title: "",
    ingredients: [],
    instructions: [],
  }
  const preamble: string[] = []
  const notes: string[] = []
  const nutrition: Record<string, string> = {}
  let mode: Mode = "preamble"
  let structured = false
  let lastWasNumbered = false

  for (const raw of lines) {
    if (!raw) {
      lastWasNumbered = false
      continue
    }

    // URL on its own line → remember as the source link
    if (/^https?:\/\/\S+$/i.test(raw)) {
      recipe.url ??= raw
      continue
    }

    const heading = headingMode(raw)
    if (heading) {
      mode = heading
      structured = true
      lastWasNumbered = false
      continue
    }

    // Metadata lines ("Prep time: 10 mins") appear before the ingredient/step lists
    if (mode === "preamble") {
      const meta = META.find(([, re]) => re.test(raw))
      if (meta && raw.length < 80) {
        const [key, re] = meta
        const value = raw.match(re)?.[1]?.trim()
        const plausible = value && (!key.endsWith("Time") || /\d/.test(value))
        if (plausible && !(recipe as Record<string, unknown>)[key]) {
          ;(recipe as Record<string, unknown>)[key] = value
          continue
        }
      }
    }

    const bulletless = raw.replace(BULLET, "").trim()
    // Numbering only means "step" in the instructions; in ingredients "2-3 cloves" is a quantity
    const numbered = mode === "instructions" || mode === "preamble" ? raw.match(NUMBERED) : null
    const text = numbered ? raw.replace(NUMBERED, "").trim() : bulletless

    switch (mode) {
      case "preamble": {
        if (!recipe.title) recipe.title = text.replace(/^#+\s*/, "")
        else preamble.push(text)
        break
      }
      case "ingredients": {
        const sub = subHeading(raw)
        if (sub) startSection(recipe.ingredients, sub)
        else pushItem(recipe.ingredients, text)
        break
      }
      case "instructions": {
        if (numbered && !text) break // "Step 1" on its own line
        const sub = !numbered ? subHeading(raw) : null
        if (sub) {
          startSection(recipe.instructions, sub)
          lastWasNumbered = false
          break
        }
        // A wrapped continuation of a numbered step
        const current = recipe.instructions[recipe.instructions.length - 1]
        if (!numbered && lastWasNumbered && current?.items.length && !BULLET.test(raw)) {
          current.items[current.items.length - 1] += ` ${text}`
          break
        }
        pushItem(recipe.instructions, text)
        lastWasNumbered = Boolean(numbered)
        break
      }
      case "notes":
        notes.push(text)
        break
      case "nutrition": {
        const m = text.match(/^([A-Za-z][A-Za-z\s]+?)\s*[:\-–]\s*(.+)$/)
        if (m) nutrition[m[1]!.trim().toLowerCase()] = m[2]!.trim()
        else notes.push(text)
        break
      }
    }
  }

  // No headings found: classify lines by shape
  if (!structured) {
    for (const line of preamble.splice(0)) {
      const text = line.replace(BULLET, "").replace(NUMBERED, "").trim()
      const inIngredients = recipe.ingredients.length > 0 && recipe.instructions.length === 0
      const shortItem = text.split(" ").length <= 8 && !looksLikeInstruction(text)
      if (
        recipe.instructions.length === 0 &&
        (looksLikeIngredient(text) || (inIngredients && shortItem))
      ) {
        pushItem(recipe.ingredients, text)
      } else if (recipe.ingredients.length > 0) {
        pushItem(recipe.instructions, text)
      } else {
        preamble.push(line)
      }
    }
  }

  recipe.title = (recipe.title || "Untitled recipe").slice(0, 300)
  recipe.description = preamble.join(" ").slice(0, 1000) || null
  recipe.notes = notes.join("\n") || null
  recipe.nutrition = Object.keys(nutrition).length ? nutrition : null
  recipe.ingredients = normalizeSections(recipe.ingredients)
  recipe.instructions = normalizeSections(recipe.instructions)

  return { recipe, structured }
}
