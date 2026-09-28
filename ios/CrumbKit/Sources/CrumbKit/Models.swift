import Foundation

// Shapes of the Crumb server's JSON (crates/crumb-core/src/model.rs and src/api.rs), the
// same as android/…/data/Models.kt. Unknown fields are ignored and missing lists default
// to empty, so the server can add fields freely.

/// Any JSON value, for the few fields kept raw (nutrition values may be strings or numbers).
public enum JSONValue: Codable, Hashable, Sendable {
  case string(String)
  case number(Double)
  case bool(Bool)
  case object([String: JSONValue])
  case array([JSONValue])
  case null

  public init(from decoder: Decoder) throws {
    let c = try decoder.singleValueContainer()
    if c.decodeNil() {
      self = .null
    } else if let b = try? c.decode(Bool.self) {
      self = .bool(b)
    } else if let n = try? c.decode(Double.self) {
      self = .number(n)
    } else if let s = try? c.decode(String.self) {
      self = .string(s)
    } else if let a = try? c.decode([JSONValue].self) {
      self = .array(a)
    } else {
      self = .object(try c.decode([String: JSONValue].self))
    }
  }

  public func encode(to encoder: Encoder) throws {
    var c = encoder.singleValueContainer()
    switch self {
    case .string(let s): try c.encode(s)
    case .number(let n): try c.encode(n)
    case .bool(let b): try c.encode(b)
    case .object(let o): try c.encode(o)
    case .array(let a): try c.encode(a)
    case .null: try c.encodeNil()
    }
  }

  /// The value as text, for display ("250 kcal", "12").
  public var text: String? {
    switch self {
    case .string(let s): return s
    case .number(let n):
      return n.rounded() == n && abs(n) < 1e15 ? String(Int64(n)) : String(n)
    case .bool(let b): return b ? "yes" : "no"
    default: return nil
    }
  }

  public subscript(key: String) -> JSONValue? {
    if case .object(let o) = self { return o[key] }
    return nil
  }
}

public struct Section: Codable, Hashable, Sendable {
  public var name: String?
  public var items: [String]

  public init(name: String? = nil, items: [String] = []) {
    self.name = name
    self.items = items
  }

  public init(from decoder: Decoder) throws {
    let c = try decoder.container(keyedBy: CodingKeys.self)
    name = try c.decodeIfPresent(String.self, forKey: .name)
    items = try c.decodeIfPresent([String].self, forKey: .items) ?? []
  }
}

public struct Recipe: Codable, Hashable, Identifiable, Sendable {
  public var id: Int64
  public var title: String
  public var url: String?
  public var source: String
  public var description: String?
  public var image: String?
  public var author: String?
  public var prepTime: String?
  public var cookTime: String?
  public var totalTime: String?
  public var freezeTime: String?
  public var recipeYield: String?
  public var recipeCategory: String?
  public var recipeCuisine: String?
  public var ingredients: [Section]
  public var instructions: [Section]
  /// Nutrition values may be strings or numbers, so the object is kept raw.
  public var nutrition: JSONValue?
  public var notes: String?
  /// The original source of a recipe saved from another Crumb's share.
  public var originalUrl: String?
  public var createdAt: String?
  public var updatedAt: String?

  public init(
    id: Int64, title: String, url: String? = nil, source: String = "manual",
    description: String? = nil, image: String? = nil, author: String? = nil,
    prepTime: String? = nil, cookTime: String? = nil, totalTime: String? = nil,
    freezeTime: String? = nil, recipeYield: String? = nil, recipeCategory: String? = nil,
    recipeCuisine: String? = nil, ingredients: [Section] = [], instructions: [Section] = [],
    nutrition: JSONValue? = nil, notes: String? = nil, originalUrl: String? = nil,
    createdAt: String? = nil, updatedAt: String? = nil
  ) {
    self.id = id
    self.title = title
    self.url = url
    self.source = source
    self.description = description
    self.image = image
    self.author = author
    self.prepTime = prepTime
    self.cookTime = cookTime
    self.totalTime = totalTime
    self.freezeTime = freezeTime
    self.recipeYield = recipeYield
    self.recipeCategory = recipeCategory
    self.recipeCuisine = recipeCuisine
    self.ingredients = ingredients
    self.instructions = instructions
    self.nutrition = nutrition
    self.notes = notes
    self.originalUrl = originalUrl
    self.createdAt = createdAt
    self.updatedAt = updatedAt
  }

