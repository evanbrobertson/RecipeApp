import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// A section's title with its "See all" link at the right, as on the web's Home.
RowLayout {
    id: header

    property string title
    property string linkText: "See all"
    signal seeAll()

    Layout.fillWidth: true
    spacing: 12

    Heading {
        level: 2
        text: header.title
        Layout.fillWidth: true
    }

    Text {
        text: header.linkText
        color: Palette.primary
        font.family: Palette.fontSans
        font.pixelSize: 15
        font.weight: Font.Bold
        font.underline: linkHover.hovered
        Layout.alignment: Qt.AlignBaseline
        Accessible.role: Accessible.Link
        activeFocusOnTab: true
        Keys.onReturnPressed: header.seeAll()

        HoverHandler {
            id: linkHover
            cursorShape: Qt.PointingHandCursor
        }

        TapHandler {
            onTapped: header.seeAll()
        }
    }
}
