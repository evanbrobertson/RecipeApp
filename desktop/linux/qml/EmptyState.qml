import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The web's EmptyState: an icon in a soft circle, a title, a line, and an optional action.
ColumnLayout {
    id: empty

    property string iconName: "cooking-pot"
    property string title
    property string text
    property string actionText
    signal action()

    spacing: 10

    Rectangle {
        Layout.alignment: Qt.AlignHCenter
        width: 64
        height: 64
        radius: 32
        color: Palette.tint

        Icon {
            anchors.centerIn: parent
            name: empty.iconName
            size: 28
            color: Palette.primary
        }
    }

    Heading {
        level: 3
        text: empty.title
        horizontalAlignment: Text.AlignHCenter
        Layout.fillWidth: true
    }

    Body {
        muted: true
        visible: empty.text !== ""
        text: empty.text
        horizontalAlignment: Text.AlignHCenter
        Layout.fillWidth: true
        Layout.maximumWidth: 420
        Layout.alignment: Qt.AlignHCenter
    }

    CrumbButton {
        visible: empty.actionText !== ""
        kind: "primary"
        text: empty.actionText
        Layout.alignment: Qt.AlignHCenter
        Layout.topMargin: 6
        onClicked: empty.action()
    }
}
