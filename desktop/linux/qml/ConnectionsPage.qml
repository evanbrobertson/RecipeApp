import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The Connections page (web pages/more/connections.astro): the apps that can reach your
// recipe box, and the ones already connected (the web keeps the latter on /more/account) with
// a Disconnect confirm.
Item {
    id: page

    property var session
    property int routeId: 0
    property var params: ({})

    property var apps: []
    readonly property int offset: -new Date().getTimezoneOffset()

    function go(name) {
        ApplicationWindow.window.go(name, {})
    }

    function loadApps() {
        requests.call("connectedApps", {}, function (list) {
            page.apps = list || []
        }, function () {
            page.apps = []
        })
    }

    function disconnect() {
        if (!confirm.app)
            return
        var app = confirm.app
        confirm.busy = true
        requests.call("disconnectApp", { "id": app.id }, function () {
            confirm.busy = false
            confirm.close()
            page.apps = page.apps.filter(function (a) { return a.id !== app.id })
            ApplicationWindow.window.toast({ "title": app.name + " disconnected", "description": "It has to be approved again." })
        }, function (error) {
            confirm.busy = false
            ApplicationWindow.window.toast({ "title": "Couldn't disconnect it", "description": error, "tone": "error" })
        })
    }

    Requests {
        id: requests
        onFailed: (error) => page.ApplicationWindow.window.toast({ "title": "Couldn't do that", "description": error, "tone": "error" })
    }

    Component.onCompleted: loadApps()

    Modal {
        id: confirm

        property var app: null
        property bool busy: false

        title: "Disconnect " + (confirm.app ? confirm.app.name : "") + "?"
        description: "It stops working until it's approved again."
        footer: [
            CrumbButton {
                kind: "ghost"
                text: "Keep"
                onClicked: confirm.close()
            },
            CrumbButton {
                kind: "danger"
                text: "Disconnect"
                enabled: !confirm.busy
                onClicked: page.disconnect()
            }
        ]
    }

    ScrollPage {
        anchors.fill: parent
        maxWidth: 600

        ColumnLayout {
            width: parent.width
            spacing: 32

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0

                CrumbButton {
                    kind: "ghost"
                    iconName: "arrow-left"
                    text: "More"
                    Layout.leftMargin: -12
                    Layout.bottomMargin: 12
                    onClicked: page.go("more")
                }

                Heading {
                    level: 1
                    text: "Connections"
                    font.pixelSize: 30
                    Layout.fillWidth: true
                }

                Body {
                    text: "Use your recipe box from other apps."
                    muted: true
                    Layout.topMargin: 8
                    Layout.fillWidth: true
                }
            }

            // Apps
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 8

                MoreHeading {
                    text: "Apps"
                    Layout.fillWidth: true
                }

                Card {
                    implicitHeight: appsColumn.implicitHeight
                    clip: true
                    Layout.fillWidth: true

                    Column {
                        id: appsColumn
                        width: parent.width

                        MoreRow {
                            width: appsColumn.width
                            iconName: "messages-square"
                            title: "Claude"
                            text: "Save, search and tweak recipes from a chat"
                            chevron: true
                            interactive: true
                            onClicked: page.go("connect")
                        }
                    }
                }
            }

            // Connected apps: only while any are approved
            ColumnLayout {
                visible: page.apps.length > 0
                Layout.fillWidth: true
                spacing: 8

                MoreHeading {
                    text: "Connected apps"
                    Layout.fillWidth: true
                }

                Card {
                    implicitHeight: connectedColumn.implicitHeight
                    clip: true
                    Layout.fillWidth: true

                    Column {
                        id: connectedColumn
                        width: parent.width

                        Repeater {
                            model: page.apps

                            delegate: MoreAppRow {
                                required property var modelData
                                required property int index

                                width: connectedColumn.width
                                app: modelData
                                first: index === 0
                                offset: page.offset
                                onDisconnectRequested: {
                                    confirm.app = modelData
                                    confirm.open()
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
