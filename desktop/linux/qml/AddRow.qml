import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// A row of the web's `.list-card` (a 44px icon well, a title, a line, an optional chevron),
// as on pages/add.astro. `first: false` draws the hairline above it.
Item {
    id: row

    property string iconName
    property string title
    property string text
    property bool chevron: false
    property bool first: true
    signal clicked

    implicitHeight: 68

    Rectangle {
        anchors.fill: parent
        color: Qt.rgba(Palette.tint.r, Palette.tint.g, Palette.tint.b, 0.55)
        visible: row.chevron && hover.hovered
    }

    Rectangle {
        visible: !row.first
        x: 12
        width: parent.width - 24
        height: 1
        color: Palette.line
    }

    RowLayout {
        anchors.fill: parent
        anchors.margins: 12
        spacing: 14

        Rectangle {
            Layout.preferredWidth: 44
            Layout.preferredHeight: 44
            Layout.alignment: Qt.AlignVCenter
            radius: 12
            color: Palette.tint

            Icon {
                anchors.centerIn: parent
                name: row.iconName
                size: 22
                color: Palette.primary
            }
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
                font.pixelSize: 17
                font.weight: Font.Bold
                elide: Text.ElideRight
            }

            Body {
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
    }

    HoverHandler {
        id: hover
        enabled: row.chevron
        cursorShape: Qt.PointingHandCursor
    }

    TapHandler {
        enabled: row.chevron
        onTapped: row.clicked()
    }

    Accessible.role: row.chevron ? Accessible.Button : Accessible.ListItem
    Accessible.name: row.title
}