  public init(from decoder: Decoder) throws {
    let c = try decoder.container(keyedBy: CodingKeys.self)
    id = try c.decode(Int64.self, forKey: .id)
    title = try c.decode(String.self, forKey: .title)
    url = try c.decodeIfPresent(String.self, forKey: .url)
    source = try c.decodeIfPresent(String.self, forKey: .source) ?? "manual"
    description = try c.decodeIfPresent(String.self, forKey: .description)
    image = try c.decodeIfPresent(String.self, forKey: .image)
    author = try c.decodeIfPresent(String.self, forKey: .author)
    prepTime = try c.decodeIfPresent(String.self, forKey: .prepTime)
    cookTime = try c.decodeIfPresent(String.self, forKey: .cookTime)
    totalTime = try c.decodeIfPresent(String.self, forKey: .totalTime)
    freezeTime = try c.decodeIfPresent(String.self, forKey: .freezeTime)
    recipeYield = try c.decodeIfPresent(String.self, forKey: .recipeYield)
    recipeCategory = try c.decodeIfPresent(String.self, forKey: .recipeCategory)
    recipeCuisine = try c.decodeIfPresent(String.self, forKey: .recipeCuisine)
    ingredients = try c.decodeIfPresent([Section].self, forKey: .ingredients) ?? []
    instructions = try c.decodeIfPresent([Section].self, forKey: .instructions) ?? []
    nutrition = try c.decodeIfPresent(JSONValue.self, forKey: .nutrition)
    notes = try c.decodeIfPresent(String.self, forKey: .notes)
    originalUrl = try c.decodeIfPresent(String.self, forKey: .originalUrl)
    createdAt = try c.decodeIfPresent(String.self, forKey: .createdAt)
    updatedAt = try c.decodeIfPresent(String.self, forKey: .updatedAt)
  }

  /// The recipe as the API's JSON, for crumb-core functions that take a recipe.
  public var coreJSON: String {
    var copy = self
    // crumb-core's Recipe requires both timestamps; a cached or hand-made recipe may lack them
    let epoch = "1970-01-01T00:00:00.000Z"
    copy.createdAt = createdAt ?? epoch
    copy.updatedAt = updatedAt ?? createdAt ?? epoch
    let data = (try? JSONEncoder().encode(copy)) ?? Data("{}".utf8)
    return String(decoding: data, as: UTF8.self)
  }
}

public struct RecipeSummary: Codable, Hashable, Identifiable, Sendable {
  public var id: Int64
  public var title: String
  public var image: String?
  public var totalTime: String?
  public var recipeYield: String?
  public var recipeCategory: String?
  public var recipeCuisine: String?
  public var source: String?
  public var createdAt: String?

  public init(
    id: Int64, title: String, image: String? = nil, totalTime: String? = nil,
    recipeYield: String? = nil, recipeCategory: String? = nil, recipeCuisine: String? = nil,
    source: String? = nil, createdAt: String? = nil
  ) {
    self.id = id
    self.title = title
    self.image = image
    self.totalTime = totalTime
    self.recipeYield = recipeYield
    self.recipeCategory = recipeCategory
    self.recipeCuisine = recipeCuisine
    self.source = source
    self.createdAt = createdAt
  }
}

public struct CookbookListItem: Codable, Hashable, Identifiable, Sendable {
  public var id: Int64
  public var name: String
  public var description: String?
  public var color: String?
  public var recipeCount: Int64
  public var createdAt: String?

