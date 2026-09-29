import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The window: sign-in until there's a session, then the nav rail and one page at a time.
//
// Pages are files named in `pages` below. Each gets `session`, `routeId` (the id in
// `recipe:7`) and `params`, and moves around with `ApplicationWindow.window.go(name, params)`,
// `.back()` and `.toast({...})`, like the web's links, history and toasts.
ApplicationWindow {
    id: window
    visible: true
    width: sizeFrom(Smoke.size, 0, 1280)
    height: sizeFrom(Smoke.size, 1, 900)
    minimumWidth: 720
    minimumHeight: 480
    title: "Crumb"
    color: Palette.bg

    Behavior on color {
        ColorAnimation { duration: 150 }
    }

    function sizeFrom(text, index, fallback) {
        var parts = String(text).split("x")
        var n = parseInt(parts[index])
        return parts.length === 2 && n > 0 ? n : fallback
    }

    Session {
        id: appSession
    }

    // ─── Routes ───
    readonly property var pages: ({
            "home": "HomePage.qml",
            "recipes": "RecipesPage.qml",
            "recipe": "RecipePage.qml",
            "cook": "CookPage.qml",
            "prep": "PrepPage.qml",
            "edit": "EditPage.qml",
            "new": "EditPage.qml",
            "shelf": "ShelfPage.qml",
            "cookbook": "CookbookPage.qml",
            "suggestions": "SuggestionsPage.qml",
            "add": "AddPage.qml",
            "import": "ImportPage.qml",
            "more": "MorePage.qml",
            "account": "AccountPage.qml",
            "connect": "ConnectPage.qml",
            "connections": "ConnectionsPage.qml",
            "trash": "TrashPage.qml",
            "preview": "PreviewPage.qml"
        })
    // The nav item each page belongs to (the web's `section`)
    readonly property var sections: ({
            "home": "kitchen",
            "recipes": "recipes",
            "recipe": "recipes",
            "cook": "recipes",
            "prep": "recipes",
            "edit": "recipes",
            "new": "add",
            "shelf": "shelf",
            "cookbook": "shelf",
            "suggestions": "suggestions",
            "add": "add",
            "import": "more",
            "more": "more",
            "account": "more",
            "connect": "more",
            "connections": "more",
            "trash": "more",
            "preview": "add"
        })
    // Pages with no nav rail (cook mode fills the window)
    readonly property var bare: ({ "cook": true })

    property var route: ({ "name": "home", "id": 0, "params": {} })
    property var history: []
    // What the nav rail needs from the server: Wee Chef checks, and lines to review.
    property var connector: ({})
    property int reviewCount: 0

    // Opens a page: go("recipe", {id: 7}). `replace` leaves no history entry.
    function go(name, params, replace) {
        if (!pages[name]) {
            console.warn("no such page:", name)
            return
        }
        params = params || {}
        if (!replace)
            history.push(route)
        route = { "name": name, "id": params.id || 0, "params": params }
    }

    function back() {
        if (history.length === 0) {
            if (route.name !== "home")
                route = { "name": "home", "id": 0, "params": {} }
            return
        }
        route = history.pop()
    }

    function toast(t) {
        toaster.show(t)
    }

    // `recipe:7` (from --route) as a route
    function parseRoute(text) {
        var parts = String(text || "home").split(":")
        var params = {}
        if (parts.length > 1) {
            var id = parseInt(parts[1])
            if (parts[0] === "preview")
                params.url = parts.slice(1).join(":")
            else if (id > 0)
                params.id = id
            else
                params.title = parts.slice(1).join(":")
        }
        return { "name": pages[parts[0]] ? parts[0] : "home", "id": params.id || 0, "params": params }
    }

    function refreshNav() {
        navRequests.call("connector", {}, function (c) {
            window.connector = c
        }, function () {})
        navRequests.call("checksReview", {}, function (list) {
            window.reviewCount = list ? list.length : 0
        }, function () {
            window.reviewCount = 0
        })
    }

    Requests {
        id: navRequests
    }

    Connections {
        target: Api

        function onUnauthorized() {
            appSession.requireLogin()
        }
    }

    Connections {
        target: appSession

        function onStateChanged() {
            if (appSession.state === "ready") {
                window.history = []
                window.route = window.parseRoute(Smoke.route)
                window.refreshNav()
            }
        }
    }

    Component.onCompleted: {
        if (appSession.state === "checking" && appSession.serverUrl !== "")
            appSession.connect(appSession.serverUrl)
    }

    Loader {
        id: rootLoader
        anchors.fill: parent
        visible: Smoke.page !== "recipe"
        sourceComponent: {
            if (appSession.state === "ready")
                return shell
            if (appSession.state === "checking")
                return checkingPage
            return loginPage
        }
    }

    Component {
        id: loginPage

        LoginPage {
            session: appSession
        }
    }

    Component {
        id: shell

        RowLayout {
            spacing: 0

            NavRail {
                visible: !window.bare[window.route.name]
                Layout.fillHeight: true
                Layout.preferredWidth: 240
                section: window.sections[window.route.name] || ""
                showSuggestions: !!window.connector.weeChefChecks
                reviewCount: window.reviewCount
                onNavigate: (name) => window.go(name, {})
            }

            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                Loader {
                    id: pageLoader
                    anchors.fill: parent
                    focus: true

                    function load() {
                        setSource(window.pages[window.route.name], {
                            "session": appSession,
                            "routeId": window.route.id,
                            "params": window.route.params
                        })
                    }

                    Component.onCompleted: load()

                    Connections {
                        target: window

                        function onRouteChanged() {
                            pageLoader.load()
                        }
                    }
                }

                TimerDock {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    anchors.margins: 24
                }
            }
        }
    }

    // `--smoke-page recipe` renders RecipePage with a built-in fixture, no network.
    Loader {
        anchors.fill: parent
        visible: Smoke.page === "recipe"
        sourceComponent: Smoke.page === "recipe" ? smokeRecipePage : null
    }

    Component {
        id: smokeRecipePage

        RecipePage {
            session: appSession
            routeId: 1
        }
    }

    Component {
        id: checkingPage

        Item {
            ColumnLayout {
                anchors.centerIn: parent
                spacing: 12

                BusyIndicator {
                    Layout.alignment: Qt.AlignHCenter
                    running: true
                }

                Body {
                    text: "Connecting…"
                    Layout.alignment: Qt.AlignHCenter
                }
            }
        }
    }

    Toaster {
        id: toaster
        anchors.fill: parent
        z: 100
    }

    Shortcut {
        sequence: "Escape"
        onActivated: window.back()
    }

    Shortcut {
        sequence: "Alt+Left"
        onActivated: window.back()
    }

    Shortcut {
        sequence: "Ctrl+F"
        onActivated: if (appSession.state === "ready") window.go("recipes", { "focusSearch": true })
    }

    Shortcut {
        sequence: "Ctrl+N"
        onActivated: if (appSession.state === "ready") window.go("add", {})
    }

    TapHandler {
        acceptedButtons: Qt.BackButton
        onTapped: window.back()
    }

    // ─── `--shot <file.png>` ───
    // Once the page's calls have been quiet for a moment, save the window and quit.
    property int quietTicks: 0

    Timer {
        interval: 250
        repeat: true
        running: Smoke.shot !== "" && (appSession.state === "ready" || appSession.state === "login"
                                         || appSession.state === "setup")
        onTriggered: {
            window.quietTicks = Api.pending === 0 ? window.quietTicks + 1 : 0
            // About two seconds of quiet, for photos to arrive
            if (window.quietTicks < 8)
                return
            running = false
            rootLoader.grabToImage(function (result) {
                if (!result.saveToFile(Smoke.shot))
                    console.warn("couldn't save the screenshot to", Smoke.shot)
                Qt.quit()
            })
        }
    }

    // The `--smoke` login-page regression check (state "setup", no server configured).
    Timer {
        interval: 50
        running: Smoke.enabled && Smoke.page === "" && appSession.state === "setup"
        onTriggered: {
            var login = rootLoader.item
            if (!login || typeof login.smokeCheck !== "function") {
                Smoke.fail("the login page did not load")
                return
            }
            login.smokeCheck()
        }
    }
}
