import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

ApplicationWindow {
    id: window
    visible: true
    width: 1100
    height: 760
    minimumWidth: 480
    minimumHeight: 400
    title: "Crumb"
    color: Palette.bg

    Behavior on color {
        ColorAnimation { duration: 150 }
    }

    Session {
        id: appSession
    }

    // ─── Navigation ───
    // The signed-in half is a StackView: RecipesPage is the root, a card pushes RecipePage.
    // Keeping RecipesPage alive preserves its scroll position and search text.
    function openRecipe(id) {
        var stack = rootLoader.item
        if (stack && typeof stack.push === "function")
            stack.push(recipePageComponent, {
                "recipeId": id
            })
    }

    function goBack() {
        var stack = rootLoader.item
        if (stack && stack.depth !== undefined && stack.depth > 1)
            stack.pop()
    }

    function focusSearch() {
        var stack = rootLoader.item
        if (stack && stack.currentItem && typeof stack.currentItem.focusSearch === "function")
            stack.currentItem.focusSearch()
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
                return readyShell
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
        id: readyShell
        StackView {
            anchors.fill: parent
            initialItem: recipesPageComponent
        }
    }

    Component {
        id: recipesPageComponent
        RecipesPage {
            session: appSession
            onOpenRecipe: (id) => window.openRecipe(id)
        }
    }

    Component {
        id: recipePageComponent
        RecipePage {
            session: appSession
            onGoBack: window.goBack()
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
            recipeId: 1
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

                Label {
                    text: "Connecting…"
                    color: Palette.text
                    font.pixelSize: 15
                    Layout.alignment: Qt.AlignHCenter
                }
            }
        }
    }

    Shortcut {
        sequence: "Escape"
        onActivated: window.goBack()
    }

    Shortcut {
        sequence: "Alt+Left"
        onActivated: window.goBack()
    }

    Shortcut {
        sequence: "Ctrl+F"
        onActivated: window.focusSearch()
    }

    TapHandler {
        acceptedButtons: Qt.BackButton
        onTapped: window.goBack()
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
