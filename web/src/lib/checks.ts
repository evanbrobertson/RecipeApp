/** Wording for Wee Chef's import checks (see src/checks.rs). */

import type { CheckFlag } from "./recipe"

/** A line quoted in a sentence, shortened when long. */
export function quote(text: string, max = 48): string {
  const t = text.trim()
  return `“${t.length > max ? `${t.slice(0, max - 1).trimEnd()}…` : t}”`
}

/** What Wee Chef did to a line on import, e.g. Made “Sauce:” a section heading. */
export function fixText(f: CheckFlag): string {
  switch (f.detail?.fix) {
    case "heading":
      return `Made ${quote(f.itemText)} a section heading`
    case "removed":
      return `Removed ${quote(f.itemText)}`
    case "notes":
      return `Moved a tip to the notes: ${quote(f.itemText)}`
    case "joined":
      return `Joined a step that was split in two: ${quote(f.itemText)}`
    case "tidy":
      return "Cleaned up stray checkboxes, web codes, repeated lines, quantities or times"
    default:
      return `Tidied ${quote(f.itemText)}`
  }
}

/** Why a line might need a look: “Sauce:” {looks like a section heading}. */
export function reviewText(f: CheckFlag): string {
  switch (f.kind) {
    case "heading":
      return "looks like a section heading"
    case "junk":
      return "doesn't look like part of the recipe"
    case "fragment":
      return "looks like part of the step next to it"
    case "not_instruction":
      return "reads like a tip rather than a step"
    case "merged":
      return "might be two ingredients on one line"
    case "step":
      return "looks like a step, not an ingredient"
    case "ingredient":
      return "looks like an ingredient, not a step"
    default:
      return "might need a look"
  }
}

export const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`
