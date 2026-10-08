/**
 * The bookshelf's layout: which books stand and which lie down, how they gather into runs and
 * stacks, and where everything sits on each shelf. Pure and seeded, so a shelf looks the same on
 * every load (and, once ported to crumb-core, in every app). Units are CSS px (dp in the apps).
 *
 * A title that fits up a spine stands; one that doesn't lies down, spine out, so it reads left to
 * right. Standing books gather into runs (the end one sometimes leaning on its neighbour); lying
 * books gather into stacks, longest at the foot, each a little off true.
 */
import { seeded, type ShelfBook } from "./books"

/** DM Serif Display advance widths, thousandths of an em, for " " to "~" (from the font file). */
// prettier-ignore
const ADVANCE = [
  218, 333, 425, 557, 532, 871, 721, 223, 368, 368, 433, 541, 300, 332, 300, 342, 502, 350, 532,
  532, 538, 532, 530, 489, 502, 532, 300, 300, 541, 541, 541, 506, 877, 650, 598, 603, 648, 555,
  538, 656, 691, 301, 427, 654, 540, 791, 653, 670, 565, 670, 628, 521, 627, 649, 632, 921, 614,
  590, 560, 342, 336, 342, 541, 532, 400, 518, 581, 494, 574, 507, 343, 531, 580, 280, 281, 582,
  286, 871, 578, 545, 578, 565, 441, 436, 361, 567, 530, 781, 568, 536, 465, 338, 267, 338, 541,
]
/** Anything outside ASCII: a wide lowercase letter, so an estimate errs towards lying down. */
const OTHER_ADVANCE = 580

/** Px a title takes in DM Serif Display at `size` px. */
export function titleWidth(text: string, size: number) {
  let units = 0
  for (const ch of text) {
    const c = ch.codePointAt(0)!
    units += c >= 32 && c < 127 ? ADVANCE[c - 32]! : OTHER_ADVANCE
  }
  return (units * size) / 1000
}

export const SHELF = {
  /** Inside height of one shelf: the tallest book and its top edge, with a finger's room */
  clearance: 206,
  /** The plank: its top face and front edge */
  plankTop: 8,
  plankFront: 14,
  /** Space under a plank for its brackets, before the next shelf's books */
  under: 26,
  /** The top edge of a book (its pages, or its cover when it lies down), seen from a little above */
  topFace: 5,
  standing: { minH: 150, maxH: 196, minW: 32, maxW: 52 },
  lying: { minL: 150, maxL: 248, minT: 30, maxT: 46 },
  /** Highest a stack goes, so a hand still fits over it */
  stackMax: 184,
  /** Space at the shelf's two ends */
  inset: 18,
  pot: { w: 46, h: 68 },
  crock: { w: 52, h: 96 },
  /** The dashed "new book" outline */
  add: { w: 46, h: 160 },
} as const

/** Rows are this far apart, plank included. */
export const ROW_HEIGHT = SHELF.clearance + SHELF.plankTop + SHELF.plankFront + SHELF.under

/**
 * How a spine is dressed: plain cloth, gilt rules at each end, a paper label holding the title, or
 * contrasting cloth at head and foot.
 */
export type SpineStyle = "plain" | "rules" | "label" | "ends"

export function spineStyle(book: ShelfBook): SpineStyle {
  const r = seeded(book.id, 3)
  return r < 0.3 ? "plain" : r < 0.6 ? "rules" : r < 0.8 ? "label" : "ends"
}

/** Spine length that holds no title, both ends together, by style. */
const END_ROOM: Record<SpineStyle, number> = { plain: 34, rules: 52, label: 46, ends: 60 }

const STANDING_FONT = 15
const LYING_FONT = 15
const LYING_FONT_TWO = 14

/** One book's spine, before it's placed. Box sizes are as drawn: w across, h up. */
export interface Spine {
  book: ShelfBook
  standing: boolean
  w: number
  h: number
  style: SpineStyle
  /** The title, as it breaks: one line, or two on a lying book */
  lines: string[]
  font: number
}

