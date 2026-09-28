import CryptoKit
import Foundation

/// The last copy of everything the server sent, as JSON files, so the recipe box and any
/// recipe opened before still work without a connection. One folder per server; signing
/// out deletes it. The server stays the source of truth: this is only ever overwritten.
/// (The same layout as Android's RecipeCache.kt.)
public final class RecipeCache: @unchecked Sendable {
  private let root: URL
  private var dir: URL?
  private let lock = NSLock()
  private let encoder = JSONEncoder()
  private let decoder = JSONDecoder()

  public init(root: URL) {
    self.root = root
  }

  /// The app's cache folder, in Application Support (kept, but not backed up).
  public static func defaultRoot() -> URL {
    let base =
      FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
      ?? FileManager.default.temporaryDirectory
    return base.appending(path: "recipes", directoryHint: .isDirectory)
  }

  public func useServer(_ server: String?) {
    lock.lock()
    defer { lock.unlock() }
    dir = server.map { root.appending(path: Self.key($0), directoryHint: .isDirectory) }
  }

  public func clear() {
    lock.lock()
    let folder = dir
    lock.unlock()
    if let folder { try? FileManager.default.removeItem(at: folder) }
  }

  public func recipes() -> [RecipeSummary]? { read("recipes") }
  public func putRecipes(_ list: [RecipeSummary]) { write("recipes", list) }

  public func recipe(id: Int64) -> Recipe? { read("recipe-\(id)") }
  public func putRecipe(_ recipe: Recipe) { write("recipe-\(recipe.id)", recipe) }

  public func cookbooks() -> [CookbookListItem]? { read("cookbooks") }
  public func putCookbooks(_ list: [CookbookListItem]) { write("cookbooks", list) }

  public func cookbook(id: Int64) -> Cookbook? { read("cookbook-\(id)") }
  public func putCookbook(_ book: Cookbook) { write("cookbook-\(book.id)", book) }

  private func file(_ name: String) -> URL? {
    lock.lock()
    defer { lock.unlock() }
    return dir?.appending(path: "\(name).json")
  }

  private func read<T: Decodable>(_ name: String) -> T? {
    guard let url = file(name), let data = try? Data(contentsOf: url) else { return nil }
    return try? decoder.decode(T.self, from: data)
  }

  private func write<T: Encodable>(_ name: String, _ value: T) {
    guard let url = file(name), let data = try? encoder.encode(value) else { return }
    var folder = url.deletingLastPathComponent()
    try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    var values = URLResourceValues()
    values.isExcludedFromBackup = true
    try? folder.setResourceValues(values)
    try? data.write(to: url, options: .atomic)
  }

  /// A short, stable folder name for a server address.
  static func key(_ server: String) -> String {
    SHA256.hash(data: Data(server.utf8)).prefix(12).map { String(format: "%02x", $0) }.joined()
  }
}

/// A value and whether it came from the phone because the server couldn't be reached.
public struct Loaded<T: Sendable>: Sendable {
  public var value: T
  public var offline: Bool

  public init(_ value: T, offline: Bool = false) {
    self.value = value
    self.offline = offline
  }
}

/// Recipes and cookbooks: from the server when it answers, otherwise the last copy on the
/// phone. Pure recipe logic (scaling, parsing, ranking) belongs to crumb-core, not here.
public final class RecipeRepository: @unchecked Sendable {
  public let api: CrumbAPI
  public let cache: RecipeCache

  public init(api: CrumbAPI, cache: RecipeCache) {
    self.api = api
    self.cache = cache
  }

  public func recipes(query: String? = nil) async throws -> Loaded<[RecipeSummary]> {
    let q = query?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    do {
      let list = try await api.recipes(query: q.isEmpty ? nil : q)
      if q.isEmpty { cache.putRecipes(list) }
      return Loaded(list)
    } catch let error as CrumbError where error.isOffline {
      guard let all = cache.recipes() else { throw error }
      return Loaded(q.isEmpty ? all : Self.filterOffline(all, query: q), offline: true)
    }
  }

  public func recipe(id: Int64) async throws -> Loaded<Recipe> {
    try await cached(
      fetch: {
        let recipe = try await self.api.recipe(id: id)
        self.cache.putRecipe(recipe)
        return recipe
      }, fallback: { self.cache.recipe(id: id) })
  }

  public func cookbooks() async throws -> Loaded<[CookbookListItem]> {
    try await cached(
      fetch: {
        let books = try await self.api.cookbooks()
        self.cache.putCookbooks(books)
        return books
      }, fallback: { self.cache.cookbooks() })
  }

  public func cookbook(id: Int64) async throws -> Loaded<Cookbook> {
    try await cached(
      fetch: {
        let book = try await self.api.cookbook(id: id)
        self.cache.putCookbook(book)
        return book
      }, fallback: { self.cache.cookbook(id: id) })
  }

  /// Offline search with crumb-core's rule: every word in the title, category or cuisine.
  public static func filterOffline(_ all: [RecipeSummary], query: String) -> [RecipeSummary] {
    all.filter {
      CrumbCore.matchesSearch(query: query, title: $0.title, category: $0.recipeCategory, cuisine: $0.recipeCuisine)
    }
  }

  private func cached<T: Sendable>(
    fetch: () async throws -> T, fallback: () -> T?
  ) async throws -> Loaded<T> {
    do {
      return Loaded(try await fetch())
    } catch let error as CrumbError where error.isOffline {
      guard let copy = fallback() else { throw error }
      return Loaded(copy, offline: true)
    }
  }
}
