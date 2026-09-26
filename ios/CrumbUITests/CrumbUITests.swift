import XCTest

/// The app end to end on a simulator. `-demo` runs it against the pretend server built into
/// Debug builds (Crumb/Debug/DemoServer.swift), so no network or real Crumb is needed.
final class CrumbUITests: XCTestCase {
  override func setUp() {
    continueAfterFailure = false
  }

  private func launch(_ arguments: [String]) -> XCUIApplication {
    let app = XCUIApplication()
    app.launchArguments = ["-ui-testing"] + arguments
    app.launch()
    return app
  }

  /// Anything on screen whose label contains `text` (cards read as one combined label).
  private func element(_ app: XCUIApplication, containing text: String) -> XCUIElement {
    app.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", text)).firstMatch
  }

  func testSignInExplainsABadAddress() {
    let app = launch([])
    let server = app.textFields["server"]
    XCTAssertTrue(server.waitForExistence(timeout: 10))
    server.tap()
    server.typeText("not a server")
    app.buttons["sign-in"].tap()
    let error = app.staticTexts["sign-in-error"]
    XCTAssertTrue(error.waitForExistence(timeout: 5))
    XCTAssertEqual(error.label, "That doesn't look like a web address.")
  }

  func testBrowseARecipeAndCookIt() {
    let app = launch(["-demo"])
    app.tabBars.buttons["Recipes"].tap()

    let card = app.buttons.matching(identifier: "recipe-card").element(boundBy: 1)
    XCTAssertTrue(card.waitForExistence(timeout: 10))
    card.tap()

    XCTAssertTrue(app.staticTexts["recipe-title"].waitForExistence(timeout: 10))
    XCTAssertEqual(app.staticTexts["recipe-title"].label, "Apple Pie")
    // Scaling and the timer come from crumb-core
    XCTAssertTrue(element(app, containing: "300g plain flour").exists)
    XCTAssertTrue(element(app, containing: "45 minutes").exists)

    app.buttons["start-cooking"].tap()
    let step = app.staticTexts.matching(identifier: "cook-step").firstMatch
    XCTAssertTrue(step.waitForExistence(timeout: 5))
    XCTAssertEqual(step.label, "Rub the butter into the flour.")
    XCTAssertTrue(element(app, containing: "150g cold butter").exists, "the step's ingredients are listed")

    app.buttons["next-step"].tap()
    XCTAssertTrue(element(app, containing: "Slice the apples").waitForExistence(timeout: 5))
    app.buttons["close-cook"].tap()
    XCTAssertTrue(app.staticTexts["recipe-title"].waitForExistence(timeout: 5))
  }

  func testHomeSuggestsAndShelfShowsCookbooks() {
    let app = launch(["-demo"])
    XCTAssertTrue(app.staticTexts["What's cooking?"].waitForExistence(timeout: 10))
    XCTAssertTrue(element(app, containing: "Leek and Potato Soup").waitForExistence(timeout: 10))

    app.tabBars.buttons["Shelf"].tap()
    let book = element(app, containing: "Weeknights")
    XCTAssertTrue(book.waitForExistence(timeout: 10))
    book.tap()
    XCTAssertTrue(element(app, containing: "Weeknight Dal").waitForExistence(timeout: 10))
  }

  func testReviewShowsWeeChefsStatus() {
    let app = launch(["-demo"])
    app.tabBars.buttons["Review"].tap()
    let status = app.staticTexts["checks-status"]
    XCTAssertTrue(status.waitForExistence(timeout: 10))
    XCTAssertEqual(status.label, "All 3 recipes checked · 1 recipe to look at")
  }

  func testSignOutReturnsToSignIn() {
    let app = launch(["-demo"])
    app.tabBars.buttons["More"].tap()
    let signOut = app.buttons["sign-out"]
    XCTAssertTrue(signOut.waitForExistence(timeout: 10))
    signOut.tap()
    XCTAssertTrue(app.textFields["server"].waitForExistence(timeout: 10))
  }
}