/** Whether a title fits up a standing spine, and the spine it gets either way. */
export function spineFor(book: ShelfBook): Spine {
  const style = spineStyle(book)
  const room = END_ROOM[style]
  const name = book.name.trim() || "Untitled"
  const { standing: S, lying: L } = SHELF
  const upright = titleWidth(name, STANDING_FONT) + room
  if (upright <= S.maxH) {
    const w = Math.round(Math.min(S.maxW, S.minW + book.recipeCount * 0.8))
    const h = Math.round(Math.max(upright, S.minH + seeded(book.id, 5) * (S.maxH - S.minH)))
    return {
      book,
      standing: true,
      w,
      h: Math.min(h, S.maxH),
      style,
      lines: [name],
      font: STANDING_FONT,
    }
  }
  // Too long to stand without squeezing: lay it down, on two lines if one won't do
  let t = Math.round(Math.min(L.maxT, L.minT + book.recipeCount * 0.6))
  let lines = [name]
  let font: number = LYING_FONT
  let need = titleWidth(name, LYING_FONT) + room
  if (need > L.maxL) {
    lines = splitTitle(name, LYING_FONT_TWO)
    font = LYING_FONT_TWO
    need = Math.max(...lines.map((l) => titleWidth(l, font))) + room
    if (lines.length > 1) t = Math.max(t, 44)
  }
  const len = Math.round(Math.max(need, L.minL + seeded(book.id, 5) * 40))
  return { book, standing: false, w: Math.min(len, L.maxL), h: t, style, lines, font }
}

/** Two lines with the longer one as short as it can be, broken between words. */
export function splitTitle(name: string, size: number): string[] {
  const words = name.trim().split(/\s+/)
  if (words.length < 2) return [name]
  let best = [name]
  let bestWidth = Infinity
  for (let i = 1; i < words.length; i++) {
    const a = words.slice(0, i).join(" ")
    const b = words.slice(i).join(" ")
    const width = Math.max(titleWidth(a, size), titleWidth(b, size))
    if (width < bestWidth) {
      bestWidth = width
      best = [a, b]
    }
  }
  return best
}

/** Where one thing sits on its shelf. */
export interface PlacedBook extends Spine {
  kind: "book"
  /** Left of the box, from the shelf's left edge, and its foot, up from the plank */
  x: number
  y: number
  /** Degrees, clockwise, about `pivot` */
  tilt: number
  /** The corner a leaning book rests on, or the middle for a book in a stack */
  pivot: "left" | "right" | "center"
  /** Draw order within its shelf */
  z: number
  /** The top book of a stack, or any standing book: its top edge shows */
  top: boolean
  /** The stack it lies in (unique within its shelf), or null when it stands */
  stack: number | null
}

export interface PlacedThing {
  kind: "pot" | "crock" | "add"
  x: number
  y: number
  w: number
  h: number
  z: number
  /** For a pot on a stack: the stack, and the book it sits on */
  stack?: number
  on?: number
}

export type Placed = PlacedBook | PlacedThing

/** Where an opened book came from, so it can fly off the shelf and back. */
export interface BookOrigin {
  /** Its spine on the shelf */
  el: HTMLElement
  spine: PlacedBook
  /** Called as it starts back, so whatever sat on it can make room */
  back?: () => void
}

export interface ShelfRow {
  items: Placed[]
  /** The row's width, at least the shelf's */
  width: number
}

export interface ShelfOptions {
  /** The shelf's inside width */
  width: number
  /** One row as long as it needs, for a shelf that scrolls sideways */
  single?: boolean
  /** End with a dashed outline for a new book */
  addable?: boolean
}

type Group = { kind: "run"; spines: Spine[] } | { kind: "stack"; spines: Spine[] }

/**
 * Gathers spines into runs and stacks, keeping the books' order but for a lying book, which joins
 * the stack nearest behind it (at most a few books back) rather than lie alone.
 */
