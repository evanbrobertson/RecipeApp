import QtQuick

import app.crumb.desktop 1.0

// The card a Bookshelf hangs in (web: `bg-tint rounded-ui overflow-hidden`): the wall and its
// planks run to the edges, so the four corners are cut back to the page.
Rectangle {
    id: card

    default property alias content: inner.data

    color: Palette.tint
    radius: 16
    implicitHeight: inner.childrenRect.height

    Item {
        id: inner
        anchors.fill: parent
        clip: true
    }

    Repeater {
        model: 4

        delegate: Canvas {
            id: corner

            required property int index
            readonly property bool atRight: index % 2 === 1
            readonly property bool atBottom: index > 1

            x: atRight ? card.width - 16 : 0
            y: atBottom ? card.height - 16 : 0
            width: 16
            height: 16
            onPaint: {
                var ctx = getContext("2d")
                ctx.reset()
                ctx.fillStyle = Palette.bg
                // The square, less a circle about the card's side of it
                ctx.beginPath()
                ctx.rect(0, 0, 16, 16)
                ctx.arc(atRight ? 0 : 16, atBottom ? 0 : 16, 16, 0, Math.PI * 2, true)
                ctx.fill()
            }
            Connections {
                target: Palette
                function onBgChanged() { corner.requestPaint() }
            }
        }
    }
}
