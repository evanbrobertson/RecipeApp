import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The web's Modal.svelte: a centred panel over a dimmed page with a title, an optional
// description, a close button, the body (children) and a `footer` row of buttons.
//   Modal { id: m; title: "Rename"; Body {...}; footer: [CrumbButton {...}] }  m.open()
Popup {
    id: modal

    property string title
    property string description
    default property alias body: bodyColumn.data
    property alias footer: footerRow.data

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(520, (parent ? parent.width : 520) - 48)
    modal: true
    focus: true
    padding: 24
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    Overlay.modal: Rectangle {
        color: Qt.rgba(0, 0, 0, 0.45)
    }

    background: Rectangle {
        radius: 16
        color: Palette.paper
        border.width: 1
        border.color: Palette.line
    }

    contentItem: ColumnLayout {
        spacing: 0

        RowLayout {
            Layout.fillWidth: true
            spacing: 12

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 6

                Heading {
                    level: 2
                    text: modal.title
                    Layout.fillWidth: true
                }

                Body {
                    visible: modal.description !== ""
                    text: modal.description
                    muted: true
                    font.pixelSize: 14
                    Layout.fillWidth: true
                }
            }

            CrumbButton {
                kind: "ghost"
                iconName: "x"
                Layout.alignment: Qt.AlignTop
                onClicked: modal.close()
            }
        }

        ColumnLayout {
            id: bodyColumn
            Layout.fillWidth: true
            Layout.topMargin: 16
            spacing: 12
        }

        RowLayout {
            id: footerRow
            visible: children.length > 0
            Layout.alignment: Qt.AlignRight
            Layout.topMargin: 24
            spacing: 8
        }
    }
}
