import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// A recipe in a grid, as the web's RecipeCard.svelte: a 4:3 photo, the title (two lines),
// then a reason (Try next), a `meta` line, or the kicker and total time. Selectable cards
// toggle instead of opening the recipe.
Item {
    id: card

    // A RecipeSummary: {id, title, image, totalTime, recipeCategory, recipeCuisine}
    property var recipe: ({})
    property bool selectable: false
    property bool selected: false
    property string reason: ""
    property bool aiReason: false
    property string meta: ""
    signal toggled(int id)

    readonly property string sub: meta !== "" ? meta
        : [Core.kicker(recipe.recipeCategory || "", recipe.recipeCuisine || ""),
           Core.displayDuration(recipe.totalTime || "")].filter(function (s) { return s }).join(" · ")

    implicitWidth: 240
    implicitHeight: column.implicitHeight

    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
    }

    TapHandler {
        onTapped: {
            if (card.selectable)
                card.toggled(card.recipe.id)
            else
                card.ApplicationWindow.window.go("recipe", { "id": card.recipe.id })
        }
    }

    ColumnLayout {
        id: column
        width: parent.width
        spacing: 10

        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: Math.round(width * 3 / 4)

            Rectangle {
                visible: card.selected
                anchors.fill: parent
                anchors.margins: -5
                radius: 16
                color: "transparent"
                border.width: 3
                border.color: Palette.tile
            }

            Photo {
                anchors.fill: parent
                recipeId: card.recipe.id || 0
                image: card.recipe.image || ""
                scale: hover.hovered && !card.selectable ? 1.02 : 1

                Behavior on scale {
                    NumberAnimation { duration: 300; easing.type: Easing.OutCubic }
                }
            }

            Rectangle {
                visible: card.selectable
                x: 10
                y: 10
                width: 28
                height: 28
                radius: 14
                color: card.selected ? Palette.tile : Qt.rgba(0, 0, 0, 0.25)
                border.width: 2
                border.color: "white"

                Icon {
                    anchors.centerIn: parent
                    visible: card.selected
                    name: "check"
                    size: 16
                    color: Palette.onTile
                }
            }
        }

        Body {
            text: card.recipe.title || ""
            bold: true
            font.pixelSize: 16
            maximumLineCount: 2
            elide: Text.ElideRight
            Layout.fillWidth: true
        }

        RowLayout {
            visible: card.reason !== "" || card.sub !== ""
            spacing: 4
            Layout.fillWidth: true
            Layout.topMargin: -6

            Icon {
                visible: card.reason !== "" && card.aiReason
                name: "sparkles"
                size: 14
                color: Palette.primary
                Layout.alignment: Qt.AlignTop
                Layout.topMargin: 2
            }

            Body {
                text: card.reason !== "" ? card.reason : card.sub
                muted: true
                font.pixelSize: 14
                maximumLineCount: card.reason !== "" ? 2 : 1
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }
    }
}
