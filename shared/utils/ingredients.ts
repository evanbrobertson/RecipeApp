/**
 * Small ingredient-line parser: "1 1/2 cups flour, sifted" →
 * { quantity: 1.5, unit: "cup", name: "flour", prep: "sifted" }.
 * Used for scaling recipes and for sizing bowls in mise en place mode.
 */

export type Vessel = "pinch" | "ramekin" | "small" | "medium" | "large" | "board" | "jar"

export interface ParsedIngredient {
  raw: string
  quantity: number | null
  quantityMax: number | null
  unit: string | null
  name: string
  prep: string | null
}

const UNICODE_FRACTIONS: Record<string, number> = {
  "¼": 0.25,
  "½": 0.5,
  "¾": 0.75,
  "⅓": 1 / 3,
  "⅔": 2 / 3,
  "⅕": 0.2,
  "⅛": 0.125,
  "⅜": 0.375,
  "⅝": 0.625,
  "⅞": 0.875,
}

/** Canonical unit → aliases. Order matters for matching (longer first within the regex). */
const UNITS: Record<string, string[]> = {
  tsp: ["teaspoons", "teaspoon", "tsps", "tsp", "t"],
  tbsp: ["tablespoons", "tablespoon", "tbsps", "tbsp", "tbs", "tbl", "T"],
  cup: ["cups", "cup", "c"],
  ml: ["milliliters", "millilitres", "milliliter", "millilitre", "ml", "mL"],
  l: ["liters", "litres", "liter", "litre", "l", "L"],
  "fl oz": ["fluid ounces", "fluid ounce", "fl oz", "fl. oz."],
  oz: ["ounces", "ounce", "oz"],
  lb: ["pounds", "pound", "lbs", "lb"],
  g: ["grams", "gram", "g", "gr"],
  kg: ["kilograms", "kilogram", "kg"],
  pinch: ["pinches", "pinch"],
  dash: ["dashes", "dash"],
  clove: ["cloves", "clove"],
  can: ["cans", "can", "tins", "tin"],
  stick: ["sticks", "stick"],
  slice: ["slices", "slice"],
  bunch: ["bunches", "bunch"],
  sprig: ["sprigs", "sprig"],
  handful: ["handfuls", "handful"],
  piece: ["pieces", "piece"],
  package: ["packages", "package", "packets", "packet", "pkg"],
}

const aliasToUnit = new Map<string, string>()
for (const [unit, aliases] of Object.entries(UNITS)) {
  for (const alias of aliases) aliasToUnit.set(alias, unit)
}
const unitPattern = [...aliasToUnit.keys()]
  .toSorted((a, b) => b.length - a.length)
  .map((a) => a.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
  .join("|")

const NUMBER = String.raw`(?:\d+\s+\d+\/\d+|\d+\/\d+|\d*\.\d+|\d+(?:\s*[¼½¾⅓⅔⅕⅛⅜⅝⅞])?|[¼½¾⅓⅔⅕⅛⅜⅝⅞])`
const LEADING = new RegExp(
  String.raw`^\s*(?:about|approx\.?|~)?\s*(${NUMBER})(?:\s*(?:-|–|to)\s*(${NUMBER}))?\s*(?:(${unitPattern})\.?(?=\s|$|,))?\s*(?:of\s+)?(.*)$`,
)
// Word units only (3+ letters) so "T" or "c" at the start of a name isn't read as a unit
const BARE_UNIT = new RegExp(
  `^(${[...aliasToUnit.keys()]
    .filter((a) => a.length >= 3 && /^[a-z]+$/.test(a))
    .toSorted((a, b) => b.length - a.length)
    .join("|")})\\s+(?:of\\s+)?(.+)$`,
  "i",
)
const WORD_QUANTITY = /^\s*(a|an|one|a few|a couple of|some)\s+(.*)$/i

export function parseNumber(value: string): number | null {
  const v = value.trim()
  if (!v) return null
  const uni = v.match(/^(\d+)?\s*([¼½¾⅓⅔⅕⅛⅜⅝⅞])$/)
  if (uni) return Number(uni[1] ?? 0) + UNICODE_FRACTIONS[uni[2]!]!
  const mixed = v.match(/^(\d+)\s+(\d+)\/(\d+)$/)
  if (mixed) return Number(mixed[1]) + Number(mixed[2]) / Number(mixed[3])
  const frac = v.match(/^(\d+)\/(\d+)$/)
  if (frac) return Number(frac[2]) === 0 ? null : Number(frac[1]) / Number(frac[2])
  const n = Number(v)
  return Number.isFinite(n) ? n : null
}

function splitPrep(rest: string): { name: string; prep: string | null } {
  // "onion, finely diced" / "butter (softened)"
  const paren = rest.match(/^(.*?)\s*\(([^)]*)\)\s*(.*)$/)
  let name = rest
  let prep: string | null = null
  if (paren) {
    name = `${paren[1]} ${paren[3]}`.trim()
    prep = paren[2]!.trim() || null
  }
  const comma = name.indexOf(",")
  if (comma > 0) {
    const after = name.slice(comma + 1).trim()
    name = name.slice(0, comma).trim()
    prep = [after, prep].filter(Boolean).join(", ") || null
  }
  return { name: name.trim(), prep }
}

