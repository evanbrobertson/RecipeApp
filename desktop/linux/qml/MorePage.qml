import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The More page (web pages/more.astro and islands/MoreSettings.svelte): the ways into your
// recipes, the account, and live shared links. The theme picker is left out (the desktop
// follows the Omarchy theme), as are the web app, extension and print.
Item {
    id: page

    property var session
    property int routeId: 0
    property var params: ({})

    property var connector: ({})
    property var shares: []
    readonly property int offset: -new Date().getTimezoneOffset()

    // `/api/auth/status`, which the desktop keeps on the session
    readonly property var status: {
        try {
            return JSON.parse(page.session && page.session.statusJson ? page.session.statusJson : "{}")
        } catch (e) {
            return {}
        }
    }
    readonly property bool signedIn: !!status.user && status.mode !== "password"
    readonly property string accountTitle: signedIn && status.user.name ? status.user.name : "Account"
    readonly property string accountText: signedIn
        ? ((status.household && status.household.name ? status.household.name : "Household") + ", sign-in and connected apps")
        : (page.connector.authEnabled ? "Connected apps and signing out" : "Connected apps")

    function go(name) {
        ApplicationWindow.window.go(name, {})
    }

    // Surprise me: one recipe at random, skipping what this session served and what was opened lately
    function surprise() {
        var seen = JSON.parse(Store.readSession("crumb:random:seen", "[]"))
        var exclude = seen.slice()
        JSON.parse(Store.read("crumb:recent", "[]")).forEach(function (v) {
            if (exclude.indexOf(v.id) < 0)
                exclude.push(v.id)
        })
        requests.call("random", { "exclude": exclude }, function (recipe) {
            var next = [recipe.id].concat(seen.filter(function (s) { return s !== recipe.id })).slice(0, 40)
            Store.writeSession("crumb:random:seen", JSON.stringify(next))
            ApplicationWindow.window.go("recipe", { "id": recipe.id })
        })
    }

    Requests {
        id: requests
        onFailed: (error) => ApplicationWindow.window.toast({ "title": "Couldn't do that", "description": error, "tone": "error" })
    }

    FolderDialog {
        id: folder
        title: "Save the backup to"
        onAccepted: requests.call("exportAll", { "dir": String(selectedFolder) }, function (saved) {
            ApplicationWindow.window.toast({ "title": "Saved", "description": saved.path })
        }, function (error) {
            ApplicationWindow.window.toast({ "title": "Couldn't save the file", "description": error, "tone": "error" })
        })
    }

    Component.onCompleted: {
        requests.call("connector", {}, function (info) {
            page.connector = info || {}
        }, function () {})
        requests.call("shares", {}, function (list) {
            page.shares = list || []
        }, function () {})
    }

    ScrollPage {
        anchors.fill: parent
        maxWidth: 600

        ColumnLayout {
            width: parent.width
            spacing: 32

            Heading {
                level: 1
                text: "More"
                font.pixelSize: 30
                Layout.fillWidth: true
            }

            // Your recipes
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 8

                MoreHeading {
                    text: "Your recipes"
                    Layout.fillWidth: true
                }

                Card {
                    implicitHeight: recipesColumn.implicitHeight
                    clip: true
                    Layout.fillWidth: true

                    Column {
                        id: recipesColumn
                        width: parent.width

                        MoreRow {
                            width: recipesColumn.width
                            iconName: "shuffle"
                            title: "Surprise me"
                            chevron: true
                            interactive: true
                            onClicked: page.surprise()
                        }

                        MoreRow {
                            width: recipesColumn.width
                            first: false
                            iconName: "pen-line"
                            title: "Write a recipe"
                            chevron: true
                            interactive: true
                            onClicked: page.go("new")
                        }

                        MoreRow {
                            width: recipesColumn.width
                            first: false
                            iconName: "file-down"
                            title: "Import recipes"
                            chevron: true
                            text: "Links, files, Paprika, Mealie"
                            interactive: true
                            onClicked: page.go("import")
                        }
                    }
                }
            }

            // Account
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 8

                MoreHeading {
                    text: "Account"
                    Layout.fillWidth: true
                }

                Card {
                    implicitHeight: accountColumn.implicitHeight
                    clip: true
                    Layout.fillWidth: true

                    Column {
                        id: accountColumn
                        width: parent.width

                        MoreRow {
                            width: accountColumn.width
                            iconName: "user-round"
                            title: page.accountTitle
                            chevron: true
                            text: page.accountText
                            interactive: true
                            onClicked: page.go("account")
                        }

                        MoreRow {
                            width: accountColumn.width
                            first: false
                            iconName: "plug"
                            title: "Connections"
                            chevron: true
                            text: "Claude"
                            interactive: true
                            onClicked: page.go("connections")
                        }

                        MoreRow {
                            width: accountColumn.width
                            first: false
                            iconName: "archive"
                            title: "Download a backup"
                            text: "Everything as one JSON file"
                            rightIcon: "download"
                            interactive: true
                            onClicked: folder.open()
                        }
                    }
                }
            }

            // Theme: the desktop follows the Omarchy theme, so the web's picker is replaced
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 8

                MoreHeading {
                    text: "Theme"
                    Layout.fillWidth: true
                }

                Card {
                    implicitHeight: themeLine.implicitHeight + 32
                    clip: true
                    Layout.fillWidth: true

                    Body {
                        id: themeLine
                        anchors.fill: parent
                        anchors.margins: 16
                        text: "Crumb follows your Omarchy theme."
                    }
                }
            }

            // Shared links
            ColumnLayout {
                visible: page.shares.length > 0
                Layout.fillWidth: true
                spacing: 8

                MoreHeading {
                    text: "Shared links"
                    Layout.fillWidth: true
                }

                Card {
                    implicitHeight: sharesColumn.implicitHeight
                    clip: true
                    Layout.fillWidth: true

                    Column {
                        id: sharesColumn
                        width: parent.width

                        Repeater {
                            model: page.shares

                            delegate: MoreShareRow {
                                required property var modelData
                                required property int index

                                width: sharesColumn.width
                                share: modelData
                                first: index === 0
                                offset: page.offset

                                onStopped: page.shares = page.shares.filter(function (s) {
                                    return s.url !== modelData.url
                                })
                            }
                        }
                    }
                }
            }
        }
    }
}
