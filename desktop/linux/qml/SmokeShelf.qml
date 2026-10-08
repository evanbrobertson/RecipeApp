import QtQuick
import QtQuick.Controls

import app.crumb.desktop 1.0

// `--smoke-page shelf`: the shelf and Home's single shelf with fixture books (the seeded test
// server's), and the open book. Hovers and opens a book with real pointer events and checks
// what happened. With `--shot <dir>` it films instead: frames of the hover, the pot riding, the
// stack settling and the open and close flights, saved as `<dir>/<scene>-<ms>.png`.
Rectangle {
    id: root

    property var opened: null
    // Narrows the shelf, so its books glide to new places
    property real squeeze: 0
    readonly property bool filming: Smoke.shot !== ""

    readonly property var books: [
        { "id": 5, "name": "Bread", "color": "butter", "recipeCount": 2 },
        { "id": 10, "name": "Breakfast", "color": "butter", "recipeCount": 4 },
        { "id": 13, "name": "Cakes", "color": "tile", "recipeCount": 6 },
        { "id": 2, "name": "Christmas baking", "color": "clay", "recipeCount": 6 },
        { "id": 7, "name": "Curries", "color": "forest", "recipeCount": 8 },
        { "id": 16, "name": "Desserts", "color": "sage", "recipeCount": 9 },
        { "id": 17, "name": "Dinner party showstoppers", "color": "butter", "recipeCount": 3 },
        { "id": 4, "name": "Grandma's Sunday roasts and other family favourites", "color": "forest", "recipeCount": 12 },
        { "id": 15, "name": "Meal prep for busy weeks", "color": "clay", "recipeCount": 5 },
        { "id": 18, "name": "Mum's", "color": "tile", "recipeCount": 1 },
        { "id": 20, "name": "Noodles", "color": "forest", "recipeCount": 11 },
        { "id": 9, "name": "Ottolenghi-ish vegetable mains", "color": "tile", "recipeCount": 7 },
        { "id": 11, "name": "Pasta", "color": "clay", "recipeCount": 10 },
        { "id": 14, "name": "Pickles & preserves", "color": "forest", "recipeCount": 3 },
        { "id": 12, "name": "Recipes from the trip to Lisbon", "color": "cream", "recipeCount": 2 },
        { "id": 19, "name": "Slow cooker", "color": "cream", "recipeCount": 7 },
        { "id": 3, "name": "Soups", "color": "cream", "recipeCount": 3 },
        { "id": 8, "name": "Summer salads", "color": "sage", "recipeCount": 5 },
        { "id": 6, "name": "Things to make when it's raining", "color": "sage", "recipeCount": 4 },
        { "id": 1, "name": "Weeknight dinners", "color": "tile", "recipeCount": 12 }
    ]

    color: Palette.bg

    // A book's middle in window coordinates, from the shelf's layout
    function centre(shelf, id) {
        for (var ri = 0; ri < shelf.rows.length; ri++) {
            var items = shelf.rows[ri].items
            for (var i = 0; i < items.length; i++) {
                var it = items[i]
                if (it.kind === "book" && it.book.id === id) {
                    var top = shelf.wallTop + ri * shelf.metrics.rowHeight + shelf.metrics.clearance + 4 - it.y - it.h
                    return shelf.mapToItem(null, it.x + it.w / 2, top + it.h / 2)
                }
            }
        }
        Smoke.fail("book " + id + " isn't on the shelf")
        return Qt.point(0, 0)
    }

    function hover(shelf, id) {
        var p = centre(shelf, id)
        Smoke.pointer(p.x, p.y, 0)
    }

    function click(shelf, id) {
        var p = centre(shelf, id)
        Smoke.pointer(p.x, p.y, 0)
        Smoke.pointer(p.x, p.y, 1)
        Smoke.pointer(p.x, p.y, 2)
    }

    function away() {
        Smoke.pointer(2, 2, 0)
    }

    function check(ok, why) {
        if (!ok)
            Smoke.fail(why)
    }

    // Where the web's shelf page puts it in a 1280-wide window, to compare frame by frame
    ShelfCard {
        id: card
        x: root.width >= 1024 ? 272 : 24
        y: root.width >= 1024 ? 120 : 24
        width: root.width - x - 32 - squeeze
        height: shelf.implicitHeight

        Bookshelf {
            id: shelf
            width: parent.width
            books: root.books
            addable: true
            pulledId: root.opened ? root.opened.id : 0
            onOpenBook: (book, origin) => {
                root.opened = book
                openBook.show(book, origin)
            }
        }
    }

    ShelfCard {
        x: card.x
        y: card.y + card.height + 24
        width: card.width
        height: home.implicitHeight

        Bookshelf {
            id: home
            width: parent.width
            books: root.books.slice(0, 15)
            single: true
            pulledId: root.opened ? root.opened.id : 0
            onOpenBook: (book, origin) => {
                root.opened = book
                openBook.show(book, origin)
            }
        }
    }

    BookOpenModal {
        id: openBook
        onDismissed: root.opened = null
    }

    // ─── The scenario: a step at a time, each after the last ───
    property var steps: []
    property int step: 0
    property string scene: ""
    property real sceneStart: 0
    property real sceneLength: 0

    function film(name, ms) {
        return { "film": name, "ms": ms }
    }
    function wait(ms) {
        return { "wait": ms }
    }
    function act(fn) {
        return { "act": fn }
    }

    function next() {
        if (step >= steps.length) {
            if (filming)
                Qt.quit()
            return
        }
        var s = steps[step++]
        if (s.act) {
            s.act()
            next()
        } else if (s.film) {
            if (!filming) {
                pause.interval = s.ms
                pause.restart()
                return
            }
            scene = s.film
            sceneLength = s.ms
            sceneStart = Date.now()
            camera.restart()
        } else {
            pause.interval = s.wait
            pause.restart()
        }
    }

    Timer {
        id: pause
        onTriggered: root.next()
    }

    // Grabs a frame, then the next as soon as that one is saved
    Timer {
        id: camera
        interval: 1
        onTriggered: {
            var at = Date.now() - root.sceneStart
            if (at > root.sceneLength) {
                root.next()
                return
            }
            var name = root.scene + "-" + ("0000" + Math.round(at)).slice(-4) + ".png"
            if (!Smoke.grab(Smoke.shot + "/" + name))
                console.warn("couldn't save", name)
            camera.restart()
        }
    }

    Component.onCompleted: {
        var christmas = 2
        var curries = 7
        var grandma = 4
        var lisbon = 12
        if (!filming) {
            // A quick check inside the smoke run's second
            steps = [
                wait(250),
                act(function () {
                    root.check(shelf.rows.length > 0, "the shelf has no rows")
                    root.hover(shelf, christmas)
                }),
                wait(120),
                act(function () {
                    root.check(shelf.hoverId === christmas, "hovering a book didn't lift it")
                    root.click(shelf, christmas)
                }),
                wait(150),
                act(function () {
                    root.check(openBook.visible && openBook.from !== null, "the book didn't fly off the shelf")
                    root.check(root.opened && root.opened.id === christmas, "the wrong book opened")
                })
            ]
        } else {
            steps = [
                wait(600),
                film("rest", 1),
                act(function () { root.hover(shelf, christmas) }),
                film("hover", 700),
                act(function () { root.away(); root.hover(home, lisbon) }),
                film("ride", 1000),
                act(function () { root.click(home, lisbon) }),
                film("settle", 900),
                act(function () { openBook.dismiss() }),
                film("settle-back", 1800),
                act(function () { root.away() }),
                wait(500),
                act(function () { root.click(shelf, curries) }),
                film("open", 1900),
                act(function () { openBook.dismiss() }),
                film("close", 1500),
                wait(400),
                act(function () { root.click(shelf, grandma) }),
                film("open-lying", 1900),
                act(function () { openBook.dismiss() }),
                film("close-lying", 1500),
                wait(700),
                act(function () { root.squeeze = 200 }),
                film("reflow", 900)
            ]
        }
        next()
    }
}