export function groupSpines(spines: Spine[]): Group[] {
  const groups: Group[] = []
  let stack: { spines: Spine[] } | null = null
  let since = 0
  for (const spine of spines) {
    if (spine.standing) {
      const last = groups[groups.length - 1]
      const cap = 4 + Math.floor(seeded(last?.spines[0]?.book.id ?? 0, 19) * 5)
      if (last?.kind === "run" && last.spines.length < cap) last.spines.push(spine)
      else groups.push({ kind: "run", spines: [spine] })
      since++
      continue
    }
    const height = stack ? stack.spines.reduce((sum, s) => sum + s.h, 0) : 0
    if (stack && since <= 3 && height + spine.h <= SHELF.stackMax) {
      stack.spines.push(spine)
    } else {
      stack = { spines: [spine] }
      groups.push({ kind: "stack", spines: stack.spines })
      since = 0
    }
  }
  return groups
}

const rad = (deg: number) => (deg * Math.PI) / 180

interface Lean {
  /** Degrees; negative leans left */
  tilt: number
  /** Space between the neighbour and the leaning book's foot */
  gap: number
  /** Width the leaning book takes, gap included */
  span: number
}

/**
 * A book resting on its neighbour at `deg`: its top corner against the neighbour's side, or
 * against the neighbour's top corner when it's the taller of the two.
 */
function leanOn(book: Spine, neighbour: Spine, deg: number): Lean {
  const a = rad(deg)
  const gap = Math.min(book.h * Math.sin(a), neighbour.h * Math.tan(a))
  return { tilt: deg, gap, span: gap + book.w * Math.cos(a) }
}

interface RunPlan {
  spines: Spine[]
  /** The last book leans left on the one before it */
  end: Lean | null
  /** The first book leans right on the one after it */
  start: Lean | null
  width: number
}

/** How a run sits: books shoulder to shoulder, with an end book that may lean. */
function planRun(spines: Spine[]): RunPlan {
  const n = spines.length
  let end: Lean | null = null
  let start: Lean | null = null
  if (n >= 3) {
    const last = spines[n - 1]!
    if (seeded(last.book.id, 13) < 0.5)
      end = leanOn(last, spines[n - 2]!, 7 + seeded(last.book.id, 17) * 8)
  }
  if (n >= 4 && !end) {
    const first = spines[0]!
    if (seeded(first.book.id, 23) < 0.4)
      start = leanOn(first, spines[1]!, 6 + seeded(first.book.id, 29) * 7)
  }
  let width = 0
  spines.forEach((s, i) => {
    if (i === n - 1 && end) width += end.span
    else if (i === 0 && start) width += start.span
    else width += s.w + (i ? 1 : 0)
  })
  return { spines, end, start, width }
}

function placeRun(plan: RunPlan, x0: number, z0: number): PlacedBook[] {
  const out: PlacedBook[] = []
  let x = x0
  const n = plan.spines.length
  plan.spines.forEach((s, i) => {
    const base = { ...s, kind: "book" as const, y: 0, z: z0 + i, top: true, stack: null }
    if (i === 0 && plan.start) {
      // Pivots on its right foot, its top resting on the next book's side
      const right = x + plan.start.span - plan.start.gap
      out.push({ ...base, x: right - s.w, tilt: plan.start.tilt, pivot: "right" })
      x += plan.start.span + 1
    } else if (i === n - 1 && plan.end) {
      out.push({ ...base, x: x + plan.end.gap, tilt: -plan.end.tilt, pivot: "left" })
      x += plan.end.span
    } else {
      out.push({ ...base, x, tilt: 0, pivot: "left" })
      x += s.w + 1
    }
  })
  return out
}

interface StackPlan {
  /** Bottom to top */
  spines: Spine[]
  offsets: number[]
  tilts: number[]
  width: number
  height: number
}

