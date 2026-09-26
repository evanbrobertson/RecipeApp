import XCTest

@testable import CrumbKit

/// crumb-core through UniFFI, and the stores built on it. These run the real Rust core, not
/// a mock: if the bindings or the XCFramework are wrong, they fail.
final class CoreBindingsTests: XCTestCase {
  func testRecipeLogicComesFromTheCore() throws {
    let recipe = try JSONDecoder().decode(Recipe.self, from: Data(recipeJSON.utf8))
    let steps = recipe.cookSteps
    XCTAssertEqual(steps.count, 3)
    XCTAssertEqual(steps[2].section, "For the glaze")
    XCTAssertEqual(recipe.ingredients(for: steps[0]).map(\.text), ["2 cups flour"])
    XCTAssertEqual(recipe.ingredients(for: steps[2]).map(\.text), ["1 egg"])
    XCTAssertEqual(recipe.kicker, "Dessert · British")
    XCTAssertEqual(recipe.sourceLink?.absoluteString, "https://example.com/pie")
    XCTAssertEqual(recipe.sourceHost, "example.com")
    XCTAssertTrue(recipe.shareText().hasPrefix("Apple Pie\n"))
    XCTAssertTrue(recipe.shareText().contains("• 2 cups flour"))
  }

  func testScalingTimersAndDurations() {
    XCTAssertEqual(RecipeText.scaled("2 cups flour", by: 1.5), "3 cups flour")
    XCTAssertEqual(RecipeText.timers(in: "Bake for 25-30 minutes").first?.seconds, 1800)
    XCTAssertEqual(RecipeText.duration("PT1H5M"), "1h 5m")
    XCTAssertEqual(RecipeText.duration("about an hour"), "about an hour")
    XCTAssertNil(RecipeText.duration(nil))
  }

  func testServerAddresses() {
    XCTAssertEqual(CrumbCore.serverUrl(input: "crumb.example.com"), "https://crumb.example.com/")
    XCTAssertNil(CrumbCore.serverUrl(input: "not a server"))
    XCTAssertTrue(CrumbCore.allowsCleartext(baseUrl: "http://192.168.1.5:3000/"))
    XCTAssertFalse(CrumbCore.allowsCleartext(baseUrl: "http://crumb.example.com/"))
  }

  func testOfflineSearch() {
    let all = [
      RecipeSummary(id: 1, title: "Apple Pie", recipeCategory: "Dessert"),
      RecipeSummary(id: 2, title: "Leek Soup", recipeCuisine: "Welsh"),
    ]
    XCTAssertEqual(RecipeRepository.filterOffline(all, query: "welsh soup").map(\.id), [2])
    XCTAssertEqual(RecipeRepository.filterOffline(all, query: "PIE").map(\.id), [1])
  }

