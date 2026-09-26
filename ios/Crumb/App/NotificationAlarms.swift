import CrumbKit
import Foundation
import UserNotifications

/// Kitchen timers as local notifications: each timer is scheduled with the system when it
/// starts, so it rings with the app in the background, closed, or the phone locked.
final class NotificationAlarms: NSObject, TimerAlarms, UNUserNotificationCenterDelegate {
  private let center = UNUserNotificationCenter.current()
  /// A tapped timer notification: the recipe it belongs to.
  var onOpenRecipe: ((Int64) -> Void)?

  override init() {
    super.init()
    center.delegate = self
  }

  /// Asks once, the first time a timer starts (never at launch).
  func requestPermission() async -> Bool {
    let settings = await center.notificationSettings()
    switch settings.authorizationStatus {
    case .authorized, .provisional, .ephemeral: return true
    case .denied: return false
    default:
      return (try? await center.requestAuthorization(options: [.alert, .sound, .badge])) ?? false
    }
  }

  func schedule(_ timer: KitchenTimer) {
    let content = UNMutableNotificationContent()
    content.title = timer.label.prefix(1).uppercased() + timer.label.dropFirst()
    content.body = timer.recipeTitle.map { "Time's up: \($0)" } ?? "Time's up"
    content.sound = .default
    content.interruptionLevel = .timeSensitive
    content.threadIdentifier = "kitchen-timers"
    if let recipeId = timer.recipeId {
      content.userInfo = ["recipeId": recipeId]
    }
    let wait = max(1, timer.endsAt.timeIntervalSinceNow)
    let trigger = UNTimeIntervalNotificationTrigger(timeInterval: wait, repeats: false)
    center.add(UNNotificationRequest(identifier: timer.id.uuidString, content: content, trigger: trigger))
  }

  func cancel(_ id: UUID) {
    center.removePendingNotificationRequests(withIdentifiers: [id.uuidString])
    center.removeDeliveredNotifications(withIdentifiers: [id.uuidString])
  }

  // A timer going off while Crumb is open still shows and sounds
  func userNotificationCenter(
    _ center: UNUserNotificationCenter, willPresent notification: UNNotification,
    withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
  ) {
    completionHandler([.banner, .sound, .list])
  }

  func userNotificationCenter(
    _ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse,
    withCompletionHandler completionHandler: @escaping () -> Void
  ) {
    let info = response.notification.request.content.userInfo
    if let id = (info["recipeId"] as? NSNumber)?.int64Value {
      DispatchQueue.main.async { self.onOpenRecipe?(id) }
    }
    completionHandler()
  }
}
