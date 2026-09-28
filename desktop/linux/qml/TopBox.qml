import QtQuick

import app.crumb.desktop 1.0

// STUB, owned by the Add-box agent (web islands/TopBox.svelte). Keep this interface.
Item {
    id: box
    // Show the Search/Add tabs (Home); without them it's just the Add block (the Add page)
    property bool tabs: true
    // Sits on the tile header; off it, the block gets a border
    property bool onTile: true
    property bool autofocus: false

    implicitHeight: 120
}