  public init(
    id: Int64, name: String, description: String? = nil, color: String? = nil,
    recipeCount: Int64 = 0, createdAt: String? = nil
  ) {
    self.id = id
    self.name = name
    self.description = description
    self.color = color
    self.recipeCount = recipeCount
    self.createdAt = createdAt
  }

  public init(from decoder: Decoder) throws {
    let c = try decoder.container(keyedBy: CodingKeys.self)
    id = try c.decode(Int64.self, forKey: .id)
    name = try c.decode(String.self, forKey: .name)
    description = try c.decodeIfPresent(String.self, forKey: .description)
    color = try c.decodeIfPresent(String.self, forKey: .color)
    recipeCount = try c.decodeIfPresent(Int64.self, forKey: .recipeCount) ?? 0
    createdAt = try c.decodeIfPresent(String.self, forKey: .createdAt)
  }
}

public struct Cookbook: Codable, Hashable, Identifiable, Sendable {
  public var id: Int64
  public var name: String
  public var description: String?
  public var color: String?
  public var recipes: [RecipeSummary]

  public init(
    id: Int64, name: String, description: String? = nil, color: String? = nil,
    recipes: [RecipeSummary] = []
  ) {
    self.id = id
    self.name = name
    self.description = description
    self.color = color
    self.recipes = recipes
  }

  public init(from decoder: Decoder) throws {
    let c = try decoder.container(keyedBy: CodingKeys.self)
    id = try c.decode(Int64.self, forKey: .id)
    name = try c.decode(String.self, forKey: .name)
    description = try c.decodeIfPresent(String.self, forKey: .description)
    color = try c.decodeIfPresent(String.self, forKey: .color)
    recipes = try c.decodeIfPresent([RecipeSummary].self, forKey: .recipes) ?? []
  }
}

public struct ImportedCookbook: Codable, Hashable, Sendable {
  public var id: Int64
  public var name: String
  public var added: Int?
  public var duplicates: Int?
  public var skipped: Int?
}

/// `POST /api/recipes/import`: the recipe (or, for a shared cookbook link, the cookbook).
public struct ImportResult: Codable, Hashable, Sendable {
  public var id: Int64
  public var title: String
  public var isNew: Bool
  public var cookbook: ImportedCookbook?
}

/// `POST /api/recipes` and `PATCH /api/recipes/{id}`: every settable field.
public struct RecipeFields: Codable, Hashable, Sendable {
  public var title: String
  public var description: String?
  public var url: String?
  public var image: String?
  public var author: String?
  public var prepTime: String?
  public var cookTime: String?
  public var totalTime: String?
  public var freezeTime: String?
  public var recipeYield: String?
  public var recipeCategory: String?
  public var recipeCuisine: String?
  public var ingredients: [Section]
  public var instructions: [Section]
  public var nutrition: [String: String]?
  public var notes: String?

  public init(
    title: String, description: String? = nil, url: String? = nil, image: String? = nil,
    author: String? = nil, prepTime: String? = nil, cookTime: String? = nil,
    totalTime: String? = nil, freezeTime: String? = nil, recipeYield: String? = nil,
    recipeCategory: String? = nil, recipeCuisine: String? = nil, ingredients: [Section] = [],
    instructions: [Section] = [], nutrition: [String: String]? = nil, notes: String? = nil
  ) {
    self.title = title
    self.description = description
    self.url = url
    self.image = image
    self.author = author
    self.prepTime = prepTime
    self.cookTime = cookTime
    self.totalTime = totalTime
    self.freezeTime = freezeTime
    self.recipeYield = recipeYield
    self.recipeCategory = recipeCategory
    self.recipeCuisine = recipeCuisine
    self.ingredients = ingredients
    self.instructions = instructions
    self.nutrition = nutrition
    self.notes = notes
  }

