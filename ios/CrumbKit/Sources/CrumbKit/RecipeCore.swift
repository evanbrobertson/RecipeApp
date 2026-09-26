import Foundation

// Recipe logic the screens need, all computed by crumb-core (crates/crumb-core) so the
// iPhone shows exactly what the web, Android and desktop apps show. Nothing here decides
// anything itself; it only moves values across the UniFFI boundary.

/// An ingredient line with its section, flattened for scaling and step matching.
public struct IngredientRow: Hashable, Identifiable, Sendable {
  public var id: String { "\(sectionIndex):\(itemIndex)" }
  public var sectionIndex: Int
  public var itemIndex: Int
  public var section: String?
  public var text: String
}

extension Recipe {
  /// Cook mode's pages: every non-blank instruction with its section.
  public var cookSteps: [CookStep] {
    (try? CrumbCore.cookSteps(recipeJson: coreJSON)) ?? []
  }

  /// Every ingredient line in order, with its section name.
  public var ingredientRows: [IngredientRow] {
    ingredients.enumerated().flatMap { s, section in
      section.items.enumerated().map { i, item in
        IngredientRow(sectionIndex: s, itemIndex: i, section: section.name, text: item)
      }
    }
  }

  /// The ingredients a step uses, preferring its own section ("For the sauce").
  public func ingredients(for step: CookStep) -> [IngredientRow] {
    let rows = ingredientRows
    let lines = rows.map { IngredientLine(raw: $0.text, section: $0.section) }
    return CrumbCore.ingredientsForStep(step: step.text, stepSection: step.section, ingredients: lines)
      .compactMap { i in rows.indices.contains(Int(i)) ? rows[Int(i)] : nil }
  }

  /// Where it came from, as a link the page may show (never a share link).
  public var sourceLink: URL? {
    guard let link = (try? CrumbCore.sourceUrl(recipeJson: coreJSON)) ?? nil else { return nil }
    return URL(string: link)
  }

  /// Plain text to share: title, "•" ingredients, numbered steps, then `link`.
  public func shareText(link: String? = nil) -> String {
    (try? CrumbCore.recipeToText(recipeJson: coreJSON, link: link)) ?? title
  }

  public var markdown: String {
    (try? CrumbCore.recipeToMarkdown(recipeJson: coreJSON)) ?? "# \(title)"
  }

  /// "Breakfast · British".
  public var kicker: String {
    CrumbCore.kicker(category: recipeCategory, cuisine: recipeCuisine)
  }

  /// "bbcgoodfood.com".
  public var sourceHost: String? {
    CrumbCore.hostOf(url: sourceLink?.absoluteString)
  }
}

/// Small helpers the screens call with plain values.
public enum RecipeText {
  /// "1h 30m" from ISO 8601, anything else as written; nil when blank.
  public static func duration(_ raw: String?) -> String? {
    CrumbCore.displayDuration(raw: raw)
  }

  /// One ingredient line at `factor` times the quantity.
  public static func scaled(_ line: String, by factor: Double) -> String {
    factor == 1 ? CrumbCore.fractionize(line: line) : CrumbCore.scaleIngredient(raw: line, factor: factor)
  }

  /// Durations in a step, for one-tap timers.
  public static func timers(in step: String) -> [StepTimer] {
    CrumbCore.findTimers(step: step)
  }

  /// "Cooked 3 times · last 2 weeks ago".
  public static func cookedLine(_ cooked: Cooked, now: Date = Date()) -> String? {
    let last = cooked.lastCookedAt.flatMap(parseISO).map { Int64($0.timeIntervalSince1970 * 1000) }
    return CrumbCore.cookedLine(
      count: UInt32(clamping: cooked.count), lastCookedMs: last, nowMs: Int64(now.timeIntervalSince1970 * 1000))
  }

  /// Wee Chef's wording for a flag: what it did, or why a line needs a look.
  public static func flagText(_ flag: Flag) -> String {
    if flag.state == "fixed" {
      return CrumbCore.fixText(
        fix: flag.detail?["fix"]?.text, itemText: flag.itemText ?? "",
        category: flag.detail?["category"]?.text, was: flag.detail?["was"]?.text)
    }
    return "\(CrumbCore.quote(text: flag.itemText ?? "", max: 48)) \(CrumbCore.reviewText(kind: flag.kind))"
  }

  /// One line on where Wee Chef's Check all stands.
  public static func checksStatus(_ s: ChecksStatus, run: Int? = nil) -> String {
    let n = { (v: Int) in UInt32(clamping: v) }
    return CrumbCore.checksStatusText(
      counts: ChecksCounts(
        eligible: n(s.eligible), checked: n(s.checked), pending: n(s.pending), tidied: n(s.tidied),
        toCheck: n(s.toCheck), due: n(s.due), restored: n(s.restored), edited: n(s.edited)),
      run: run.map(n))
  }

  static func parseISO(_ text: String) -> Date? {
    let f = ISO8601DateFormatter()
    f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    if let d = f.date(from: text) { return d }
    f.formatOptions = [.withInternetDateTime]
    return f.date(from: text)
  }
}
