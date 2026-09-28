import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The web's `.list-card`: a paper card of rows split by hairlines. Rows are HomeRow items.
Rectangle {
    id: card

    default property alias rows: column.data

    color: Palette.paper
    radius: 16
    border.width: 1
    border.color: Palette.line
    implicitHeight: column.implicitHeight + 2

    ColumnLayout {
        id: column
        x: 1
        y: 1
        width: parent.width - 2
        spacing: 0
    }
}
