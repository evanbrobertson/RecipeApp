/** Wording for Wee Chef's import checks (see src/checks.rs). */

import type { CheckFlag, ChecksStatus } from "./recipe"

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
    case "category": {
      const { category, was } = f.detail ?? {}
      if (category && was) return `Changed the category from ${quote(was)} to ${category}`
      if (category) return `Set the category to ${category}`
      return `Cleared the category ${quote(was ?? "")}: it isn't one of Crumb's`
    }
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

/**
 * One line on where Wee Chef's Check all stands: "5 recipes to check (2 restored) · …",
 * "Checking… 3 of 12 done", "All 79 recipes checked".
 */
export function checksStatusText(c: ChecksStatus, run?: number): string {
  // `run`: how many this Check all queued, so progress counts failures as done too
  if (c.pending > 0 && run && run >= c.pending) return `Checking… ${run - c.pending} of ${run} done`
  if (c.pending > 0) return `Checking… ${c.checked} of ${c.eligible} done`
  const parts: string[] = []
  if (c.due > 0) {
    // Never checked, restored from a backup, or edited since Wee Chef last looked
    const why = [
      c.restored && `${c.restored} restored`,
      c.edited && `${c.edited} edited since`,
    ].filter(Boolean)
    parts.push(`${plural(c.due, "recipe")} to check${why.length ? ` (${why.join(", ")})` : ""}`)
    // On recipes already in the box it only suggests (plus an undoable symbol clean-up)
    parts.push("suggests fixes, tidies only stray symbols")
  } else if (c.checked < c.eligible) {
    // The rest failed too often; a recipe's own menu can still try again
    parts.push(`${c.checked} of ${c.eligible} checked`)
  } else {
    parts.push(`All ${plural(c.checked, "recipe")} checked`)
  }
  if (c.tidied) parts.push(`${plural(c.tidied, "thing")} tidied`)
  if (c.toCheck) parts.push(`${plural(c.toCheck, "recipe")} to look at`)
  return parts.join(" · ")
}
