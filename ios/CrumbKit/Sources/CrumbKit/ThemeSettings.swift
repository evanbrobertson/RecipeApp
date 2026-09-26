import Foundation
import Observation

/// The More page's Theme setting. "Sunrise & sunset" is the default, as on the web.
public enum ThemeMode: String, CaseIterable, Identifiable, Sendable {
  case light, dark, system, sun

  public var id: String { rawValue }

  public var label: String {
    switch self {
    case .light: return "Light"
    case .dark: return "Dark"
    case .system: return "System"
    case .sun: return "Sunrise & sunset"
    }
  }

  public var detail: String {
    switch self {
    case .light: return "Always light"
    case .dark: return "Always dark"
    case .system: return "Follows your iPhone"
    case .sun: return "Dark from sunset to sunrise"
    }
  }
}

/// Light or dark, and when to look again.
public struct ThemeDecision: Equatable, Sendable {
  /// nil: follow the system.
  public var dark: Bool?
  /// When "Sunrise & sunset" next flips (nil for the other modes).
  public var recheckAt: Date?
}

/// Theme mode and the location saved by "Use my location", on this phone only. The sunrise
/// maths is crumb-core's (the web's theme-boot.js), so the phone flips when the web does.
@Observable
public final class ThemeSettings {
  public var mode: ThemeMode {
    didSet { defaults.set(mode.rawValue, forKey: Self.modeKey) }
  }
  public var location: SunLocation? {
    didSet {
      if let location {
        defaults.set([location.lat, location.lng], forKey: Self.locationKey)
      } else {
        defaults.removeObject(forKey: Self.locationKey)
      }
    }
  }

  private let defaults: UserDefaults
  static let modeKey = "theme"
  static let locationKey = "sun-location"

  public init(defaults: UserDefaults = AppGroup.defaults) {
    self.defaults = defaults
    mode = ThemeMode(rawValue: defaults.string(forKey: Self.modeKey) ?? "") ?? .sun
    if let saved = defaults.array(forKey: Self.locationKey) as? [Double], saved.count == 2 {
      location = SunLocation(lat: saved[0], lng: saved[1])
    } else {
      location = nil
    }
  }

  /// What to show at `now`, in `zone` (for the estimate when no location is saved).
  public func decide(now: Date = Date(), zone: TimeZone = .current) -> ThemeDecision {
    switch mode {
    case .light: return ThemeDecision(dark: false)
    case .dark: return ThemeDecision(dark: true)
    case .system: return ThemeDecision(dark: nil)
    case .sun:
      let sun = CrumbCore.sunState(
        nowMs: now.timeIntervalSince1970 * 1000, location: location ?? Self.estimate(zone: zone, now: now))
      // Flip at sunrise/sunset, re-checked at least hourly (as theme-boot.js does)
      let wait = min(max(sun.nextChangeMs / 1000 - now.timeIntervalSince1970 + 1, 60), 3600)
      return ThemeDecision(dark: sun.dark, recheckAt: now.addingTimeInterval(wait))
    }
  }

  /// A location from the time zone alone: its standard (non-DST) offset and its region.
  public static func estimate(zone: TimeZone, now: Date = Date()) -> SunLocation {
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = zone
    let year = calendar.component(.year, from: now)
    let offsets = [1, 7].compactMap { month in
      calendar.date(from: DateComponents(year: year, month: month, day: 1, hour: 12))
        .map { zone.secondsFromGMT(for: $0) }
    }
    let standard = offsets.min() ?? zone.secondsFromGMT(for: now)
    return CrumbCore.estimateLocation(zoneId: zone.identifier, standardOffsetEastSecs: Int32(standard))
  }
}
