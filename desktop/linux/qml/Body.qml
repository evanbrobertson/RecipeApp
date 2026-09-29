import QtQuick

import app.crumb.desktop 1.0

// Body text in Nunito Sans. `muted` for the web's `text-ink-muted`; nothing under 13px.
Text {
    property bool muted: false
    property bool bold: false

    color: muted ? Palette.textMuted : Palette.text
    font.family: Palette.fontSans
    font.pixelSize: 15
    font.weight: bold ? Font.Bold : Font.Normal
    wrapMode: Text.WordWrap
    linkColor: Palette.primary
    lineHeight: 1.15
}
