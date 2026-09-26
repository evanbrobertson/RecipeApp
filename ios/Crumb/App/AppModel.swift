import CrumbKit
import Foundation
import Observation
import UIKit

/// The app's tabs, as on the web's phone tab bar (Layout.astro): Suggestions is "Review".
enum AppTab: String, CaseIterable, Hashable {
  case home, recipes, shelf, review, more

  var label: String {
    switch self {
    case .home: return "Home"
    case .recipes: return "Recipes"
    case .shelf: return "Shelf"
    case .review: return "Review"
    case .more: return "More"
    }
  }

  var systemImage: String {
    switch self {
    case .home: return "house"
    case .recipes: return "book"
    case .shelf: return "books.vertical"
    case .review: return "sparkles"
    case .more: return "ellipsis"
    }
  }
}

/// Where a navigation stack can go.
enum Route: Hashable {
  case recipe(Int64)
  case cookbook(Int64)
}

/// A short message over the app, like the web's toasts.
struct Toast: Equatable, Identifiable {
  enum Tone { case info, success, error }
  let id = UUID()
  var title: String
  var message: String?
  var tone: Tone = .info
}

/// How the app was launched: normally, for UI tests (nothing saved between runs), or as a
/// demo against a built-in fake server (UI tests and screenshots, no network).
struct LaunchMode {
  var uiTesting = false
  var demo = false

  static var current: LaunchMode {
    let args = ProcessInfo.processInfo.arguments
    return LaunchMode(uiTesting: args.contains("-ui-testing"), demo: args.contains("-demo"))
  }
}

/// Everything the screens share, built once: the session, the API, the offline cache,
/// kitchen timers, the theme, and app-wide UI state (tab, Add sheet, toasts).
@Observable
@MainActor
final class AppModel {
  let sessions: SessionStore
  let cache: RecipeCache
  let api: CrumbAPI
  let repo: RecipeRepository
  let timers: KitchenTimers
  let theme: ThemeSettings
  let alarms: NotificationAlarms
  let mode: LaunchMode

  var tab: AppTab = .home
  var paths: [AppTab: [Route]] = [:]
  /// Text for Add (a crumb://add link or a paste), and whether Add is showing.
  var addDraft: String? = nil
  var showAdd = false
  /// The recipe open in cook mode.
  var cooking: Recipe? = nil
  var toast: Toast? = nil
  /// What the server can do (Wee Chef, photo reading), once asked.
  var connector: ConnectorInfo? = nil

  init(mode: LaunchMode = .current) {
    self.mode = mode
    let defaults: UserDefaults
    let secrets: SecretStore
    let cacheRoot: URL
    if mode.uiTesting || mode.demo {
      // A fresh, throwaway store per launch
      let suite = "crumb-ui-\(UUID().uuidString)"
      defaults = UserDefaults(suiteName: suite) ?? .standard
      secrets = MemorySecretStore()
      cacheRoot = FileManager.default.temporaryDirectory.appending(path: suite)
    } else {
      defaults = AppGroup.defaults
      secrets = KeychainStore()
      cacheRoot = RecipeCache.defaultRoot()
    }
    let sessions = SessionStore(secrets: secrets, defaults: defaults)
    self.sessions = sessions
    cache = RecipeCache(root: cacheRoot)
    #if DEBUG
      let http = mode.demo ? DemoServer.session() : CrumbAPI.makeSession()
    #else
      let http = CrumbAPI.makeSession()
    #endif
    api = CrumbAPI(http: http, session: { sessions.current })
    repo = RecipeRepository(api: api, cache: cache)
    alarms = NotificationAlarms()
    timers = KitchenTimers(defaults: defaults)
    theme = ThemeSettings(defaults: defaults)
    timers.attach(alarms)
    #if DEBUG
      if mode.demo, sessions.current == nil {
        sessions.signIn(server: DemoServer.url, cookie: "demo")
      }
    #endif
    cache.useServer(sessions.current?.server.absoluteString)
  }

  var session: Session? { sessions.session }

  // MARK: Signing in and out

  /// Signs in and returns nil, or a message to show under the form.
  func signIn(server input: String, password: String) async -> String? {
    guard let text = CrumbCore.serverUrl(input: input), let server = URL(string: text) else {
      return "That doesn't look like a web address."
    }
    guard CrumbCore.allowsCleartext(baseUrl: text) else {
      return "Use an https:// address for your Crumb. Plain http:// only works on your home network."
    }
    do {
      try await api.health(server: server)
      // A server without APP_PASSWORD accepts anything, but the field can't be empty
      let cookie = try await api.login(server: server, password: password.isEmpty ? " " : password)
      sessions.signIn(server: server, cookie: cookie)
      cache.useServer(server.absoluteString)
      connector = nil
      return nil
    } catch CrumbError.api(status: 401, message: _) {
      return "That password didn't work."
    } catch CrumbError.offline {
      return "Couldn't reach a Crumb server there. Check the address."
    } catch {
      return error.friendlyMessage
    }
  }

  /// Signs out and forgets this server's offline copy.
  func signOut() async {
    await api.logout()
    cache.clear()
    PhotoPipeline.shared.clear()
    sessions.signOut()
    paths = [:]
    tab = .home
  }

  /// A request failed: a 401 means the password changed or the session ran out, so sign in
  /// again (the offline copy stays for next time). Returns the message to show otherwise.
  @discardableResult
  func handle(_ error: Error) -> String {
    if (error as? CrumbError)?.isSignedOut == true {
      sessions.signOut()
    }
    return error.friendlyMessage
  }

  // MARK: Navigation

  func open(_ route: Route, in tab: AppTab? = nil) {
    let target = tab ?? self.tab
    self.tab = target
    paths[target, default: []].append(route)
  }

  func startAdd(with text: String? = nil) {
    addDraft = text
    showAdd = true
  }

  /// `crumb://recipes/12`, `crumb://cookbooks/3`, `crumb://add?text=…`.
  func handle(url: URL) {
    guard url.scheme == "crumb" else { return }
    let parts = [url.host].compactMap { $0 } + url.pathComponents.filter { $0 != "/" }
    switch (parts.first, parts.dropFirst().first.flatMap { Int64($0) }) {
    case ("recipes", let id?):
      open(.recipe(id), in: .recipes)
    case ("cookbooks", let id?):
      open(.cookbook(id), in: .shelf)
    case ("add", _):
      let text = URLComponents(url: url, resolvingAgainstBaseURL: false)?
        .queryItems?.first { $0.name == "text" || $0.name == "url" }?.value
      startAdd(with: text)
    default:
      break
    }
  }

  // MARK: Messages

  func show(_ title: String, _ message: String? = nil, tone: Toast.Tone = .info) {
    toast = Toast(title: title, message: message, tone: tone)
    if tone == .error {
      UINotificationFeedbackGenerator().notificationOccurred(.error)
    }
  }

  /// Loads the server's capabilities once per session (for photo import).
  func loadConnector() async {
    guard connector == nil, session != nil else { return }
    connector = try? await api.connector()
  }
}
