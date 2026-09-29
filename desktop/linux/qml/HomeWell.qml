import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The web's `.well`: a 44px tint square holding a primary-coloured icon.
Rectangle {
    property string iconName

    Layout.preferredWidth: 44
    Layout.preferredHeight: 44
    radius: 12
    color: Palette.tint

    Icon {
        anchors.centerIn: parent
        name: parent.iconName
        size: 22
        color: Palette.primary
    }
}
