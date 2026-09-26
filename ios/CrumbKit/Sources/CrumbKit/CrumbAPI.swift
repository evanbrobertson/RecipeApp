@_exported import CrumbCore
import Foundation

#if canImport(FoundationNetworking)
  import FoundationNetworking
#endif

/// Why a request failed, with a message written for people.
public enum CrumbError: Error, Equatable, LocalizedError {
  /// The server answered and refused. `message` is crumb-core's wording of its reply.
  case api(status: Int, message: String)
  /// No answer at all: no network, wrong address, server down.
  case offline(String)
  /// The server answered with something that isn't the JSON we expected.
  case decoding(String)
  /// There's no session to send the request with.
  case signedOut

  public var isSignedOut: Bool {
    switch self {
    case .signedOut: return true
    case .api(let status, _): return status == 401
    default: return false
    }
  }

  public var isOffline: Bool {
    if case .offline = self { return true }
    return false
  }

  public var errorDescription: String? {
    switch self {
    case .api(_, let message): return message
    case .offline: return "Can't reach your Crumb server. Check your connection and try again."
    case .decoding: return "Your Crumb server sent something unexpected. Is the app up to date?"
    case .signedOut: return "Please sign in again."
    }
  }
}

extension Error {
  /// What to tell someone when this fails, in kitchen words.
  public var friendlyMessage: String {
    if let crumb = self as? CrumbError { return crumb.errorDescription ?? "Something went wrong." }
    return "Something went wrong. Try again."
  }
}

/// A server and the `crumb_session` cookie to send it (nil for a server without a password).
public struct Session: Codable, Equatable, Sendable {
  /// Base URL ending in "/", as `serverUrl` in crumb-core normalises it.
  public var server: URL
  public var cookie: String?

  public init(server: URL, cookie: String?) {
    self.server = server
    self.cookie = cookie
  }

  /// The address without scheme or trailing slash, as people type it.
  public var displayServer: String {
    var text = server.absoluteString
    if text.hasSuffix("/") { text.removeLast() }
    if text.hasPrefix("https://") { text.removeFirst("https://".count) }
    return text
  }
}

/// The Crumb server's REST API (src/api.rs): the same calls as the web app and Android's
/// CrumbApi.kt. Requests carry the session cookie from `session()`; cookies are never
/// stored by URLSession, so signing out really forgets them.
public final class CrumbAPI: @unchecked Sendable {
  private let http: URLSession
  private let session: @Sendable () -> Session?
  private let decoder = JSONDecoder()
  private let encoder = JSONEncoder()

  public init(http: URLSession = CrumbAPI.makeSession(), session: @escaping @Sendable () -> Session?) {
    self.http = http
    self.session = session
  }

  /// No cookie storage (the session lives in the Keychain), generous timeouts because an
  /// import may scrape the page in headless Chromium before answering.
  public static func makeSession(protocolClasses: [AnyClass]? = nil) -> URLSession {
    let config = URLSessionConfiguration.default
    config.httpShouldSetCookies = false
    config.httpCookieAcceptPolicy = .never
    config.httpCookieStorage = nil
    config.timeoutIntervalForRequest = 90
    config.timeoutIntervalForResource = 180
    config.waitsForConnectivity = false
    if let protocolClasses { config.protocolClasses = protocolClasses }
    return URLSession(configuration: config)
  }

  // MARK: Signing in

  /// `GET /api/health`: is there a Crumb at this address? Needs no password.
  public func health(server: URL) async throws {
    _ = try await send(URLRequest(url: server.appending(path: "api/health")), withSession: false)
  }

  /// `POST /api/auth/login`. Returns the session cookie, or nil for a server without a
  /// password (it accepts anything and sets none).
  public func login(server: URL, password: String) async throws -> String? {
    var request = URLRequest(url: server.appending(path: "api/auth/login"))
    request.httpMethod = "POST"
    request.setValue("application/json", forHTTPHeaderField: "Content-Type")
    request.httpBody = try encoder.encode(["password": password])
    let (_, response) = try await send(request, withSession: false)
    return Self.sessionCookie(from: response)
  }

  /// `POST /api/auth/logout`; failures are ignored, the app forgets the session anyway.
  public func logout() async {
    _ = try? await send(post("api/auth/logout", json: Data("{}".utf8)))
  }

