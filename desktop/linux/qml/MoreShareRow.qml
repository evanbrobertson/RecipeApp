import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// One live link in More's "Shared links" list (islands/MoreSettings.svelte): its icon, title
// and meta, with Copy and Stop sharing, and the inline "The link stops working for everyone."
// confirm. `first: false` draws the hairline above it.
Item {
    id: row

    property var share: ({})
    property bool first: true
    // The viewer's UTC offset, for Core.dateLabel (the web's local date)
    property int offset: -new Date().getTimezoneOffset()
    property bool confirming: false
    property bool stopping: false
    signal stopped()

    readonly property bool cookbook: share.kind === "cookbook"
    readonly property string iconName: cookbook ? "book-open" : "cooking-pot"
    readonly property string meta: (cookbook ? "Cookbook" : "Recipe") + " shared "
        + dateOf(share.createdAt) + " · "
        + (share.lastOpenedAt ? "last opened " + dateOf(share.lastOpenedAt) : "not opened yet")

    implicitHeight: confirming ? 56 + 4 + confirmRow.implicitHeight : 56

    function dateOf(iso) {
        return Core.dateLabel(new Date(iso).getTime(), row.offset)
    }

    function copy() {
        clip.text = String(share.url)
        clip.selectAll()
        clip.copy()
        ApplicationWindow.window.toast({ "title": "Link copied" })
    }

    function stop() {
        stopping = true
        requests.call("stopShare", { "kind": share.kind, "id": share.id }, function () {
            stopping = false
            confirming = false
            row.stopped()
            ApplicationWindow.window.toast({ "title": "Stopped sharing", "description": "The link no longer works." })
        }, function (error) {
            stopping = false
            ApplicationWindow.window.toast({ "title": "Couldn't stop sharing", "description": error, "tone": "error" })
        })
    }

    Requests {
        id: requests
    }

    TextEdit {
        id: clip
        visible: false
    }

    Rectangle {
        anchors.fill: parent
        color: Qt.rgba(Palette.tint.r, Palette.tint.g, Palette.tint.b, 0.55)
        visible: titleHover.hovered
    }

    Rectangle {
        visible: !row.first
        x: 50
        width: parent.width - 50
        height: 1
        color: Palette.line
    }

    RowLayout {
        id: top

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.leftMargin: 16
        anchors.rightMargin: 16
        height: 56
        spacing: 14

        Icon {
            name: row.iconName
            size: 20
            color: Palette.textMuted
            Layout.alignment: Qt.AlignVCenter
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
            spacing: 0

            Text {
                Layout.fillWidth: true
                text: row.share.title
                color: titleHover.hovered ? Palette.primary : Palette.text
                font.family: Palette.fontSans
                font.pixelSize: 16
                font.weight: Font.Bold
                font.underline: titleHover.hovered
                elide: Text.ElideRight
                Accessible.role: Accessible.Link
                Accessible.name: row.share.title

                HoverHandler {
                    id: titleHover
                    cursorShape: Qt.PointingHandCursor
                }

                TapHandler {
                    onTapped: ApplicationWindow.window.go(row.cookbook ? "cookbook" : "recipe", { "id": row.share.id })
                }
            }

            Body {
                Layout.fillWidth: true
                text: row.meta
                muted: true
                font.pixelSize: 14
                elide: Text.ElideRight
            }
        }

        CrumbButton {
            visible: !row.confirming
            kind: "ghost"
            iconName: "copy"
            Accessible.name: "Copy the link to " + row.share.title
            Layout.alignment: Qt.AlignVCenter
            onClicked: row.copy()
        }

        CrumbButton {
            visible: !row.confirming
            kind: "ghost"
            iconName: "unlink"
            Accessible.name: "Stop sharing " + row.share.title
            Layout.alignment: Qt.AlignVCenter
            onClicked: row.confirming = true
        }
    }

    RowLayout {
        id: confirmRow
        visible: row.confirming

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: top.bottom
        anchors.leftMargin: 50
        anchors.rightMargin: 16
        anchors.topMargin: 4
        spacing: 8

        Body {
            Layout.fillWidth: true
            text: "The link stops working for everyone."
            muted: true
            font.pixelSize: 14
        }

        CrumbButton {
            kind: "ghost"
            text: "Keep"
            onClicked: row.confirming = false
        }

        CrumbButton {
            kind: "danger"
            text: "Stop sharing"
            enabled: !row.stopping
            onClicked: row.stop()
        }
    }
}
