import QtQuick
import QtQuick.Shapes

import app.crumb.desktop 1.0

// The shelf, as the web's Bookshelf.svelte: books stacked into towers, towers standing on
// planks, a herb pot on the shortest tower of the first shelf, and (when `addable`) a dashed
// outline of a new book on the last stack. Sizes, leans and stacking come from Core. Sits on
// a ShelfCard, which supplies the tint backdrop.
Item {
    id: shelf

    // [{id, name, color, recipeCount}]
    property var books: []
    property int pulledId: 0
    property bool addable: false
    // One row of short towers that scrolls sideways (Home) instead of more shelves
    property bool single: false
    signal openBook(var book)
    signal add()

    readonly property real availableWidth: width > 0 ? width : 900
    // Phones get one tall tower per shelf, centred
    readonly property bool phone: !single && availableWidth < 560
    // The length of the longest book in a tower
    readonly property real towerLen: single ? 172 : phone ? Math.min(300, availableWidth - 88) : 180
    readonly property real towerW: towerLen + 16
    // Room for a tower: its books, a nudge and a slide-out either side, and the gap
    readonly property real slotExtra: 36
    readonly property real potHeight: 66
    readonly property real slotHeight: 48
    readonly property real plankHeight: 12
    // [shelf][tower] = [book objects, top to bottom]
    readonly property var shelves: layoutShelves(books, availableWidth, single, phone, addable, towerLen)
    readonly property int lastShelf: shelves.length - 1
    readonly property int lastTower: shelves.length ? shelves[lastShelf].length - 1 : 0
    readonly property int potTower: findPotTower(shelves, addable, lastShelf, lastTower)
    // A single row's natural width: it scrolls when it doesn't fit
    readonly property real rowWidth: shelves.length ? 40 + shelves[0].length * towerW + (shelves[0].length - 1) * 28 : 0

    implicitHeight: flick.contentHeight

    function thickness(book) {
        return JSON.parse(Core.bookShape(JSON.stringify(book), false)).thickness
    }

    function towerHeight(tower) {
        var sum = 0
        for (var i = 0; i < tower.length; i++)
            sum += shelf.thickness(tower[i])
        return sum
    }

    // A tower's height on its shelf: its books, the herb pot and the new-book slot
    function standHeight(shelfIndex, towerIndex, tower) {
        return towerHeight(tower)
            + (shelfIndex === 0 && towerIndex === potTower ? potHeight - 1 : 0)
            + (addable && shelfIndex === lastShelf && towerIndex === lastTower ? slotHeight : 0)
    }

    // Books stack into towers; towers stand side by side on shelves (planks)
    function layoutShelves(books, w, single, phone, addable, towerLen) {
        var n = books.length
        if (!n)
            return addable ? [[[]]] : []
        var count
        var perShelf
        if (single) {
            count = Math.ceil(n / 4)
            perShelf = 0
        } else if (phone) {
            count = Math.ceil(n / 10)
            perShelf = 1
        } else {
            var fit = Math.max(1, Math.floor((w - 48) / (towerLen + slotExtra)))
            // 4 to 7 books a tower, spread over as many towers as fit
            count = Math.max(Math.ceil(n / 7), Math.min(fit, Math.ceil(n / 4)))
            perShelf = fit
        }
        var stacks = JSON.parse(Core.stackBooks(JSON.stringify(books), count))
        var towers = stacks.map(function (indexes) {
            return indexes.map(function (i) { return books[i] })
        })
        if (perShelf === 0)
            perShelf = towers.length
        var out = []
        for (var i = 0; i < towers.length; i += perShelf)
            out.push(towers.slice(i, i + perShelf))
        return out
    }

    // The herb pot sits on the shortest tower of the first shelf (not the one holding the
    // new-book slot, unless that is the only one)
    function findPotTower(shelves, addable, lastShelf, lastTower) {
        var first = shelves.length ? shelves[0] : []
        var skip = addable && lastShelf === 0 && first.length > 1 ? lastTower : -1
        var best = -1
        var bestHeight = 0
        for (var i = 0; i < first.length; i++) {
            if (i === skip)
                continue
            var h = towerHeight(first[i])
            if (best < 0 || h < bestHeight) {
                best = i
                bestHeight = h
            }
        }
        return best
    }

    // Where a tower stands on its shelf: spread evenly (Home's row starts at the left when narrow)
    function towerX(i, n, rowW) {
        if (phone)
            return (rowW - towerW) / 2
        var pad = single ? 20 : 24
        var gap = single ? 28 : 24
        var start = single && rowW < 640 ? true : false
        var free = Math.max(0, rowW - 2 * pad - n * towerW - (n - 1) * gap)
        if (start)
            return pad + i * (towerW + gap)
        return pad + free / (n + 1) * (i + 1) + i * (towerW + gap)
    }

    Flickable {
        id: flick

        width: shelf.width
        height: contentHeight
        contentWidth: shelf.single ? Math.max(shelf.width, shelf.rowWidth) : shelf.width
        contentHeight: shelvesColumn.height
        interactive: shelf.single && contentWidth > width
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: Flickable.HorizontalFlick
        clip: true

        Column {
            id: shelvesColumn

            width: flick.contentWidth
            spacing: 20

            Repeater {
                model: shelf.shelves

                delegate: Item {
                    id: shelfRow

                    required property var modelData
                    required property int index
                    readonly property real towersHeight: {
                        var max = 0
                        for (var i = 0; i < modelData.length; i++)
                            max = Math.max(max, shelf.standHeight(index, i, modelData[i]))
                        return max
                    }

                    width: shelvesColumn.width
                    height: towersHeight + shelf.plankHeight

                    Repeater {
                        id: towerColumns
                        model: shelfRow.modelData

                        delegate: Column {
                            id: tower

                            required property var modelData
                            required property int index

                            x: shelf.towerX(index, shelfRow.modelData.length, shelfRow.width)
                            y: shelfRow.towersHeight - height
                            width: shelf.towerW

                            // A little herb pot, because why not
                            Item {
                                visible: shelfRow.index === 0 && tower.index === shelf.potTower
                                width: tower.width
                                height: visible ? shelf.potHeight - 1 : 0

                                Shape {
                                    // Off-centre, as if someone put it down in passing
                                    x: (tower.width - (44 + 0.28 * tower.width)) / 2 + 0.28 * tower.width
                                    width: 60
                                    height: 90
                                    scale: 44 / 60
                                    transformOrigin: Item.TopLeft

                                    ShapePath {
                                        strokeColor: "transparent"
                                        fillColor: "#a9c4ae"
                                        PathSvg { path: "M30 52 C 18 38, 8 36, 4 24 C 16 24, 26 32, 30 46 Z" }
                                    }
                                    ShapePath {
                                        strokeColor: "transparent"
                                        fillColor: "#2f6b4f"
                                        PathSvg { path: "M30 52 C 42 36, 52 32, 57 18 C 44 20, 34 30, 30 46 Z" }
                                    }
                                    ShapePath {
                                        strokeColor: "transparent"
                                        fillColor: "#3b7a5a"
                                        PathSvg { path: "M30 54 C 26 36, 28 20, 34 8 C 38 22, 36 38, 31 52 Z" }
                                    }
                                    ShapePath {
                                        strokeColor: "transparent"
                                        fillColor: "#a55a40"
                                        PathSvg { path: "M14 58 h32 l-4 32 h-24 z" }
                                    }
                                    ShapePath {
                                        strokeColor: "transparent"
                                        fillColor: "#8f4c35"
                                        PathSvg { path: "M11 52 h38 v8 h-38 z" }
                                    }
                                }
                            }

                            // After the books in tab order, but drawn on top of the stack
                            Item {
                                id: newBook

                                visible: shelf.addable && shelfRow.index === shelf.lastShelf && tower.index === shelf.lastTower
                                width: tower.width
                                height: visible ? shelf.slotHeight : 0
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

                                readonly property bool lit: newHover.hovered || newBook.activeFocus

                                Item {
                                    x: (parent.width - width) / 2 - 4
                                    y: newBook.lit ? -3 : 0
                                    width: shelf.towerLen * 0.82
                                    height: 44
                                    rotation: -1.5

                                    Behavior on y {
                                        NumberAnimation { duration: 180; easing.type: Easing.OutCubic }
                                    }

                                    Shape {
                                        anchors.fill: parent
                                        ShapePath {
                                            strokeWidth: 2
                                            strokeColor: newBook.lit ? Palette.primary : Palette.line
                                            strokeStyle: ShapePath.DashLine
                                            dashPattern: [3, 3]
                                            fillColor: "transparent"
                                            startX: 1
                                            startY: 1
                                            PathLine { x: 1; y: 43 }
                                            PathLine { x: shelf.towerLen * 0.82 - 1; y: 43 }
                                            PathLine { x: shelf.towerLen * 0.82 - 1; y: 1 }
                                            PathLine { x: 1; y: 1 }
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

                            Repeater {
                                model: tower.modelData

                                delegate: BookSpine {
                                    required property var modelData
                                    required property int index

                                    book: modelData
                                    atFoot: index === tower.modelData.length - 1
                                    pulled: shelf.pulledId === modelData.id
                                    towerLen: shelf.towerLen
                                    towerW: shelf.towerW
                                    onOpened: shelf.openBook(modelData)
                                }
                            }
                        }
                    }

                    // A flat tile-green plank
                    Rectangle {
                        y: parent.height - shelf.plankHeight
                        width: parent.width
                        height: shelf.plankHeight
                        color: Palette.tile
                    }
                }
            }
        }
    }
}
