import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The cover colour choice, as the web's BookColorPicker.svelte: six little books standing on
// their feet. `value` is a stored colour name (a legacy name picks its nearest cover).
Flow {
    id: picker

    property string value: "tile"
    readonly property string current: JSON.parse(Core.bookLook(value)).name

    spacing: 4
    Accessible.role: Accessible.RadioButton
    Accessible.name: "Cover colour"

    Repeater {
        model: JSON.parse(Core.bookColors())

        delegate: Item {
            id: swatch

            required property string modelData
            readonly property var look: JSON.parse(Core.bookLook(modelData))
            readonly property bool checked: picker.current === modelData

            width: 48
            height: 56
            activeFocusOnTab: true
            Accessible.role: Accessible.RadioButton
            Accessible.name: modelData.charAt(0).toUpperCase() + modelData.slice(1)
            Accessible.checked: checked
            Keys.onReturnPressed: picker.value = modelData
            Keys.onSpacePressed: picker.value = modelData

            HoverHandler {
                id: hover
                cursorShape: Qt.PointingHandCursor
            }

            TapHandler {
                onTapped: picker.value = swatch.modelData
            }

            Rectangle {
                anchors.fill: parent
                radius: 12
                color: Palette.tint
                opacity: hover.hovered ? 1 : 0
            }

            Rectangle {
                visible: swatch.activeFocus
                anchors.fill: parent
                radius: 12
                color: "transparent"
                border.width: 2
                border.color: Palette.primary
            }

            // A little book standing on its foot
            Rectangle {
                x: 11
                y: 10 - (hover.hovered || swatch.checked ? 3 : 0)
                width: 26
                height: 40
                topLeftRadius: 3
                topRightRadius: 3
                color: swatch.look.cloth

                Behavior on y {
                    NumberAnimation { duration: 180; easing.type: Easing.OutCubic }
                }

                Rectangle {
                    anchors.fill: parent
                    topLeftRadius: 3
                    topRightRadius: 3
                    color: "transparent"
                    border.width: 1
                    border.color: Qt.rgba(Palette.text.r, Palette.text.g, Palette.text.b, 0.16)
                }
            }

            Rectangle {
                visible: swatch.checked
                x: parent.width - 24
                y: 2
                width: 20
                height: 20
                radius: 10
                color: Palette.tile
                border.width: 2
                border.color: Palette.paper

                Icon {
                    anchors.centerIn: parent
                    name: "check"
                    size: 12
                    color: Palette.onTile
                }
            }
        }
    }
}
