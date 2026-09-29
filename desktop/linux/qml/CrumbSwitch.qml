import QtQuick

import app.crumb.desktop 1.0

// The web's `.switch`: a pill that fills with tile green when `on`. Only the look; the row
// around it takes the click and says what it switches.
Rectangle {
    id: track

    property bool on: false

    implicitWidth: 44
    implicitHeight: 26
    radius: 13
    color: on ? Palette.tile : Palette.line

    Rectangle {
        x: track.on ? 21 : 3
        y: 3
        width: 20
        height: 20
        radius: 10
        color: Palette.paper

        Behavior on x {
            NumberAnimation { duration: 150 }
        }
    }
}
