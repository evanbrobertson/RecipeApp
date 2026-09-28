import QtQuick

import app.crumb.desktop 1.0

// STUB, owned by the shelf agent (web Bookshelf.svelte). Keep this interface.
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

    implicitHeight: 180
}
