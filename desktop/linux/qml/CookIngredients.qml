import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The ingredient list cook mode shows, in the pinned panel and in the narrow-screen modal:
// every line scaled, tap to tick. `rows` is `[{section, head, raw}]` (`head`: a section name
// starts here); a tick is the line's index in that list, `checked` an object keyed by it.
Flickable {
    id: list

    property var rows: []
    property var checked: ({})
    property real scale: 1

    signal toggled(int index)

    contentHeight: column.implicitHeight
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ScrollBar {}

    ColumnLayout {
        id: column
        width: list.width - 12
        spacing: 0

        Repeater {
            model: list.rows

            delegate: ColumnLayout {
                id: line
                required property var modelData
                required property int index
                readonly property bool on: !!list.checked[index]

                Layout.fillWidth: true
                spacing: 0

                Text {
                    visible: line.modelData.head
                    text: line.modelData.section
                    color: Palette.primary
                    font.family: Palette.fontSans
                    font.pixelSize: 13
                    font.weight: Font.Bold
                    font.capitalization: Font.AllUppercase
                    font.letterSpacing: 0.5
                    Layout.topMargin: 16
                    Layout.bottomMargin: 4
                }

                MouseArea {
                    Layout.fillWidth: true
                    implicitHeight: Math.max(48, label.implicitHeight + 20)
                    cursorShape: Qt.PointingHandCursor
                    Accessible.role: Accessible.CheckBox
                    Accessible.checked: line.on
                    Accessible.name: label.text
                    Accessible.onPressAction: clicked(null)
                    onClicked: list.toggled(line.index)

                    Rectangle {
                        anchors.fill: parent
                        radius: 12
                        color: Palette.tint
                        opacity: parent.pressed ? 1 : 0
                    }

                    Rectangle {
                        id: box
                        x: 4
                        y: 12
                        width: 24
                        height: 24
                        radius: 12
                        color: line.on ? Palette.tile : "transparent"
                        border.width: 2
                        border.color: line.on ? Palette.tile : Palette.line

                        Icon {
                            visible: line.on
                            anchors.centerIn: parent
                            name: "check"
                            size: 16
                            color: Palette.onTile
                        }
                    }

                    Text {
                        id: label
                        anchors.left: box.right
                        anchors.leftMargin: 12
                        anchors.right: parent.right
                        anchors.rightMargin: 4
                        y: 10
                        text: Core.scaleIngredient(line.modelData.raw, list.scale)
                        color: line.on ? Palette.textMuted : Palette.text
                        font.family: Palette.fontSans
                        font.pixelSize: 18
                        font.strikeout: line.on
                        wrapMode: Text.WordWrap
                    }
                }
            }
        }
    }
}
