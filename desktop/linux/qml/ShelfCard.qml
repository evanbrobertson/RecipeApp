import QtQuick

import app.crumb.desktop 1.0

// The tint backdrop a Bookshelf stands on (web: `bg-tint rounded-ui overflow-hidden`): rounded
// corners, the plank running to the edges, so the bottom corners are cut back to the page.
Rectangle {
    id: card

    default property alias content: inner.data

    color: Palette.tint
    radius: 16
    implicitHeight: inner.childrenRect.height + 20

    Item {
        id: inner
        anchors.fill: parent
        anchors.topMargin: 20
    }

    Repeater {
        model: 2

        delegate: Canvas {
            id: corner

            required property int index

            x: index === 0 ? 0 : card.width - 16
            y: card.height - 16
            width: 16
            height: 16
            onPaint: {
                var ctx = getContext("2d")
                ctx.reset()
                ctx.fillStyle = Palette.bg
                ctx.beginPath()
                if (index === 0) {
                    ctx.moveTo(0, 16)
                    ctx.lineTo(0, 0)
                    ctx.arc(16, 0, 16, Math.PI, Math.PI / 2, true)
                } else {
                    ctx.moveTo(16, 16)
                    ctx.lineTo(16, 0)
                    ctx.arc(0, 0, 16, 0, Math.PI / 2, false)
                }
                ctx.closePath()
                ctx.fill()
            }
            Connections {
                target: Palette
                function onBgChanged() { corner.requestPaint() }
            }
        }
    }
}
