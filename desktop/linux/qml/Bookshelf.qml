import QtQuick
import QtQuick.Shapes

import app.crumb.desktop 1.0

// The shelf, as the web's Bookshelf.svelte: a tiled wall, painted planks on brackets, books
// standing in runs and lying in stacks, a pot of basil, a crock of spoons and (when `addable`)
// a dashed outline of a new book. Where everything goes comes from `Core.shelfLayout`; this only
// draws it and moves it. The wall and planks take the theme's colours, the books keep their own.
Item {
    id: shelf

    // [{id, name, color, recipeCount}]
    property var books: []
    // The book that's off the shelf (open): its place stays empty until it's back
    property int pulledId: 0
    property bool addable: false
    // One shelf that scrolls sideways (Home) instead of more shelves
    property bool single: false
    // `origin` is {item, spine, back}: the spine on the shelf, core's placed spine, and a call
    // for when the book is nearly back (what lay on it lifts again)
    signal openBook(var book, var origin)
    signal add()

    readonly property var metrics: JSON.parse(Core.shelfMetrics())
    // Room above the first shelf's tallest book
    readonly property real wallTop: 14
    readonly property var rows: width > 0 || single
        ? JSON.parse(Core.shelfLayout(JSON.stringify(books), width, single, addable)) : []
    readonly property real wallHeight: wallTop + rows.length * metrics.rowHeight
    readonly property real wallWidth: single
        ? Math.max(width, rows.reduce(function (w, r) { return Math.max(w, r.width) }, 0)) : width

    // Books glide to new places on a resize or a new book, but not into their first ones
    property bool ready: false
    // The book under the pointer or keyboard focus: a pot on it slides out with it
    property int hoverId: 0
    // A book back from being open settles into its place
    property int landedId: 0
    property int lastPulled: 0
    // The book whose place in a stack is empty: what lay on it settles onto the book below once
    // it has gone, and lifts again as it comes back
    property int gap: 0
    readonly property var drops: dropsFor(rows, gap)

    implicitHeight: wallHeight

    function itemKey(item, ri) {
        return item.kind === "book" ? "b" + item.book.id : item.kind + "-" + ri
    }

    function dropsFor(rows, gap) {
        var out = {}
        if (!gap)
            return out
        for (var ri = 0; ri < rows.length; ri++) {
            var items = rows[ri].items
            var g = null
            for (var i = 0; i < items.length; i++) {
                if (items[i].kind === "book" && items[i].book.id === gap)
                    g = items[i]
            }
            if (!g || g.stack === null)
                continue
            for (var j = 0; j < items.length; j++) {
                if (items[j].stack === g.stack && items[j].y > g.y)
                    out[itemKey(items[j], ri)] = g.h
            }
        }
        return out
    }

    // A hovered standing book's standing neighbours give way a hair: -1 the one before, 1 after
    function tipFor(ri, index) {
        if (!hoverId || ri >= rows.length)
            return 0
        var items = rows[ri].items
        var standing = function (it) { return it && it.kind === "book" && it.book.standing }
        if (!standing(items[index]))
            return 0
        if (standing(items[index + 1]) && items[index + 1].book.id === hoverId)
            return -1
        if (standing(items[index - 1]) && items[index - 1].book.id === hoverId)
            return 1
        return 0
    }

    // Keeps one delegate per thing, keyed, so a book that moves glides instead of reappearing
    function sync() {
        var want = []
        var keys = {}
        for (var ri = 0; ri < rows.length; ri++) {
            var items = rows[ri].items
            for (var i = 0; i < items.length; i++) {
                var it = items[i]
                var key = itemKey(it, ri)
                keys[key] = true
                want.push({ "key": key, "kind": it.kind, "row": ri, "slot": i, "px": it.x, "py": it.y,
                            "pw": it.w, "ph": it.h, "pz": it.z, "json": JSON.stringify(it) })
            }
        }
        for (var r = placed.count - 1; r >= 0; r--) {
            if (!keys[placed.get(r).key])
                placed.remove(r)
        }
        for (var n = 0; n < want.length; n++) {
            var at = -1
            for (var m = n; m < placed.count; m++) {
                if (placed.get(m).key === want[n].key) {
                    at = m
                    break
                }
            }
            if (at < 0) {
                placed.insert(n, want[n])
            } else {
                if (at !== n)
                    placed.move(at, n, 1)
                placed.set(n, want[n])
            }
        }
        if (!ready && placed.count)
            readyTimer.start()
    }

    onRowsChanged: sync()
    Component.onCompleted: sync()

    onPulledIdChanged: {
        if (lastPulled && !pulledId) {
            landedId = lastPulled
            landedTimer.restart()
        }
        lastPulled = pulledId
        if (pulledId) {
            gapTimer.restart()
        } else {
            gapTimer.stop()
            gap = 0
        }
    }

    ListModel {
        id: placed
    }

    Timer {
        id: readyTimer
        interval: 50
        onTriggered: shelf.ready = true
    }

    Timer {
        id: gapTimer
        interval: 180
        onTriggered: shelf.gap = shelf.pulledId
    }

    Timer {
        id: landedTimer
        interval: 600
        onTriggered: shelf.landedId = 0
    }

    // The colours the wall and planks take from the theme
    function mix(a, b, w) {
        a = Qt.color(a)
        b = Qt.color(b)
        return Qt.rgba(a.r * w + b.r * (1 - w), a.g * w + b.g * (1 - w), a.b * w + b.b * (1 - w), 1)
    }

    // Soft shadows are painted (Canvas shadows), so they look the same with or without a GPU:
    // `draw(ctx)` fills the thing's own shape, which the thing then covers
    readonly property string shadowColor: "rgba(20, 32, 25, %1)"
    readonly property color wall: Palette.tint
    readonly property color grout: Palette.dark ? mix(Palette.tint, "black", 0.7) : mix(Palette.tint, Palette.text, 0.84)
    readonly property color plank: Palette.tile

    Flickable {
        id: flick

        width: shelf.width
        height: shelf.wallHeight
        contentWidth: shelf.wallWidth
        contentHeight: shelf.wallHeight
        interactive: shelf.single && contentWidth > width
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: Flickable.HorizontalFlick
        clip: shelf.single

        Item {
            id: wallItem

            width: shelf.wallWidth
            height: shelf.wallHeight

            // A glossy tiled backsplash, quieter than the brand tile
            Canvas {
                id: tiles

                anchors.fill: parent
                onWidthChanged: requestPaint()
                onHeightChanged: requestPaint()

                onPaint: {
                    var ctx = getContext("2d")
                    ctx.reset()
                    var size = 58
                    // One tile, then the wall papered with it
                    ctx.fillStyle = shelf.wall
                    ctx.fillRect(0, 0, size, size)
                    var dark = ctx.createLinearGradient(0, 0, size, size)
                    dark.addColorStop(0.7, "rgba(0, 0, 0, 0)")
                    dark.addColorStop(1, Palette.dark ? "rgba(0, 0, 0, 0.08)" : "rgba(0, 0, 0, 0.03)")
                    ctx.fillStyle = dark
                    ctx.fillRect(0, 0, size, size)
                    var gloss = ctx.createRadialGradient(0, 0, 0, 0, 0, size * 1.3 * 0.42)
                    gloss.addColorStop(0, Palette.dark ? "rgba(255, 255, 255, 0.07)" : "rgba(255, 255, 255, 0.45)")
                    gloss.addColorStop(1, "rgba(255, 255, 255, 0)")
                    ctx.fillStyle = gloss
                    ctx.fillRect(0, 0, size, size)
                    ctx.fillStyle = shelf.grout
                    ctx.fillRect(0, 0, size, 2)
                    ctx.fillRect(0, 0, 2, size)
                    var tile = ctx.getImageData(0, 0, size, size)
                    ctx.fillStyle = ctx.createPattern(tile, "repeat")
                    ctx.translate(-1, -1)
                    ctx.fillRect(0, 0, width + 1, height + 1)
                }

                Connections {
                    target: Palette
                    function onTintChanged() { tiles.requestPaint() }
                    function onTextChanged() { tiles.requestPaint() }
                    function onDarkChanged() { tiles.requestPaint() }
                }
            }

            // The planks run to the wall's edges
            Repeater {
                model: shelf.rows.length

                delegate: Item {
                    id: plankRow

                    required property int index

                    y: shelf.wallTop + index * shelf.metrics.rowHeight + shelf.metrics.clearance
                    width: wallItem.width
                    height: shelf.metrics.plankTop + shelf.metrics.plankFront

                    // Its shadow on the tiles
                    Rectangle {
                        y: parent.height
                        width: parent.width
                        height: 18
                        gradient: Gradient {
                            GradientStop { position: 0; color: Qt.rgba(20 / 255, 32 / 255, 25 / 255, Palette.dark ? 0.3 : 0.22) }
                            GradientStop { position: 1; color: Qt.rgba(20 / 255, 32 / 255, 25 / 255, 0) }
                        }
                    }

                    Repeater {
                        model: 2

                        // Stepped brackets under the plank, 9% in from each end
                        delegate: Item {
                            required property int index

                            x: index === 0 ? plankRow.width * 0.09 : plankRow.width * 0.91 - width
                            y: plankRow.height
                            width: 16
                            height: 24
                            Canvas {
                                id: corbel

                                x: -8
                                y: -8
                                width: 36
                                height: 44
                                onPaint: {
                                    var ctx = getContext("2d")
                                    ctx.reset()
                                    ctx.translate(8, 8)
                                    ctx.shadowColor = shelf.shadowColor.arg(0.25)
                                    ctx.shadowOffsetX = 3
                                    ctx.shadowOffsetY = 4
                                    ctx.shadowBlur = 3
                                    ctx.fillStyle = shelf.mix(shelf.plank, "black", 0.82)
                                    ctx.beginPath()
                                    ctx.moveTo(0, 0)
                                    ctx.lineTo(16, 0)
                                    ctx.lineTo(16, 7)
                                    ctx.lineTo(14, 7)
                                    ctx.lineTo(14, 12)
                                    ctx.lineTo(12, 12)
                                    ctx.lineTo(12, 17)
                                    ctx.arc(8, 17, 4, 0, Math.PI, false)
                                    ctx.lineTo(4, 12)
                                    ctx.lineTo(2, 12)
                                    ctx.lineTo(2, 7)
                                    ctx.lineTo(0, 7)
                                    ctx.closePath()
                                    ctx.fill()
                                }

                                Connections {
                                    target: Palette
                                    function onTileChanged() { corbel.requestPaint() }
                                }
                            }

                            Rectangle {
                                width: 16
                                height: 2
                                color: Qt.rgba(1, 1, 1, 0.12)
                            }
                        }
                    }

                    // A painted plank: its top face catching the light, then the front edge
                    Canvas {
                        id: plankPaint

                        readonly property string grainUrl: "qrc:/shelf/grain.png"

                        anchors.fill: parent
                        Component.onCompleted: loadImage(grainUrl)
                        onImageLoaded: requestPaint()
                        onWidthChanged: requestPaint()

                        onPaint: {
                            var ctx = getContext("2d")
                            var w = width
                            var h = height
                            var p = shelf.plank
                            var at = function (c) { return Qt.rgba(c.r, c.g, c.b, 1).toString() }
                            ctx.reset()
                            ctx.fillStyle = at(p)
                            ctx.fillRect(0, 0, w, h)
                            if (isImageLoaded(grainUrl)) {
                                ctx.save()
                                ctx.globalCompositeOperation = "qt-soft-light"
                                ctx.fillStyle = ctx.createPattern(grainUrl, "repeat")
                                ctx.fillRect(0, 0, w, h)
                                ctx.restore()
                            }
                            var g = ctx.createLinearGradient(0, 0, 0, h)
                            g.addColorStop(0, at(shelf.mix(p, "black", 0.7)))
                            g.addColorStop(7 / h, at(shelf.mix(p, "white", 0.78)))
                            g.addColorStop(8 / h, at(shelf.mix(p, "white", 0.9)))
                            g.addColorStop(8.01 / h, at(shelf.mix(p, "white", 0.7)))
                            g.addColorStop(9 / h, Qt.rgba(p.r, p.g, p.b, 0).toString())
                            g.addColorStop(1 - 3 / h, Qt.rgba(p.r, p.g, p.b, 0).toString())
                            g.addColorStop(1, at(shelf.mix(p, "black", 0.75)))
                            ctx.fillStyle = g
                            ctx.fillRect(0, 0, w, h)
                        }

                        Connections {
                            target: Palette
                            function onTileChanged() { plankPaint.requestPaint() }
                        }
                    }
                }
            }

            Repeater {
                model: placed

                delegate: Item {
                    id: thing

                    required property string key
                    required property string kind
                    required property int row
                    required property int slot
                    required property real px
                    required property real py
                    required property real pw
                    required property real ph
                    required property int pz
                    required property string json
                    readonly property var item: JSON.parse(json)
                    readonly property real drop: shelf.drops[key] || 0

                    x: px
                    y: shelf.wallTop + row * shelf.metrics.rowHeight + shelf.metrics.clearance + 4 - py - ph
                    width: pw
                    height: ph
                    z: pz + 1

                    Behavior on x {
                        enabled: shelf.ready
                        NumberAnimation { duration: 600; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.2, 0.8, 0.2, 1, 1, 1] }
                    }
                    Behavior on y {
                        enabled: shelf.ready
                        NumberAnimation { duration: 600; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.2, 0.8, 0.2, 1, 1, 1] }
                    }

                    Loader {
                        anchors.fill: parent
                        sourceComponent: thing.kind === "book" ? bookThing
                            : thing.kind === "add" ? addThing : propThing
                    }

                    Component {
                        id: bookThing

                        Item {
                            id: book

                            readonly property var spine: thing.item.book
                            readonly property var info: shelf.books[spine.index] || ({})
                            readonly property bool pulled: shelf.pulledId === spine.id
                            readonly property bool lit: hover.hovered || keyboard
                            readonly property bool keyboard: activeFocus
                                && (focusReason === Qt.TabFocusReason || focusReason === Qt.BacktabFocusReason)
                            readonly property bool pressed: tap.pressed
                            readonly property int tip: shelf.tipFor(thing.row, thing.slot)

                            function open() {
                                shelf.openBook(book.info, {
                                    "item": spineArt,
                                    "spine": book.spine,
                                    "back": function () { shelf.gap = 0 }
                                })
                            }

                            // Off the shelf: the open book is drawn by BookOpenModal
                            visible: !pulled
                            activeFocusOnTab: true
                            Accessible.role: Accessible.Button
                            Accessible.name: "Open " + (info.name || "") + ", " + (info.recipeCount || 0) + " recipes"
                            Accessible.onPressAction: open()
                            Keys.onReturnPressed: open()
                            Keys.onSpacePressed: open()

                            onLitChanged: {
                                if (lit)
                                    shelf.hoverId = spine.id
                                else if (shelf.hoverId === spine.id)
                                    shelf.hoverId = 0
                            }

                            HoverHandler {
                                id: hover
                                blocking: true
                                cursorShape: Qt.PointingHandCursor
                            }

                            TapHandler {
                                id: tap
                                onTapped: book.open()
                            }

                            Item {
                                id: lean

                                width: parent.width
                                height: parent.height
                                transform: [
                                    Rotation {
                                        origin.x: book.spine.pivot === "left" ? 0 : book.spine.pivot === "right" ? lean.width : lean.width / 2
                                        origin.y: book.spine.pivot === "center" ? lean.height / 2 : lean.height
                                        angle: book.spine.tilt
                                    },
                                    // Settles onto the book below when the one under it is taken
                                    Translate {
                                        y: thing.drop
                                        Behavior on y {
                                            NumberAnimation { duration: 450; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.5, 0, 0.55, 1.35, 1, 1] }
                                        }
                                    }
                                ]

                                Item {
                                    id: lift

                                    // Standing: up and out of the row, as if a finger tipped it by its
                                    // head. Lying: slid out of the stack, towards the reader's hand.
                                    readonly property bool up: book.spine.standing
                                    readonly property int speed: book.pressed ? 120 : 500
                                    property real settle: 0

                                    width: parent.width
                                    height: parent.height
                                    transform: [
                                        // Its neighbours give way
                                        Rotation {
                                            origin.x: 0
                                            origin.y: lift.height
                                            angle: book.tip < 0 ? -1.2 : 0
                                            Behavior on angle { NumberAnimation { duration: 500; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.3, 1.5, 0.5, 1, 1, 1] } }
                                        },
                                        Rotation {
                                            origin.x: lift.width
                                            origin.y: lift.height
                                            angle: book.tip > 0 ? 1.2 : 0
                                            Behavior on angle { NumberAnimation { duration: 500; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.3, 1.5, 0.5, 1, 1, 1] } }
                                        },
                                        Scale {
                                            origin.x: lift.width / 2
                                            origin.y: lift.height / 2
                                            xScale: book.pressed ? 0.98 : 1
                                            yScale: xScale
                                            Behavior on xScale { NumberAnimation { duration: lift.speed; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.3, 1.5, 0.5, 1, 1, 1] } }
                                        },
                                        Translate {
                                            x: lift.up ? 0 : book.pressed ? 10 : book.lit ? 16 : 0
                                            y: lift.settle + (lift.up ? (book.pressed ? -6 : book.lit ? -14 : 0) : (book.lit && !book.pressed ? -1 : 0))
                                            Behavior on x { NumberAnimation { duration: lift.speed; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.3, 1.5, 0.5, 1, 1, 1] } }
                                            Behavior on y {
                                                enabled: !landing.running
                                                NumberAnimation { duration: lift.speed; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.3, 1.5, 0.5, 1, 1, 1] }
                                            }
                                        }
                                    ]

                                    // Back from being open: a small settle into its place
                                    NumberAnimation {
                                        id: landing
                                        target: lift
                                        property: "settle"
                                        from: -8
                                        to: 0
                                        duration: 500
                                        easing.type: Easing.BezierSpline
                                        easing.bezierCurve: [0.3, 1.5, 0.5, 1, 1, 1]
                                    }

                                    Connections {
                                        target: shelf
                                        function onLandedIdChanged() {
                                            if (shelf.landedId === book.spine.id)
                                                landing.restart()
                                        }
                                    }

                                    // Its shadow, at rest and lifted
                                    Repeater {
                                        model: [
                                            { "x": 3, "y": 3, "blur": 3, "alpha": 0.28 },
                                            lift.up ? { "x": 6, "y": 10, "blur": 8, "alpha": 0.3 } : { "x": 6, "y": 6, "blur": 6, "alpha": 0.3 }
                                        ]

                                        delegate: Canvas {
                                            required property var modelData
                                            required property int index

                                            readonly property real pad: 24

                                            x: -pad
                                            y: -pad
                                            width: lift.width + 2 * pad
                                            height: lift.height + 2 * pad
                                            opacity: (index === 1) === book.lit ? 1 : 0
                                            Behavior on opacity { NumberAnimation { duration: 300; easing.type: Easing.OutQuad } }
                                            onPaint: {
                                                var ctx = getContext("2d")
                                                ctx.reset()
                                                ctx.translate(pad, pad)
                                                ctx.shadowColor = shelf.shadowColor.arg(modelData.alpha)
                                                ctx.shadowOffsetX = modelData.x
                                                ctx.shadowOffsetY = modelData.y
                                                ctx.shadowBlur = modelData.blur
                                                ctx.fillStyle = shelf.shadowColor.arg(modelData.alpha)
                                                var face = book.spine.top ? shelf.metrics.topFace : 0
                                                spineArt.outline(ctx, lift.width, lift.height)
                                                ctx.fill()
                                                if (face) {
                                                    var inset = lift.up ? 1 : 2
                                                    ctx.fillRect(inset, -face, lift.width - 2 * inset, face + 3)
                                                }
                                            }
                                        }
                                    }

                                    // The top edge, seen from a little above: pages between boards, or a
                                    // lying book's cover
                                    Rectangle {
                                        visible: book.spine.top
                                        x: lift.up ? 1 : 2
                                        y: -shelf.metrics.topFace
                                        width: lift.width - 2 * x
                                        height: shelf.metrics.topFace
                                        topLeftRadius: 2
                                        topRightRadius: 2
                                        color: lift.up ? "#f4eedd" : Qt.tint(spineArt.look.cloth, Qt.rgba(1, 1, 1, 0.2))
                                        gradient: lift.up ? pages : null

                                        Gradient {
                                            id: pages
                                            GradientStop { position: 0; color: "#d9cfb4" }
                                            GradientStop { position: 1; color: "#f4eedd" }
                                        }

                                        // Boards either side of the pages
                                        Rectangle {
                                            visible: lift.up
                                            width: 2
                                            height: parent.height
                                            color: Qt.rgba(0, 0, 0, 0.3)
                                        }
                                        Rectangle {
                                            visible: lift.up
                                            x: parent.width - 2
                                            width: 2
                                            height: parent.height
                                            color: Qt.rgba(0, 0, 0, 0.3)
                                        }
                                        // The cover's back edge in shadow
                                        Rectangle {
                                            visible: !lift.up
                                            anchors.fill: parent
                                            topLeftRadius: 2
                                            topRightRadius: 2
                                            gradient: Gradient {
                                                GradientStop { position: 0; color: Qt.rgba(0, 0, 0, 0.25) }
                                                GradientStop { position: 0.7; color: Qt.rgba(0, 0, 0, 0) }
                                            }
                                        }
                                    }

                                    BookSpine {
                                        id: spineArt
                                        spine: book.spine
                                        color: book.info.color || ""
                                    }

                                    Rectangle {
                                        visible: book.keyboard
                                        anchors.fill: parent
                                        anchors.margins: -5
                                        radius: 6
                                        color: "transparent"
                                        border.width: 2
                                        border.color: Palette.primary
                                    }
                                }

                            }
                        }
                    }

                    Component {
                        id: propThing

                        // A pot of basil or a crock of spoons, because why not
                        Item {
                            id: prop

                            readonly property bool pot: thing.kind === "pot"
                            // Sits on its book: slides out with it, and drops when it's taken
                            readonly property bool ride: pot && thing.item.on === shelf.hoverId && thing.item.on !== shelf.pulledId
                            readonly property string moved: ride + "-" + thing.drop

                            onMovedChanged: {
                                if (ride || thing.drop)
                                    wobble.restart()
                            }

                            Item {
                                id: art

                                readonly property string source: prop.pot ? "qrc:/shelf/pot.svg" : "qrc:/shelf/crock.svg"

                                width: parent.width
                                height: parent.height

                                // Its shadow, from the drawing's own outline
                                Canvas {
                                    id: propShadow

                                    x: -16
                                    y: -16
                                    width: art.width + 32
                                    height: art.height + 32
                                    Component.onCompleted: loadImage(art.source)
                                    onImageLoaded: requestPaint()
                                    onPaint: {
                                        var ctx = getContext("2d")
                                        ctx.reset()
                                        if (!isImageLoaded(art.source))
                                            return
                                        ctx.shadowColor = shelf.shadowColor.arg(0.22)
                                        ctx.shadowOffsetX = 3
                                        ctx.shadowOffsetY = 3
                                        ctx.shadowBlur = 3
                                        ctx.drawImage(art.source, 16, 16, art.width, art.height)
                                    }
                                }

                                Image {
                                    anchors.fill: parent
                                    source: art.source
                                    sourceSize: Qt.size(art.width * 2, art.height * 2)
                                    smooth: true
                                }

                                transform: [
                                    // A pot that's been moved rocks on its base before it settles
                                    Rotation {
                                        id: rock
                                        origin.x: art.width / 2
                                        origin.y: art.height
                                    },
                                    Translate {
                                        x: prop.ride ? 16 : 0
                                        y: thing.drop - (prop.ride ? 1 : 0)
                                        Behavior on x { NumberAnimation { duration: 500; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.3, 1.4, 0.5, 1, 1, 1] } }
                                        Behavior on y { NumberAnimation { duration: 500; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.3, 1.4, 0.5, 1, 1, 1] } }
                                    }
                                ]
                            }

                            SequentialAnimation {
                                id: wobble
                                NumberAnimation { target: rock; property: "angle"; to: -5; duration: 200; easing.type: Easing.OutQuad }
                                NumberAnimation { target: rock; property: "angle"; to: 3; duration: 200; easing.type: Easing.OutQuad }
                                NumberAnimation { target: rock; property: "angle"; to: -1.2; duration: 200; easing.type: Easing.OutQuad }
                                NumberAnimation { target: rock; property: "angle"; to: 0; duration: 200; easing.type: Easing.OutQuad }
                            }
                        }
                    }

                    Component {
                        id: addThing

                        // A dashed outline of a book, waiting at the end of the last shelf
                        Item {
                            id: newBook

                            readonly property bool lit: newHover.hovered || activeFocus
                            readonly property color ink: lit ? Palette.primary : Qt.rgba(Palette.text.r, Palette.text.g, Palette.text.b, 0.3)

                            activeFocusOnTab: true
                            Accessible.role: Accessible.Button
                            Accessible.name: "New cookbook"
                            Accessible.onPressAction: shelf.add()
                            Keys.onReturnPressed: shelf.add()
                            Keys.onSpacePressed: shelf.add()

                            HoverHandler {
                                id: newHover
                                cursorShape: Qt.PointingHandCursor
                            }

                            TapHandler {
                                onTapped: shelf.add()
                            }

                            Item {
                                width: parent.width
                                height: parent.height
                                y: newBook.lit ? -8 : 0

                                Behavior on y {
                                    NumberAnimation { duration: 400; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.3, 1.5, 0.5, 1, 1, 1] }
                                }

                                Shape {
                                    anchors.fill: parent
                                    ShapePath {
                                        strokeWidth: 2
                                        strokeColor: newBook.ink
                                        strokeStyle: ShapePath.DashLine
                                        dashPattern: [3, 2]
                                        fillColor: "transparent"
                                        startX: 1
                                        startY: newBook.height - 1
                                        PathLine { x: 1; y: 4 }
                                        PathArc { x: 4; y: 1; radiusX: 3; radiusY: 3 }
                                        PathLine { x: newBook.width - 4; y: 1 }
                                        PathArc { x: newBook.width - 1; y: 4; radiusX: 3; radiusY: 3 }
                                        PathLine { x: newBook.width - 1; y: newBook.height - 1 }
                                        PathLine { x: 1; y: newBook.height - 1 }
                                    }
                                }

                                Icon {
                                    anchors.centerIn: parent
                                    name: "plus"
                                    size: 20
                                    color: newBook.lit ? Palette.primary : Palette.textMuted
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
