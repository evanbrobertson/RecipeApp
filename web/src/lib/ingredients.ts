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
  "⅙": 1 / 6,
  "⅚": 5 / 6,
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
// Spread and sort rather than toSorted, which Safari only has from 16
const unitPattern = [...aliasToUnit.keys()]
  .sort((a, b) => b.length - a.length)
  .map((a) => a.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
  .join("|")

const NUMBER = String.raw`(?:\d+\s+\d+\/\d+|\d+\/\d+|\d*\.\d+|\d+(?:\s*[¼½¾⅓⅔⅕⅙⅚⅛⅜⅝⅞])?|[¼½¾⅓⅔⅕⅙⅚⅛⅜⅝⅞])`
const LEADING = new RegExp(
  String.raw`^\s*(?:about|approx\.?|~)?\s*(${NUMBER})(?:\s*(?:-|–|to)\s*(${NUMBER}))?\s*(?:(${unitPattern})\.?(?=\s|$|,))?\s*(?:of\s+)?(.*)$`,
)
// Word units only (3+ letters) so "T" or "c" at the start of a name isn't read as a unit
const BARE_UNIT = new RegExp(
  `^(${[...aliasToUnit.keys()]
    .filter((a) => a.length >= 3 && /^[a-z]+$/.test(a))
    .sort((a, b) => b.length - a.length)
    .join("|")})\\s+(?:of\\s+)?(.+)$`,
  "i",
)
const WORD_QUANTITY = /^\s*(a|an|one|a few|a couple of|some)\s+(.*)$/i

export function parseNumber(value: string): number | null {
  const v = value.trim()
  if (!v) return null
  const uni = v.match(/^(\d+)?\s*([¼½¾⅓⅔⅕⅙⅚⅛⅜⅝⅞])$/)
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

/** Checkbox and bullet glyphs some recipe sites put in front of every line ("▢ 1 cup flour"). */
const GLYPHS = /^(?:[▢☐□■◻◽◾▪▫☑☒✓✔✅•◦●○∙]\s*|\*\s+)+/

export function parseIngredient(raw: string): ParsedIngredient {
  const text = raw.replace(/\s+/g, " ").trim().replace(GLYPHS, "")
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

// Eighths, sixths, quarters, thirds and halves: the same set the importer turns float
// quantities back into (src/fractions.rs)
const NICE_FRACTIONS: [number, string][] = [
  [0.125, "⅛"],
  [1 / 6, "⅙"],
  [0.25, "¼"],
  [1 / 3, "⅓"],
  [0.375, "⅜"],
  [0.5, "½"],
  [0.625, "⅝"],
  [2 / 3, "⅔"],
  [0.75, "¾"],
  [5 / 6, "⅚"],
  [0.875, "⅞"],
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
  /\b(onions?|shallots?|carrots?|potato(?:es)?|tomato(?:es)?|(?:bell|sweet|red|green|yellow|orange|chil[ei]|jalape[nñ]o) peppers?|jalape[nñ]os?|zucchini|courgettes?|cucumbers?|apples?|lemons?|limes?|oranges?|avocados?|chicken|beef|pork|lamb|fish|salmon|steaks?|breasts?|thighs?|fillets?|cabbage|cauliflower|broccoli|squash|eggplants?|aubergines?|leeks?|celery|mushrooms?|bananas?|mango(?:es)?|mince|ground (?:beef|pork|turkey)|shrimp|prawns?|tofu)\b/i
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
/** Pantry forms of produce and meat: "onion powder", "chicken broth", "tomato paste",
 * "red pepper flakes". */
const PANTRY =
  /\b(powder|broth|stock|bouillon|paste|pur[eé]e|sauce|ketchup|juice|flakes|extract|seasoning|soup|granules|canned|tinned)\b/i
/** "Crushed tomatoes" are a can when the line says so or measures them ("28 oz crushed
 * tomatoes"); counted ("2 diced tomatoes") they're fresh ones to cut. */
const CUT_TOMATOES = /\b(?:crushed|diced|chopped|stewed) tomato(?:es)?\b/i
const CONTAINER = /\b(cans?|tins?|canned|tinned|jars?|cartons?)\b/i
const MEASURED = new Set(["tsp", "tbsp", "cup", "ml", "l", "fl oz", "oz", "lb", "g", "kg", "can"])
/** Fresh citrus squeezed into a bowl: "juice of 1 lemon", "zest and juice of 2 limes". */
const JUICE_OF = /\bjuice (?:of|from)\b/i
/** Knife work; grating, zesting and juicing end up in a bowl. */
const BOARD_TASKS = new Set([
  "chop",
  "dice",
  "mince",
  "slice",
  "julienne",
  "cube",
  "halve",
  "quarter",
  "trim",
  "peel",
  "cut",
])
/** Units that count pieces rather than measure them: "3 cloves garlic, minced" is board work. */
const PIECES = new Set(["clove", "stick", "bunch", "sprig", "piece", "slice", "handful"])

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
    if (JUICE_OF.test(line)) {
      const count = p.quantityMax ?? p.quantity ?? Number(line.match(/\d+/)?.[0] ?? 1)
      return { ...p, vessel: count <= 1 ? "ramekin" : "small", volume: null, task: "juice" }
    }
    const pantry =
      PANTRY.test(p.name) ||
      (CUT_TOMATOES.test(p.name) &&
        (CONTAINER.test(line) || (p.unit !== null && MEASURED.has(p.unit))))
    const produce = !pantry && WHOLE_PRODUCE.test(p.name)
    const prepText = [p.prep, line].filter(Boolean).join(" ")
    const cut = pantry ? null : (prepText.match(CUTTING)?.[1]?.toLowerCase() ?? null)
    const task = cut ? (CUT_TASKS[cut] ?? null) : null

    if (p.quantity === null && TO_TASTE.test(line)) {
      return { ...p, vessel: "jar", volume: null, task }
    }
    const perUnit = p.unit ? ML_PER_UNIT[p.unit] : undefined
    const byWeight = p.unit === "g" || p.unit === "kg" || p.unit === "lb" || p.unit === "oz"
    // "500g chicken thighs" goes on a board or plate, not in a bowl, and so does anything
    // weighed or counted in pieces that still needs cutting ("3 lbs onions, sliced",
    // "2 sticks celery, diced")
    const knife = task !== null && BOARD_TASKS.has(task)
    if (
      (byWeight && (produce || knife)) ||
      (knife && p.unit !== null && PIECES.has(p.unit)) ||
      (knife && p.unit === null && p.quantity !== null)
    ) {
      return { ...p, vessel: "board", volume: null, task }
    }
    if (p.quantity !== null && perUnit !== undefined) {
      const volume = (p.quantityMax ?? p.quantity) * perUnit
      return { ...p, vessel: vesselFor(volume), volume, task }
    }
    // Whole items ("2 onions, diced") go on the board; small counts of other things in a bowl
    if (produce || cut) {
      return { ...p, vessel: "board", volume: null, task }
    }
    if (p.quantity !== null) {
      const count = p.quantityMax ?? p.quantity
      return { ...p, vessel: count <= 2 ? "small" : "medium", volume: null, task }
    }
    return { ...p, vessel: "jar", volume: null, task }
  })
}

// ─── Timers ────────────────────────────────────────────────────────────────

const NUMBER_WORDS: Record<string, number> = {
  a: 1,
  an: 1,
  one: 1,
  two: 2,
  three: 3,
  four: 4,
  five: 5,
  six: 6,
  seven: 7,
  eight: 8,
  nine: 9,
  ten: 10,
  eleven: 11,
  twelve: 12,
  thirteen: 13,
  fourteen: 14,
  fifteen: 15,
  sixteen: 16,
  seventeen: 17,
  eighteen: 18,
  nineteen: 19,
  twenty: 20,
  thirty: 30,
  forty: 40,
  fifty: 50,
  sixty: 60,
  ninety: 90,
}
// A number in a step, as digits ("1 1/2", "2.5", "1½") or words ("three", "twenty-five",
// "a couple of", "half an"), optionally "and a half". The Rust port in
// crates/crumb-core/src/ingredients.rs builds the same pattern.
const ONES = "one|two|three|four|five|six|seven|eight|nine"
const TIME_WORD = String.raw`\b(?:(?:twenty|thirty|forty|fifty|sixty|ninety)(?:[\s-](?:${ONES}))?|ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|${ONES}|a\s+couple(?:\s+of)?|half\s+an?|an?)`
const TIME_NUMBER = String.raw`(?:(?:[0-9]+\s+[0-9]+/[0-9]+|[0-9]+/[0-9]+|[0-9]*\.[0-9]+|[0-9]+(?:\s*[¼½¾⅓⅔⅕⅙⅚⅛⅜⅝⅞])?|[¼½¾⅓⅔⅕⅙⅚⅛⅜⅝⅞]|${TIME_WORD})(?:\s+and\s+a\s+half)?)`
const TIME_UNIT = String.raw`(?:hours?|hrs?|h|minutes?|mins?|m|seconds?|secs?)\b`
// "25–30 minutes", "15 more minutes", "an hour and a half"
const TIME = new RegExp(
  String.raw`(${TIME_NUMBER})(?:\s*(?:-|–|—|to|or)\s*(${TIME_NUMBER}))?(?:\s+(?:more|additional|extra|further))?\s*(${TIME_UNIT})(\s+and\s+a\s+half\b)?`,
  "gi",
)
// The smaller part of "1 hour 40 minutes" or "2 minutes, 30 seconds", right after the first
const TIME_REST = new RegExp(String.raw`^,?\s*(?:and\s+)?(${TIME_NUMBER})\s*(${TIME_UNIT})`, "i")

const HAS_DIGIT = /[0-9¼½¾⅓⅔⅕⅙⅚⅛⅜⅝⅞]/

/** The shortest and longest timer: ten seconds to a day. */
export const TIMER_MIN = 10
export const TIMER_MAX = 24 * 3600
/** What "Set a timer" starts from on a step that mentions no time. */
export const TIMER_DEFAULT = 5 * 60

function timeNumber(text: string): number | null {
  let t = text.toLowerCase().replace(/\s+/g, " ").trim()
  let half = 0
  if (t.endsWith(" and a half")) {
    half = 0.5
    t = t.slice(0, -" and a half".length)
  }
  let n: number | null
  if (t.startsWith("half ")) n = 0.5
  else if (t.startsWith("a couple")) n = 2
  else if (/^[a-z]/.test(t)) {
    n = 0
    for (const w of t.split(/[ -]/)) n += NUMBER_WORDS[w] ?? 0
  } else n = parseNumber(t)
  return n === null ? null : n + half
}

function unitSeconds(unit: string): number {
  const u = unit.toLowerCase()
  return u.startsWith("h") ? 3600 : u.startsWith("s") ? 1 : 60
}

/**
 * Finds durations in a step ("bake 25–30 minutes", "1 hour 40 minutes", "three to four
 * minutes", "an hour and a half"). A range counts as its longer end.
 */
export function findTimers(step: string): { label: string; seconds: number }[] {
  const out: { label: string; seconds: number }[] = []
  TIME.lastIndex = 0
  let m: RegExpExecArray | null
  while ((m = TIME.exec(step))) {
    const number = m[2] ?? m[1]!
    const unit = m[3]!
    // "h" and "m" only after digits: "a M&M" isn't a minute
    if (unit.length === 1 && !HAS_DIGIT.test(number)) continue
    const value = timeNumber(number)
    if (value === null) continue
    const mult = unitSeconds(unit)
    let seconds = (value + (m[4] ? 0.5 : 0)) * mult
    let label = m[0]
    const rest = step.slice(TIME.lastIndex).match(TIME_REST)
    if (rest && !m[4] && unitSeconds(rest[2]!) < mult) {
      const more = timeNumber(rest[1]!)
      if (more !== null && (rest[2]!.length > 1 || HAS_DIGIT.test(rest[1]!))) {
        seconds += more * unitSeconds(rest[2]!)
        label += rest[0]
        TIME.lastIndex += rest[0].length
      }
    }
    seconds = Math.round(seconds)
    if (seconds >= TIMER_MIN && seconds <= TIMER_MAX) out.push({ label: label.trim(), seconds })
  }
  return out
}

/** A step's timer choices, one per length: the first is the one cook mode suggests. */
export function suggestTimers(step: string): { label: string; seconds: number }[] {
  const found = findTimers(step)
  return found.filter((t, i) => found.findIndex((u) => u.seconds === t.seconds) === i)
}

/**
 * One tap longer or shorter: 15 seconds under a minute, a minute to half an hour, 5 minutes
 * to two hours, then 15, landing on whole steps ("4 min 30 sec" goes to 5, then 6).
 */
export function nudgeTimer(seconds: number, longer: boolean): number {
  const from = longer ? seconds : seconds - 1
  const step = from < 60 ? 15 : from < 1800 ? 60 : from < 7200 ? 300 : 900
  const next = longer
    ? Math.floor(seconds / step) * step + step
    : Math.ceil(seconds / step) * step - step
  return Math.min(TIMER_MAX, Math.max(TIMER_MIN, next))
}

/** A length the cook typed in minutes, kept within a timer's range; null if it isn't one. */
export function timerFromMinutes(minutes: number): number | null {
  if (!Number.isFinite(minutes) || minutes <= 0) return null
  return Math.min(TIMER_MAX, Math.max(TIMER_MIN, Math.round(minutes * 60)))
}

/** A timer's length in words: "1 hr 30 min", "45 sec". */
export function timerWords(seconds: number): string {
  const s = Math.max(0, Math.round(seconds))
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const sec = s % 60
  const parts = [h && `${h} hr`, m && `${m} min`, sec && `${sec} sec`].filter(Boolean)
  return parts.length ? parts.join(" ") : "0 sec"
}

// ─── Ingredients used in a step (cooking mode) ────────────────────────────

/** Words that describe an ingredient rather than name it ("2 large eggs, chilled"). */
const DESCRIPTORS = new Set(
  (
    "large small medium big fresh freshly ground chopped minced diced sliced grated shredded " +
    "chilled cold warm hot softened melted room temperature unsalted salted extra virgin full " +
    "fat low reduced light dark packed finely roughly coarsely thinly organic whole raw dried " +
    "frozen ripe boneless skinless plain pure stick sticks piece pieces optional divided heaping " +
    "level good quality homemade store bought prepared canned jarred fine coarse about more " +
    "plus taste needed serving garnish baking and or of the a an for to with into in on at"
  ).split(" "),
)
/**
 * Words for a form or cut of something ("pumpkin puree", "garlic cloves", "pork belly"): a step
 * that names only the first part ("the pumpkin") still means this ingredient. Not "paste",
 * "sugar" or "vinegar": "the tomatoes" isn't tomato paste and "golden brown" isn't brown sugar.
 */
const FORMS = new Set(
  (
    "puree clove leave leaf fillet breast thigh leg drumstick wing chop loin tenderloin belly " +
    "shoulder steak rib mince noodle floret stalk sprig head bulb wedge juice zest"
  ).split(" "),
)
/** Words in a section heading that don't name what the section makes ("Make the filling"). */
const SECTION_FILLER = new Set(
  "make making prepare prep for the a an and of to your step start finish assemble assembly".split(
    " ",
  ),
)

/** "Tomatoes" and "tomato", "cherries" and "cherry", "eggs" and "egg" compare equal. */
function stem(word: string): string {
  if (word.length > 4 && word.endsWith("ies")) return `${word.slice(0, -3)}y`
  if (word.length > 4 && /(?:ches|shes|xes|oes|sses)$/.test(word)) return word.slice(0, -2)
  if (word.length > 3 && word.endsWith("s") && !word.endsWith("ss")) return word.slice(0, -1)
  return word
}

function words(text: string): string[] {
  return text
    .toLowerCase()
    .replace(/[^a-z\s-]/g, " ")
    .split(/[\s-]+/)
    .filter((w) => w.length >= 3)
    .map(stem)
}

/** The words that name an ingredient: "¾ cup light brown sugar" → brown, sugar. */
function nameWords(raw: string): string[] {
  // Stray brackets and alternatives the parser leaves behind: "oil (canola", "/ 1/2 cup butter )"
  const name = parseIngredient(raw).name.split(/[()]/)[0]!
  const all = words(name).filter((w) => !DESCRIPTORS.has(w) && !aliasToUnit.has(w))
  return [...new Set(all)]
}

/**
 * The ingredients a step uses, from the words it mentions.
 *
 * - When the step's section shares a word with an ingredient section ("Make the Crust" and
 *   "Pie Crust"), that section's ingredients are tried first, so "sugar" in the crust steps is
 *   the crust's sugar, not the filling's.
 * - Each word in the step goes to the ingredients it names most completely: "pumpkin" alone is
 *   the pumpkin puree, while "pumpkin pie spice" is the spice.
 * - Words are matched whole, so "salt" doesn't match "unsalted butter", and a stray word
 *   doesn't count: "pie crust" isn't the pumpkin pie spice.
 */
export function ingredientsForStep<T extends { raw: string; section: string | null }>(
  step: { text: string; section: string | null },
  ingredients: T[],
): T[] {
  const mentioned = new Set(words(step.text))
  if (!mentioned.size) return []
  const named = ingredients.map((ing) => {
    const ws = nameWords(ing.raw)
    const hits = ws.filter((w) => mentioned.has(w))
    // A partial mention counts if it's the main word ("sugar" for brown sugar), or names what a
    // form or cut is of ("pumpkin" for pumpkin puree); not a stray word ("pie" for pumpkin pie
    // spice, "brown" for brown sugar)
    const head = ws[ws.length - 1]!
    const counts = hits.includes(head) || (FORMS.has(head) && hits.length * 2 >= ws.length)
    return { ing, ws, hits: counts ? hits : [] }
  })

  const stepSection = new Set(words(step.section ?? "").filter((w) => !SECTION_FILLER.has(w)))
  const inSection = (section: string | null) =>
    words(section ?? "").some((w) => !SECTION_FILLER.has(w) && stepSection.has(w))
  const local = named.filter((n) => inSection(n.ing.section))

  // Hand each mentioned word to the ingredients it covers best, section first
  const chosen = new Set<T>()
  const claimed = new Set<string>()
  for (const pool of [local, named]) {
    const best = new Map<string, { ratio: number; ings: T[] }>()
    for (const { ing, ws, hits } of pool) {
      for (const w of hits) {
        if (claimed.has(w)) continue
        const ratio = hits.length / ws.length
        const cur = best.get(w)
        if (!cur || ratio > cur.ratio) best.set(w, { ratio, ings: [ing] })
        else if (ratio === cur.ratio) cur.ings.push(ing)
      }
    }
    for (const [w, { ings }] of best) {
      claimed.add(w)
      for (const ing of ings) chosen.add(ing)
    }
  }
  return ingredients.filter((ing) => chosen.has(ing))
}
