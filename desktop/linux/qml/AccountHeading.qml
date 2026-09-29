import QtQuick

import app.crumb.desktop 1.0

// The web's `.settings-heading` (AccountPage.svelte): a small uppercase label above a list card.
Text {
    color: Palette.textMuted
    font.family: Palette.fontSans
    font.pixelSize: 13
    font.weight: Font.Bold
    font.letterSpacing: 0.5
    font.capitalization: Font.AllUppercase
    leftPadding: 16
    bottomPadding: 8
}