  /// A recipe's fields as the editor starts them.
  public init(_ recipe: Recipe) {
    var nutrition: [String: String]?
    if case .object(let values) = recipe.nutrition {
      nutrition = values.compactMapValues(\.text)
    }
    self.init(
      title: recipe.title, description: recipe.description, url: recipe.url,
      image: recipe.image, author: recipe.author, prepTime: recipe.prepTime,
      cookTime: recipe.cookTime, totalTime: recipe.totalTime, freezeTime: recipe.freezeTime,
      recipeYield: recipe.recipeYield, recipeCategory: recipe.recipeCategory,
      recipeCuisine: recipe.recipeCuisine, ingredients: recipe.ingredients,
      instructions: recipe.instructions, nutrition: nutrition, notes: recipe.notes)
  }
}

/// `POST /api/recipes/{id}/cooked`: the cook stats plus the new event (nil if deduped).
public struct Cooked: Codable, Hashable, Sendable {
  public var count: Int
  public var lastCookedAt: String?
  public var eventId: Int64?

  public init(from decoder: Decoder) throws {
    let c = try decoder.container(keyedBy: CodingKeys.self)
    count = try c.decodeIfPresent(Int.self, forKey: .count) ?? 0
    lastCookedAt = try c.decodeIfPresent(String.self, forKey: .lastCookedAt)
    eventId = try c.decodeIfPresent(Int64.self, forKey: .eventId)
  }
}

/// A downloaded file.
public struct Download: Hashable, Sendable {
  public var fileName: String
  public var mimeType: String
  public var data: Data
}

/// One file for `POST /api/import/files`.
public struct UploadFile: Hashable, Sendable {
  public var name: String
  public var mimeType: String
  public var data: Data

  public init(name: String, mimeType: String, data: Data) {
    self.name = name
    self.mimeType = mimeType
    self.data = data
  }
}

public struct CreatedRecipe: Codable, Hashable, Sendable {
  public var id: Int64
  public var title: String
}

/// `POST /api/import/files`: what one uploaded file gave.
public struct ImportSummary: Codable, Hashable, Sendable {
  public var file: String
  public var created: [CreatedRecipe]
  public var duplicates: Int
  public var skipped: Int
  public var error: String?

  public init(from decoder: Decoder) throws {
    let c = try decoder.container(keyedBy: CodingKeys.self)
    file = try c.decode(String.self, forKey: .file)
    created = try c.decodeIfPresent([CreatedRecipe].self, forKey: .created) ?? []
    duplicates = try c.decodeIfPresent(Int.self, forKey: .duplicates) ?? 0
    skipped = try c.decodeIfPresent(Int.self, forKey: .skipped) ?? 0
    error = try c.decodeIfPresent(String.self, forKey: .error)
  }
}

/// One "Try next" card: a recipe and why Wee Chef or the algorithm picked it.
public struct Suggestion: Codable, Hashable, Sendable {
  public var recipe: RecipeSummary
  public var reason: String
  public var reasonKind: String
  public var ai: Bool?
}

/// A dish Wee Chef thinks the cook would like that isn't in their box.
public struct Idea: Codable, Hashable, Sendable {
  public var title: String
  public var why: String
  public var searchUrl: String
}

/// `GET /api/suggestions`: the cards, the AI status (`ready`/`pending`/`off`) and an idea.
public struct Suggestions: Codable, Hashable, Sendable {
  public var items: [Suggestion]
  public var ai: String
  public var idea: Idea?

  public init(items: [Suggestion] = [], ai: String = "off", idea: Idea? = nil) {
    self.items = items
    self.ai = ai
    self.idea = idea
  }

  public init(from decoder: Decoder) throws {
    let c = try decoder.container(keyedBy: CodingKeys.self)
    items = try c.decodeIfPresent([Suggestion].self, forKey: .items) ?? []
    ai = try c.decodeIfPresent(String.self, forKey: .ai) ?? "off"
    idea = try c.decodeIfPresent(Idea.self, forKey: .idea)
  }
}

/// One line Wee Chef flagged: what it saw and what was done about it.
public struct Flag: Codable, Hashable, Identifiable, Sendable {
  public var id: Int64
  public var field: String
  public var itemText: String?
  public var kind: String
  public var state: String
  /// The fix's data (`{p, fix, category, was, …}`).
  public var detail: JSONValue?
}

