import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// One recipe with lines to review on the Suggestions page, as the `.list-row` in
// web/src/islands/SuggestionsPage.svelte: photo, title, what's flagged, a count. Opens the editor.
Item {
    id: row

    // {id, title, image, count, fields}
    property var recipe: ({})
    property string summary: ""
    property bool first: false
    signal opened()

    implicitHeight: 76

    Rectangle {
        anchors.fill: parent
        color: Palette.tint
        opacity: hover.hovered ? 0.55 : 0
    }

    Rectangle {
        visible: !row.first
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: 12
        anchors.rightMargin: 12
        height: 1
        color: Palette.line
    }

    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
    }

    TapHandler {
        onTapped: row.opened()
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 12
        spacing: 14

        Photo {
            recipeId: row.recipe.id || 0
            image: row.recipe.image || ""
            Layout.preferredWidth: 56
            Layout.preferredHeight: 56
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 0

            Body {
                text: row.recipe.title || ""
                bold: true
                elide: Text.ElideRight
                Layout.fillWidth: true
                maximumLineCount: 1
            }

            Body {
                text: row.summary
                muted: true
                font.pixelSize: 14
                elide: Text.ElideRight
                Layout.fillWidth: true
                maximumLineCount: 1
            }
        }

        Rectangle {
            Layout.preferredWidth: Math.max(28, countLabel.implicitWidth + 16)
            Layout.preferredHeight: 28
            radius: 14
            color: Palette.tint

            Text {
                id: countLabel
                anchors.centerIn: parent
                text: row.recipe.count
                color: Palette.primary
                font.family: Palette.fontSans
                font.pixelSize: 13
                font.weight: Font.ExtraBold
            }
        }

        Icon {
            name: "chevron-right"
            size: 20
            color: Palette.textMuted
        }
    }
}
