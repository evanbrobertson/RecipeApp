import CrumbKit
import XCTest

@testable import Crumb

@MainActor
final class AppModelTests: XCTestCase {
  private func model() -> AppModel {
    AppModel(mode: LaunchMode(uiTesting: true, demo: true))
  }

  func testDemoStartsSignedIn() {
    let app = model()
    XCTAssertEqual(app.session?.server, DemoServer.url)
  }

  func testDeepLinksOpenTheRightTab() throws {
    let app = model()
    app.handle(url: try XCTUnwrap(URL(string: "crumb://recipes/12")))
    XCTAssertEqual(app.tab, .recipes)
    XCTAssertEqual(app.paths[.recipes], [.recipe(12)])

    app.handle(url: try XCTUnwrap(URL(string: "crumb://cookbooks/3")))
    XCTAssertEqual(app.tab, .shelf)
    XCTAssertEqual(app.paths[.shelf], [.cookbook(3)])

    app.handle(url: try XCTUnwrap(URL(string: "crumb://add?text=https%3A%2F%2Fexample.com%2Fpie")))
    XCTAssertTrue(app.showAdd)
    XCTAssertEqual(app.addDraft, "https://example.com/pie")

    app.handle(url: try XCTUnwrap(URL(string: "https://example.com/recipes/1")))
    XCTAssertEqual(app.paths[.recipes], [.recipe(12)])
  }

  func testSignInRejectsBadAddressesBeforeAnyRequest() async {
    let app = model()
    let bad = await app.signIn(server: "not a server", password: "x")
    XCTAssertEqual(bad, "That doesn't look like a web address.")
    let cleartext = await app.signIn(server: "http://crumb.example.com", password: "x")
    XCTAssertEqual(
      cleartext, "Use an https:// address for your Crumb. Plain http:// only works on your home network.")
  }

  func testSignOutForgetsTheSession() async {
    let app = model()
    await app.signOut()
    XCTAssertNil(app.session)
    XCTAssertEqual(app.tab, .home)
  }

  func testUnauthorizedSignsOut() {
    let app = model()
    let message = app.handle(CrumbError.api(status: 401, message: "Please sign in again."))
    XCTAssertEqual(message, "Please sign in again.")
    XCTAssertNil(app.session)
  }

  func testDemoServerSpeaksTheAPI() async throws {
    let app = model()
    let recipes = try await app.api.recipes()
    XCTAssertEqual(recipes.map(\.title), ["Weeknight Dal", "Apple Pie", "Leek and Potato Soup"])
    let pie = try await app.api.recipe(id: 2)
    XCTAssertEqual(pie.cookSteps.count, 4)
    XCTAssertEqual(pie.cookSteps[3].section, "For the glaze")
    let book = try await app.api.cookbook(id: 1)
    XCTAssertEqual(book.recipes.count, 2)
  }

  func testThemeTokensDifferInDark() {
    let light = UIColor(Palette.canvas).resolvedColor(with: UITraitCollection(userInterfaceStyle: .light))
    let dark = UIColor(Palette.canvas).resolvedColor(with: UITraitCollection(userInterfaceStyle: .dark))
    XCTAssertNotEqual(light, dark)
  }

  func testFontsAreBundled() {
    for name in [
      "NunitoSans-Regular", "NunitoSans-SemiBold", "NunitoSans-Bold", "NunitoSans-ExtraBold",
      "DMSerifDisplay-Regular", "Caveat-Bold",
    ] {
      XCTAssertNotNil(UIFont(name: name, size: 17), "\(name) isn't registered")
    }
  }
}
