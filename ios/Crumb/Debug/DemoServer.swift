// swift-format-ignore-file: LineLength
// The fixtures below are JSON as the server sends it, one response per line.

#if DEBUG
  import CrumbKit
  import Foundation

  /// A pretend Crumb server inside the app, for UI tests and screenshots (`-demo`): the
  /// same JSON shapes as src/api.rs, a few recipes and a cookbook, no network. Debug only.
  final class DemoServer: URLProtocol {
    static let url = URL(string: "https://demo.crumb.invalid/")!

    static func session() -> URLSession {
      CrumbAPI.makeSession(protocolClasses: [DemoServer.self])
    }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func stopLoading() {}

    override func startLoading() {
      let path = request.url?.path ?? "/"
      let method = request.httpMethod ?? "GET"
      let (status, body) = Self.answer(method: method, path: path)
      let response = HTTPURLResponse(
        url: request.url ?? Self.url, statusCode: status, httpVersion: "HTTP/1.1",
        headerFields: ["Content-Type": "application/json"])
      if let response {
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
      }
      client?.urlProtocol(self, didLoad: Data(body.utf8))
      client?.urlProtocolDidFinishLoading(self)
    }

    static func answer(method: String, path: String) -> (Int, String) {
      switch (method, path) {
      case (_, "/api/health"), (_, "/api/auth/login"), (_, "/api/auth/logout"):
        return (200, #"{"ok":true}"#)
      case ("GET", "/api/recipes"):
        return (200, "[\(recipes.map(summary).joined(separator: ","))]")
      case ("GET", "/api/recipes/random"):
        return (200, summary(recipes[1]))
      case ("GET", "/api/cookbooks"):
        return (
          200,
          #"[{"id":1,"name":"Weeknights","description":"Quick ones","color":"tile","recipeCount":2,"createdAt":"2026-09-01T10:00:00.000Z"},{"id":2,"name":"Baking","color":"butter","recipeCount":1,"createdAt":"2026-09-02T10:00:00.000Z"}]"#
        )
      case ("GET", "/api/cookbooks/1"):
        return (
          200,
          #"{"id":1,"name":"Weeknights","description":"Quick ones","color":"tile","recipes":[\#(summary(recipes[0])),\#(summary(recipes[2]))]}"#
        )
      case ("GET", "/api/suggestions"):
        return (
          200,
          #"{"items":[{"recipe":\#(summary(recipes[2])),"reason":"You haven't made this in a while","reasonKind":"rest"},{"recipe":\#(summary(recipes[1])),"reason":"A bake for the weekend","reasonKind":"season","ai":true}],"ai":"ready"}"#
        )
      case ("GET", "/api/checks"):
        return (200, #"{"enabled":true,"eligible":3,"checked":3,"toCheck":1}"#)
      case ("GET", "/api/checks/review"):
        return (200, #"{"recipes":[{"id":2,"title":"Apple Pie","count":1,"fields":{"ingredients":1}}]}"#)
      case ("GET", "/api/connector"):
        return (200, #"{"mcpUrl":"https://demo.crumb.invalid/mcp","weeChef":true,"vision":true}"#)
      case ("POST", "/api/recipes/import"):
        return (200, #"{"id":3,"title":"Leek and Potato Soup","isNew":true}"#)
      default:
        break
      }
      let parts = path.split(separator: "/").map(String.init)
      if parts.count >= 3, parts[0] == "api", parts[1] == "recipes", let id = Int(parts[2]),
        let recipe = recipes.first(where: { $0.id == id })
      {
        switch (method, parts.dropFirst(3).first) {
        case ("GET", nil): return (200, full(recipe))
        case ("POST", "viewed"): return (204, "")
        case ("POST", "cooked"): return (200, #"{"count":1,"lastCookedAt":"2026-09-26T18:00:00.000Z","eventId":1}"#)
        case ("GET", "cookbooks"): return (200, "[1]")
        case ("GET", "checks"):
          return (
            200,
            #"{"status":"done","canUndo":false,"flags":[{"id":1,"field":"ingredients","itemText":"salt and pepper","kind":"merged","state":"review"}]}"#
          )
        default: break
        }
      }
      return (404, #"{"statusCode":404,"statusMessage":"Not Found","message":"Recipe not found"}"#)
    }

    private struct DemoRecipe {
      var id: Int
      var title: String
      var category: String
      var cuisine: String
      var total: String
      var yield: String
      var ingredients: [(String?, [String])]
      var steps: [(String?, [String])]
      var notes: String?
    }

    private static let recipes: [DemoRecipe] = [
      DemoRecipe(
        id: 1, title: "Weeknight Dal", category: "Main", cuisine: "Indian", total: "PT35M", yield: "4",
        ingredients: [(nil, ["250g red lentils", "1 onion, chopped", "2 cloves garlic", "1 tbsp curry powder"])],
        steps: [
          (nil, ["Fry the onion and garlic until soft.", "Stir in the curry powder and lentils."]),
          (nil, ["Simmer for 20 minutes, stirring now and then."]),
        ], notes: "Lime and coriander at the end."),
      DemoRecipe(
        id: 2, title: "Apple Pie", category: "Dessert", cuisine: "British", total: "PT1H20M", yield: "8",
        ingredients: [
          (nil, ["300g plain flour", "150g cold butter", "6 apples"]),
          ("For the glaze", ["1 egg", "1 tbsp sugar"]),
        ],
        steps: [
          (nil, ["Rub the butter into the flour.", "Slice the apples and fill the pie.", "Bake for 45 minutes."]),
          ("For the glaze", ["Brush with the egg and scatter the sugar."]),
        ], notes: nil),
      DemoRecipe(
        id: 3, title: "Leek and Potato Soup", category: "Soup", cuisine: "Welsh", total: "PT40M", yield: "4",
        ingredients: [(nil, ["2 leeks", "3 potatoes", "1 litre stock"])],
        steps: [(nil, ["Soften the leeks.", "Add the potatoes and stock and simmer for 25 minutes.", "Blend."])],
        notes: nil),
    ]

    private static func json(_ value: String?) -> String {
      guard let value else { return "null" }
      let data = try? JSONSerialization.data(withJSONObject: [value], options: [])
      let array = data.map { String(decoding: $0, as: UTF8.self) } ?? "[\"\"]"
      return String(array.dropFirst().dropLast())
    }

    private static func sections(_ list: [(String?, [String])]) -> String {
      var parts: [String] = []
      for (name, items) in list {
        let lines = items.map { json($0) }.joined(separator: ",")
        parts.append("{\"name\":\(json(name)),\"items\":[\(lines)]}")
      }
      return "[" + parts.joined(separator: ",") + "]"
    }

    private static func summary(_ r: DemoRecipe) -> String {
      #"{"id":\#(r.id),"title":\#(json(r.title)),"image":null,"totalTime":"\#(r.total)","recipeYield":"\#(r.yield)","recipeCategory":"\#(r.category)","recipeCuisine":"\#(r.cuisine)","source":"manual","createdAt":"2026-09-01T10:00:00.000Z"}"#
    }

    private static func full(_ r: DemoRecipe) -> String {
      #"{"id":\#(r.id),"url":null,"source":"manual","title":\#(json(r.title)),"description":null,"image":null,"author":null,"prepTime":null,"cookTime":null,"totalTime":"\#(r.total)","freezeTime":null,"recipeYield":"\#(r.yield)","recipeCategory":"\#(r.category)","recipeCuisine":"\#(r.cuisine)","ingredients":\#(sections(r.ingredients)),"instructions":\#(sections(r.steps)),"nutrition":null,"notes":\#(json(r.notes)),"originalUrl":null,"createdAt":"2026-09-01T10:00:00.000Z","updatedAt":"2026-09-01T10:00:00.000Z"}"#
    }
  }
#endif
