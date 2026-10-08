import QtQuick

import app.crumb.desktop 1.0

// A cloth-bound spine, as the web's BookSpine.svelte: grain and weave, a rounded back caught by
// light from the upper left, and its dressing (gilt rules, a paper label or contrasting ends).
// Only paint; the shelf makes it a button and the open book's flight turns it into the side of
// a book. `spine` is core's spine (`Core.spineFor`, or a shelf item's `book`); the cloth is
// `Core.bookLook`'s, the same in light and dark.
Item {
    id: art

    // {standing, w, h, style, lines, font}
    property var spine: ({})
    property string color: ""
    // A lying book drawn standing, to be turned a quarter left (the open book's flight): its
    // light still has to come from above once it's turned
    property bool turned: false

    readonly property var look: JSON.parse(Core.bookLook(color))
    readonly property bool up: !!spine.standing || turned
    // Foil on dark cloth is stamped in; dark type on pale cloth catches light on its lower lip
    readonly property bool pale: ["sage", "butter", "cream"].indexOf(look.name) >= 0
    readonly property string style: spine.style || "plain"
    readonly property var lines: spine.lines || []

    width: turned ? spine.h || 0 : spine.w || 0
    height: turned ? spine.w || 0 : spine.h || 0

    // The rounded box both canvases clip to: 3px at the head, 2px at the foot standing
    function outline(ctx, w, h) {
        var top = 3
        var bottom = art.up ? 2 : 3
        ctx.beginPath()
        ctx.moveTo(top, 0)
        ctx.lineTo(w - top, 0)
        ctx.arcTo(w, 0, w, top, top)
        ctx.lineTo(w, h - bottom)
        ctx.arcTo(w, h, w - bottom, h, bottom)
        ctx.lineTo(bottom, h)
        ctx.arcTo(0, h, 0, h - bottom, bottom)
        ctx.lineTo(0, top)
        ctx.arcTo(0, 0, top, 0, top)
        ctx.closePath()
    }

    function grain(ctx, x, y, w, h) {
        if (!cloth.isImageLoaded(cloth.grainUrl))
            return
        ctx.save()
        ctx.globalCompositeOperation = "qt-soft-light"
        ctx.fillStyle = ctx.createPattern(cloth.grainUrl, "repeat")
        ctx.fillRect(x, y, w, h)
        ctx.restore()
    }

    function gradient(ctx, x0, y0, x1, y1, stops) {
        var g = ctx.createLinearGradient(x0, y0, x1, y1)
        for (var i = 0; i < stops.length; i++)
            g.addColorStop(Math.max(0, Math.min(1, stops[i][0])), stops[i][1])
        return g
    }

    // Cloth, weave, grain and the dressing under the title
    Canvas {
        id: cloth

        readonly property string grainUrl: "qrc:/shelf/grain.png"

        anchors.fill: parent
        Component.onCompleted: loadImage(grainUrl)
        onImageLoaded: requestPaint()
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()

        onPaint: {
            var ctx = getContext("2d")
            var w = width
            var h = height
            ctx.reset()
            if (w <= 0 || h <= 0)
                return
            ctx.save()
            art.outline(ctx, w, h)
            ctx.clip()
            ctx.fillStyle = art.look.cloth
            ctx.fillRect(0, 0, w, h)
            // A fine weave: dark threads one way, light the other
            ctx.fillStyle = "rgba(0, 0, 0, 0.05)"
            for (var x = 0; x < w; x += 3)
                ctx.fillRect(x, 0, 1, h)
            ctx.fillStyle = "rgba(255, 255, 255, 0.06)"
            for (var y = h - 1; y > -1; y -= 3)
                ctx.fillRect(0, y, w, 1)
            art.grain(ctx, 0, 0, w, h)

            var orn = art.look.bands[0]
            if (art.style === "ends") {
                // Contrasting cloth at head and foot, edged with a foil line
                var ends = art.up ? [[0, 0, w, 14], [0, h - 14, w, 14]] : [[0, 0, 14, h], [w - 14, 0, 14, h]]
                for (var i = 0; i < 2; i++) {
                    var e = ends[i]
                    ctx.fillStyle = orn
                    ctx.fillRect(e[0], e[1], e[2], e[3])
                    art.grain(ctx, e[0], e[1], e[2], e[3])
                }
                ctx.fillStyle = art.look.foil
                if (art.up) {
                    ctx.fillRect(0.5, 14, w - 1, 1.5)
                    ctx.fillRect(0.5, h - 15.5, w - 1, 1.5)
                } else {
                    ctx.fillRect(14, 0.5, 1.5, h - 1)
                    ctx.fillRect(w - 15.5, 0.5, 1.5, h - 1)
                }
            } else if (art.style === "rules") {
                // Gilt: two fine rules at each end
                ctx.globalAlpha = 0.9
                ctx.fillStyle = orn
                var at = [12, 16.5, (art.up ? h : w) - 18, (art.up ? h : w) - 13.5]
                for (var j = 0; j < at.length; j++) {
                    if (art.up)
                        ctx.fillRect(3, at[j], w - 6, 1.5)
                    else
                        ctx.fillRect(at[j], 3, 1.5, h - 6)
                }
            }
            ctx.restore()
        }

        Connections {
            target: art
            function onLookChanged() { cloth.requestPaint() }
            function onStyleChanged() { cloth.requestPaint() }
            function onUpChanged() { cloth.requestPaint() }
        }
    }

    // The title: up the spine top to bottom, the way an English spine reads, or across a lying
    // one. Core has already decided it fits; it only elides if the font disagrees.
    Item {
        id: title

        readonly property bool label: art.style === "label"
        readonly property color ink: label ? "#1c2b22" : art.look.foil
        // Room along the line, and across it
        readonly property real along: art.up ? art.height - 26 : art.width - 28
        readonly property real lineHeight: Math.round((art.spine.font || 15) * 1.12)

        anchors.centerIn: parent
        width: art.up ? art.height : art.width
        height: art.up ? art.width : art.height
        rotation: art.up ? 90 : 0

        Rectangle {
            id: paper

            // A paper label, gummed on by hand
            visible: title.label
            anchors.centerIn: parent
            width: Math.min(words.width, title.along - 20) + 20
            height: words.height + (art.up ? 8 : 7)
            radius: 2
            rotation: art.up ? 0.6 : -0.6
            color: "#f3ead2"
            border.width: 1
            border.color: Qt.rgba(28 / 255, 43 / 255, 34 / 255, 0.18)
            antialiasing: true

            Rectangle {
                anchors.fill: parent
                anchors.margins: 1
                radius: 2
                gradient: Gradient {
                    GradientStop { position: 0; color: Qt.rgba(1, 1, 1, 0.3) }
                    GradientStop { position: 0.45; color: Qt.rgba(1, 1, 1, 0) }
                    GradientStop { position: 1; color: Qt.rgba(150 / 255, 110 / 255, 40 / 255, 0.1) }
                }
            }

            Rectangle {
                anchors.fill: parent
                anchors.margins: 3
                color: "transparent"
                border.width: 1
                border.color: Qt.rgba(28 / 255, 43 / 255, 34 / 255, 0.28)
            }

            Rectangle {
                z: -1
                y: 1
                width: parent.width
                height: parent.height
                radius: 2
                color: Qt.rgba(0, 0, 0, 0.14)
            }
        }

        Column {
            id: words

            anchors.centerIn: parent
            anchors.verticalCenterOffset: title.label ? (art.up ? 0 : -0.5) : 0
            rotation: title.label ? paper.rotation : 0

            Repeater {
                model: art.lines

                delegate: Item {
                    id: line

                    required property string modelData

                    anchors.horizontalCenter: parent ? parent.horizontalCenter : undefined
                    width: Math.min(face.implicitWidth, title.along - (title.label ? 20 : 0))
                    height: title.lineHeight

                    // Stamped into dark cloth; a light lower lip on pale cloth
                    Text {
                        visible: !title.label
                        x: 0
                        y: art.pale ? 1 : -1
                        width: parent.width
                        height: parent.height
                        verticalAlignment: Text.AlignVCenter
                        horizontalAlignment: Text.AlignHCenter
                        text: line.modelData
                        elide: Text.ElideRight
                        font: face.font
                        color: art.pale ? Qt.rgba(1, 1, 1, 0.35) : Qt.rgba(0, 0, 0, 0.28)
                    }

                    Text {
                        id: face
                        width: parent.width
                        height: parent.height
                        verticalAlignment: Text.AlignVCenter
                        horizontalAlignment: Text.AlignHCenter
                        text: line.modelData
                        elide: Text.ElideRight
                        color: title.ink
                        font.family: Palette.fontSerif
                        font.pixelSize: art.spine.font || 15
                    }
                }
            }
        }
    }

    // The rounded back of the spine, lit from the upper left, darker at head and foot
    Canvas {
        id: shine

        anchors.fill: parent
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()

        onPaint: {
            var ctx = getContext("2d")
            var w = width
            var h = height
            ctx.reset()
            if (w <= 0 || h <= 0)
                return
            art.outline(ctx, w, h)
            ctx.clip()
            var k = "rgba(0, 0, 0, "
            var l = "rgba(255, 255, 255, "
            if (art.up) {
                // Across the back, then along it; turned, the light keeps coming from above
                var across = art.turned ? [w, 0, 0, 0] : [0, 0, w, 0]
                var along = art.turned ? [0, 0, w, 0] : [0, 0, 0, h]
                var len = art.turned ? w : h
                ctx.fillStyle = art.gradient(ctx, across[0], across[1], across[2], across[3], [
                    [0, k + "0.3)"], [0.13, l + "0.16)"], [0.36, l + "0.05)"], [0.52, l + "0)"],
                    [0.52, k + "0)"], [0.76, k + "0.1)"], [1, k + "0.36)"]])
                ctx.fillRect(0, 0, w, h)
                ctx.fillStyle = art.gradient(ctx, along[0], along[1], along[2], along[3], [
                    [0, l + "0.14)"], [8 / len, l + "0)"], [8 / len, k + "0)"],
                    [1 - 7 / len, k + "0)"], [1, k + "0.26)"]])
                ctx.fillRect(0, 0, w, h)
            } else {
                ctx.fillStyle = art.gradient(ctx, 0, 0, 0, h, [
                    [0, l + "0.2)"], [0.26, l + "0.07)"], [0.48, l + "0)"], [0.48, k + "0)"],
                    [0.74, k + "0.1)"], [1, k + "0.34)"]])
                ctx.fillRect(0, 0, w, h)
                ctx.fillStyle = art.gradient(ctx, 0, 0, w, 0, [
                    [0, k + "0.2)"], [6 / w, k + "0)"], [1 - 6 / w, k + "0)"], [1, k + "0.22)"]])
                ctx.fillRect(0, 0, w, h)
            }
        }

        Connections {
            target: art
            function onUpChanged() { shine.requestPaint() }
            function onTurnedChanged() { shine.requestPaint() }
        }
    }
}
