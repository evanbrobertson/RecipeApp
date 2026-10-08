import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Shapes

import app.crumb.desktop 1.0

// A cookbook taken off the shelf, as the web's OpenBook.svelte: it leaves its place spine
// first, turns to show its cover as it comes to the middle, and the cover swings open on a
// table of contents. Closing puts it back.
//
// The closed book is a solid (cover and spine) as deep as its spine on the shelf is wide, so at
// the start of the flight its spine sits exactly over the one on the shelf. Qt Quick has no 3D
// scene for flat items, so each face gets its own 4×4 matrix: perspective × flight × stage ×
// face, the same chain of transforms the web's CSS builds, with the depth flattened away.
//
// Call `show(book, origin)` with a shelf book {id, name, description, color, recipeCount} and the
// Bookshelf's origin {item, spine, back} (or none: it fades in); `dismissed` fires once it has
// closed and is back on the shelf.
Popup {
    id: modal

    property var book: null
    property var origin: null
    property var details: null
    property bool loading: false
    // The scrim is up
    property bool shown: false
    // The cover is swung open
    property bool isOpen: false
    property bool shutting: false
    signal dismissed()

    readonly property var look: JSON.parse(Core.bookLook(book && book.color ? book.color : ""))
    readonly property bool pale: ["sage", "butter", "cream"].indexOf(look.name) >= 0
    // The spine as the shelf drew it, or as it would have
    readonly property var spine: origin ? origin.spine : book ? JSON.parse(Core.spineFor(JSON.stringify(book))) : null
    readonly property string style: spine ? spine.style : "plain"

    // Phones: a single page, the cover swings away to show the contents
    readonly property bool phone: width <= 640
    readonly property real pageW: phone ? Math.min(width * 0.86, 380) : Math.min(380, width * 0.44)
    readonly property real stageW: phone ? pageW : pageW * 2
    readonly property real stageH: Math.min(560, height * 0.8)
    readonly property real stageX: (width - stageW) / 2
    readonly property real stageY: (height - stageH) / 2
    // The cover, page-sized with a lip, and the closed book's depth
    readonly property real coverW: pageW + 6
    readonly property real coverH: stageH + 12
    readonly property real hinge: phone ? 0 : pageW
    property real depth: 0

    readonly property real perspective: 2200
    // Closed: the cover (the right half) in the middle
    property real shift: phone || isOpen ? 0 : -pageW / 2
    property real coverAngle: isOpen ? -180 : 0
    property real coverSlide: phone && isOpen ? -8 : 0
    property real coverFade: phone && isOpen ? 0 : 1
    // Darkens as a face turns away from the light
    property real frontShade: isOpen ? 0.4 : 0
    property real insideShade: isOpen ? 0 : 0.35
    // The cover's shadow sweeping off the page as it opens
    property real pageShade: isOpen ? 0 : 1

    Behavior on shift {
        NumberAnimation { duration: modal.shutting ? 500 : 700; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.2, 0.8, 0.2, 1, 1, 1] }
    }
    Behavior on coverAngle {
        NumberAnimation {
            duration: modal.shutting ? 500 : modal.phone ? 900 : 950
            easing.type: Easing.BezierSpline
            easing.bezierCurve: modal.shutting ? [0.5, 0, 0.3, 1, 1, 1] : modal.phone ? [0.3, 0.7, 0.2, 1, 1, 1] : [0.35, 0.65, 0.2, 1, 1, 1]
        }
    }
    Behavior on coverSlide {
        NumberAnimation { duration: modal.shutting ? 500 : 900; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.3, 0.7, 0.2, 1, 1, 1] }
    }
    Behavior on coverFade {
        enabled: modal.isOpen
        SequentialAnimation {
            PauseAnimation { duration: 600 }
            NumberAnimation { duration: 300 }
        }
    }
    Behavior on frontShade {
        NumberAnimation { duration: 450; easing.type: Easing.InQuad }
    }
    Behavior on insideShade {
        SequentialAnimation {
            PauseAnimation { duration: modal.shutting ? 0 : 450 }
            NumberAnimation { duration: 500; easing.type: Easing.OutQuad }
        }
    }
    Behavior on pageShade {
        SequentialAnimation {
            PauseAnimation { duration: 250 }
            NumberAnimation { duration: 600; easing.type: Easing.OutQuad }
        }
    }

    // ─── The flight ───
    // "in" off the shelf or "out" back onto it, `clock` ms into it; `from` is where the closed
    // book starts (its spine over the shelf's), or null to fade
    property string way: "in"
    property real clock: 0
    property var from: null
    readonly property bool flying: flight.running

    function show(next, at) {
        book = next
        origin = at || null
        details = null
        loading = true
        isOpen = false
        shutting = false
        from = null
        clock = 0
        way = "in"
        open()
        requests.call("cookbook", { "id": next.id }, function (d) {
            if (modal.book && modal.book.id === next.id)
                modal.details = d
            modal.loading = false
        }, function () {
            modal.details = null
            modal.loading = false
        })
        // Once the popup has its size, put the book over its spine; the flight starts once
        // that's on screen, so the first frame's work never eats into it
        Qt.callLater(function () {
            modal.from = modal.onShelf()
            modal.shown = true
            modal.armed = true
        })
    }

    property bool armed: false

    Connections {
        target: modal.contentItem ? modal.contentItem.Window.window : null
        enabled: modal.armed
        function onFrameSwapped() {
            modal.armed = false
            modal.fly("in")
            // The cover starts to swing as the book settles, not after
            swing.restart()
        }
    }

    function dismiss() {
        if (!visible || shutting)
            return
        shutting = true
        armed = false
        swing.stop()
        if (flying)
            return // `flight` finishes, then carries on here
        shut()
    }

    function shut() {
        if (isOpen) {
            isOpen = false
            closeWait.restart()
        } else {
            flyBack()
        }
    }

    function flyBack() {
        shown = false
        from = onShelf()
        // What sat on the book lifts as it nears its place
        lift.restart()
        fly("out")
    }

    function landed() {
        lift.stop()
        if (origin && origin.back)
            origin.back()
        close()
        shutting = false
        origin = null
        from = null
        dismissed()
    }

    function fly(direction) {
        way = direction
        flight.stop()
        clock = 0
        flight.duration = from ? (direction === "in" ? 730 : 680) : 260
        flight.to = flight.duration
        flight.start()
    }

    Timer {
        id: swing
        interval: 640
        onTriggered: if (!modal.shutting) modal.isOpen = true
    }

    Timer {
        id: closeWait
        interval: 500
        onTriggered: modal.flyBack()
    }

    Timer {
        id: lift
        interval: 260
        onTriggered: if (modal.origin && modal.origin.back) modal.origin.back()
    }

    NumberAnimation {
        id: flight
        target: modal
        property: "clock"
        from: 0
        onFinished: {
            if (modal.way === "out")
                modal.landed()
            else if (modal.shutting)
                modal.shut()
        }
    }

    /**
     * Where the flight starts: the transforms that put the closed book's spine over the shelf's,
     * for a flight whose untransformed self has the closed book facing us in the middle.
     */
    function onShelf() {
        if (!origin || !origin.item || !spine)
            return null
        var item = origin.item
        var centre = item.mapToItem(scene, item.width / 2, item.height / 2)
        var along = spine.standing ? spine.h : spine.w
        var across = spine.standing ? spine.w : spine.h
        var H = coverH
        depth = H * across / along
        // The closed cover's box, before the flight moves it
        var left = stageX + (phone ? 0 : pageW / 2)
        var cx = left + coverW / 2
        var cy = stageY - 6 + coverH / 2
        var ox = width / 2
        var oy = height / 2
        // Turned to face us, the spine is half the cover's width nearer than the book's middle, so
        // the perspective draws it larger and further out; scale and place it to land exactly
        var s = along / H
        var f = 1
        for (var i = 0; i < 3; i++) {
            f = perspective / (perspective - (-depth / 2 + s * coverW / 2))
            s = along / (H * f)
        }
        return {
            "cx": cx,
            "cy": cy,
            "cz": -depth / 2,
            "tx": ox + (centre.x - ox) / f - cx,
            "ty": oy + (centre.y - oy) / f - cy,
            "s": s,
            "tilt": spine.tilt || 0,
            "lie": spine.standing ? 0 : -90
        }
    }

    // CSS's cubic-bezier() easing
    function bezier(x1, y1, x2, y2, x) {
        if (x <= 0)
            return 0
        if (x >= 1)
            return 1
        var cx = 3 * x1, bx = 3 * (x2 - x1) - cx, ax = 1 - cx - bx
        var cy = 3 * y1, by = 3 * (y2 - y1) - cy, ay = 1 - cy - by
        var t = x
        for (var i = 0; i < 8; i++) {
            var err = ((ax * t + bx) * t + cx) * t - x
            var slope = (3 * ax * t + 2 * bx) * t + cx
            if (Math.abs(err) < 1e-6 || Math.abs(slope) < 1e-6)
                break
            t -= err / slope
        }
        return ((ay * t + by) * t + cy) * t
    }

    function clamp(x) {
        return Math.max(0, Math.min(1, x))
    }

    // How far along the move (shelf → middle) and the turn the book is: the turn lags the move
    // going out, and leads it coming back, as a hand would
    readonly property real moved: way === "in"
        ? bezier(0.25, 0.8, 0.25, 1, clamp(clock / 720))
        : bezier(0.6, 0, 0.6, 1, 1 - clamp((clock - 60) / 620))
    readonly property real turned: way === "in"
        ? bezier(0.5, 0, 0.2, 1, clamp((clock - 110) / 620))
        : bezier(0.6, 0, 0.3, 1, 1 - clamp(clock / 560))
    // Nowhere to fly from (or to): fade
    readonly property real faded: way === "in" ? 1 - Math.pow(1 - clamp(clock / 260), 2) : 1 - clamp(clock / 260)

    // Cloth over board: grain and weave, light from the upper left; the front also gets the
    // hinge's groove and a two-tone book's corners
    function paintCloth(canvas, front) {
        var ctx = canvas.getContext("2d")
        var w = canvas.width
        var h = canvas.height
        var look = modal.look
        ctx.reset()
        ctx.beginPath()
        ctx.moveTo(0, 0)
        ctx.lineTo(w - 16, 0)
        ctx.arcTo(w, 0, w, 16, 16)
        ctx.lineTo(w, h - 16)
        ctx.arcTo(w, h, w - 16, h, 16)
        ctx.lineTo(0, h)
        ctx.closePath()
        ctx.clip()
        ctx.fillStyle = look.cloth
        ctx.fillRect(0, 0, w, h)
        ctx.fillStyle = "rgba(0, 0, 0, 0.05)"
        for (var x = 0; x < w; x += 3)
            ctx.fillRect(x, 0, 1, h)
        ctx.fillStyle = "rgba(255, 255, 255, 0.05)"
        for (var y = h - 1; y > -1; y -= 3)
            ctx.fillRect(0, y, w, 1)
        var grain = function (x0, y0, gw, gh) {
            if (!canvas.isImageLoaded(canvas.grainUrl))
                return
            ctx.save()
            ctx.globalCompositeOperation = "qt-soft-light"
            ctx.fillStyle = ctx.createPattern(canvas.grainUrl, "repeat")
            ctx.fillRect(x0, y0, gw, gh)
            ctx.restore()
        }
        grain(0, 0, w, h)
        var bottom = ctx.createLinearGradient(0, 0, 0, h)
        bottom.addColorStop(0.85, "rgba(0, 0, 0, 0)")
        bottom.addColorStop(1, "rgba(0, 0, 0, 0.12)")
        ctx.fillStyle = bottom
        ctx.fillRect(0, 0, w, h)
        ctx.save()
        ctx.translate(w * 0.2, 0)
        ctx.scale(1.2 * w, 0.9 * h)
        var light = ctx.createRadialGradient(0, 0, 0, 0, 0, 1)
        light.addColorStop(0, "rgba(255, 255, 255, 0.12)")
        light.addColorStop(0.6, "rgba(255, 255, 255, 0)")
        ctx.fillStyle = light
        ctx.fillRect(-1, 0, 2, 2)
        ctx.restore()
        if (!front)
            return
        var groove = ctx.createLinearGradient(0, 0, w, 0)
        groove.addColorStop(0, "rgba(0, 0, 0, 0.3)")
        groove.addColorStop(0.03, "rgba(255, 255, 255, 0.08)")
        groove.addColorStop(0.05, "rgba(0, 0, 0, 0.14)")
        groove.addColorStop(0.09, "rgba(0, 0, 0, 0)")
        ctx.fillStyle = groove
        ctx.fillRect(0, 0, w, h)
        // Two-tone books get contrasting corners
        if (modal.style === "ends") {
            var corners = [[w, 0], [w, h]]
            for (var i = 0; i < 2; i++) {
                ctx.save()
                ctx.translate(corners[i][0], corners[i][1])
                ctx.rotate(Math.PI / 4)
                ctx.fillStyle = look.bands[0]
                ctx.fillRect(-46, -46, 92, 92)
                grain(-46, -46, 92, 92)
                ctx.restore()
            }
        }
    }

    // ─── 4×4 matrices, row-major, acting on column vectors ───
    function mul(a, b) {
        var out = []
        for (var r = 0; r < 4; r++) {
            for (var c = 0; c < 4; c++) {
                var sum = 0
                for (var k = 0; k < 4; k++)
                    sum += a[r * 4 + k] * b[k * 4 + c]
                out.push(sum)
            }
        }
        return out
    }
    function chain(list) {
        var m = list[0]
        for (var i = 1; i < list.length; i++)
            m = mul(m, list[i])
        return m
    }
    function translate(x, y, z) {
        return [1, 0, 0, x, 0, 1, 0, y, 0, 0, 1, z, 0, 0, 0, 1]
    }
    function scaleXY(s) {
        return [s, 0, 0, 0, 0, s, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
    }
    function rotateY(deg) {
        var a = deg * Math.PI / 180
        var c = Math.cos(a), s = Math.sin(a)
        return [c, 0, s, 0, 0, 1, 0, 0, -s, 0, c, 0, 0, 0, 0, 1]
    }
    function rotateZ(deg) {
        var a = deg * Math.PI / 180
        var c = Math.cos(a), s = Math.sin(a)
        return [c, -s, 0, 0, s, c, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
    }
    // About a point
    function around(x, y, z, m) {
        return chain([translate(x, y, z), m, translate(-x, -y, -z)])
    }

    // The viewer, 2200px in front of the window's middle
    readonly property var eye: around(width / 2, height / 2, 0,
                                      [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, -1 / perspective, 1])
    readonly property var flightMatrix: {
        var at = from
        if (!at) {
            var k = 0.96 + 0.04 * faded
            return around(width / 2, height / 2, 0, scaleXY(k))
        }
        var m = moved
        var p
        if (m <= 0.4)
            p = [at.tx + (0.55 - 1) * at.tx * m / 0.4, at.ty + (0.55 - 1) * at.ty * m / 0.4, 120 * m / 0.4]
        else
            p = [0.55 * at.tx * (1 - (m - 0.4) / 0.6), 0.55 * at.ty * (1 - (m - 0.4) / 0.6), 120 * (1 - (m - 0.4) / 0.6)]
        var left = 1 - turned
        return around(at.cx, at.cy, at.cz, chain([
            translate(p[0], p[1], p[2]),
            scaleXY(at.s + (1 - at.s) * m),
            rotateZ(at.tilt * left),
            rotateZ(at.lie * left),
            rotateY(90 * left)
        ]))
    }
    readonly property var stageMatrix: chain([eye, flightMatrix, translate(stageX + shift, stageY, 0)])
    readonly property var coverMatrix: chain([translate(hinge, -6, 0),
                                              around(0, coverH / 2, 0, chain([rotateY(coverAngle), translate(coverSlide, 0, 0)]))])

    // A face's matrix for Qt: depth dropped once the perspective has used it
    function flat(m) {
        return Qt.matrix4x4(m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], 0, 0, 0, 0, m[12], m[13], m[14], m[15])
    }
    // Whether a face of `w`×`h` under `m` faces us (CSS's backface-visibility: hidden)
    function facing(m, w, h) {
        var project = function (x, y) {
            var wv = m[12] * x + m[13] * y + m[15]
            return [(m[0] * x + m[1] * y + m[3]) / wv, (m[4] * x + m[5] * y + m[7]) / wv]
        }
        var a = project(0, 0), b = project(w, 0), c = project(0, h)
        return (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) > 0
    }

    // var(--menu-shadow), 0 16px 28px -16px: a blur built from layered boxes, cheap to draw
    component MenuShadow: Item {
        anchors.fill: parent

        Repeater {
            model: 8

            delegate: Rectangle {
                required property int index
                readonly property real grow: -14 + index * 4

                x: 16 - grow
                y: 16 - 16 - grow
                width: parent.width - 32 + 2 * grow
                height: parent.height + 2 * grow
                radius: 16 + Math.max(0, grow)
                color: Palette.dark ? Qt.rgba(0, 0, 0, 0.7 / 6) : Qt.rgba(28 / 255, 43 / 255, 34 / 255, 0.45 / 6)
            }
        }
    }

    parent: Overlay.overlay
    x: 0
    y: 0
    width: parent ? parent.width : 900
    height: parent ? parent.height : 700
    padding: 0
    modal: true
    focus: true
    closePolicy: Popup.NoAutoClose
    enter: null
    exit: null

    // The popup sets its dimmer's own opacity, so the scrim fades inside it
    Overlay.modal: Item {
        Rectangle {
            anchors.fill: parent
            color: Qt.rgba(13 / 255, 19 / 255, 15 / 255, 0.6)
            opacity: modal.shown ? 1 : 0

            Behavior on opacity {
                NumberAnimation { duration: 400; easing.type: Easing.OutQuad }
            }
        }
    }

    background: Item {}

    Requests {
        id: requests
    }

    contentItem: Item {
        id: scene

        focus: true
        Keys.onEscapePressed: modal.dismiss()

        TapHandler {
            onTapped: modal.dismiss()
        }

        // Right page: the contents
        Item {
            id: page

            readonly property var matrix: modal.chain([modal.stageMatrix, modal.translate(modal.hinge, 0, -1)])

            width: modal.pageW
            height: modal.stageH
            opacity: modal.from ? 1 : modal.faded
            transform: Matrix4x4 { matrix: modal.flat(page.matrix) }

            // Swallow taps on the book itself
            TapHandler {}

            MenuShadow {}

            Rectangle {
                anchors.fill: parent
                color: Palette.paper
                topLeftRadius: modal.phone ? 16 : 0
                bottomLeftRadius: modal.phone ? 16 : 0
                topRightRadius: 16
                bottomRightRadius: 16
                clip: true

                // The gutter
                Rectangle {
                    visible: !modal.phone
                    width: parent.width * 0.08
                    height: parent.height
                    gradient: Gradient {
                        orientation: Gradient.Horizontal
                        GradientStop { position: 0; color: Qt.rgba(0, 0, 0, 0.08) }
                        GradientStop { position: 1; color: "transparent" }
                    }
                }

                ColumnLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 26
                    anchors.rightMargin: 26
                    anchors.topMargin: 28
                    anchors.bottomMargin: 20
                    spacing: 0

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.bottomMargin: 16
                        spacing: 8

                        Heading {
                            level: 2
                            text: "Contents"
                            Layout.fillWidth: true
                        }

                        Body {
                            text: (modal.book ? modal.book.recipeCount : 0) + " recipes"
                            muted: true
                            font.pixelSize: 13
                            font.weight: Font.DemiBold
                        }
                    }

                    ColumnLayout {
                        visible: modal.loading
                        spacing: 12
                        Layout.fillWidth: true

                        Repeater {
                            model: 5

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 16
                                radius: 12
                                color: Palette.tint
                            }
                        }
                    }

                    Body {
                        visible: !modal.loading && !(modal.details && modal.details.recipes.length)
                        text: "Blank pages, for now. Add recipes from any recipe page or with “Select” on the recipes list."
                        muted: true
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }

                    ListView {
                        id: toc

                        visible: !modal.loading && modal.details && modal.details.recipes.length > 0
                        model: modal.details ? modal.details.recipes : []
                        clip: true
                        boundsBehavior: Flickable.StopAtBounds
                        ScrollBar.vertical: ScrollBar {}
                        Layout.fillWidth: true
                        Layout.fillHeight: true

                        delegate: Item {
                            id: entry

                            required property var modelData
                            required property int index

                            width: toc.width - 8
                            height: 44

                            // The contents rise in, one after another, as the cover opens
                            function rise() {
                                if (modal.isOpen)
                                    rising.restart()
                            }
                            Component.onCompleted: rise()
                            Connections {
                                target: modal
                                function onIsOpenChanged() { entry.rise() }
                            }

                            SequentialAnimation {
                                id: rising
                                PropertyAction { target: row; property: "opacity"; value: 0 }
                                PropertyAction { target: row; property: "y"; value: 6 }
                                PauseAnimation { duration: 250 + Math.min(entry.index, 12) * 40 }
                                ParallelAnimation {
                                    NumberAnimation { target: row; property: "opacity"; to: 1; duration: 400; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.2, 0.8, 0.2, 1, 1, 1] }
                                    NumberAnimation { target: row; property: "y"; to: 0; duration: 400; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.2, 0.8, 0.2, 1, 1, 1] }
                                }
                            }

                            HoverHandler {
                                id: entryHover
                                cursorShape: Qt.PointingHandCursor
                            }

                            TapHandler {
                                onTapped: {
                                    // Before closing: `dismissed` may take this delegate with it
                                    const window = entry.ApplicationWindow.window
                                    const id = entry.modelData.id
                                    modal.close()
                                    modal.dismissed()
                                    window.go("recipe", { "id": id })
                                }
                            }

                            RowLayout {
                                id: row

                                width: parent.width
                                height: parent.height
                                spacing: 6

                                Text {
                                    text: entry.modelData.title
                                    color: entryHover.hovered ? Palette.primary : Palette.text
                                    font.family: Palette.fontSerif
                                    font.pixelSize: 16
                                    elide: Text.ElideRight
                                    Layout.maximumWidth: entry.width - 60
                                }

                                // The dotted leader
                                Shape {
                                    Layout.fillWidth: true
                                    Layout.minimumWidth: 16
                                    Layout.preferredHeight: 2
                                    Layout.alignment: Qt.AlignBottom
                                    Layout.bottomMargin: 14
                                    ShapePath {
                                        strokeWidth: 2
                                        strokeColor: Palette.line
                                        strokeStyle: ShapePath.DashLine
                                        dashPattern: [1, 1.5]
                                        capStyle: ShapePath.RoundCap
                                        startX: 0
                                        startY: 1
                                        PathLine { x: 400; y: 1 }
                                    }
                                }

                                Body {
                                    text: entry.index + 1
                                    muted: true
                                    font.pixelSize: 14
                                    Layout.alignment: Qt.AlignBottom
                                    Layout.bottomMargin: 10
                                }
                            }
                        }
                    }

                    Item {
                        Layout.fillHeight: true
                        visible: !toc.visible
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.topMargin: 16

                        Item {
                            Layout.fillWidth: true
                        }

                        CrumbButton {
                            kind: "soft"
                            text: "Open cookbook"
                            iconName: "arrow-right"
                            onClicked: {
                                var id = modal.book.id
                                modal.close()
                                modal.dismissed()
                                ApplicationWindow.window.go("cookbook", { "id": id })
                            }
                        }
                    }
                }

                // The cover's shadow, sweeping off as it opens
                Rectangle {
                    anchors.fill: parent
                    opacity: modal.pageShade
                    gradient: Gradient {
                        orientation: Gradient.Horizontal
                        GradientStop { position: 0.3; color: Qt.rgba(20 / 255, 30 / 255, 24 / 255, 0) }
                        GradientStop { position: 1; color: Qt.rgba(20 / 255, 30 / 255, 24 / 255, 0.28) }
                    }
                }
            }
        }

        // The spine, square to the cover at the hinge: only seen while the book turns
        Item {
            id: spineFace

            readonly property var matrix: modal.chain([modal.stageMatrix, modal.translate(modal.hinge - modal.depth, -6, 0),
                                                       modal.around(modal.depth, modal.coverH / 2, 0, modal.rotateY(-90))])

            visible: !!modal.from && modal.depth > 0 && modal.facing(matrix, width, height)
            width: modal.depth
            height: modal.coverH
            transform: Matrix4x4 { matrix: modal.flat(spineFace.matrix) }

            BookSpine {
                spine: modal.spine || ({})
                color: modal.book && modal.book.color ? modal.book.color : ""
                turned: !!modal.spine && !modal.spine.standing
                transformOrigin: Item.TopLeft
                scale: modal.spine ? modal.coverH / (modal.spine.standing ? modal.spine.h : modal.spine.w) : 1
            }
        }

        // The cover: endpaper inside, front outside
        Item {
            id: inside

            readonly property var matrix: modal.chain([modal.stageMatrix, modal.coverMatrix,
                                                       modal.around(modal.coverW / 2, modal.coverH / 2, 0, modal.rotateY(180))])

            visible: modal.facing(matrix, width, height)
            width: modal.coverW
            height: modal.coverH
            opacity: modal.coverFade * (modal.from ? 1 : modal.faded)
            transform: Matrix4x4 { matrix: modal.flat(inside.matrix) }

            // Endpaper: paper faintly striped with the cover colour
            Canvas {
                id: endpaper

                anchors.fill: parent
                onWidthChanged: requestPaint()
                onHeightChanged: requestPaint()
                onPaint: {
                    var ctx = getContext("2d")
                    var w = width
                    var h = height
                    var cloth = Qt.color(modal.look.cloth)
                    var paper = Palette.paper
                    var tint = function (k) {
                        return Qt.rgba(cloth.r * k + paper.r * (1 - k), cloth.g * k + paper.g * (1 - k),
                                       cloth.b * k + paper.b * (1 - k), 1).toString()
                    }
                    ctx.reset()
                    ctx.beginPath()
                    ctx.moveTo(16, 0)
                    ctx.lineTo(w, 0)
                    ctx.lineTo(w, h)
                    ctx.lineTo(16, h)
                    ctx.arcTo(0, h, 0, h - 16, 16)
                    ctx.lineTo(0, 16)
                    ctx.arcTo(0, 0, 16, 0, 16)
                    ctx.closePath()
                    ctx.clip()
                    ctx.fillStyle = tint(0.07)
                    ctx.fillRect(0, 0, w, h)
                    // 135° bands, 10px each
                    ctx.fillStyle = tint(0.12)
                    var step = 20 * Math.SQRT2
                    for (var x = -h; x < w + h; x += step) {
                        ctx.beginPath()
                        ctx.moveTo(x, 0)
                        ctx.lineTo(x + step / 2, 0)
                        ctx.lineTo(x + step / 2 - h, h)
                        ctx.lineTo(x - h, h)
                        ctx.closePath()
                        ctx.fill()
                    }
                }

                Connections {
                    target: modal
                    function onLookChanged() { endpaper.requestPaint() }
                }
                Connections {
                    target: Palette
                    function onPaperChanged() { endpaper.requestPaint() }
                }
            }

            Rectangle {
                anchors.fill: parent
                topLeftRadius: 16
                bottomLeftRadius: 16
                color: "transparent"
                border.width: 1
                border.color: Palette.line
            }

            ColumnLayout {
                anchors.fill: parent
                anchors.leftMargin: 30
                anchors.rightMargin: 30
                anchors.topMargin: 34
                anchors.bottomMargin: 34
                spacing: 0

                Text {
                    text: "EX LIBRIS"
                    color: Palette.textMuted
                    font.family: Palette.fontSans
                    font.pixelSize: 13
                    font.weight: Font.Bold
                    font.letterSpacing: 3.9
                }

                Text {
                    text: modal.book ? modal.book.name : ""
                    color: Palette.text
                    font.family: Palette.fontSerif
                    font.pixelSize: 24
                    lineHeight: 1.1
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                    Layout.topMargin: 12
                }

                Body {
                    visible: !!modal.book && !!modal.book.description
                    text: modal.book && modal.book.description ? modal.book.description : ""
                    font.pixelSize: 14
                    Layout.fillWidth: true
                    Layout.topMargin: 12
                }

                Item {
                    Layout.fillHeight: true
                }

                Body {
                    text: "A cookbook of " + (modal.book ? modal.book.recipeCount : 0) + " recipes"
                    muted: true
                    font.pixelSize: 13
                    font.weight: Font.DemiBold
                    Layout.fillWidth: true
                }
            }

            Rectangle {
                anchors.fill: parent
                topLeftRadius: 16
                bottomLeftRadius: 16
                color: Qt.rgba(10 / 255, 16 / 255, 12 / 255, 1)
                opacity: modal.insideShade
            }

            // Shut, this face is the book's underside or back: plain cloth, not the endpaper
            Canvas {
                id: back

                readonly property string grainUrl: "qrc:/shelf/grain.png"

                visible: modal.coverAngle > -90
                anchors.fill: parent
                transform: Scale { origin.x: back.width / 2; xScale: -1 }
                Component.onCompleted: loadImage(grainUrl)
                onImageLoaded: requestPaint()
                onWidthChanged: requestPaint()
                onHeightChanged: requestPaint()
                onPaint: modal.paintCloth(back, false)

                Connections {
                    target: modal
                    function onLookChanged() { back.requestPaint() }
                }
            }
        }

        Item {
            id: front

            readonly property var matrix: modal.chain([modal.stageMatrix, modal.coverMatrix])

            visible: modal.facing(matrix, width, height)
            width: modal.coverW
            height: modal.coverH
            opacity: modal.coverFade * (modal.from ? 1 : modal.faded)
            transform: Matrix4x4 { matrix: modal.flat(front.matrix) }

            MenuShadow {}

            // Cloth over board: grain and weave, the hinge's groove, light from the upper left
            Canvas {
                id: cloth

                readonly property string grainUrl: "qrc:/shelf/grain.png"

                anchors.fill: parent
                Component.onCompleted: loadImage(grainUrl)
                onImageLoaded: requestPaint()
                onWidthChanged: requestPaint()
                onHeightChanged: requestPaint()

                onPaint: modal.paintCloth(cloth, true)

                Connections {
                    target: modal
                    function onLookChanged() { cloth.requestPaint() }
                    function onStyleChanged() { cloth.requestPaint() }
                }
            }

            Rectangle {
                anchors.fill: parent
                topRightRadius: 16
                bottomRightRadius: 16
                color: "transparent"
                border.width: 1
                border.color: modal.look.edge ? Qt.rgba(28 / 255, 43 / 255, 34 / 255, modal.look.edgeAlpha) : Qt.rgba(0, 0, 0, 0.12)
            }

            // The foil frame, doubled
            Item {
                id: frame

                readonly property color ink: modal.style === "rules" ? modal.look.bands[0] : modal.look.foil

                anchors.fill: parent
                anchors.margins: 28

                Rectangle {
                    anchors.fill: parent
                    color: "transparent"
                    radius: 3
                    border.width: 1.5
                    border.color: frame.ink
                }

                Rectangle {
                    anchors.fill: parent
                    anchors.margins: -6.5
                    color: "transparent"
                    radius: 6
                    border.width: 1
                    border.color: frame.ink
                }

                ColumnLayout {
                    anchors.centerIn: parent
                    width: parent.width - 40
                    spacing: 14

                    Icon {
                        name: "chef-hat"
                        size: 32
                        color: modal.look.foil
                        Layout.alignment: Qt.AlignHCenter
                    }

                    Item {
                        Layout.fillWidth: true
                        Layout.preferredHeight: coverTitle.height

                        // Labelled books carry the title on a paper label on the cover too
                        Rectangle {
                            visible: modal.style === "label"
                            anchors.centerIn: coverTitle
                            width: Math.min(parent.width, coverTitle.contentWidth + 36)
                            height: coverTitle.height + 28
                            radius: 3
                            rotation: -0.8
                            color: "#f3ead2"
                            border.width: 1
                            border.color: Qt.rgba(28 / 255, 43 / 255, 34 / 255, 0.18)
                            antialiasing: true

                            Rectangle {
                                anchors.fill: parent
                                anchors.margins: 4
                                color: "transparent"
                                border.width: 1
                                border.color: Qt.rgba(28 / 255, 43 / 255, 34 / 255, 0.28)
                            }
                        }

                        // Stamped into dark cloth; a light lower lip on pale cloth
                        Text {
                            visible: modal.style !== "label"
                            x: coverTitle.x
                            y: coverTitle.y + (modal.pale ? 1 : -1)
                            width: coverTitle.width
                            text: coverTitle.text
                            font: coverTitle.font
                            lineHeight: coverTitle.lineHeight
                            wrapMode: coverTitle.wrapMode
                            horizontalAlignment: Text.AlignHCenter
                            color: modal.pale ? Qt.rgba(1, 1, 1, 0.4) : Qt.rgba(0, 0, 0, 0.3)
                        }

                        Text {
                            id: coverTitle

                            width: parent.width - (modal.style === "label" ? 36 : 0)
                            anchors.horizontalCenter: parent.horizontalCenter
                            rotation: modal.style === "label" ? -0.8 : 0
                            text: modal.book ? modal.book.name : ""
                            color: modal.style === "label" ? "#1c2b22" : modal.look.foil
                            font.family: Palette.fontSerif
                            font.pixelSize: modal.phone ? 26 : 30
                            lineHeight: 1.15
                            wrapMode: Text.WordWrap
                            horizontalAlignment: Text.AlignHCenter
                        }
                    }

                    Rectangle {
                        color: modal.look.foil
                        Layout.preferredWidth: parent.width * 0.4
                        Layout.preferredHeight: 1
                        Layout.alignment: Qt.AlignHCenter
                    }

                    Text {
                        text: "RECIPES"
                        color: modal.look.foil
                        font.family: Palette.fontSans
                        font.pixelSize: 13
                        font.weight: Font.Bold
                        font.letterSpacing: 2.6
                        Layout.alignment: Qt.AlignHCenter
                    }
                }
            }

            Rectangle {
                anchors.fill: parent
                topRightRadius: 16
                bottomRightRadius: 16
                color: Qt.rgba(10 / 255, 16 / 255, 12 / 255, 1)
                opacity: modal.frontShade
            }
        }

        Rectangle {
            id: closeButton

            readonly property var matrix: modal.chain([modal.stageMatrix, modal.translate(modal.stageW - 44, -52, 0)])

            width: 44
            height: 44
            radius: 22
            opacity: modal.isOpen ? 1 : 0
            color: closeHover.hovered ? Qt.rgba(1, 253 / 255, 248 / 255, 0.26) : Qt.rgba(1, 253 / 255, 248 / 255, 0.16)
            transform: Matrix4x4 { matrix: modal.flat(closeButton.matrix) }
            activeFocusOnTab: true
            Accessible.role: Accessible.Button
            Accessible.name: "Close book"
            Accessible.onPressAction: modal.dismiss()
            Keys.onReturnPressed: modal.dismiss()
            Keys.onSpacePressed: modal.dismiss()

            Behavior on opacity {
                NumberAnimation { duration: 300; easing.type: Easing.OutQuad }
            }

            HoverHandler {
                id: closeHover
                cursorShape: Qt.PointingHandCursor
            }

            TapHandler {
                onTapped: modal.dismiss()
            }

            Icon {
                anchors.centerIn: parent
                name: "x"
                size: 22
                color: "#fffdf8"
            }
        }
    }
}
