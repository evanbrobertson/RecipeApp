import QtQuick
import QtQuick.Controls

import app.crumb.desktop 1.0

// The editor's multi-line input: grows with its text, never shorter than `rows` lines, and
// Tab moves on (web `textarea.input` with `use:autosize`). Mirrors RecipeEditor.svelte.
Rectangle {
    id: area

    property string value: ""
    property int rows: 3
    property string placeholder: ""
    property string accessibleName: ""
    // The cook's own hand, as the notes on the recipe page
    property bool hand: false
    readonly property real fontSize: hand ? 21 : 15
    readonly property alias field: input

    signal edited(string text)

    implicitHeight: Math.max(80, input.implicitHeight + 2)
    radius: 12
    color: Palette.paper
    border.width: input.activeFocus ? 2 : 1
    border.color: input.activeFocus ? Palette.primary : Palette.line

    FontMetrics {
        id: metrics
        font.family: area.hand ? Palette.fontNotes : Palette.fontSans
        font.pixelSize: area.fontSize
    }

    TextArea {
        id: input
        anchors.fill: parent
        anchors.margins: 1
        text: area.value
        wrapMode: TextArea.Wrap
        leftPadding: 14
        rightPadding: 14
        topPadding: 10
        bottomPadding: 10
        color: Palette.text
        placeholderText: area.placeholder
        placeholderTextColor: Palette.textMuted
        font.family: area.hand ? Palette.fontNotes : Palette.fontSans
        font.pixelSize: area.fontSize
        selectionColor: Palette.tile
        selectedTextColor: Palette.onTile
        background: null
        Accessible.name: area.accessibleName
        onTextChanged: if (activeFocus) area.edited(text)
        Keys.onTabPressed: nextItemInFocusChain(true).forceActiveFocus(Qt.TabFocusReason)
        Keys.onBacktabPressed: nextItemInFocusChain(false).forceActiveFocus(Qt.BacktabFocusReason)
    }
}