/** Longest at the foot, mostly; each book a few px off the one below and a hair off level. */
function planStack(spines: Spine[]): StackPlan {
  const sorted = [...spines].sort((a, b) => b.w - a.w)
  // A near tie swaps now and then, so it isn't too tidy
  for (let i = 0; i + 1 < sorted.length; i++) {
    const a = sorted[i]!
    const b = sorted[i + 1]!
    if (a.w - b.w < 18 && seeded(a.book.id + b.book.id, 31) < 0.5) {
      sorted[i] = b
      sorted[i + 1] = a
    }
  }
  const widest = Math.max(...sorted.map((s) => s.w))
  const offsets = sorted.map((s, i) => {
    const centred = (widest - s.w) / 2
    if (!i) return centred
    return centred + Math.round((seeded(s.book.id, 37) * 2 - 1) * 9)
  })
  const tilts = sorted.map((s, i) =>
    i ? Math.round((seeded(s.book.id, 41) * 2 - 1) * 12) / 10 : 0,
  )
  const left = Math.min(0, ...offsets)
  const right = Math.max(...sorted.map((s, i) => offsets[i]! + s.w))
  return {
    spines: sorted,
    offsets: offsets.map((o) => o - left),
    tilts,
    width: right - left,
    height: sorted.reduce((sum, s) => sum + s.h, 0),
  }
}

function placeStack(plan: StackPlan, x0: number, z0: number): PlacedBook[] {
  let y = 0
  const placed = plan.spines.map((s, i) => {
    const book: PlacedBook = {
      ...s,
      kind: "book",
      x: x0 + plan.offsets[i]!,
      y,
      tilt: plan.tilts[i]!,
      pivot: "center",
      z: z0 + i,
      top: i === plan.spines.length - 1,
      stack: z0,
    }
    y += s.h
    return book
  })
  // Listed top to bottom, the way the stack reads (and tabs)
  return placed.toReversed()
}

type Piece =
  | { kind: "run"; plan: RunPlan }
  | { kind: "stack"; plan: StackPlan }
  | { kind: "pot" | "crock" | "add"; width: number }

const pieceWidth = (p: Piece) => (p.kind === "run" || p.kind === "stack" ? p.plan.width : p.width)

/** Space before a piece: books bunch, props get a little room. */
function gapBefore(p: Piece, i: number) {
  const id = p.kind === "run" || p.kind === "stack" ? p.plan.spines[0]!.book.id : i
  return Math.round(14 + seeded(id, 43) * 20)
}

/**
 * Lays the books out on as many shelves as the width needs. Rows list their things left to right,
 * a stack's books top to bottom, so the DOM (and tab) order follows what you see.
 */
export function layoutShelf(books: ShelfBook[], opts: ShelfOptions): ShelfRow[] {
  const groups = groupSpines(books.map(spineFor))
  if (opts.single) return [placeRow(fill(groups, Infinity, opts)[0]!, 0, opts, Infinity)]
  const inner = Math.max(120, opts.width - SHELF.inset * 2)
  // Fill shelves greedily, then again to an even share each so the last one isn't left bare
  let rows = fill(groups, inner, opts)
  if (rows.length > 1) {
    const total = rows.reduce((sum, r) => sum + rowContent(r), 0)
    const widest = Math.max(...rows.flat().map(pieceWidth))
    for (let share = Math.max(widest, total / rows.length); share < inner; share += 16) {
      const even = fill(groups, share, opts)
      if (even.length === rows.length) {
        rows = even
        break
      }
    }
  }
  return rows.map((pieces, ri) => placeRow(pieces, ri, opts, inner))
}

const rowContent = (pieces: Piece[]) =>
  pieces.reduce((sum, p, i) => sum + pieceWidth(p) + (i ? gapBefore(p, i) : 0), 0)

/** Fills shelves up to `inner` wide, splitting a run of standing books that doesn't fit. */
function fill(groups: Group[], inner: number, opts: ShelfOptions): Piece[][] {
  const rows: Piece[][] = [[]]
  const fits = (p: Piece) => {
    const row = rows[rows.length - 1]!
    return rowContent(row) + (row.length ? gapBefore(p, row.length) : 0) + pieceWidth(p) <= inner
  }
  const push = (p: Piece) => {
    if (!fits(p) && rows[rows.length - 1]!.length) rows.push([])
    rows[rows.length - 1]!.push(p)
  }
  for (const g of groups) {
    if (g.kind === "stack") {
      push({ kind: "stack", plan: planStack(g.spines) })
      continue
    }
    let rest = g.spines
    while (rest.length) {
      const whole: Piece = { kind: "run", plan: planRun(rest) }
      if (fits(whole)) {
        push(whole)
        break
      }
      // As many as fit here; the rest start the next shelf
      let k = rest.length - 1
      while (k > 0 && !fits({ kind: "run", plan: planRun(rest.slice(0, k)) })) k--
      if (k === 0) {
        if (rows[rows.length - 1]!.length) {
          rows.push([])
          continue
        }
        k = 1
      }
      push({ kind: "run", plan: planRun(rest.slice(0, k)) })
      rest = rest.slice(k)
      rows.push([])
    }
  }
  if (!rows[rows.length - 1]!.length && rows.length > 1) rows.pop()
  if (opts.addable) push({ kind: "add", width: SHELF.add.w })
  return rows
}

