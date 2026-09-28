import QtQuick

import app.crumb.desktop 1.0

// Titles in DM Serif Display. `level` 1 is a page title (36px), 2 a section title (24px),
// 3 a card title (20px).
Text {
    property int level: 2

    color: Palette.text
    font.family: Palette.fontSerif
    font.pixelSize: level === 1 ? 36 : level === 2 ? 24 : 20
    wrapMode: Text.WordWrap
    elide: Text.ElideRight
}
