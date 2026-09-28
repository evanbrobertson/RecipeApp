import QtQuick

import app.crumb.desktop 1.0

// One book lying flat with its spine to the reader, as the web's CookbookSpine.svelte. Its
// size, lean and band come from `Core.bookShape`, its cloth from `Core.bookLook`, and the
// cloth is the same in light and dark. Slides out a little on hover, further when pulled.
Item {
    id: spine

    // {id, name, color, recipeCount}
    property var book: ({})
    // At the foot of its tower, so it sits flatter on the plank
    property bool atFoot: false
    property bool pulled: false
    // The tower's longest book, and the tower's width
    property real towerLen: 180
    property real towerW: 196
    signal opened()

    readonly property var look: JSON.parse(Core.bookLook(book.color || ""))
    readonly property var shape: JSON.parse(Core.bookShape(JSON.stringify(book), atFoot))
    // Its own length, stretched to fit the title, but never much past the tower's longest
    readonly property real bookWidth: Math.min(towerLen * 1.06, Math.max(towerLen * shape.length, shape.title))
    readonly property bool lifted: hover.hovered || spine.activeFocus
    readonly property color hairline: Qt.rgba(Palette.text.r, Palette.text.g, Palette.text.b, 0.09)

    width: towerW
    height: shape.thickness
    z: lifted || pulled ? 1 : 0
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: "Open " + book.name + ", " + book.recipeCount + " recipes"
    Accessible.onPressAction: spine.opened()
    Keys.onReturnPressed: spine.opened()
    Keys.onSpacePressed: spine.opened()

    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
    }

    TapHandler {
        onTapped: spine.opened()
    }

    Rectangle {
        id: cloth

        x: (spine.width - width) / 2 + spine.shape.nudge + (spine.pulled ? 40 : spine.lifted ? 14 : 0)
        width: spine.bookWidth
        height: spine.height
        // Square board edges, the boards a shade darker than the spine cloth
        radius: 3
        color: spine.look.cloth
        rotation: spine.pulled ? 0 : spine.lifted ? spine.shape.tilt * 0.4 : spine.shape.tilt

        Behavior on x {
            NumberAnimation { duration: 350; easing.type: Easing.OutCubic }
        }
        Behavior on rotation {
            NumberAnimation { duration: 350; easing.type: Easing.OutCubic }
        }

        Rectangle {
            x: 0
            y: parent.height - 3
            width: parent.width
            height: 3
            radius: 3
            color: spine.look.shade
        }

        // Two 2px bands, 10px in from each end
        Rectangle {
            visible: !!spine.shape.band
            x: 10
            width: 2
            height: parent.height
            color: spine.shape.band || "transparent"
        }
        Rectangle {
            visible: !!spine.shape.band
            x: parent.width - 12
            width: 2
            height: parent.height
            color: spine.shape.band || "transparent"
        }

        // A faint hairline in the text colour keeps dark covers visible on a dark shelf
        Rectangle {
            anchors.fill: parent
            radius: 3
            color: "transparent"
            border.width: 1
            border.color: spine.hairline
        }

        // A pale cover gets a thin inset edge
        Rectangle {
            visible: !!spine.look.edge
            anchors.fill: parent
            radius: 3
            color: "transparent"
            border.width: 1
            border.color: spine.look.edge ? Qt.rgba(Qt.color(spine.look.edge).r, Qt.color(spine.look.edge).g,
                                                    Qt.color(spine.look.edge).b, spine.look.edgeAlpha) : "transparent"
        }

        Text {
            x: spine.shape.band ? 26 : 14
            width: parent.width - 2 * x
            height: parent.height
            bottomPadding: 2
            verticalAlignment: Text.AlignVCenter
            text: spine.book.name
            color: spine.look.foil
            font.family: Palette.fontSerif
            font.pixelSize: 14
            elide: Text.ElideRight
            maximumLineCount: 1
        }

        Rectangle {
            visible: spine.activeFocus
            anchors.fill: parent
            anchors.margins: -2
            radius: 4
            color: "transparent"
            border.width: 2
            border.color: Palette.primary
        }
    }
}
