import XCTest

@testable import CrumbKit

final class CrumbAPITests: XCTestCase {
  override func setUp() {
    StubServer.reset()
  }

  func testLoginReturnsTheSessionCookieAndSendsThePassword() async throws {
    StubServer.reply { _ in
      .init(
        body: #"{"ok":true}"#,
        headers: [
          "Content-Type": "application/json",
          "Set-Cookie": "crumb_session=123.abc; Path=/; HttpOnly; SameSite=Lax; Expires=Wed, 21 Oct 2026 07:28:00 GMT",
        ])
    }
    let cookie = try await makeAPI(cookie: nil).login(server: testServer, password: "hunter2")
    XCTAssertEqual(cookie, "123.abc")
    let request = try XCTUnwrap(StubServer.last)
    XCTAssertEqual(request.url?.absoluteString, "https://crumb.example.com/api/auth/login")
    XCTAssertEqual(request.httpMethod, "POST")
    XCTAssertEqual(request.bodyJSON?["password"] as? String, "hunter2")
    XCTAssertNil(request.value(forHTTPHeaderField: "Cookie"))
  }

  func testLoginWithoutAPasswordServerHasNoCookie() async throws {
    StubServer.reply { _ in .init(body: #"{"ok":true}"#) }
    let cookie = try await makeAPI(cookie: nil).login(server: testServer, password: " ")
    XCTAssertNil(cookie)
  }

  func testWrongPasswordIsA401WithTheServersMessage() async {
    StubServer.reply { _ in
      .init(status: 401, body: #"{"statusCode":401,"statusMessage":"Unauthorized","message":"Incorrect password"}"#)
    }
    do {
      _ = try await makeAPI(cookie: nil).login(server: testServer, password: "nope")
      XCTFail("expected an error")
    } catch let error as CrumbError {
      XCTAssertEqual(error, .api(status: 401, message: "Incorrect password"))
      XCTAssertTrue(error.isSignedOut)
    } catch {
      XCTFail("unexpected \(error)")
    }
  }

  func testRequestsCarryTheSessionCookie() async throws {
    StubServer.reply { _ in .init(body: "[]") }
    _ = try await makeAPI().recipes(query: "  pie ", limit: 5)
    let request = try XCTUnwrap(StubServer.last)
    XCTAssertEqual(request.value(forHTTPHeaderField: "Cookie"), "crumb_session=issued.sig")
    let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
    XCTAssertEqual(components?.path, "/api/recipes")
    XCTAssertEqual(components?.queryItems?.first { $0.name == "q" }?.value, "pie")
    XCTAssertEqual(components?.queryItems?.first { $0.name == "limit" }?.value, "5")
  }

  func testDecodesARecipeIgnoringUnknownFields() async throws {
    StubServer.reply { _ in .init(body: recipeJSON) }
    let recipe = try await makeAPI().recipe(id: 7)
    XCTAssertEqual(recipe.title, "Apple Pie")
    XCTAssertEqual(recipe.ingredients.count, 2)
    XCTAssertEqual(recipe.ingredients[1].name, "For the glaze")
    XCTAssertEqual(recipe.nutrition?["calories"]?.text, "250 kcal")
    XCTAssertEqual(recipe.nutrition?["protein"]?.text, "4")
    XCTAssertEqual(StubServer.last?.url?.path, "/api/recipes/7")
  }

  func testImportClassifiesLinksAndText() async throws {
    StubServer.reply { _ in .init(body: #"{"id":3,"title":"Pie","isNew":true}"#) }
    let api = makeAPI()
    let linked = try await api.importInput("Look at this https://example.com/pie")
    XCTAssertEqual(linked.id, 3)
    XCTAssertEqual(StubServer.last?.bodyJSON?["url"] as? String, "https://example.com/pie")
    _ = try await api.importInput("2 eggs\nWhisk them.")
    XCTAssertEqual(StubServer.last?.bodyJSON?["text"] as? String, "2 eggs\nWhisk them.")
  }

  func testImportOfASharedCookbook() async throws {
    StubServer.reply { _ in
      .init(
        body: #"{"id":9,"title":"Bakes","isNew":true,"#
          + #""cookbook":{"id":9,"name":"Bakes","added":4,"duplicates":1}}"#)
    }
    let result = try await makeAPI().importURL("https://other.example/s/abcdefghijklmnop")
    XCTAssertEqual(result.cookbook?.id, 9)
    XCTAssertEqual(result.cookbook?.added, 4)
  }

  func testPhotoImportIsMultipart() async throws {
    StubServer.reply { _ in .init(body: #"{"id":5,"title":"Card","isNew":true}"#) }
    _ = try await makeAPI().importPhotos([Data([0xFF, 0xD8, 0xFF]), Data([0xFF, 0xD8])], hint: "Gran's")
    let request = try XCTUnwrap(StubServer.last)
    XCTAssertEqual(request.url?.path, "/api/recipes/import/photos")
    let type = try XCTUnwrap(request.value(forHTTPHeaderField: "Content-Type"))
    XCTAssertTrue(type.hasPrefix("multipart/form-data; boundary="))
    let body = request.bodyText
    XCTAssertEqual(body.components(separatedBy: "name=\"photo\"").count - 1, 2)
    XCTAssertTrue(body.contains("name=\"text\"\r\n\r\nGran's"))
  }

  func testMissingRecipeReadsLikeTheServerMeant() async {
    StubServer.reply { _ in .init(status: 404, body: "") }
    do {
      _ = try await makeAPI().recipe(id: 99)
      XCTFail("expected an error")
    } catch {
      XCTAssertEqual(error.friendlyMessage, "That isn't in your recipe box any more.")
    }
  }

  func testValidationMessagesLoseTheirFieldPrefix() async {
    StubServer.reply { _ in
      .init(
        status: 400, body: #"{"statusCode":400,"statusMessage":"Bad Request","message":"title: Title is required"}"#)
    }
    do {
      _ = try await makeAPI().createRecipe(RecipeFields(title: ""))
      XCTFail("expected an error")
    } catch {
      XCTAssertEqual(error.friendlyMessage, "Title is required")
    }
  }

  func testNoConnectionIsOffline() async {
    StubServer.offline = true
    do {
      _ = try await makeAPI().cookbooks()
      XCTFail("expected an error")
    } catch let error as CrumbError {
      XCTAssertTrue(error.isOffline)
    } catch {
      XCTFail("unexpected \(error)")
    }
  }

  func testRandomOnAnEmptyBoxIsNil() async throws {
    StubServer.reply { _ in .init(status: 404, body: #"{"message":"No recipes saved yet"}"#) }
    let random = try await makeAPI().randomRecipe(exclude: [1, 2])
    XCTAssertNil(random)
    XCTAssertEqual(StubServer.last?.url?.query, "exclude=1,2")
  }

  func testCookedAndUndo() async throws {
    StubServer.reply { request in
      request.httpMethod == "POST"
        ? .init(body: #"{"count":2,"lastCookedAt":"2026-09-26T16:30:33.000Z","eventId":41}"#)
        : .init(body: #"{"count":1}"#)
    }
    let api = makeAPI()
    let cooked = try await api.markCooked(id: 7)
    XCTAssertEqual(cooked.eventId, 41)
    try await api.undoCooked(id: 7, eventId: 41)
    XCTAssertEqual(StubServer.last?.httpMethod, "DELETE")
    XCTAssertEqual(StubServer.last?.url?.query, "event=41")
  }

  func testSuggestionsAndChecks() async throws {
    StubServer.reply { request in
      switch request.url?.path {
      case "/api/suggestions":
        return .init(
          body: #"{"items":[{"recipe":{"id":1,"title":"Soup"},"reason":"Not cooked in a while","#
            + #""reasonKind":"rest"}],"ai":"ready"}"#)
      case "/api/checks":
        return .init(body: #"{"enabled":true,"eligible":10,"checked":10,"toCheck":2}"#)
      default:
        return .init(body: "null")
      }
    }
    let api = makeAPI()
    let suggestions = try await api.suggestions(limit: 3)
    XCTAssertEqual(suggestions.items.first?.recipe.title, "Soup")
    XCTAssertEqual(suggestions.ai, "ready")
    let status = try await api.checksStatus()
    XCTAssertEqual(RecipeText.checksStatus(status), "All 10 recipes checked · 2 recipes to look at")
    let checks = try await api.recipeChecks(id: 1)
    XCTAssertNil(checks)
  }

  func testDownloadsUseTheServersFileName() async throws {
    StubServer.reply { _ in
      .init(
        body: "# Pie",
        headers: [
          "Content-Type": "text/markdown; charset=utf-8",
          "Content-Disposition": "attachment; filename=\"pie.md\"; filename*=UTF-8''apple%20pie.md",
        ])
    }
    let file = try await makeAPI().exportRecipe(id: 7, format: "md")
    XCTAssertEqual(file.fileName, "apple pie.md")
    XCTAssertEqual(file.mimeType, "text/markdown")
    XCTAssertEqual(String(decoding: file.data, as: UTF8.self), "# Pie")
  }

  func testPhotoURLsMatchTheWeb() {
    let api = makeAPI()
    let url = api.photoURL(recipeId: 7, image: "a", pixels: 300)
    XCTAssertEqual(url?.absoluteString, "https://crumb.example.com/img/7/320?v=e40c292c")
    XCTAssertNil(api.photoURL(recipeId: 7, image: nil, pixels: 300))
    let photo = api.photoRequest(url!)
    XCTAssertEqual(photo.value(forHTTPHeaderField: "Cookie"), "crumb_session=issued.sig")
    let elsewhere = api.photoRequest(URL(string: "https://example.com/pie.jpg")!)
    XCTAssertNil(elsewhere.value(forHTTPHeaderField: "Cookie"))
  }

  func testSignedOutWithoutASession() async {
    let api = CrumbAPI(http: CrumbAPI.makeSession(protocolClasses: [StubServer.self]), session: { nil })
    do {
      _ = try await api.recipes()
      XCTFail("expected an error")
    } catch let error as CrumbError {
      XCTAssertEqual(error, .signedOut)
    } catch {
      XCTFail("unexpected \(error)")
    }
  }
}
