import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The Connect page (web pages/connect.astro and islands/ConnectorUrl.svelte): the MCP URL to
// paste into Claude, the steps for the app and Claude Code, and things to try asking.
Item {
    id: page

    property var session
    property int routeId: 0
    property var params: ({})

    property var connector: ({})
    property bool loaded: false
    property bool copied: false
    readonly property string mcpUrl: loaded && connector.mcpUrl ? connector.mcpUrl : ""
    readonly property string codeUrl: mcpUrl !== "" ? mcpUrl : "https://<your-app>/mcp"

    function copy() {
        clip.text = page.mcpUrl
        clip.selectAll()
        clip.copy()
        page.copied = true
        copiedTimer.restart()
        ApplicationWindow.window.toast({ "title": "Copied" })
    }

    Requests {
        id: requests
        onFailed: (error) => ApplicationWindow.window.toast({ "title": "Couldn't do that", "description": error, "tone": "error" })
    }

    Component.onCompleted: {
        requests.call("connector", {}, function (info) {
            page.connector = info || {}
            page.loaded = true
        }, function () {
            page.loaded = true
        })
    }

    Timer {
        id: copiedTimer
        interval: 2000
        onTriggered: page.copied = false
    }

    TextEdit {
        id: clip
        visible: false
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

                Heading {
                    level: 1
                    text: "Connect to Claude"
                    font.pixelSize: 30
                    Layout.fillWidth: true
                }

                Body {
                    text: "Save, search and tweak your recipes from any Claude chat."
                    muted: true
                    Layout.topMargin: 8
                    Layout.fillWidth: true
                }
            }

            // Connector URL
            Card {
                implicitHeight: urlColumn.implicitHeight + 32
                Layout.fillWidth: true

                ColumnLayout {
                    id: urlColumn
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 8

                    Text {
                        text: "Connector URL"
                        color: Palette.text
                        font.family: Palette.fontSans
                        font.pixelSize: 15
                        font.weight: Font.Bold
                        Layout.fillWidth: true
                    }

                    RowLayout {
                        spacing: 8
                        Layout.fillWidth: true

                        StyledField {
                            readOnly: true
                            text: page.mcpUrl
                            font.pixelSize: 14
                            font.family: "monospace"
                            Accessible.name: "Connector URL"
                            Layout.fillWidth: true
                            onActiveFocusChanged: if (activeFocus) selectAll()
                        }

                        CrumbButton {
                            kind: "primary"
                            iconName: page.copied ? "check" : "copy"
                            text: page.copied ? "Copied" : "Copy"
                            onClicked: page.copy()
                        }
                    }

                    RowLayout {
                        visible: page.loaded && !page.connector.authEnabled
                        spacing: 8
                        Layout.fillWidth: true
                        Layout.topMargin: 4

                        Icon {
                            name: "shield-alert"
                            size: 16
                            color: Palette.error
                            Layout.alignment: Qt.AlignTop
                        }

                        Body {
                            textFormat: Text.RichText
                            text: "<b>No password set.</b> Anyone with this URL can change your recipes. Set APP_PASSWORD before you deploy."
                            muted: true
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }
                    }
                }
            }

            // Claude app
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 8

                MoreHeading {
                    text: "Claude app"
                    Layout.fillWidth: true
                }

                Card {
                    implicitHeight: stepsColumn.implicitHeight
                    clip: true
                    Layout.fillWidth: true

                    Column {
                        id: stepsColumn
                        width: parent.width

                        Repeater {
                            model: [
                                "In Claude, open <b>Settings → Connectors</b>.",
                                "Choose <b>Add custom connector</b>, name it “Crumb” and paste the URL.",
                                "Click <b>Connect</b> and approve it with your app password.",
                                "In a chat, switch the connector on in the tools menu."
                            ]

                            delegate: Item {
                                id: step

                                required property string modelData
                                required property int index

                                width: stepsColumn.width
                                height: Math.max(56, stepText.implicitHeight + 20)

                                Rectangle {
                                    visible: step.index > 0
                                    x: 50
                                    width: parent.width - 50
                                    height: 1
                                    color: Palette.line
                                }

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 16
                                    anchors.rightMargin: 16
                                    anchors.topMargin: 10
                                    anchors.bottomMargin: 10
                                    spacing: 14

                                    Text {
                                        text: step.index + 1
                                        color: Palette.textMuted
                                        font.family: Palette.fontSans
                                        font.pixelSize: 15
                                        font.weight: Font.Bold
                                        horizontalAlignment: Text.AlignHCenter
                                        Layout.preferredWidth: 20
                                        Layout.alignment: Qt.AlignVCenter
                                    }

                                    Body {
                                        id: stepText
                                        textFormat: Text.RichText
                                        text: step.modelData
                                        Layout.fillWidth: true
                                        Layout.alignment: Qt.AlignVCenter
                                    }
                                }
                            }
                        }
                    }
                }

                Body {
                    text: "Works in the mobile apps too. On Team and Enterprise plans an owner may need to add it first."
                    muted: true
                    font.pixelSize: 13
                    leftPadding: 16
                    rightPadding: 16
                    Layout.fillWidth: true
                }
            }

            // Claude Code
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 8

                MoreHeading {
                    text: "Claude Code"
                    Layout.fillWidth: true
                }

                Rectangle {
                    implicitHeight: codeText.implicitHeight + 32
                    color: Palette.tint
                    radius: 12
                    Layout.fillWidth: true

                    Text {
                        id: codeText
                        x: 16
                        y: 16
                        width: parent.width - 32
                        text: "claude mcp add --transport http recipes " + page.codeUrl
                        color: Palette.text
                        font.family: "monospace"
                        font.pixelSize: 14
                        wrapMode: Text.WrapAnywhere
                    }
                }

                Body {
                    textFormat: Text.RichText
                    text: "Then run <b>/mcp</b> to sign in."
                    muted: true
                    font.pixelSize: 13
                    leftPadding: 16
                    rightPadding: 16
                    Layout.fillWidth: true
                }
            }

            // Try asking
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 8

                MoreHeading {
                    text: "Try asking"
                    Layout.fillWidth: true
                }

                Card {
                    implicitHeight: examplesColumn.implicitHeight
                    clip: true
                    Layout.fillWidth: true

                    Column {
                        id: examplesColumn
                        width: parent.width

                        Repeater {
                            model: [
                                "Save this recipe to my recipe box: …",
                                "What can I make with chicken thighs and lemons?",
                                "Double my banana bread and save it."
                            ]

                            delegate: Item {
                                required property string modelData
                                required property int index

                                width: examplesColumn.width
                                height: 56

                                Rectangle {
                                    visible: index > 0
                                    x: 16
                                    width: parent.width - 32
                                    height: 1
                                    color: Palette.line
                                }

                                Body {
                                    anchors.fill: parent
                                    anchors.leftMargin: 16
                                    anchors.rightMargin: 16
                                    text: "“" + modelData + "”"
                                    verticalAlignment: Text.AlignVCenter
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
