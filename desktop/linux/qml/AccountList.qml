import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The web's `.list-card`: a card of rows with inset dividers (AccountRow, AccountFormRow).
Card {
    default property alias rows: column.data

    Layout.fillWidth: true
    implicitHeight: column.implicitHeight + 2
    clip: true

    ColumnLayout {
        id: column
        x: 1
        y: 1
        width: parent.width - 2
        spacing: 0
    }
}