  func testFlagWording() throws {
    let fixed = try JSONDecoder().decode(
      Flag.self,
      from: Data(#"{"id":1,"field":"ingredients","itemText":"Sauce:","kind":"heading","state":"fixed","detail":{"fix":"heading"}}"#.utf8))
    XCTAssertEqual(RecipeText.flagText(fixed), "Made “Sauce:” a section heading")
    let review = try JSONDecoder().decode(
      Flag.self,
      from: Data(#"{"id":2,"field":"ingredients","itemText":"salt and pepper","kind":"merged","state":"review"}"#.utf8))
    XCTAssertEqual(RecipeText.flagText(review), "“salt and pepper” might be two ingredients on one line")
  }
}

final class StoreTests: XCTestCase {
  private func defaults() -> UserDefaults {
    let name = "crumb-tests-\(UUID().uuidString)"
    return UserDefaults(suiteName: name)!
  }

  func testSessionsRoundTripAndSignOutKeepsTheServer() {
    let secrets = MemorySecretStore()
    let prefs = defaults()
    let store = SessionStore(secrets: secrets, defaults: prefs)
    XCTAssertNil(store.session)
    store.signIn(server: testServer, cookie: "abc")
    XCTAssertEqual(SessionStore(secrets: secrets, defaults: prefs).session, Session(server: testServer, cookie: "abc"))
    store.signOut()
    let again = SessionStore(secrets: secrets, defaults: prefs)
    XCTAssertNil(again.session)
    XCTAssertEqual(again.lastServer, testServer.absoluteString)
  }

  func testPasswordlessServersStaySignedIn() {
    let secrets = MemorySecretStore()
    let prefs = defaults()
    SessionStore(secrets: secrets, defaults: prefs).signIn(server: testServer, cookie: nil)
    XCTAssertEqual(SessionStore(secrets: secrets, defaults: prefs).session, Session(server: testServer, cookie: nil))
  }

  func testCacheIsPerServerAndCleared() throws {
    let root = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let cache = RecipeCache(root: root)
    cache.useServer("https://a.example/")
    cache.putRecipes([RecipeSummary(id: 1, title: "Pie")])
    XCTAssertEqual(cache.recipes()?.first?.title, "Pie")
    cache.useServer("https://b.example/")
    XCTAssertNil(cache.recipes())
    cache.useServer("https://a.example/")
    cache.clear()
    XCTAssertNil(cache.recipes())
  }

  func testRepositoryFallsBackToTheCacheOffline() async throws {
    StubServer.reset()
    let root = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let cache = RecipeCache(root: root)
    cache.useServer(testServer.absoluteString)
    let repo = RecipeRepository(api: makeAPI(), cache: cache)

    StubServer.reply { _ in .init(body: recipeJSON) }
    let online = try await repo.recipe(id: 7)
    XCTAssertFalse(online.offline)

    StubServer.offline = true
    let offline = try await repo.recipe(id: 7)
    XCTAssertTrue(offline.offline)
    XCTAssertEqual(offline.value.title, "Apple Pie")

    do {
      _ = try await repo.recipe(id: 8)
      XCTFail("nothing cached for 8")
    } catch let error as CrumbError {
      XCTAssertTrue(error.isOffline)
    }
  }

  func testTimersPersistRestartAndRing() {
    final class Alarms: TimerAlarms {
      var scheduled: [UUID] = []
      var cancelled: [UUID] = []
      func schedule(_ timer: KitchenTimer) { scheduled.append(timer.id) }
      func cancel(_ id: UUID) { cancelled.append(id) }
    }
    let alarms = Alarms()
    let prefs = defaults()
    let now = Date(timeIntervalSince1970: 1_000_000)
    let timers = KitchenTimers(defaults: prefs, alarms: alarms)
    let bake = timers.start(label: "Bake", seconds: 1500, recipeId: 7, now: now)
    XCTAssertEqual(bake.clock(at: now), "25:00")
    XCTAssertEqual(bake.clock(at: now.addingTimeInterval(1255)), "4:05")
    XCTAssertEqual(alarms.scheduled, [bake.id])

    // Starting the same step again restarts it rather than adding a second
    let again = timers.start(label: "Bake", seconds: 1500, recipeId: 7, now: now.addingTimeInterval(10))
    XCTAssertEqual(timers.timers.map(\.id), [again.id])
    XCTAssertEqual(alarms.cancelled, [bake.id])

    XCTAssertEqual(KitchenTimers(defaults: prefs).timers.map(\.id), [again.id])
    XCTAssertTrue(timers.done(at: now.addingTimeInterval(1511)).contains { $0.id == again.id })

    timers.addMinute(again.id, now: now.addingTimeInterval(2000))
    XCTAssertEqual(timers.timers.first?.remaining(at: now.addingTimeInterval(2000)), 60)
    timers.dismiss(again.id)
    XCTAssertTrue(KitchenTimers(defaults: prefs).timers.isEmpty)
  }

  func testThemeModes() {
    let prefs = defaults()
    let theme = ThemeSettings(defaults: prefs)
    XCTAssertEqual(theme.mode, .sun)
    theme.mode = .dark
    XCTAssertEqual(ThemeSettings(defaults: prefs).mode, .dark)
    XCTAssertEqual(theme.decide().dark, true)
    theme.mode = .system
    XCTAssertNil(theme.decide().dark)

    // London, midsummer: light at noon UTC, dark at 23:00 UTC
    theme.mode = .sun
    theme.location = SunLocation(lat: 51.5, lng: -0.13)
    let noon = Date(timeIntervalSince1970: 1_782_043_200)
    XCTAssertEqual(theme.decide(now: noon).dark, false)
    XCTAssertEqual(theme.decide(now: noon.addingTimeInterval(11 * 3600)).dark, true)
    let recheck = try? XCTUnwrap(theme.decide(now: noon).recheckAt)
    XCTAssertLessThanOrEqual(recheck?.timeIntervalSince(noon) ?? 0, 3600)
    XCTAssertEqual(ThemeSettings(defaults: prefs).location, SunLocation(lat: 51.5, lng: -0.13))
  }

  func testLocationEstimateFromTheTimeZone() {
    let tokyo = ThemeSettings.estimate(zone: TimeZone(identifier: "Asia/Tokyo")!)
    XCTAssertEqual(tokyo, SunLocation(lat: 30, lng: 135))
    // New York's standard offset (-5h), not its summer one
    let newYork = ThemeSettings.estimate(zone: TimeZone(identifier: "America/New_York")!)
    XCTAssertEqual(newYork, SunLocation(lat: 40, lng: -75))
  }
}
