import QtQuick

import app.crumb.desktop 1.0

// The web's `.settings-heading`: a small caps muted title over a list card, as on
// pages/more.astro, pages/connect.astro and pages/more/connections.astro.
Text {
    color: Palette.textMuted
    font.family: Palette.fontSans
    font.pixelSize: 13
    font.weight: Font.Bold
    font.letterSpacing: 0.5
    font.capitalization: Font.AllUppercase
    leftPadding: 16
    rightPadding: 16
    elide: Text.ElideRight
}
