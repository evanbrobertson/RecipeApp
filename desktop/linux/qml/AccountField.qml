import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The web's `.label` over an `.input` (LoginForm.svelte, AccountPage.svelte), with an optional
// hint under it. `large` is `.input-lg`.
ColumnLayout {
    id: field

    property string label
    property string hint
    property bool large: false
    property alias text: input.text
    property alias echoMode: input.echoMode
    property alias placeholderText: input.placeholderText
    property alias maximumLength: input.maximumLength
    property alias input: input

    signal accepted()

    Layout.fillWidth: true
    spacing: 6

    Text {
        visible: field.label !== ""
        text: field.label
        color: Palette.text
        font.family: Palette.fontSans
        font.pixelSize: 15
        font.weight: Font.Bold
    }

    StyledField {
        id: input
        Layout.fillWidth: true
        Layout.preferredHeight: field.large ? 56 : 44
        font.pixelSize: field.large ? 16 : 15
        font.family: Palette.fontSans
        onAccepted: field.accepted()
    }

    Text {
        visible: field.hint !== ""
        text: field.hint
        color: Palette.textMuted
        font.family: Palette.fontSans
        font.pixelSize: 14
        Layout.topMargin: -2
    }
}