  /// The `crumb_session` value from a response's `Set-Cookie` header(s). URLSession joins
  /// repeated headers with commas, so each piece is tried.
  static func sessionCookie(from response: HTTPURLResponse) -> String? {
    guard let header = response.value(forHTTPHeaderField: "Set-Cookie") else { return nil }
    return header.components(separatedBy: ",").lazy.compactMap {
      CrumbCore.sessionCookie(setCookie: $0.trimmingCharacters(in: .whitespaces))
    }.first
  }

  // MARK: Recipes

  /// `GET /api/recipes?q=&limit=`.
  public func recipes(query: String? = nil, limit: Int? = nil) async throws -> [RecipeSummary] {
    var items: [URLQueryItem] = []
    if let q = query?.trimmingCharacters(in: .whitespacesAndNewlines), !q.isEmpty {
      items.append(URLQueryItem(name: "q", value: String(q.prefix(200))))
    }
    if let limit { items.append(URLQueryItem(name: "limit", value: String(limit))) }
    return try await get("api/recipes", query: items)
  }

  public func recipe(id: Int64) async throws -> Recipe {
    try await get("api/recipes/\(id)")
  }

  /// `POST /api/recipes/import` with a link.
  public func importURL(_ link: String) async throws -> ImportResult {
    try await decode(send(post("api/recipes/import", body: ["url": link])))
  }

  /// `POST /api/recipes/import` with pasted recipe text.
  public func importText(_ text: String) async throws -> ImportResult {
    try await decode(send(post("api/recipes/import", body: ["text": text])))
  }

  /// A paste or share, as crumb-core classifies it: a link to fetch or the recipe itself.
  public func importInput(_ raw: String) async throws -> ImportResult {
    switch CrumbCore.classifyImport(raw: raw) {
    case .link(let url): return try await importURL(url)
    case .text(let text): return try await importText(text)
    case nil: throw CrumbError.api(status: 400, message: "Paste a link or a recipe first.")
    }
  }

  /// `POST /api/recipes/import/photos`: 1–6 JPEG page photos read by Wee Chef.
  public func importPhotos(_ photos: [Data], hint: String? = nil) async throws -> ImportResult {
    var form = MultipartForm()
    for (i, photo) in photos.enumerated() {
      form.addFile(name: "photo", fileName: "photo-\(i + 1).jpg", mimeType: "image/jpeg", data: photo)
    }
    if let hint, !hint.isEmpty { form.addField(name: "text", value: hint) }
    return try await decode(send(form.request(url: url("api/recipes/import/photos"))))
  }

  /// `POST /api/import/files`: one `file` part per file (Paprika, JSON, HTML, PDF, text, zip).
  public func importFiles(_ files: [UploadFile]) async throws -> [ImportSummary] {
    var form = MultipartForm()
    for file in files {
      form.addFile(name: "file", fileName: file.name, mimeType: file.mimeType, data: file.data)
    }
    return try await decode(send(form.request(url: url("api/import/files"))))
  }

  /// `POST /api/recipes`.
  public func createRecipe(_ fields: RecipeFields) async throws -> Recipe {
    try await decode(send(post("api/recipes", json: try encoder.encode(fields))))
  }

  /// `PATCH /api/recipes/{id}`.
  public func updateRecipe(id: Int64, _ fields: RecipeFields) async throws -> Recipe {
    try await decode(send(request("api/recipes/\(id)", method: "PATCH", json: try encoder.encode(fields))))
  }

  /// `DELETE /api/recipes/{id}`.
  public func deleteRecipe(id: Int64) async throws {
    _ = try await send(request("api/recipes/\(id)", method: "DELETE"))
  }

  /// `POST /api/recipes/bulk-delete`: how many were deleted.
  public func deleteRecipes(ids: [Int64]) async throws -> Int {
    let result: Counted = try await decode(send(post("api/recipes/bulk-delete", body: ["ids": ids])))
    return result.deleted ?? 0
  }

  /// `GET /api/recipes/random`: nil when the box is empty (404).
  public func randomRecipe(exclude: [Int64] = [], current: Int64? = nil) async throws -> RecipeSummary? {
    var items: [URLQueryItem] = []
    if !exclude.isEmpty {
      items.append(URLQueryItem(name: "exclude", value: exclude.map(String.init).joined(separator: ",")))
    }
    if let current { items.append(URLQueryItem(name: "current", value: String(current))) }
    do {
      return try await get("api/recipes/random", query: items) as RecipeSummary
    } catch CrumbError.api(status: 404, message: _) {
      return nil
    }
  }