/// `GET`/`POST /api/recipes/{id}/checks`: nil when the recipe was never checked.
public struct RecipeChecks: Codable, Hashable, Sendable {
  public var status: String
  public var canUndo: Bool
  public var flags: [Flag]

  public init(from decoder: Decoder) throws {
    let c = try decoder.container(keyedBy: CodingKeys.self)
    status = try c.decode(String.self, forKey: .status)
    canUndo = try c.decodeIfPresent(Bool.self, forKey: .canUndo) ?? false
    flags = try c.decodeIfPresent([Flag].self, forKey: .flags) ?? []
  }
}

/// `POST /api/recipes/{id}/checks/undo`: the restored recipe and its new check.
public struct UndoResult: Codable, Hashable, Sendable {
  public var recipe: Recipe
  public var checks: RecipeChecks?
}

/// `GET`/`POST /api/checks`: Wee Chef's import check progress.
public struct ChecksStatus: Codable, Hashable, Sendable {
  public var enabled: Bool = false
  public var eligible: Int = 0
  public var checked: Int = 0
  public var pending: Int = 0
  public var failed: Int = 0
  public var tidied: Int = 0
  public var toCheck: Int = 0
  public var due: Int = 0
  public var restored: Int = 0
  public var edited: Int = 0
  /// Only on `POST /api/checks`: how many "Check all" queued.
  public var queued: Int = 0

  public init() {}

  public init(from decoder: Decoder) throws {
    let c = try decoder.container(keyedBy: CodingKeys.self)
    func int(_ key: CodingKeys) throws -> Int { try c.decodeIfPresent(Int.self, forKey: key) ?? 0 }
    enabled = try c.decodeIfPresent(Bool.self, forKey: .enabled) ?? false
    eligible = try int(.eligible)
    checked = try int(.checked)
    pending = try int(.pending)
    failed = try int(.failed)
    tidied = try int(.tidied)
    toCheck = try int(.toCheck)
    due = try int(.due)
    restored = try int(.restored)
    edited = try int(.edited)
    queued = try int(.queued)
  }
}

/// One recipe on the Review page with how many flags it has per field.
public struct ReviewRecipe: Codable, Hashable, Identifiable, Sendable {
  public var id: Int64
  public var title: String
  public var image: String?
  public var count: Int
  public var fields: [String: Int]

  public init(from decoder: Decoder) throws {
    let c = try decoder.container(keyedBy: CodingKeys.self)
    id = try c.decode(Int64.self, forKey: .id)
    title = try c.decode(String.self, forKey: .title)
    image = try c.decodeIfPresent(String.self, forKey: .image)
    count = try c.decodeIfPresent(Int.self, forKey: .count) ?? 0
    fields = try c.decodeIfPresent([String: Int].self, forKey: .fields) ?? [:]
  }
}

/// Which kind of thing a share link shows; the raw value is the API path segment.
public enum ShareKind: String, Sendable {
  case recipe = "recipes"
  case cookbook = "cookbooks"
}

/// `POST`/`PATCH /api/{recipes|cookbooks}/{id}/share`.
public struct Share: Codable, Hashable, Sendable {
  public var token: String
  public var url: String
  public var includeNotes: Bool?
  public var createdAt: String?
}

/// `GET /api/shares`: one live share link.
public struct SharedLink: Codable, Hashable, Sendable {
  public var kind: String
  public var id: Int64
  public var title: String
  public var url: String
  public var includeNotes: Bool?
  public var createdAt: String?
  public var lastOpenedAt: String?
}

/// `GET /api/connector`: what this server has configured.
public struct ConnectorInfo: Codable, Hashable, Sendable {
  public var mcpUrl: String?
  public var authEnabled: Bool?
  public var weeChef: Bool?
  public var claudeParsing: Bool?
  public var aiProvider: String?
  public var vision: Bool?
  public var browserScraping: Bool?
  public var weeChefChecks: Bool?
}
