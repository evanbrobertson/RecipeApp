import QtQuick

import app.crumb.desktop 1.0

// A staple's bar in the Suggestions tip, as `.staple-bar` in web/src/islands/SuggestionsPage.svelte:
// fuller and stronger the more recipes use it (`share`, 0-1), faint for the fewest.
Rectangle {
    id: bar

    property real share: 0

    height: 6
    radius: 3
    color: Palette.line
    clip: true

    Rectangle {
        width: Math.max(8, parent.width * bar.share)
        height: parent.height
        radius: 3
        color: Palette.butter
        opacity: 0.4 + bar.share * 0.6
    }
}