  /// `GET /api/recipes/{id}/cookbooks`: ids of the cookbooks holding the recipe.
  public func recipeCookbooks(id: Int64) async throws -> [Int64] {
    try await get("api/recipes/\(id)/cookbooks")
  }

  /// `POST /api/recipes/{id}/viewed` (204).
  public func logViewed(id: Int64) async throws {
    _ = try await send(post("api/recipes/\(id)/viewed", json: Data("{}".utf8)))
  }

  /// `POST /api/recipes/{id}/cooked`: `eventId` is nil when deduped.
  public func markCooked(id: Int64) async throws -> Cooked {
    try await decode(send(post("api/recipes/\(id)/cooked", json: Data("{}".utf8))))
  }

  /// `DELETE /api/recipes/{id}/cooked?event=`: undo one logged cook.
  public func undoCooked(id: Int64, eventId: Int64) async throws {
    _ = try await send(
      request("api/recipes/\(id)/cooked", method: "DELETE", query: [URLQueryItem(name: "event", value: String(eventId))]))
  }

  /// `GET /api/recipes/{id}/export?format=json|md`.
  public func exportRecipe(id: Int64, format: String) async throws -> Download {
    try await download(
      request("api/recipes/\(id)/export", query: [URLQueryItem(name: "format", value: format)]),
      fallbackName: "recipe.\(format)")
  }

  /// `GET /api/export`: the whole box as a backup.
  public func exportAll() async throws -> Download {
    try await download(request("api/export"), fallbackName: "crumb.json")
  }

  // MARK: Cookbooks

  public func cookbooks() async throws -> [CookbookListItem] {
    try await get("api/cookbooks")
  }

  public func cookbook(id: Int64) async throws -> Cookbook {
    try await get("api/cookbooks/\(id)")
  }

  /// `POST /api/cookbooks`.
  public func createCookbook(name: String, description: String?, color: String) async throws -> CookbookListItem {
    try await decode(
      send(post("api/cookbooks", body: ["name": name, "description": description ?? "", "color": color])))
  }

  /// `PATCH /api/cookbooks/{id}`.
  public func updateCookbook(id: Int64, name: String, description: String?, color: String) async throws -> Cookbook {
    let body = try encoder.encode(["name": name, "description": description ?? "", "color": color])
    return try await decode(send(request("api/cookbooks/\(id)", method: "PATCH", json: body)))
  }

  public func deleteCookbook(id: Int64) async throws {
    _ = try await send(request("api/cookbooks/\(id)", method: "DELETE"))
  }

  /// `POST /api/cookbooks/{id}/recipes`: how many were added.
  public func addToCookbook(bookId: Int64, recipeIds: [Int64]) async throws -> Int {
    let result: Counted = try await decode(
      send(post("api/cookbooks/\(bookId)/recipes", body: ["recipeIds": recipeIds])))
    return result.added ?? 0
  }

  public func removeFromCookbook(bookId: Int64, recipeId: Int64) async throws {
    _ = try await send(request("api/cookbooks/\(bookId)/recipes/\(recipeId)", method: "DELETE"))
  }

  /// `GET /api/cookbooks/{id}/export`.
  public func exportCookbook(id: Int64) async throws -> Download {
    try await download(request("api/cookbooks/\(id)/export"), fallbackName: "cookbook.json")
  }

  // MARK: Wee Chef

  /// `GET /api/suggestions`: Try next's cards, the AI status and (some days) an idea.
  public func suggestions(limit: Int = 4, seed: Int? = nil, exclude: [Int64] = []) async throws -> Suggestions {
    var items = [URLQueryItem(name: "limit", value: String(limit))]
    if let seed { items.append(URLQueryItem(name: "seed", value: String(seed))) }
    if !exclude.isEmpty {
      items.append(URLQueryItem(name: "exclude", value: exclude.map(String.init).joined(separator: ",")))
    }
    return try await get("api/suggestions", query: items)
  }

  /// `GET /api/recipes/{id}/checks`: nil when Wee Chef never checked the recipe.
  public func recipeChecks(id: Int64) async throws -> RecipeChecks? {
    try await get("api/recipes/\(id)/checks")
  }