export function parseIngredient(raw: string): ParsedIngredient {
  const text = raw.replace(/\s+/g, " ").trim()
  const m = text.match(LEADING)
  if (m && m[4] !== undefined) {
    const split = splitPrep(m[4])
    let unit = m[3] ? (aliasToUnit.get(m[3]) ?? aliasToUnit.get(m[3].toLowerCase()) ?? null) : null
    let name = split.name
    // "1 (14 oz) can tomatoes": the unit comes after a parenthetical
    if (!unit) {
      const after = name.match(BARE_UNIT)
      if (after) {
        unit = aliasToUnit.get(after[1]!.toLowerCase()) ?? null
        name = after[2]!
      }
    }
    return {
      raw,
      quantity: parseNumber(m[1]!),
      quantityMax: m[2] ? parseNumber(m[2]) : null,
      unit,
      name: name || text,
      prep: split.prep,
    }
  }
  // "Pinch of salt", "Dash of hot sauce"
  const bare = text.match(BARE_UNIT)
  if (bare) {
    const { name, prep } = splitPrep(bare[2]!)
    return {
      raw,
      quantity: 1,
      quantityMax: null,
      unit: aliasToUnit.get(bare[1]!.toLowerCase()) ?? null,
      name,
      prep,
    }
  }
  const w = text.match(WORD_QUANTITY)
  if (w) {
    const { name, prep } = splitPrep(w[2]!)
    const word = w[1]!.toLowerCase()
    const quantity = word === "a" || word === "an" || word === "one" ? 1 : null
    // "a pinch of salt"
    const unitMatch = name.match(new RegExp(`^(${unitPattern})\\s+(?:of\\s+)?(.*)$`))
    if (unitMatch) {
      return {
        raw,
        quantity,
        quantityMax: null,
        unit: aliasToUnit.get(unitMatch[1]!) ?? null,
        name: unitMatch[2]!,
        prep,
      }
    }
    return { raw, quantity, quantityMax: null, unit: null, name, prep }
  }
  const { name, prep } = splitPrep(text)
  return { raw, quantity: null, quantityMax: null, unit: null, name, prep }
}

// ─── Formatting & scaling ──────────────────────────────────────────────────

const NICE_FRACTIONS: [number, string][] = [
  [0.125, "⅛"],
  [0.25, "¼"],
  [1 / 3, "⅓"],
  [0.5, "½"],
  [2 / 3, "⅔"],
  [0.75, "¾"],
]

export function formatQuantity(n: number): string {
  if (n >= 10) return String(Math.round(n))
  const whole = Math.floor(n)
  const frac = n - whole
  if (frac < 0.06) return String(whole || n.toFixed(2).replace(/\.?0+$/, ""))
  if (frac > 0.94) return String(whole + 1)
  const nice = NICE_FRACTIONS.find(([v]) => Math.abs(v - frac) < 0.02)
  if (nice) return whole ? `${whole} ${nice[1]}` : nice[1]
  return n.toFixed(1).replace(/\.0$/, "")
}

/** Scales the leading quantity of an ingredient line, keeping the rest verbatim. */
export function scaleIngredient(raw: string, factor: number): string {
  if (factor === 1) return raw
  const m = raw.match(
    new RegExp(
      String.raw`^(\s*(?:about|approx\.?|~)?\s*)(${NUMBER})(?:(\s*(?:-|–|to)\s*)(${NUMBER}))?`,
    ),
  )
  if (!m) return raw
  const a = parseNumber(m[2]!)
  if (a === null) return raw
  const b = m[4] ? parseNumber(m[4]) : null
  const scaled =
    formatQuantity(a * factor) + (b !== null ? `${m[3]}${formatQuantity(b * factor)}` : "")
  return `${m[1]}${scaled}${raw.slice(m[0].length)}`
}

const COUNTABLE = new Set([
  "cup",
  "clove",
  "can",
  "stick",
  "slice",
  "sprig",
  "handful",
  "piece",
  "package",
])
const ES_PLURAL = new Set(["pinch", "dash", "bunch"])

/** "3 clove" → "3 cloves" for count-like units; abbreviations stay as they are. */
export function pluralUnit(unit: string, quantity: number): string {
  if (quantity <= 1) return unit
  if (COUNTABLE.has(unit)) return `${unit}s`
  if (ES_PLURAL.has(unit)) return `${unit}es`
  return unit
}

