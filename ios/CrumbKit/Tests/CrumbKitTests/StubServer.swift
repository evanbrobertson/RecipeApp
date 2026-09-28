import Foundation

@testable import CrumbKit

/// A fake Crumb server: a URLProtocol that answers from a handler and records requests.
final class StubServer: URLProtocol {
  struct Reply {
    var status: Int = 200
    var body: String = "{}"
    var headers: [String: String] = ["Content-Type": "application/json"]
  }

  nonisolated(unsafe) static var handler: ((URLRequest) -> Reply)?
  nonisolated(unsafe) static var requests: [URLRequest] = []
  nonisolated(unsafe) static var offline = false
  private static let lock = NSLock()

  static func reset() {
    lock.lock()
    defer { lock.unlock() }
    handler = nil
    requests = []
    offline = false
  }

  static func reply(_ make: @escaping (URLRequest) -> Reply) {
    lock.lock()
    defer { lock.unlock() }
    handler = make
  }

  static var last: URLRequest? {
    lock.lock()
    defer { lock.unlock() }
    return requests.last
  }

  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

  override func startLoading() {
    var recorded = request
    // URLSession moves the body into a stream; keep it readable for assertions
    if recorded.httpBody == nil, let stream = request.httpBodyStream {
      recorded.httpBody = Self.read(stream)
    }
    Self.lock.lock()
    Self.requests.append(recorded)
    let handler = Self.handler
    let offline = Self.offline
    Self.lock.unlock()

    if offline {
      client?.urlProtocol(self, didFailWithError: URLError(.notConnectedToInternet))
      return
    }
    let reply = handler?(recorded) ?? Reply(status: 404, body: "")
    let response = HTTPURLResponse(
      url: request.url!, statusCode: reply.status, httpVersion: "HTTP/1.1", headerFields: reply.headers)!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(reply.body.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }

  override func stopLoading() {}

  private static func read(_ stream: InputStream) -> Data {
    stream.open()
    defer { stream.close() }
    var data = Data()
    var buffer = [UInt8](repeating: 0, count: 4096)
    while stream.hasBytesAvailable {
      let n = stream.read(&buffer, maxLength: buffer.count)
      if n <= 0 { break }
      data.append(buffer, count: n)
    }
    return data
  }
}

extension URLRequest {
  var bodyText: String { httpBody.map { String(decoding: $0, as: UTF8.self) } ?? "" }
  var bodyJSON: [String: Any]? {
    httpBody.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }
  }
}

let testServer = URL(string: "https://crumb.example.com/")!

func makeAPI(cookie: String? = "issued.sig") -> CrumbAPI {
  let session = Session(server: testServer, cookie: cookie)
  return CrumbAPI(http: CrumbAPI.makeSession(protocolClasses: [StubServer.self]), session: { session })
}

let recipeJSON = """
  {"id":7,"url":"https://example.com/pie","source":"url","title":"Apple Pie",
  "description":"A pie.","image":"https://example.com/pie.jpg","author":null,
  "prepTime":"PT20M","cookTime":"PT45M","totalTime":"PT1H5M","freezeTime":null,
  "recipeYield":"8","recipeCategory":"Dessert","recipeCuisine":"British",
  "ingredients":[{"name":null,"items":["2 cups flour","3 apples"]},
                 {"name":"For the glaze","items":["1 egg"]}],
  "instructions":[{"name":null,"items":["Mix the flour.","Bake for 25-30 minutes."]},
                  {"name":"For the glaze","items":["Brush with the egg."]}],
  "nutrition":{"calories":"250 kcal","protein":4},"notes":"Best warm.",
  "originalUrl":null,"createdAt":"2026-09-26T16:30:33.000Z","updatedAt":"2026-09-26T16:30:33.000Z",
  "somethingNew":true}
  """
