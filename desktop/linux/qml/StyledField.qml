import QtQuick
import QtQuick.Controls

import app.crumb.desktop 1.0

// The one text input style: paper fill, hairline border, primary focus ring. Overriding
// `background` keeps the Qt Basic theme's white boxes out of the window.
TextField {
    id: field

    color: Palette.text
    placeholderTextColor: Palette.textMuted
    selectionColor: Palette.tile
    selectedTextColor: Palette.onTile
    font.pixelSize: 14
    leftPadding: 12
    rightPadding: 12
    topPadding: 10
    bottomPadding: 10

    background: Rectangle {
        radius: 12
        color: Palette.paper
        border.width: field.activeFocus ? 2 : 1
        border.color: field.activeFocus ? Palette.primary : Palette.line

        Behavior on color {
            ColorAnimation { duration: 150 }
        }
        Behavior on border.color {
            ColorAnimation { duration: 150 }
        }
    }
}