  /// `POST /api/recipes/{id}/checks`: check again now.
  public func checkRecipe(id: Int64) async throws -> RecipeChecks? {
    try await decode(send(post("api/recipes/\(id)/checks", json: Data("{}".utf8))))
  }

  /// `POST /api/recipes/{id}/checks/undo`: put back what Wee Chef changed.
  public func undoChecks(id: Int64) async throws -> UndoResult {
    try await decode(send(post("api/recipes/\(id)/checks/undo", json: Data("{}".utf8))))
  }

  /// `POST /api/recipes/{id}/flags/{flag}/dismiss`: "Keep as is".
  public func dismissFlag(recipeId: Int64, flagId: Int64) async throws -> RecipeChecks? {
    try await decode(send(post("api/recipes/\(recipeId)/flags/\(flagId)/dismiss", json: Data("{}".utf8))))
  }

  public func checksStatus() async throws -> ChecksStatus {
    try await get("api/checks")
  }

  /// `POST /api/checks`: "Check all".
  public func checkAll() async throws -> ChecksStatus {
    try await decode(send(post("api/checks", json: Data("{}".utf8))))
  }

  /// `GET /api/checks/review`: recipes with suggestions waiting.
  public func reviewList() async throws -> [ReviewRecipe] {
    let list: ReviewList = try await get("api/checks/review")
    return list.recipes ?? []
  }

  // MARK: Sharing

  /// `POST /api/{recipes|cookbooks}/{id}/share`: make (or return) the share link.
  public func createShare(_ kind: ShareKind, id: Int64) async throws -> Share {
    try await decode(send(post("api/\(kind.rawValue)/\(id)/share", json: Data("{}".utf8))))
  }

  /// `PATCH …/share`: show or hide the cook's notes.
  public func setShareNotes(_ kind: ShareKind, id: Int64, includeNotes: Bool) async throws -> Share {
    let body = try encoder.encode(["includeNotes": includeNotes])
    return try await decode(send(request("api/\(kind.rawValue)/\(id)/share", method: "PATCH", json: body)))
  }

  /// `DELETE …/share`: stop sharing for good.
  public func stopSharing(_ kind: ShareKind, id: Int64) async throws {
    _ = try await send(request("api/\(kind.rawValue)/\(id)/share", method: "DELETE"))
  }

  public func shares() async throws -> [SharedLink] {
    try await get("api/shares")
  }

  /// `GET /api/connector`: what this server has (Wee Chef, vision, scraping).
  public func connector() async throws -> ConnectorInfo {
    try await get("api/connector")
  }

  // MARK: Photos

  /// The server's resized WebP of a recipe photo shown about `points × scale` pixels wide
  /// (crumb-core's `photoPath`, the same URLs as the web).
  public func photoURL(recipeId: Int64, image: String?, pixels: Int) -> URL? {
    guard let server = session()?.server,
      let path = CrumbCore.photoPath(recipeId: recipeId, image: image, px: UInt32(clamping: pixels))
    else { return nil }
    return URL(string: path, relativeTo: server)?.absoluteURL
  }

  /// A request for a photo: the session cookie goes only to the signed-in server.
  public func photoRequest(_ url: URL) -> URLRequest {
    var request = URLRequest(url: url)
    if let current = session(), current.server.host == url.host, current.server.port == url.port,
      let cookie = current.cookie
    {
      request.setValue("\(CrumbCore.sessionCookieName())=\(cookie)", forHTTPHeaderField: "Cookie")
    }
    return request
  }

  // MARK: Plumbing

  private func url(_ path: String, query: [URLQueryItem] = []) throws -> URL {
    guard let server = session()?.server else { throw CrumbError.signedOut }
    var url = server.appending(path: path)
    if !query.isEmpty { url.append(queryItems: query) }
    return url
  }

  private func request(
    _ path: String, method: String = "GET", query: [URLQueryItem] = [], json: Data? = nil
  ) throws -> URLRequest {
    var request = URLRequest(url: try url(path, query: query))
    request.httpMethod = method
    request.setValue("application/json", forHTTPHeaderField: "Accept")
    if let json {
      request.setValue("application/json", forHTTPHeaderField: "Content-Type")
      request.httpBody = json
    }
    return request
  }

  private func post(_ path: String, json: Data) throws -> URLRequest {
    try request(path, method: "POST", json: json)
  }

