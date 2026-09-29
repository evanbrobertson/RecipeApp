import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// One connected app in the Connections page (islands/AccountPage.svelte's "Connected apps"):
// its name and when it was approved, with a Disconnect button. `first: false` draws the
// hairline above it.
Item {
    id: row

    property var app: ({})
    property bool first: true
    // The viewer's UTC offset, for Core.dateLabel (the web's local date)
    property int offset: -new Date().getTimezoneOffset()
    signal disconnectRequested()

    readonly property string meta: {
        var parts = []
        if (row.app.household && row.app.household.name)
            parts.push(row.app.household.name)
        parts.push("connected " + Core.dateLabel(row.app.connectedAt * 1000, row.offset))
        return parts.join(" · ")
    }

    implicitHeight: 56

    Rectangle {
        anchors.fill: parent
        color: Qt.rgba(Palette.tint.r, Palette.tint.g, Palette.tint.b, 0.55)
        visible: hover.hovered
    }

    Rectangle {
        visible: !row.first
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

        Icon {
            name: "plug"
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
                text: row.app.name
                color: Palette.text
                font.family: Palette.fontSans
                font.pixelSize: 16
                font.weight: Font.Bold
                elide: Text.ElideRight
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
            kind: "ghost"
            iconName: "unlink"
            Accessible.name: "Disconnect " + row.app.name
            Layout.alignment: Qt.AlignVCenter
            onClicked: row.disconnectRequested()
        }
    }

    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
    }
}
