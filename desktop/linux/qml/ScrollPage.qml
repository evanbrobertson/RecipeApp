import QtQuick
import QtQuick.Controls

import app.crumb.desktop 1.0

// A page's scrolling body, laid out like the web's <main>: up to 1200px wide, centred,
// 32px from the sides and top. Put a ColumnLayout (or anything with an implicitHeight) in it.
Flickable {
    id: page

    default property alias content: column.data
    property int maxWidth: 1200
    property int sidePadding: 32
    property int topPadding: 32
    property int bottomPadding: 64
    readonly property real contentWidthAvailable: column.width

    clip: true
    contentWidth: width
    contentHeight: column.implicitHeight + topPadding + bottomPadding
    boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ScrollBar {}

    Column {
        id: column
        x: Math.max(page.sidePadding, (page.width - width) / 2)
        y: page.topPadding
        width: Math.min(page.maxWidth, page.width - 2 * page.sidePadding)
        spacing: 0
    }
}
