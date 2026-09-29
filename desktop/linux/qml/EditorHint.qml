import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// One of Wee Chef's hints in the editor: a tint bar with the chef hat, the line quoted and
// what looked off, a one-tap fix and "Keep as is". Mirrors the hint bars in RecipeEditor.svelte.
Rectangle {
    id: hint

    // Bold lead, then the plain rest of the sentence
    property string lead: ""
    property string rest: ""
    property string fixLabel: ""
    property string dismissLabel: "Keep as is"

    signal fix()
    signal dismiss()

    radius: 12
    color: Palette.tint
    implicitHeight: row.implicitHeight + 12

    RowLayout {
        id: row
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        anchors.leftMargin: 12
        anchors.rightMargin: 6
        spacing: 12

        RowLayout {
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignTop
            Layout.topMargin: 6
            Layout.bottomMargin: 6
            spacing: 8

            Icon {
                name: "chef-hat"
                size: 16
                color: Palette.primary
                Layout.alignment: Qt.AlignTop
                Layout.topMargin: 2
            }

            Text {
                Layout.fillWidth: true
                textFormat: Text.StyledText
                wrapMode: Text.WordWrap
                color: Palette.text
                font.family: Palette.fontSans
                font.pixelSize: 14
                text: "<b>" + hint.markup(hint.lead) + "</b> " + hint.markup(hint.rest)
                Accessible.name: "Wee Chef: " + hint.lead + " " + hint.rest
            }
        }

        Row {
            spacing: 4
            Layout.alignment: Qt.AlignVCenter

            CrumbButton {
                visible: hint.fixLabel !== ""
                kind: "outline"
                text: hint.fixLabel
                onClicked: hint.fix()
            }

            CrumbButton {
                kind: "ghost"
                text: hint.dismissLabel
                onClicked: hint.dismiss()
            }
        }
    }

    function markup(s) {
        return String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
    }
}
