import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The web's ShareSheet.svelte for one recipe: its share link (made on request, notes on or
// off, stopped with "Stop sharing"), Send (copy as text, email) and Download (files).
// `share` is the link ({url, includeNotes}) or null; `updated(share)` reports each change.
Modal {
    id: sheet

    property var recipe: null
    property var share: null
    property bool busy: false
    property bool confirmStop: false
    property string exportFormat: "json"

    signal updated(var share)

    title: "Share"
    onClosed: confirmStop = false

    readonly property string notesHint: recipe && recipe.notes ? "Shown on the shared page"
                                                               : "This recipe has no notes yet"

    function toast(t) {
        ApplicationWindow.window.toast(t)
    }

    function setShare(value) {
        share = value
        updated(value)
    }

    function create() {
        busy = true
        requests.call("createShare", {"kind": "recipe", "id": recipe.id}, function (link) {
            busy = false
            setShare(link)
        }, function (error) {
            busy = false
            toast({"title": "Couldn't make a link", "description": error, "tone": "error"})
        })
    }

    function copy(text, done) {
        clipboard.copy(text)
        toast({"title": done})
    }

    function setNotes(include) {
        if (!share)
            return
        var before = share
        setShare(Object.assign({}, share, {"includeNotes": include}))
        requests.call("updateShare", {"kind": "recipe", "id": recipe.id, "includeNotes": include}, function (link) {
            setShare(link)
        }, function (error) {
            setShare(before)
            toast({"title": "Couldn't change that", "description": error, "tone": "error"})
        })
    }

    function stop() {
        busy = true
        requests.call("stopShare", {"kind": "recipe", "id": recipe.id}, function () {
            busy = false
            confirmStop = false
            setShare(null)
            toast({"title": "Stopped sharing", "description": "The link no longer works."})
        }, function (error) {
            busy = false
            toast({"title": "Couldn't stop sharing", "description": error, "tone": "error"})
        })
    }

    function copyText() {
        copy(Core.recipeText(JSON.stringify(recipe), share ? share.url : ""), "Recipe copied")
    }

    function copyMarkdown() {
        copy(Core.recipeMarkdown(JSON.stringify(recipe)), "Recipe copied")
    }

    function email() {
        Qt.openUrlExternally("mailto:?subject=" + encodeURIComponent(recipe.title) + "&body="
                             + encodeURIComponent(share.url))
    }

    function download(format) {
        exportFormat = format
        folder.open()
    }

    Requests {
        id: requests
    }

    RecipeClipboard {
        id: clipboard
    }

    FolderDialog {
        id: folder
        title: "Save the recipe to"
        onAccepted: {
            requests.call("exportRecipe", {"id": sheet.recipe.id, "format": sheet.exportFormat, "dir": String(selectedFolder)}, function (res) {
                sheet.toast({"title": "Saved", "description": res.path})
            }, function (error) {
                sheet.toast({"title": "Couldn't save", "description": error, "tone": "error"})
            })
        }
    }

    component GroupTitle: Text {
        color: Palette.textMuted
        font.family: Palette.fontSans
        font.pixelSize: 13
        font.weight: Font.Bold
        font.capitalization: Font.AllUppercase
        font.letterSpacing: 0.5
        Layout.bottomMargin: 2
    }

    // A row of the Send / Download lists
    component SheetRow: Item {
        id: row
        property string icon
        property string label
        property bool first: false
        signal clicked

        Layout.fillWidth: true
        implicitHeight: 56

        Rectangle {
            visible: !row.first
            x: 50
            width: parent.width - 50
            height: 1
            color: Palette.line
        }
        Rectangle {
            anchors.fill: parent
            color: Palette.tint
            opacity: hover.hovered ? 0.55 : 0
        }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 16
            anchors.rightMargin: 16
            spacing: 14

            Icon {
                name: row.icon
                size: 20
                color: Palette.textMuted
            }
            Body {
                text: row.label
                bold: true
                Layout.fillWidth: true
            }
        }
        HoverHandler {
            id: hover
            cursorShape: Qt.PointingHandCursor
        }
        TapHandler {
            onTapped: row.clicked()
        }
        Accessible.role: Accessible.Button
        Accessible.name: row.label
        Accessible.onPressAction: row.clicked()
    }

    component ListCard: Rectangle {
        default property alias rows: rowColumn.data
        Layout.fillWidth: true
        implicitHeight: rowColumn.implicitHeight + 2
        radius: 16
        color: Palette.paper
        border.width: 1
        border.color: Palette.line

        ColumnLayout {
            id: rowColumn
            anchors.fill: parent
            anchors.margins: 1
            spacing: 0
        }
    }

    ColumnLayout {
        Layout.fillWidth: true
        spacing: 24

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 8

            GroupTitle {
                text: "Share a link"
            }

            ColumnLayout {
                visible: !sheet.share
                Layout.fillWidth: true
                spacing: 12

                Body {
                    text: "Anyone with the link can see this recipe, not the rest of your box."
                    muted: true
                    Layout.fillWidth: true
                }
                CrumbButton {
                    text: "Create link"
                    iconName: "link"
                    enabled: !sheet.busy
                    onClicked: sheet.create()
                }
            }

            ColumnLayout {
                visible: !!sheet.share
                Layout.fillWidth: true
                spacing: 12

                StyledField {
                    readOnly: true
                    text: sheet.share ? sheet.share.url : ""
                    Layout.fillWidth: true
                    Accessible.name: "Share link"
                    onActiveFocusChanged: if (activeFocus) selectAll()
                }

                CrumbButton {
                    text: "Copy link"
                    iconName: "copy"
                    onClicked: sheet.copy(sheet.share.url, "Link copied")
                }

                // The "Include my notes" switch
                Item {
                    Layout.fillWidth: true
                    implicitHeight: Math.max(44, notesText.implicitHeight)
                    Accessible.role: Accessible.CheckBox
                    Accessible.name: "Include my notes"
                    Accessible.checked: !!(sheet.share && sheet.share.includeNotes)
                    Accessible.onPressAction: sheet.setNotes(!sheet.share.includeNotes)

                    RowLayout {
                        anchors.fill: parent
                        spacing: 12

                        ColumnLayout {
                            id: notesText
                            Layout.fillWidth: true
                            spacing: 0

                            Body {
                                text: "Include my notes"
                                bold: true
                                Layout.fillWidth: true
                            }
                            Body {
                                text: sheet.notesHint
                                muted: true
                                font.pixelSize: 14
                                Layout.fillWidth: true
                            }
                        }

                        CrumbSwitch {
                            on: !!(sheet.share && sheet.share.includeNotes)
                            Layout.preferredWidth: 44
                            Layout.preferredHeight: 26
                        }
                    }
                    HoverHandler {
                        cursorShape: Qt.PointingHandCursor
                    }
                    TapHandler {
                        onTapped: sheet.setNotes(!sheet.share.includeNotes)
                    }
                }

                Rectangle {
                    visible: sheet.confirmStop
                    Layout.fillWidth: true
                    implicitHeight: confirmColumn.implicitHeight + 32
                    radius: 12
                    color: Palette.tint

                    ColumnLayout {
                        id: confirmColumn
                        anchors.fill: parent
                        anchors.margins: 16
                        spacing: 4

                        Body {
                            text: "Stop sharing?"
                            bold: true
                        }
                        Body {
                            text: "The link stops working for everyone. A new link can be made later."
                            muted: true
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }
                        RowLayout {
                            Layout.alignment: Qt.AlignRight
                            Layout.topMargin: 8
                            spacing: 8

                            CrumbButton {
                                text: "Keep sharing"
                                kind: "ghost"
                                onClicked: sheet.confirmStop = false
                            }
                            CrumbButton {
                                text: "Stop sharing"
                                kind: "danger"
                                enabled: !sheet.busy
                                onClicked: sheet.stop()
                            }
                        }
                    }
                }

                Text {
                    visible: !sheet.confirmStop
                    text: "Stop sharing"
                    color: Palette.error
                    font.family: Palette.fontSans
                    font.pixelSize: 15
                    font.weight: Font.Bold
                    verticalAlignment: Text.AlignVCenter
                    Layout.preferredHeight: 44
                    Accessible.role: Accessible.Button
                    Accessible.name: text

                    HoverHandler {
                        cursorShape: Qt.PointingHandCursor
                    }
                    TapHandler {
                        onTapped: sheet.confirmStop = true
                    }
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 8

            GroupTitle {
                text: "Send"
            }
            ListCard {
                SheetRow {
                    first: true
                    icon: "copy"
                    label: "Copy as text"
                    onClicked: sheet.copyText()
                }
                SheetRow {
                    icon: "file-text"
                    label: "Copy as Markdown"
                    onClicked: sheet.copyMarkdown()
                }
                SheetRow {
                    visible: !!sheet.share
                    icon: "mail"
                    label: "Email"
                    onClicked: sheet.email()
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 8

            GroupTitle {
                text: "Download"
            }
            ListCard {
                SheetRow {
                    first: true
                    icon: "file-braces"
                    label: "For another Crumb (.json)"
                    onClicked: sheet.download("json")
                }
                SheetRow {
                    icon: "file-text"
                    label: "Markdown (.md)"
                    onClicked: sheet.download("markdown")
                }
            }
        }
    }
}
