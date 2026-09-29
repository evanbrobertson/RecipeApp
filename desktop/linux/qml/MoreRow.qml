import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// A row of the web's `.settings-row`: a small muted icon, a bold title, a muted line and
// either a chevron or a trailing muted icon. As on pages/more.astro, pages/connect.astro and
// pages/more/connections.astro. `first: false` draws the hairline above it; a row links
// somewhere only when `interactive`.
Item {
    id: row

    property string iconName
    property string title
    property string text
    property bool chevron: false
    property string rightIcon: ""
    property bool interactive: false
    property bool first: true
    signal clicked

    implicitHeight: 56

    Rectangle {
        anchors.fill: parent
        color: Qt.rgba(Palette.tint.r, Palette.tint.g, Palette.tint.b, 0.55)
        visible: row.interactive && hover.hovered
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
                text: row.title
                color: Palette.text
                font.family: Palette.fontSans
                font.pixelSize: 16
                font.weight: Font.Bold
                elide: Text.ElideRight
            }

            Body {
                visible: row.text !== ""
                Layout.fillWidth: true
                text: row.text
                muted: true
                font.pixelSize: 14
            }
        }

        Icon {
            visible: row.chevron
            name: "chevron-right"
            size: 20
            color: Palette.textMuted
            Layout.alignment: Qt.AlignVCenter
        }

        Icon {
            visible: row.rightIcon !== ""
            name: row.rightIcon
            size: 20
            color: Palette.textMuted
            Layout.alignment: Qt.AlignVCenter
        }
    }

    HoverHandler {
        id: hover
        enabled: row.interactive
        cursorShape: Qt.PointingHandCursor
    }

    TapHandler {
        enabled: row.interactive
        onTapped: row.clicked()
    }

    Accessible.role: row.interactive ? Accessible.Button : Accessible.ListItem
    Accessible.name: row.title
}
