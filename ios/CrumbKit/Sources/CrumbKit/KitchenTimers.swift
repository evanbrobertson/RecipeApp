import Foundation
import Observation

/// One running kitchen timer: "Simmer" for 20 minutes from a cook-mode step.
public struct KitchenTimer: Codable, Hashable, Identifiable, Sendable {
  public var id: UUID
  public var label: String
  public var recipeId: Int64?
  public var recipeTitle: String?
  public var seconds: Int
  /// When it goes off.
  public var endsAt: Date

  public init(
    id: UUID = UUID(), label: String, recipeId: Int64? = nil, recipeTitle: String? = nil,
    seconds: Int, endsAt: Date
  ) {
    self.id = id
    self.label = label
    self.recipeId = recipeId
    self.recipeTitle = recipeTitle
    self.seconds = seconds
    self.endsAt = endsAt
  }

  public func remaining(at now: Date) -> TimeInterval {
    max(0, endsAt.timeIntervalSince(now))
  }

  public func isDone(at now: Date) -> Bool {
    endsAt <= now
  }

  /// "4:05", counting down (crumb-core's `formatClock`, as on the web).
  public func clock(at now: Date) -> String {
    CrumbCore.formatClock(seconds: remaining(at: now).rounded(.up))
  }
}

/// Tells the system about timers so they ring with the app closed or the phone locked.
public protocol TimerAlarms: AnyObject {
  func schedule(_ timer: KitchenTimer)
  func cancel(_ id: UUID)
}

/// The timers running on this phone. They live in the App Group's defaults, so they survive
/// the app being closed; each one is also a local notification, so it rings anyway.
@Observable
public final class KitchenTimers {
  public private(set) var timers: [KitchenTimer] = []

  private let defaults: UserDefaults
  @ObservationIgnored private weak var alarms: TimerAlarms?
  static let key = "kitchen-timers"

  public init(defaults: UserDefaults = AppGroup.defaults, alarms: TimerAlarms? = nil) {
    self.defaults = defaults
    self.alarms = alarms
    if let data = defaults.data(forKey: Self.key),
      let saved = try? JSONDecoder().decode([KitchenTimer].self, from: data)
    {
      timers = saved.sorted { $0.endsAt < $1.endsAt }
    }
  }

  public func attach(_ alarms: TimerAlarms) {
    self.alarms = alarms
  }

  /// Starts a timer and returns it. Starting the same step's timer again restarts it.
  @discardableResult
  public func start(
    label: String, seconds: Int, recipeId: Int64? = nil, recipeTitle: String? = nil, now: Date = Date()
  ) -> KitchenTimer {
    if let same = timers.first(where: { $0.label == label && $0.recipeId == recipeId }) {
      dismiss(same.id)
    }
    let timer = KitchenTimer(
      label: label, recipeId: recipeId, recipeTitle: recipeTitle, seconds: seconds,
      endsAt: now.addingTimeInterval(TimeInterval(seconds)))
    timers.append(timer)
    timers.sort { $0.endsAt < $1.endsAt }
    save()
    alarms?.schedule(timer)
    return timer
  }

  /// Adds a minute to a running or finished timer.
  public func addMinute(_ id: UUID, now: Date = Date()) {
    guard let i = timers.firstIndex(where: { $0.id == id }) else { return }
    let base = max(timers[i].endsAt, now)
    timers[i].endsAt = base.addingTimeInterval(60)
    timers[i].seconds += 60
    timers.sort { $0.endsAt < $1.endsAt }
    save()
    alarms?.cancel(id)
    if let timer = timers.first(where: { $0.id == id }) { alarms?.schedule(timer) }
  }

  public func dismiss(_ id: UUID) {
    timers.removeAll { $0.id == id }
    save()
    alarms?.cancel(id)
  }

  /// Timers that have finished (ringing until dismissed).
  public func done(at now: Date = Date()) -> [KitchenTimer] {
    timers.filter { $0.isDone(at: now) }
  }

  private func save() {
    if let data = try? JSONEncoder().encode(timers) {
      defaults.set(data, forKey: Self.key)
    }
  }
}
