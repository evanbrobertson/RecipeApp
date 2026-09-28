import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The web's `.list-row`: at least 68px tall, 12px padding, 14px between its parts, a hairline
// above every row but the first, a tint on hover. Put the row's parts inside it.
Item {
    id: row

    default property alias parts: layout.data
    property bool first: false
    property string label
    signal clicked()

    Layout.fillWidth: true
    implicitHeight: Math.max(68, layout.implicitHeight + 24)
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: label
    Accessible.onPressAction: row.clicked()
    Keys.onReturnPressed: row.clicked()
    Keys.onSpacePressed: row.clicked()

    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
    }

    TapHandler {
        onTapped: row.clicked()
    }

    Rectangle {
        anchors.fill: parent
        color: Palette.tint
        opacity: hover.hovered ? 0.55 : 0
    }

    Rectangle {
        visible: !row.first
        x: 12
        width: parent.width - 24
        height: 1
        color: Palette.line
    }

    Rectangle {
        visible: row.activeFocus
        anchors.fill: parent
        color: "transparent"
        border.width: 2
        border.color: Palette.primary
    }

    RowLayout {
        id: layout
        x: 12
        y: 12
        width: parent.width - 24
        height: parent.height - 24
        spacing: 14
    }
}
