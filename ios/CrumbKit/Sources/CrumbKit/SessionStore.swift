import Foundation
import Observation

#if canImport(Security)
  import Security
#endif

/// Where the session cookie is kept. The app uses the Keychain; tests use memory.
public protocol SecretStore: Sendable {
  func read(_ key: String) -> String?
  func write(_ key: String, _ value: String?)
}

/// App Group shared by the app and the Share extension (project.yml's entitlements).
public enum AppGroup {
  /// `group.<bundle id>`, from the `CrumbAppGroup` Info.plist key (Config/Crumb.xcconfig).
  public static var id: String {
    (Bundle.main.object(forInfoDictionaryKey: "CrumbAppGroup") as? String) ?? "group.app.crumb.ios"
  }

  /// Settings both targets read: the server address, the theme. Falls back to the app's
  /// own defaults if the group isn't available (an unsigned build).
  public static var defaults: UserDefaults {
    UserDefaults(suiteName: id) ?? .standard
  }
}

#if canImport(Security)
  /// Generic-password Keychain items, readable after the first unlock so the Share
  /// extension can save while the phone is locked in a pocket. The app and extension share
  /// the first `keychain-access-groups` entry, the default group for both.
  public struct KeychainStore: SecretStore {
    let service: String

    public init(service: String = "app.crumb.ios.session") {
      self.service = service
    }

    private func query(_ key: String) -> [String: Any] {
      [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrService as String: service,
        kSecAttrAccount as String: key,
      ]
    }

    public func read(_ key: String) -> String? {
      var q = query(key)
      q[kSecReturnData as String] = true
      q[kSecMatchLimit as String] = kSecMatchLimitOne
      var item: CFTypeRef?
      guard SecItemCopyMatching(q as CFDictionary, &item) == errSecSuccess, let data = item as? Data
      else { return nil }
      return String(data: data, encoding: .utf8)
    }

    public func write(_ key: String, _ value: String?) {
      SecItemDelete(query(key) as CFDictionary)
      guard let value else { return }
      var q = query(key)
      q[kSecValueData as String] = Data(value.utf8)
      q[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
      SecItemAdd(q as CFDictionary, nil)
    }
  }
#endif

/// Secrets kept in memory only, for tests and previews.
public final class MemorySecretStore: SecretStore, @unchecked Sendable {
  private var values: [String: String] = [:]
  private let lock = NSLock()

  public init(_ values: [String: String] = [:]) {
    self.values = values
  }

  public func read(_ key: String) -> String? {
    lock.lock()
    defer { lock.unlock() }
    return values[key]
  }

  public func write(_ key: String, _ value: String?) {
    lock.lock()
    defer { lock.unlock() }
    values[key] = value
  }
}

/// The server address and session cookie. The password is never stored. The address is
/// in the App Group's defaults (it isn't secret, and stays after signing out to prefill the
/// form); the cookie is in the Keychain.
@Observable
public final class SessionStore: @unchecked Sendable {
  public private(set) var session: Session? = nil
  /// The last server used, kept after signing out so it's prefilled next time.
  public private(set) var lastServer: String? = nil

  private let secrets: SecretStore
  private let defaults: UserDefaults
  private let lock = NSLock()

  static let serverKey = "server"
  static let cookieKey = "cookie"
  /// Marks a signed-in server without a password (no cookie to keep).
  static let noCookie = "-"

  public init(secrets: SecretStore, defaults: UserDefaults = AppGroup.defaults) {
    self.secrets = secrets
    self.defaults = defaults
    reload()
  }

  /// Re-reads what's saved (the other target may have changed it).
  public func reload() {
    let server = defaults.string(forKey: Self.serverKey)
    lastServer = server
    let cookie = secrets.read(Self.cookieKey)
    if let server, let url = URL(string: server), let cookie {
      session = Session(server: url, cookie: cookie == Self.noCookie ? nil : cookie)
    } else {
      session = nil
    }
  }

  /// The session right now, from any thread.
  public var current: Session? {
    lock.lock()
    defer { lock.unlock() }
    return session
  }

  public func signIn(server: URL, cookie: String?) {
    defaults.set(server.absoluteString, forKey: Self.serverKey)
    secrets.write(Self.cookieKey, cookie ?? Self.noCookie)
    lock.lock()
    lastServer = server.absoluteString
    session = Session(server: server, cookie: cookie)
    lock.unlock()
  }

  public func signOut() {
    secrets.write(Self.cookieKey, nil)
    lock.lock()
    session = nil
    lock.unlock()
  }
}