function placeRow(pieces: Piece[], ri: number, opts: ShelfOptions, inner: number): ShelfRow {
  const gaps = pieces.map((p, i) => (i ? gapBefore(p, i) : 0))
  const content = rowContent(pieces)
  const width = Number.isFinite(inner) ? inner : content
  let spare = Math.max(0, width - content)

  // A herb pot on the first shelf: on a stack with headroom, else on the plank if there's room
  let potOn = -1
  let potAlone = false
  if (ri === 0) {
    let best = -1
    pieces.forEach((p, i) => {
      if (p.kind !== "stack" || p.plan.height + SHELF.pot.h > SHELF.clearance - 6) return
      if (best < 0 || p.plan.height < (pieces[best] as { plan: StackPlan }).plan.height) best = i
    })
    if (best >= 0) potOn = best
    else if (spare >= SHELF.pot.w + 24 || opts.single) potAlone = true
  }
  if (potAlone) spare = Math.max(0, spare - (SHELF.pot.w + 24))
  // A crock of spoons where a lower shelf has room to spare
  const crock = ri > 0 && spare >= SHELF.crock.w + 60 && seeded(ri, 47) < 0.6
  if (crock) spare = Math.max(0, spare - (SHELF.crock.w + 24))

  // Loosen the gaps a little, then centre what's left
  const slack = Math.min(spare, Math.max(0, pieces.length - 1) * 14)
  const each = pieces.length > 1 ? slack / (pieces.length - 1) : 0
  let x = SHELF.inset + (spare - slack) / 2

  const items: Placed[] = []
  let z = 0
  const potX = () => {
    items.push({ kind: "pot", x, y: 0, w: SHELF.pot.w, h: SHELF.pot.h, z: z++ })
    x += SHELF.pot.w + 24
  }
  // The pot goes at the left end on odd rows and the crock at the right, so they don't crowd
  if (potAlone && seeded(pieces.length, 53) < 0.5) potX()
  pieces.forEach((p, i) => {
    if (i) x += gaps[i]! + each
    if (p.kind === "run") {
      const placed = placeRun(p.plan, x, z)
      items.push(...placed)
      z += placed.length
    } else if (p.kind === "stack") {
      const placed = placeStack(p.plan, x, z)
      items.push(...placed)
      z += placed.length
      if (i === potOn) {
        const topBook = placed[0]!
        const at = topBook.x + topBook.w / 2 - SHELF.pot.w / 2 + (seeded(ri + i, 59) * 2 - 1) * 20
        items.push({
          kind: "pot",
          x: at,
          y: p.plan.height,
          w: SHELF.pot.w,
          h: SHELF.pot.h,
          z: z++,
          stack: topBook.stack!,
          on: topBook.book.id,
        })
      }
    } else {
      const size = SHELF[p.kind]
      items.push({ kind: p.kind, x, y: 0, w: size.w, h: size.h, z: z++ })
    }
    x += pieceWidth(p)
  })
  if (potAlone && !items.some((it) => it.kind === "pot")) {
    x += 24
    potX()
  }
  if (crock) {
    x += 24
    items.push({ kind: "crock", x, y: 0, w: SHELF.crock.w, h: SHELF.crock.h, z: z++ })
    x += SHELF.crock.w
  }
  const end = opts.single ? x + SHELF.inset : width + SHELF.inset * 2
  return { items, width: end }
}
