import QtQuick

import app.crumb.desktop 1.0

// Bowls, a board and a jar for mise en place (web components/PrepBowl.svelte), drawn on a
// Canvas from the same paths. Bigger quantity, bigger vessel; `filled` shows the ingredient
// in it, in `fillColor`. `vessel` is one of large, medium, small, ramekin, pinch, board, jar.
Canvas {
    id: bowl

    property string vessel: "medium"
    property bool filled: false
    property string fillColor: "#d8bf9a"

    readonly property var sizes: ({ "pinch": 44, "ramekin": 58, "small": 74, "medium": 94, "large": 116, "board": 112, "jar": 46 })
    readonly property real size: sizes[vessel] || 94
    readonly property real box: vessel === "jar" ? 130 : vessel === "board" ? 62 : 72
    // 0 (empty) to 1 (filled), so the fill eases in
    property real amount: filled ? 1 : 0
    readonly property var glaze: Palette.dark ? darkGlaze : lightGlaze
    readonly property var lightGlaze: ({
            "bowl": "#fbf8ef", "bowlInside": "#e4ece3", "bowlEdge": "#c9d6ca", "gloss": "rgba(255,255,255,0.8)",
            "board": "#e6c792", "boardEdge": "#cfa96f", "boardHole": "#b98f58",
            "jarGlass": "#eef3ec", "jarEdge": "#b8c8bb", "jarCap": "#2f6b4f", "jarHoles": "#e4ece3",
            "shadow": "rgba(28,43,34,0.12)"
        })
    readonly property var darkGlaze: ({
            "bowl": "#2c3d33", "bowlInside": "#1a241e", "bowlEdge": "#43584a", "gloss": "rgba(255,255,255,0.14)",
            "board": "#b3915f", "boardEdge": "#94744a", "boardHole": "#7c603c",
            "jarGlass": "#24322a", "jarEdge": "#4a5e50", "jarCap": "#3b7a5a", "jarHoles": "#1a241e",
            "shadow": "rgba(0,0,0,0.35)"
        })

    width: size
    height: size * box / 100
    antialiasing: true

    Behavior on amount {
        NumberAnimation { duration: 300; easing.type: Easing.OutBack }
    }

    onAmountChanged: requestPaint()
    onVesselChanged: requestPaint()
    onFillColorChanged: requestPaint()
    onGlazeChanged: requestPaint()
    onWidthChanged: requestPaint()

    function ellipse(ctx, cx, cy, rx, ry) {
        ctx.beginPath()
        ctx.ellipse(cx - rx, cy - ry, rx * 2, ry * 2)
    }

    function round(ctx, x, y, w, h, r) {
        ctx.beginPath()
        ctx.roundedRect(x, y, w, h, r, r)
    }

    // The fill pops in from a little smaller and lower, as the web's `.bowl-fill` does
    function pop(ctx, cx, cy) {
        var s = 0.6 + 0.4 * Math.min(1, amount)
        ctx.translate(cx, cy + 6 * (1 - amount))
        ctx.scale(s, s)
        ctx.translate(-cx, -cy)
        ctx.globalAlpha = Math.max(0, Math.min(1, amount))
    }

    function chunk(ctx, x, y, angle) {
        ctx.save()
        ctx.translate(x + 4, y + 4)
        ctx.rotate(angle * Math.PI / 180)
        ctx.translate(-x - 4, -y - 4)
        round(ctx, x, y, 9, 9, 2)
        ctx.fill()
        ctx.restore()
    }

    onPaint: {
        var ctx = getContext("2d")
        ctx.reset()
        ctx.scale(width / 100, width / 100)
        var g = glaze
        if (vessel === "board") {
            ctx.fillStyle = g.shadow
            ellipse(ctx, 50, 56, 46, 5)
            ctx.fill()
            ctx.fillStyle = g.boardEdge
            round(ctx, 4, 10, 86, 44, 10)
            ctx.fill()
            ctx.fillStyle = g.board
            round(ctx, 4, 10, 86, 40, 10)
            ctx.fill()
            ctx.fillStyle = g.boardHole
            ellipse(ctx, 84, 20, 3.5, 3.5)
            ctx.fill()
            ctx.strokeStyle = g.boardEdge
            ctx.lineWidth = 1.5
            ctx.beginPath()
            ctx.moveTo(14, 22); ctx.quadraticCurveTo(34, 19, 54, 22)
            ctx.moveTo(18, 34); ctx.quadraticCurveTo(42, 37, 64, 33)
            ctx.moveTo(12, 44); ctx.quadraticCurveTo(32, 42, 46, 45)
            ctx.stroke()
            if (amount > 0) {
                ctx.save()
                pop(ctx, 50, 32)
                ctx.fillStyle = fillColor
                chunk(ctx, 24, 24, -8)
                chunk(ctx, 37, 28, 12)
                chunk(ctx, 30, 36, 0)
                chunk(ctx, 46, 20, 20)
                chunk(ctx, 50, 33, -15)
                chunk(ctx, 62, 26, 5)
                ctx.restore()
            }
        } else if (vessel === "jar") {
            ctx.fillStyle = g.shadow
            ellipse(ctx, 50, 124, 34, 5)
            ctx.fill()
            ctx.fillStyle = g.jarGlass
            ctx.strokeStyle = g.jarEdge
            ctx.lineWidth = 3
            round(ctx, 22, 30, 56, 92, 14)
            ctx.fill()
            ctx.stroke()
            var h = 62 * Math.min(1, amount)
            if (h > 0.5) {
                ctx.fillStyle = fillColor
                round(ctx, 26, 118 - h, 48, h, 10)
                ctx.fill()
            }
            ctx.fillStyle = g.jarCap
            round(ctx, 26, 12, 48, 22, 6)
            ctx.fill()
            ctx.fillStyle = g.jarHoles
            for (var i = 0; i < 3; i++) {
                ellipse(ctx, 40 + i * 10, 22, 2.5, 2.5)
                ctx.fill()
            }
        } else {
            ctx.fillStyle = g.shadow
            ellipse(ctx, 50, 68, 30, 4)
            ctx.fill()
            ctx.fillStyle = g.bowl
            ctx.strokeStyle = g.bowlEdge
            ctx.lineWidth = 2.5
            ctx.beginPath()
            ctx.moveTo(4, 26)
            ctx.quadraticCurveTo(6, 66, 50, 66)
            ctx.quadraticCurveTo(94, 66, 96, 26)
            ctx.closePath()
            ctx.fill()
            ctx.stroke()
            ctx.fillStyle = g.bowlInside
            ellipse(ctx, 50, 26, 46, 12)
            ctx.fill()
            ctx.stroke()
            if (amount > 0) {
                ctx.save()
                pop(ctx, 50, 26)
                ctx.fillStyle = fillColor
                ellipse(ctx, 50, 27, 38, 8)
                ctx.fill()
                ctx.fillStyle = Qt.lighter(fillColor, 1.08)
                ellipse(ctx, 50, 24, 26, 5)
                ctx.fill()
                ctx.restore()
            }
            ctx.strokeStyle = g.gloss
            ctx.lineWidth = 3
            ctx.lineCap = "round"
            ctx.beginPath()
            ctx.moveTo(14, 36)
            ctx.quadraticCurveTo(20, 54, 40, 58)
            ctx.stroke()
        }
    }
}