  private func post<Body: Encodable>(_ path: String, body: Body) throws -> URLRequest {
    try request(path, method: "POST", json: try encoder.encode(body))
  }

  private func get<T: Decodable>(_ path: String, query: [URLQueryItem] = []) async throws -> T {
    try await decode(send(request(path, query: query)))
  }

  private func decode<T: Decodable>(_ result: (Data, HTTPURLResponse)) throws -> T {
    do {
      return try decoder.decode(T.self, from: result.0)
    } catch {
      throw CrumbError.decoding(String(describing: error))
    }
  }

  private func download(_ request: URLRequest, fallbackName: String) async throws -> Download {
    let (data, response) = try await send(request)
    let type = response.value(forHTTPHeaderField: "Content-Type")?
      .split(separator: ";").first.map { $0.trimmingCharacters(in: .whitespaces) } ?? ""
    return Download(
      fileName: Self.fileName(response.value(forHTTPHeaderField: "Content-Disposition"), fallback: fallbackName),
      mimeType: type, data: data)
  }

  /// `attachment; filename="pie.json"; filename*=UTF-8''…` as just the file name.
  static func fileName(_ header: String?, fallback: String) -> String {
    guard let header else { return fallback }
    if let range = header.range(of: "filename*=UTF-8''") {
      let encoded = header[range.upperBound...].prefix { $0 != ";" }
      if let decoded = String(encoded).removingPercentEncoding, !decoded.isEmpty { return decoded }
    }
    if let range = header.range(of: "filename=") {
      let plain = header[range.upperBound...].prefix { $0 != ";" }
        .trimmingCharacters(in: CharacterSet(charactersIn: "\" "))
      if !plain.isEmpty { return plain }
    }
    return fallback
  }

  @discardableResult
  private func send(_ request: URLRequest, withSession: Bool = true) async throws -> (Data, HTTPURLResponse) {
    var request = request
    if withSession, let cookie = session()?.cookie {
      request.setValue("\(CrumbCore.sessionCookieName())=\(cookie)", forHTTPHeaderField: "Cookie")
    }
    let data: Data
    let response: URLResponse
    do {
      (data, response) = try await http.data(for: request)
    } catch let error as URLError where error.code == .cancelled {
      throw CancellationError()
    } catch {
      throw CrumbError.offline(error.localizedDescription)
    }
    guard let httpResponse = response as? HTTPURLResponse else {
      throw CrumbError.offline("Not an HTTP response")
    }
    guard (200..<300).contains(httpResponse.statusCode) else {
      let body = String(decoding: data, as: UTF8.self)
      throw CrumbError.api(
        status: httpResponse.statusCode,
        message: CrumbCore.errorMessage(status: UInt16(clamping: httpResponse.statusCode), body: body))
    }
    return (data, httpResponse)
  }
}

/// `{deleted}` / `{added}` counts.
private struct Counted: Decodable {
  var deleted: Int?
  var added: Int?
}

private struct ReviewList: Decodable {
  var recipes: [ReviewRecipe]?
}

/// A `multipart/form-data` body.
struct MultipartForm {
  let boundary = "crumb-\(UUID().uuidString)"
  private(set) var body = Data()

  mutating func addField(name: String, value: String) {
    body.append(Data("--\(boundary)\r\nContent-Disposition: form-data; name=\"\(name)\"\r\n\r\n\(value)\r\n".utf8))
  }

  mutating func addFile(name: String, fileName: String, mimeType: String, data: Data) {
    let safeName = fileName.replacingOccurrences(of: "\"", with: "'")
    body.append(
      Data(
        "--\(boundary)\r\nContent-Disposition: form-data; name=\"\(name)\"; filename=\"\(safeName)\"\r\nContent-Type: \(mimeType.isEmpty ? "application/octet-stream" : mimeType)\r\n\r\n"
          .utf8))
    body.append(data)
    body.append(Data("\r\n".utf8))
  }

  func request(url: URL) -> URLRequest {
    var request = URLRequest(url: url)
    request.httpMethod = "POST"
    request.setValue("multipart/form-data; boundary=\(boundary)", forHTTPHeaderField: "Content-Type")
    request.setValue("application/json", forHTTPHeaderField: "Accept")
    var full = body
    full.append(Data("--\(boundary)--\r\n".utf8))
    request.httpBody = full
    return request
  }
}
