import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The web's Menu.svelte: a small dropdown under its trigger. `groups` is a list of lists of
// {label, icon, danger, onselect}; groups are split by a line.
//   CrumbMenu { id: menu; groups: [[{label: "Edit", icon: "pencil", onselect: () => …}]] }
//   CrumbButton { iconName: "ellipsis"; onClicked: menu.popup(this) }
Popup {
    id: menu

    property var groups: []

    // Opens under `item`, right-aligned to it.
    function popup(item) {
        var p = item.mapToItem(Overlay.overlay, item.width, item.height + 6)
        x = Math.max(8, p.x - width)
        y = p.y
        open()
    }

    parent: Overlay.overlay
    padding: 6
    width: 240
    focus: true
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    background: Rectangle {
        radius: 16
        color: Palette.paper
        border.width: 1
        border.color: Palette.line

        // Menus are the one place with a shadow
        Rectangle {
            z: -1
            anchors.fill: parent
            anchors.topMargin: 4
            anchors.leftMargin: 1
            anchors.rightMargin: -1
            anchors.bottomMargin: -6
            radius: 18
            color: Qt.rgba(0, 0, 0, 0.12)
        }
    }

    contentItem: ColumnLayout {
        spacing: 2

        Repeater {
            model: menu.groups

            delegate: ColumnLayout {
                id: group
                required property var modelData
                required property int index
                spacing: 2
                Layout.fillWidth: true

                Rectangle {
                    visible: group.index > 0
                    Layout.fillWidth: true
                    Layout.preferredHeight: 1
                    Layout.topMargin: 4
                    Layout.bottomMargin: 4
                    color: Palette.line
                }

                Repeater {
                    model: group.modelData

                    delegate: Rectangle {
                        id: row
                        required property var modelData
                        Layout.fillWidth: true
                        Layout.preferredHeight: 40
                        radius: 12
                        color: rowHover.hovered ? Palette.tint : "transparent"

                        HoverHandler {
                            id: rowHover
                            cursorShape: Qt.PointingHandCursor
                        }

                        TapHandler {
                            onTapped: {
                                menu.close()
                                if (row.modelData.onselect)
                                    row.modelData.onselect()
                            }
                        }

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 10
                            anchors.rightMargin: 10
                            spacing: 10

                            Icon {
                                name: row.modelData.icon || ""
                                visible: !!row.modelData.icon
                                size: 18
                                color: row.modelData.danger ? Palette.error : Palette.textMuted
                            }

                            Body {
                                text: row.modelData.label
                                color: row.modelData.danger ? Palette.error : Palette.text
                                font.pixelSize: 15
                                bold: false
                                Layout.fillWidth: true
                            }
                        }
                    }
                }
            }
        }
    }
}
