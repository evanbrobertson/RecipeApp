import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The quiet note when Wee Chef tidied or flagged lines on import (web WeeChefCard.svelte):
// what it fixed (See / Undo), what might need a look, and a photo link that's broken.
// `undone(recipe, checks)` fires after Undo.
Card {
    id: card

    property int recipeId: 0
    property var checks: null
    property bool open: false
    property bool undoing: false

    signal undone(var recipe, var checks)
    signal editRequested

    readonly property var flags: checks && checks.flags ? checks.flags : []
    readonly property var fixed: flags.filter(f => f.state === "fixed")
    readonly property var review: flags.filter(f => f.state === "review" && f.field !== "image")
    readonly property var photo: flags.find(f => f.state === "review" && f.field === "image")

    visible: fixed.length > 0 || review.length > 0 || !!photo
    implicitHeight: column.implicitHeight + 8
    Accessible.name: "Wee Chef"

    Requests {
        id: requests
    }

    function undo() {
        if (undoing)
            return
        undoing = true
        requests.call("undoChecks", {"id": recipeId}, function (res) {
            undoing = false
            card.undone(res.recipe, res.checks)
            ApplicationWindow.window.toast({"title": "Put back as it was imported"})
        }, function (error) {
            undoing = false
            ApplicationWindow.window.toast({"title": "Couldn't undo", "description": error, "tone": "error"})
        })
    }

    // One line of the card: an icon slot, text, and link-style actions
    component CardLine: ColumnLayout {
        id: line
        property bool first: true
        property bool divider: false
        property string text: ""
        property bool bold: false
        default property alias actions: actionRow.data

        Layout.fillWidth: true
        spacing: 0

        Rectangle {
            visible: line.divider
            Layout.fillWidth: true
            Layout.preferredHeight: 1
            color: Palette.line
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Item {
                Layout.preferredWidth: 18
                Layout.preferredHeight: 18
                Layout.alignment: Qt.AlignVCenter

                Icon {
                    visible: line.first
                    name: "chef-hat"
                    size: 18
                    color: Palette.primary
                }
            }

            Body {
                text: line.text
                bold: line.bold
                Layout.fillWidth: true
                Layout.topMargin: 10
                Layout.bottomMargin: 10
            }

            RowLayout {
                id: actionRow
                spacing: 8
            }
        }
    }

    component LinkButton: Text {
        id: link
        signal clicked
        property bool busy: false

        color: Palette.primary
        opacity: busy ? 0.5 : 1
        font.family: Palette.fontSans
        font.pixelSize: 15
        font.weight: Font.Bold
        font.underline: hover.hovered
        verticalAlignment: Text.AlignVCenter
        Layout.preferredHeight: 44
        Accessible.role: Accessible.Link
        Accessible.name: text
        Accessible.onPressAction: link.clicked()

        HoverHandler {
            id: hover
            cursorShape: Qt.PointingHandCursor
        }
        TapHandler {
            onTapped: if (!link.busy) link.clicked()
        }
    }

    ColumnLayout {
        id: column
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: 16
        anchors.topMargin: 4
        spacing: 0

        CardLine {
            visible: card.fixed.length > 0
            first: true
            bold: true
            text: Core.tidiedTitle(card.fixed.length)

            LinkButton {
                text: card.open ? "Hide" : "See"
                onClicked: card.open = !card.open
            }
            Text {
                visible: !!(card.checks && card.checks.canUndo)
                text: "·"
                color: Palette.textMuted
                font.pixelSize: 15
            }
            LinkButton {
                visible: !!(card.checks && card.checks.canUndo)
                text: "Undo"
                busy: card.undoing
                onClicked: card.undo()
            }
        }

        ColumnLayout {
            visible: card.fixed.length > 0 && card.open
            Layout.fillWidth: true
            Layout.leftMargin: 26
            Layout.bottomMargin: 12
            spacing: 6

            Repeater {
                model: card.fixed

                delegate: Body {
                    required property var modelData
                    text: Core.fixText(JSON.stringify(modelData.detail || {}), modelData.itemText || "")
                    muted: true
                    font.pixelSize: 14
                    Layout.fillWidth: true
                }
            }
        }

        CardLine {
            visible: card.review.length > 0
            first: card.fixed.length === 0
            divider: card.fixed.length > 0
            text: Core.lookTitle(card.review.length)

            LinkButton {
                text: "Edit"
                onClicked: card.editRequested()
            }
        }

        CardLine {
            visible: !!card.photo
            first: card.fixed.length === 0 && card.review.length === 0
            divider: card.fixed.length > 0 || card.review.length > 0
            text: card.photo ? Core.reviewText(card.photo.kind) : ""

            LinkButton {
                text: "Edit"
                onClicked: card.editRequested()
            }
        }
    }
}
