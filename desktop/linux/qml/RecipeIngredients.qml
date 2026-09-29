import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The Ingredients panel of the recipe page (web RecipePage.svelte): the scale control and
// each section's lines, scaled with Core and ticked off as you go. Ticks are in memory, keyed
// by position, and clear when the recipe is replaced.
ColumnLayout {
    id: panel

    property var recipe: null
    property real scale: 1
    property bool secondary: false

    signal scaleEdited(real value)
    // A click or keyboard focus landed inside: the page makes this panel the wide one
    signal engaged

    property var checked: ({})
    readonly property int count: {
        var n = 0
        var sections = recipe ? recipe.ingredients : []
        for (var i = 0; i < sections.length; i++)
            n += sections[i].items.length
        return n
    }

    onRecipeChanged: checked = ({})

    function toggle(key) {
        var next = Object.assign({}, checked)
        if (next[key])
            delete next[key]
        else
            next[key] = true
        checked = next
    }

    spacing: 0

    Flow {
        Layout.fillWidth: true
        Layout.bottomMargin: 20
        spacing: 12

        Heading {
            text: "Ingredients"
            color: panel.secondary ? Palette.textMuted : Palette.text
            height: 52
            verticalAlignment: Text.AlignVCenter

            Behavior on color {
                ColorAnimation { duration: 250 }
            }
        }

        ScaleControl {
            visible: panel.count > 0
            value: panel.scale
            onEdited: value => panel.scaleEdited(value)
        }
    }

    Body {
        visible: panel.count === 0
        text: "No ingredients listed."
        muted: true
        font.pixelSize: 14
    }

    ColumnLayout {
        Layout.fillWidth: true
        spacing: 24

        Repeater {
            model: panel.recipe ? panel.recipe.ingredients : []

            delegate: ColumnLayout {
                id: section
                required property var modelData
                required property int index
                Layout.fillWidth: true
                spacing: 10

                Text {
                    visible: !!section.modelData.name
                    text: section.modelData.name || ""
                    color: Palette.primary
                    font.family: Palette.fontSans
                    font.pixelSize: 13
                    font.weight: Font.Bold
                    font.capitalization: Font.AllUppercase
                    font.letterSpacing: 0.5
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }

                Card {
                    Layout.fillWidth: true
                    implicitHeight: items.implicitHeight + 2
                    clip: true

                    ColumnLayout {
                        id: items
                        anchors.fill: parent
                        anchors.margins: 1
                        spacing: 0

                        Repeater {
                            model: section.modelData.items

                            delegate: Item {
                                id: row
                                required property string modelData
                                required property int index
                                readonly property string key: section.index + "-" + index
                                readonly property bool on: !!panel.checked[key]

                                Layout.fillWidth: true
                                implicitHeight: Math.max(48, line.implicitHeight + 24)
                                activeFocusOnTab: true
                                Accessible.role: Accessible.CheckBox
                                Accessible.checked: on
                                Accessible.name: text.text
                                Accessible.onPressAction: panel.toggle(key)
                                Keys.onSpacePressed: panel.toggle(key)
                                Keys.onReturnPressed: panel.toggle(key)
                                onActiveFocusChanged: if (activeFocus) panel.engaged()

                                Rectangle {
                                    visible: row.index > 0
                                    x: 16
                                    width: parent.width - 32
                                    height: 1
                                    color: Palette.line
                                }
                                Rectangle {
                                    anchors.fill: parent
                                    color: Palette.tint
                                    opacity: hover.hovered ? 0.55 : 0
                                }
                                Rectangle {
                                    anchors.fill: parent
                                    visible: row.activeFocus
                                    color: "transparent"
                                    border.width: 2
                                    border.color: Palette.primary
                                    radius: 4
                                }

                                RowLayout {
                                    id: line
                                    anchors.fill: parent
                                    anchors.leftMargin: 16
                                    anchors.rightMargin: 16
                                    anchors.topMargin: 12
                                    anchors.bottomMargin: 12
                                    spacing: 12

                                    Rectangle {
                                        Layout.alignment: Qt.AlignTop
                                        Layout.topMargin: 1
                                        Layout.preferredWidth: 22
                                        Layout.preferredHeight: 22
                                        radius: 11
                                        color: row.on ? Palette.tile : "transparent"
                                        border.width: 2
                                        border.color: row.on ? Palette.tile : Palette.textMuted

                                        Icon {
                                            visible: row.on
                                            anchors.centerIn: parent
                                            name: "check"
                                            size: 14
                                            color: Palette.onTile
                                        }
                                    }

                                    Body {
                                        id: text
                                        text: Core.scaleIngredient(row.modelData, panel.scale)
                                        color: row.on ? Palette.textMuted : Palette.text
                                        font.strikeout: row.on
                                        font.pixelSize: 16
                                        lineHeight: 1.0
                                        Layout.fillWidth: true
                                    }
                                }

                                HoverHandler {
                                    id: hover
                                    cursorShape: Qt.PointingHandCursor
                                }
                                TapHandler {
                                    onTapped: {
                                        panel.toggle(row.key)
                                        panel.engaged()
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
