import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The cookbook fields the web asks for in New book and Rename: a name, an optional
// description and the cover colour.
ColumnLayout {
    id: form

    property alias name: nameField.text
    property alias description: descriptionField.text
    property string color: "tile"
    // A taller name box, as on the cookbook page's edit form
    property bool large: false
    signal accepted()

    function focusName() {
        nameField.forceActiveFocus()
    }

    spacing: 16

    ColumnLayout {
        spacing: 6
        Layout.fillWidth: true

        Body {
            text: "Name"
            bold: true
        }

        StyledField {
            id: nameField
            placeholderText: "Weeknight dinners"
            font.pixelSize: 16
            topPadding: form.large ? 16 : 10
            bottomPadding: form.large ? 16 : 10
            Layout.fillWidth: true
            onAccepted: form.accepted()
        }
    }

    ColumnLayout {
        spacing: 6
        Layout.fillWidth: true

        Body {
            text: "Description"
            bold: true
        }

        TextArea {
            id: descriptionField
            placeholderText: "Optional"
            placeholderTextColor: Palette.textMuted
            color: Palette.text
            selectionColor: Palette.tile
            selectedTextColor: Palette.onTile
            font.family: Palette.fontSans
            font.pixelSize: 16
            wrapMode: TextEdit.Wrap
            leftPadding: 14
            rightPadding: 14
            topPadding: 10
            bottomPadding: 10
            Layout.fillWidth: true
            Layout.minimumHeight: 80

            background: Rectangle {
                radius: 12
                color: Palette.paper
                border.width: descriptionField.activeFocus ? 2 : 1
                border.color: descriptionField.activeFocus ? Palette.primary : Palette.line
            }
        }
    }

    ColumnLayout {
        spacing: 6
        Layout.fillWidth: true

        Body {
            text: "Cover colour"
            bold: true
        }

        BookColorPicker {
            value: form.color
            onValueChanged: form.color = value
            Layout.fillWidth: true
        }
    }
}
