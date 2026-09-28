import QtQuick

import app.crumb.desktop 1.0

// Not built yet: see PARITY.md.
Item {
    property var session
    property int routeId: 0
    property var params: ({})

    EmptyState {
        anchors.centerIn: parent
        iconName: "cooking-pot"
        title: "CookbookPage"
        text: "This page isn't in the desktop app yet."
    }
}
