import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts

import app.crumb.desktop 1.0

// Share, send and download one cookbook, as the web's ShareSheet.svelte does for a book: its
// link (made on request, notes on or off, stopped for good with "Stop sharing"), plain text,
// email, and the file for another Crumb. A cookbook's link is live: recipes added later show up.
Modal {
    id: sheet

    // {id, name, recipes: [{title}]}
    property var cookbook: ({})
    // {url, includeNotes}, or null until a link is made
    property var share: null
    property bool busy: false
    property bool confirmStop: false
    signal linkChanged(var share)

    function copy(text, done) {
        clip.text = text
        clip.selectAll()
        clip.copy()
        ApplicationWindow.window.toast({ "title": done })
    }

    function toastError(title, error) {
        ApplicationWindow.window.toast({ "title": title, "description": error, "tone": "error" })
    }

    function create() {
        busy = true
        requests.call("createShare", { "kind": "cookbook", "id": cookbook.id }, function (link) {
            busy = false
            sheet.share = link
            sheet.linkChanged(link)
        }, function (error) {
            busy = false
            sheet.toastError("Couldn't make a link", error)
        })
    }

    function setNotes(include) {
        if (!share)
            return
        var before = share
        sheet.share = { "url": share.url, "includeNotes": include }
        requests.call("updateShare", { "kind": "cookbook", "id": cookbook.id, "includeNotes": include }, function (link) {
            sheet.share = link
            sheet.linkChanged(link)
        }, function (error) {
            sheet.share = before
            sheet.toastError("Couldn't change that", error)
        })
    }

    function stop() {
        busy = true
        requests.call("stopShare", { "kind": "cookbook", "id": cookbook.id }, function () {
            busy = false
            sheet.share = null
            sheet.confirmStop = false
            sheet.linkChanged(null)
            ApplicationWindow.window.toast({ "title": "Stopped sharing", "description": "The link no longer works." })
        }, function (error) {
            busy = false
            sheet.toastError("Couldn't stop sharing", error)
        })
    }

    function copyText() {
        var titles = (cookbook.recipes || []).map(function (r) { return r.title })
        copy(Core.bookText(cookbook.name, JSON.stringify(titles), share ? share.url : ""), "Cookbook copied")
    }

    title: "Share"
    onClosed: confirmStop = false

    Requests {
        id: requests
    }

    TextEdit {
        id: clip
        visible: false
    }

    FolderDialog {
        id: folder
        title: "Save the cookbook to"
        onAccepted: requests.call("exportCookbook", { "id": sheet.cookbook.id, "dir": String(selectedFolder) }, function (saved) {
            ApplicationWindow.window.toast({ "title": "Saved", "description": saved.path })
        }, function (error) {
            sheet.toastError("Couldn't save the file", error)
        })
    }

    // Share a link
    ColumnLayout {
        spacing: 10
        Layout.fillWidth: true

        SheetTitle {
            text: "Share a link"
        }

        ColumnLayout {
            visible: !sheet.share
            spacing: 4
            Layout.fillWidth: true

            Body {
                text: "Anyone with the link sees this book and every recipe in it, including ones you add later."
                muted: true
                Layout.fillWidth: true
            }

            Body {
                text: "Links to single recipes in this book open the whole book."
                muted: true
                font.pixelSize: 13
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            CrumbButton {
                kind: "soft"
                text: "Create link"
                iconName: "link"
                enabled: !sheet.busy
                Layout.topMargin: 6
                onClicked: sheet.create()
            }
        }

        ColumnLayout {
            visible: !!sheet.share
            spacing: 10
            Layout.fillWidth: true

            StyledField {
                readOnly: true
                text: sheet.share ? sheet.share.url : ""
                font.pixelSize: 15
                Accessible.name: "Share link"
                Layout.fillWidth: true
                onActiveFocusChanged: if (activeFocus) selectAll()
            }

            Body {
                text: "Links to single recipes in this book open the whole book."
                muted: true
                font.pixelSize: 13
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            CrumbButton {
                kind: "soft"
                text: "Copy link"
                iconName: "copy"
                onClicked: sheet.copy(sheet.share.url, "Link copied")
            }

            // Include my notes
            Item {
                id: notes

                Layout.fillWidth: true
                Layout.preferredHeight: Math.max(44, notesText.implicitHeight)
                activeFocusOnTab: true
                Accessible.role: Accessible.CheckBox
                Accessible.name: "Include my notes"
                Accessible.checked: sheet.share ? sheet.share.includeNotes : false
                Keys.onReturnPressed: sheet.setNotes(!sheet.share.includeNotes)
                Keys.onSpacePressed: sheet.setNotes(!sheet.share.includeNotes)

                TapHandler {
                    onTapped: sheet.setNotes(!sheet.share.includeNotes)
                }

                HoverHandler {
                    cursorShape: Qt.PointingHandCursor
                }

                RowLayout {
                    anchors.fill: parent
                    spacing: 12

                    ColumnLayout {
                        id: notesText
                        spacing: 0
                        Layout.fillWidth: true

                        Body {
                            text: "Include my notes"
                            bold: true
                            Layout.fillWidth: true
                        }

                        Body {
                            text: "Shown on each recipe's page"
                            muted: true
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }
                    }

                    Rectangle {
                        readonly property bool on: sheet.share ? sheet.share.includeNotes : false

                        Layout.preferredWidth: 44
                        Layout.preferredHeight: 26
                        radius: 13
                        color: on ? Palette.tile : Palette.line

                        Rectangle {
                            x: parent.on ? 21 : 3
                            y: 3
                            width: 20
                            height: 20
                            radius: 10
                            color: Palette.paper

                            Behavior on x {
                                NumberAnimation { duration: 150 }
                            }
                        }
                    }
                }
            }

            Rectangle {
                visible: sheet.confirmStop
                radius: 12
                color: Palette.tint
                Layout.fillWidth: true
                Layout.preferredHeight: stopColumn.implicitHeight + 32

                ColumnLayout {
                    id: stopColumn
                    x: 16
                    y: 16
                    width: parent.width - 32
                    spacing: 4

                    Body {
                        text: "Stop sharing?"
                        bold: true
                        Layout.fillWidth: true
                    }

                    Body {
                        text: "The link stops working for everyone. A new link can be made later."
                        muted: true
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }

                    RowLayout {
                        spacing: 8
                        Layout.alignment: Qt.AlignRight
                        Layout.topMargin: 8

                        CrumbButton {
                            kind: "ghost"
                            text: "Keep sharing"
                            onClicked: sheet.confirmStop = false
                        }

                        CrumbButton {
                            kind: "danger"
                            text: "Stop sharing"
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
                font.underline: stopHover.hovered
                Layout.preferredHeight: 44
                verticalAlignment: Text.AlignVCenter
                Accessible.role: Accessible.Button
                activeFocusOnTab: true
                Keys.onReturnPressed: sheet.confirmStop = true

                HoverHandler {
                    id: stopHover
                    cursorShape: Qt.PointingHandCursor
                }

                TapHandler {
                    onTapped: sheet.confirmStop = true
                }
            }
        }
    }

    // Send
    ColumnLayout {
        spacing: 10
        Layout.fillWidth: true

        SheetTitle {
            text: "Send"
        }

        HomeListCard {
            Layout.fillWidth: true

            HomeRow {
                first: true
                label: "Copy as text"
                onClicked: sheet.copyText()

                Icon {
                    name: "copy"
                    size: 20
                    color: Palette.primary
                }

                Body {
                    text: "Copy as text"
                    bold: true
                    Layout.fillWidth: true
                }
            }

            HomeRow {
                visible: !!sheet.share
                label: "Email"
                onClicked: Qt.openUrl("mailto:?subject=" + encodeURIComponent(sheet.cookbook.name)
                                      + "&body=" + encodeURIComponent(sheet.share.url))

                Icon {
                    name: "mail"
                    size: 20
                    color: Palette.primary
                }

                Body {
                    text: "Email"
                    bold: true
                    Layout.fillWidth: true
                }
            }
        }
    }

    // Download
    ColumnLayout {
        spacing: 10
        Layout.fillWidth: true

        SheetTitle {
            text: "Download"
        }

        HomeListCard {
            Layout.fillWidth: true

            HomeRow {
                first: true
                label: "For another Crumb (.json)"
                onClicked: folder.open()

                Icon {
                    name: "file-braces"
                    size: 20
                    color: Palette.primary
                }

                Body {
                    text: "For another Crumb (.json)"
                    bold: true
                    Layout.fillWidth: true
                }
            }
        }
    }

    // A group's small caps title
    component SheetTitle: Text {
        color: Palette.textMuted
        font.family: Palette.fontSans
        font.pixelSize: 13
        font.weight: Font.Bold
        font.letterSpacing: 0.5
        font.capitalization: Font.AllUppercase
    }
}
