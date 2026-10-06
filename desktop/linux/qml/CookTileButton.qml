import QtQuick
import QtQuick.Controls

import app.crumb.desktop 1.0

// A round icon button sitting on cook mode's tile header (web CookPage's `on-tile-btn`):
// paper-coloured icon, a darker wash when hovered or pressed.
Button {
    id: button

    property string iconName
    property string tip
    // A toggle that's on: the darker wash stays
    property bool active: false

    implicitWidth: 44
    implicitHeight: 44
    hoverEnabled: true
    Accessible.name: tip
    Accessible.checked: active

    HoverHandler {
        cursorShape: Qt.PointingHandCursor
    }

    background: Rectangle {
        radius: 12
        color: "black"
        opacity: button.down || button.active ? 0.12 : button.hovered ? 0.08 : 0
        border.width: button.visualFocus ? 2 : 0
        border.color: Palette.onTile
    }

    contentItem: Item {
        Icon {
            anchors.centerIn: parent
            name: button.iconName
            size: 22
            color: Palette.onTile
        }
    }
}
