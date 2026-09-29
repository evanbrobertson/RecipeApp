import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The web's ScaleControl.svelte: a ½× 1× 2× 3× radio group on a tint track. `value` is the
// factor (1 = as written); `edited(value)` fires when a different one is picked.
Rectangle {
    id: control

    property real value: 1
    signal edited(real value)

    implicitWidth: row.implicitWidth + 8
    implicitHeight: 52
    radius: 16
    color: Palette.tint
    Accessible.role: Accessible.RadioButton
    Accessible.name: "Scale recipe"

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 2

        Repeater {
            model: [0.5, 1, 2, 3]

            delegate: Rectangle {
                id: option
                required property real modelData
                readonly property bool on: control.value === modelData

                width: Math.max(44, label.implicitWidth + 20)
                height: 44
                radius: 12
                color: on ? Palette.tile : "transparent"
                Accessible.role: Accessible.RadioButton
                Accessible.checked: on
                Accessible.name: label.text
                Accessible.onPressAction: tap.tapped(null, Qt.LeftButton)

                Text {
                    id: label
                    anchors.centerIn: parent
                    text: option.modelData === 0.5 ? "½×" : option.modelData + "×"
                    color: option.on ? Palette.onTile : (hover.hovered ? Palette.text : Palette.textMuted)
                    font.family: Palette.fontSans
                    font.pixelSize: 15
                    font.weight: Font.Bold
                }

                HoverHandler {
                    id: hover
                    cursorShape: Qt.PointingHandCursor
                }

                TapHandler {
                    id: tap
                    onTapped: if (!option.on) control.edited(option.modelData)
                }
            }
        }
    }
}