// ─── Mise en place ─────────────────────────────────────────────────────────

const ML_PER_UNIT: Record<string, number> = {
  tsp: 5,
  tbsp: 15,
  cup: 240,
  ml: 1,
  l: 1000,
  "fl oz": 30,
  oz: 28,
  lb: 450,
  g: 1,
  kg: 1000,
  pinch: 0.5,
  dash: 0.6,
  clove: 6,
  can: 400,
  stick: 115,
  handful: 40,
  bunch: 150,
  sprig: 2,
}

const TO_TASTE =
  /\b(to taste|as needed|for serving|for garnish|optional|for frying|for greasing)\b/i
const WHOLE_PRODUCE =
  /\b(onions?|shallots?|carrots?|potato(?:es)?|tomato(?:es)?|peppers?|zucchini|courgettes?|cucumbers?|apples?|lemons?|limes?|oranges?|avocados?|chicken|beef|pork|lamb|fish|salmon|steaks?|breasts?|thighs?|fillets?|cabbage|cauliflower|broccoli|squash|eggplants?|aubergines?|leeks?|celery|mushrooms?|bananas?|mango(?:es)?|mince|ground (?:beef|pork|turkey)|shrimp|prawns?|tofu)\b/i
const CUT_TASKS: Record<string, string> = {
  chopped: "chop",
  diced: "dice",
  minced: "mince",
  sliced: "slice",
  grated: "grate",
  julienned: "julienne",
  cubed: "cube",
  crushed: "crush",
  peeled: "peel",
  halved: "halve",
  quartered: "quarter",
  shredded: "shred",
  trimmed: "trim",
  zested: "zest",
  juiced: "juice",
  cut: "cut",
}
const CUTTING = new RegExp(`\\b(${Object.keys(CUT_TASKS).join("|")})\\b`, "i")

export interface MiseItem extends ParsedIngredient {
  vessel: Vessel
  /** Rough volume in ml, when it can be estimated */
  volume: number | null
  /** A short prep instruction, e.g. "dice" */
  task: string | null
}

function vesselFor(volume: number): Vessel {
  if (volume <= 3) return "pinch"
  if (volume <= 35) return "ramekin"
  if (volume <= 150) return "small"
  if (volume <= 500) return "medium"
  return "large"
}

/** Decides what to prep each ingredient into: a pinch bowl, ramekin, bowl, board or jar. */
export function miseEnPlace(lines: string[]): MiseItem[] {
  return lines.map((line) => {
    const p = parseIngredient(line)
    const prepText = [p.prep, line].filter(Boolean).join(" ")
    const cut = prepText.match(CUTTING)?.[1]?.toLowerCase() ?? null
    const task = cut ? (CUT_TASKS[cut] ?? null) : null

    if (p.quantity === null && TO_TASTE.test(line)) {
      return { ...p, vessel: "jar", volume: null, task }
    }
    const perUnit = p.unit ? ML_PER_UNIT[p.unit] : undefined
    const byWeight = p.unit === "g" || p.unit === "kg" || p.unit === "lb" || p.unit === "oz"
    // "500g chicken thighs" goes on a board or plate, not in a bowl
    if (byWeight && WHOLE_PRODUCE.test(p.name)) {
      return { ...p, vessel: "board", volume: null, task }
    }
    if (p.quantity !== null && perUnit !== undefined) {
      const volume = (p.quantityMax ?? p.quantity) * perUnit
      return { ...p, vessel: vesselFor(volume), volume, task }
    }
    // Whole items ("2 onions, diced") go on the board; small counts of other things in a bowl
    if (WHOLE_PRODUCE.test(p.name) || cut) {
      return { ...p, vessel: "board", volume: null, task }
    }
    if (p.quantity !== null) {
      const count = p.quantityMax ?? p.quantity
      return { ...p, vessel: count <= 2 ? "small" : "medium", volume: null, task }
    }
    return { ...p, vessel: "jar", volume: null, task }
  })
}

/** Finds durations in a step ("bake 25–30 minutes") for one-tap timers. */
export function findTimers(step: string): { label: string; seconds: number }[] {
  const out: { label: string; seconds: number }[] = []
  const re =
    /(\d+(?:\.\d+)?|[½¼¾])(?:\s*(?:-|–|to)\s*(\d+(?:\.\d+)?))?\s*(hours?|hrs?|h\b|minutes?|mins?|m\b|seconds?|secs?)/gi
  for (const m of step.matchAll(re)) {
    const value = parseNumber(m[2] ?? m[1]!)
    if (value === null) continue
    const unit = m[3]!.toLowerCase()
    const mult = unit.startsWith("h") ? 3600 : unit.startsWith("s") ? 1 : 60
    const seconds = Math.round(value * mult)
    if (seconds >= 10 && seconds <= 24 * 3600) out.push({ label: m[0], seconds })
  }
  return out
}
