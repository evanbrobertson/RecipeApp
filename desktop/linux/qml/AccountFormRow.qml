import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// A settings row that holds a small form (change email, delete account): the muted icon at the
// top left and the form (children) beside it. The web's `.list-row.settings-row.items-start`.
Item {
    id: row

    property string iconName
    default property alias form: column.data

    Layout.fillWidth: true
    implicitHeight: column.implicitHeight + 20

    Rectangle {
        anchors.left: parent.left
        anchors.leftMargin: 50
        anchors.right: parent.right
        height: 1
        color: Palette.line
        visible: row.y > 0
    }

    Icon {
        x: 16
        y: 12
        name: row.iconName
        size: 20
        color: Palette.textMuted
    }

    ColumnLayout {
        id: column
        x: 50
        y: 10
        width: parent.width - 50 - 16
        spacing: 12
    }
}
