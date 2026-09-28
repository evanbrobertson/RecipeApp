import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The Recipes list: a server-side search, category pills, a card grid and a selection bar
// with bulk add/delete. Mirrors web/src/islands/RecipesPage.svelte and
// web/src/pages/recipes/index.astro; labels and arithmetic come from Core.
Item {
    id: page

    property var session
    property int routeId: 0
    property var params: ({})

    // ─── Data ───
    property bool loading: true
    property bool searching: false
    property bool busy: false
    property var recipes: []
    property var cookbooks: []
    property int loadPending: 0
    property string q: ""
    property string category: "all"
    property int requestId: 0

    // ─── Selection ───
    property bool selecting: false
    property var selected: ({})

    readonly property int selectedCount: Object.keys(page.selected).length
    readonly property var categoryList: page.categoriesIn(page.recipes)
    readonly property var visibleRecipes: page.category === "all" ? page.recipes : page.recipes.filter(function (r) {
        return r.recipeCategory === page.category
    })
    readonly property bool allSelected: page.visibleRecipes.length > 0 && page.visibleRecipes.every(function (r) {
        return page.isSelected(r.id)
    })

    readonly property bool showGrid: !page.loading && page.visibleRecipes.length > 0
    readonly property bool showNoRecipes: !page.loading && page.recipes.length === 0 && page.q === ""
    readonly property bool showNothingMatches: !page.loading && !page.showNoRecipes && page.visibleRecipes.length === 0

    // The grid: 4 columns at ≥1100px of page width, 3 at ≥800, else 2.
    readonly property real gridWidth: scroller.contentWidthAvailable
    readonly property int gridColumns: page.gridWidth >= 1100 ? 4 : page.gridWidth >= 800 ? 3 : 2
    readonly property real gridGapX: page.gridColumns === 2 ? 12 : 16
    readonly property real gridCellWidth: Math.max(0, Math.floor((page.gridWidth - (page.gridColumns - 1) * page.gridGapX) / page.gridColumns))

    function isSelected(id) {
        return page.selected["r" + id] === true
    }

    function selectedIds() {
        var keys = Object.keys(page.selected)
        var out = []
        for (var i = 0; i < keys.length; i++)
            out.push(parseInt(keys[i].slice(1), 10))
        return out
    }

    function toggle(id) {
        var next = {}
        var keys = Object.keys(page.selected)
        for (var i = 0; i < keys.length; i++)
            next[keys[i]] = true
        var key = "r" + id
        if (next[key])
            delete next[key]
        else
            next[key] = true
        page.selected = next
    }

    function toggleSelecting() {
        page.selecting = !page.selecting
        page.selected = ({})
    }

    function toggleAll() {
        var next = {}
        if (!page.allSelected) {
            for (var i = 0; i < page.visibleRecipes.length; i++)
                next["r" + page.visibleRecipes[i].id] = true
        }
        page.selected = next
    }

    // The categories that have a recipe, in Core's display order (the web's categoriesIn).
    function categoriesIn(list) {
        var all = JSON.parse(Core.categories())
        var have = ({})
        for (var i = 0; i < list.length; i++)
            have[list[i].recipeCategory] = true
        var out = []
        for (var j = 0; j < all.length; j++) {
            if (have[all[j]])
                out.push(all[j])
        }
        return out
    }

    function pillValues() {
        var out = [{ "label": "Everything", "value": "all" }]
        for (var i = 0; i < page.categoryList.length; i++)
            out.push({ "label": page.categoryList[i], "value": page.categoryList[i] })
        return out
    }

    function finishLoad() {
        page.loadPending = Math.max(0, page.loadPending - 1)
        if (page.loadPending === 0)
            page.loading = false
    }

    function load() {
        page.loadPending = 2
        // 500 is the API's largest page; the web omits the limit and gets its default.
        requests.call("recipes", { "query": page.q, "limit": 500 }, function (list) {
            page.recipes = list || []
            page.finishLoad()
        }, function () {
            page.finishLoad()
        })
        requests.call("cookbooks", {}, function (list) {
            page.cookbooks = list || []
            page.finishLoad()
        }, function () {
            page.finishLoad()
        })
    }

    function runSearch() {
        var value = searchField.text.trim()
        if (value === page.q)
            return
        page.q = value
        var mine = page.requestId + 1
        page.requestId = mine
        page.searching = true
        requests.call("recipes", { "query": value, "limit": 500 }, function (list) {
            if (mine === page.requestId)
                page.recipes = list || []
            if (mine === page.requestId)
                page.searching = false
        }, function (error) {
            ApplicationWindow.window.toast({ "title": "Search failed", "description": error, "tone": "error" })
            if (mine === page.requestId)
                page.searching = false
        })
    }

    function clearSearch() {
        searchField.text = ""
        page.category = "all"
        page.runSearch()
    }

    // Drop deleted recipes from the "recently viewed" list (the web's forgetViewed).
    function forgetViewed(ids) {
        var recent = []
        try {
            recent = JSON.parse(Store.read("crumb:recent", "[]"))
        } catch (e) {
            recent = []
        }
        var kept = []
        for (var i = 0; i < recent.length; i++) {
            if (ids.indexOf(recent[i].id) === -1)
                kept.push(recent[i])
        }
        Store.write("crumb:recent", JSON.stringify(kept))
    }

    function addToCookbook(book) {
        if (page.busy)
            return
        page.busy = true
        var ids = page.selectedIds()
        requests.call("addToCookbook", { "id": book.id, "recipeIds": ids }, function () {
            page.busy = false
            cookbooksModal.close()
            ApplicationWindow.window.toast({ "title": "Added " + ids.length + " to " + book.name, "tone": "success" })
            page.toggleSelecting()
        }, function (error) {
            page.busy = false
            ApplicationWindow.window.toast({ "title": "Couldn't add to cookbook", "description": error, "tone": "error" })
        })
    }

    function deleteSelected() {
        if (page.busy)
            return
        page.busy = true
        var ids = page.selectedIds()
        requests.call("bulkDelete", { "ids": ids }, function () {
            page.busy = false
            page.forgetViewed(ids)
            ApplicationWindow.window.toast({ "title": "Deleted " + Core.plural(ids.length, "recipe"), "tone": "success" })
            page.recipes = page.recipes.filter(function (r) {
                return !page.isSelected(r.id)
            })
            deleteModal.close()
            page.toggleSelecting()
        }, function (error) {
            page.busy = false
            ApplicationWindow.window.toast({ "title": "Couldn't delete", "description": error, "tone": "error" })
        })
    }

    function focusSearch() {
        searchField.forceActiveFocus()
    }

    Requests {
        id: requests
    }

    Timer {
        id: searchDebounce
        interval: 250
        repeat: false
        onTriggered: page.runSearch()
    }

    Component.onCompleted: {
        if (page.params && page.params.q)
            searchField.text = String(page.params.q)
        page.q = searchField.text.trim()
        page.load()
        if (page.params && page.params.focusSearch)
            focusSearch()
    }

    ScrollPage {
        id: scroller
        anchors.fill: parent
        bottomPadding: page.selecting ? 64 + 80 : 64

        ColumnLayout {
            width: parent.width
            spacing: 0

            // ─── Header: title, Select/Done, Add ───
            RowLayout {
                Layout.fillWidth: true
                Layout.bottomMargin: 20
                spacing: 12

                Heading {
                    level: 1
                    text: "Recipes"
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignVCenter
                }

                Button {
                    id: selectButton
                    visible: page.recipes.length > 0
                    Layout.alignment: Qt.AlignVCenter
                    implicitHeight: 44
                    leftPadding: 16
                    rightPadding: 16
                    hoverEnabled: true
                    onClicked: page.toggleSelecting()

                    background: Rectangle {
                        radius: 12
                        color: page.selecting ? Palette.tile : Palette.paper
                        border.width: page.selecting ? 0 : 1
                        border.color: Palette.line

                        Rectangle {
                            anchors.fill: parent
                            radius: parent.radius
                            color: Palette.text
                            opacity: selectButton.hovered ? 0.06 : 0
                        }
                    }

                    contentItem: RowLayout {
                        spacing: 8

                        Icon {
                            name: page.selecting ? "x" : "square-check"
                            size: 18
                            color: page.selecting ? Palette.onTile : Palette.text
                        }

                        Text {
                            text: page.selecting ? "Done" : "Select"
                            color: page.selecting ? Palette.onTile : Palette.text
                            font.family: Palette.fontSans
                            font.pixelSize: 15
                            font.weight: Font.Bold
                        }
                    }
                }

                CrumbButton {
                    kind: "primary"
                    iconName: "plus"
                    text: "Add"
                    visible: page.width < 768
                    Layout.alignment: Qt.AlignVCenter
                    onClicked: ApplicationWindow.window.go("add", {})
                }
            }

            // ─── Search and category pills ───
            ColumnLayout {
                Layout.fillWidth: true
                Layout.bottomMargin: 28
                spacing: 12

                Item {
                    id: searchWrap
                    Layout.preferredWidth: Math.min(672, page.gridWidth)
                    Layout.alignment: Qt.AlignLeft
                    implicitHeight: 56

                    StyledField {
                        id: searchField
                        anchors.fill: parent
                        leftPadding: 48
                        rightPadding: 56
                        placeholderText: page.recipes.length > 0 && page.q === ""
                                         ? "Search " + page.recipes.length + " recipes"
                                         : "Search by name or ingredient…"
                        onTextChanged: searchDebounce.restart()
                    }

                    Item {
                        width: 20
                        height: 20
                        anchors.left: parent.left
                        anchors.leftMargin: 16
                        anchors.verticalCenter: parent.verticalCenter

                        Icon {
                            anchors.fill: parent
                            visible: !page.searching
                            name: "search"
                            size: 20
                            color: Palette.textMuted
                        }

                        Item {
                            anchors.fill: parent
                            visible: page.searching

                            Icon {
                                anchors.fill: parent
                                name: "loader-circle"
                                size: 20
                                color: Palette.textMuted
                            }

                            RotationAnimation on rotation {
                                running: page.searching
                                loops: Animation.Infinite
                                from: 0
                                to: 360
                                duration: 1000
                            }
                        }
                    }

                    CrumbButton {
                        visible: searchField.text !== ""
                        kind: "ghost"
                        iconName: "x"
                        width: 44
                        height: 44
                        anchors.right: parent.right
                        anchors.rightMargin: 2
                        anchors.verticalCenter: parent.verticalCenter
                        onClicked: page.clearSearch()
                    }
                }

                Flow {
                    Layout.fillWidth: true
                    visible: page.categoryList.length > 1
                    spacing: 8

                    Repeater {
                        model: page.pillValues()

                        delegate: Rectangle {
                            required property var modelData

                            height: 44
                            width: chipText.implicitWidth + 32
                            radius: height / 2
                            color: page.category === modelData.value ? Palette.tile : Palette.tint

                            Text {
                                id: chipText
                                anchors.centerIn: parent
                                text: modelData.label
                                color: page.category === modelData.value ? Palette.onTile : Palette.primary
                                font.family: Palette.fontSans
                                font.pixelSize: 14
                                font.weight: Font.Bold
                            }

                            HoverHandler {
                                cursorShape: Qt.PointingHandCursor
                            }

                            TapHandler {
                                onTapped: page.category = modelData.value
                            }
                        }
                    }
                }
            }

            // ─── Loading skeleton ───
            Grid {
                Layout.fillWidth: true
                visible: page.loading
                columns: page.gridColumns
                columnSpacing: page.gridGapX
                rowSpacing: 20

                Repeater {
                    model: 8

                    delegate: ColumnLayout {
                        width: page.gridCellWidth
                        spacing: 0

                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: Math.round(page.gridCellWidth * 3 / 4)
                            radius: 12
                            color: Palette.tint
                        }

                        Rectangle {
                            Layout.preferredWidth: Math.round(page.gridCellWidth * 4 / 5)
                            Layout.preferredHeight: 16
                            Layout.topMargin: 10
                            radius: 8
                            color: Palette.tint
                        }

                        Rectangle {
                            Layout.preferredWidth: Math.round(page.gridCellWidth * 2 / 5)
                            Layout.preferredHeight: 14
                            Layout.topMargin: 8
                            radius: 8
                            color: Palette.tint
                        }
                    }
                }
            }

            // ─── Empty states ───
            EmptyState {
                visible: page.showNoRecipes
                Layout.fillWidth: true
                Layout.topMargin: 40
                iconName: "book-open"
                title: "No recipes yet"
                text: "Paste a link or some recipe text to save your first one."
                actionText: "Add a recipe"
                onAction: ApplicationWindow.window.go("add", {})
            }

            EmptyState {
                visible: page.showNothingMatches
                Layout.fillWidth: true
                Layout.topMargin: 40
                iconName: "search-x"
                title: "Nothing matches"
                text: page.q !== "" ? "No recipes found for \u201C" + page.q + "\u201D." : "Try another category."
                actionText: "Clear search"
                onAction: page.clearSearch()
            }

            // ─── The grid ───
            Grid {
                Layout.fillWidth: true
                visible: page.showGrid
                columns: page.gridColumns
                columnSpacing: page.gridGapX
                rowSpacing: 20

                Repeater {
                    model: page.visibleRecipes

                    delegate: RecipeCard {
                        required property var modelData

                        width: page.gridCellWidth
                        recipe: modelData
                        selectable: page.selecting
                        selected: page.isSelected(modelData.id)
                        onToggled: function (id) {
                            page.toggle(id)
                        }
                    }
                }
            }
        }
    }

    // ─── Selection bar ───
    Rectangle {
        id: selectionBar
        visible: page.selecting
        z: 50
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 64
        color: Palette.paper

        Rectangle {
            anchors.top: parent.top
            width: parent.width
            height: 1
            color: Palette.line
        }

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 16
            anchors.rightMargin: 16
            spacing: 12

            CrumbButton {
                kind: "ghost"
                text: page.allSelected ? "Clear" : "Select all"
                onClicked: page.toggleAll()
            }

            Item {
                Layout.fillWidth: true
            }

            Body {
                text: page.selectedCount + " selected"
                bold: true
                font.pixelSize: 15
            }

            Item {
                Layout.fillWidth: true
            }

            CrumbButton {
                kind: "soft"
                iconName: "book-plus"
                text: "Cookbook"
                enabled: page.selectedCount > 0 && !page.busy
                onClicked: cookbooksModal.open()
            }

            CrumbButton {
                kind: "outline"
                iconName: "trash-2"
                width: 44
                enabled: page.selectedCount > 0 && !page.busy
                onClicked: deleteModal.open()
            }
        }
    }

    // ─── Add to cookbook ───
    Modal {
        id: cookbooksModal
        title: "Add to cookbook"

        ColumnLayout {
            visible: page.cookbooks.length === 0
            Layout.fillWidth: true
            spacing: 12

            Body {
                text: "You don't have any cookbooks yet."
                muted: true
                horizontalAlignment: Text.AlignHCenter
                Layout.fillWidth: true
            }

            CrumbButton {
                kind: "soft"
                text: "Create a cookbook"
                Layout.alignment: Qt.AlignHCenter
                onClicked: {
                    cookbooksModal.close()
                    ApplicationWindow.window.go("shelf", {})
                }
            }
        }

        ColumnLayout {
            visible: page.cookbooks.length > 0
            Layout.fillWidth: true
            spacing: 4

            Repeater {
                model: page.cookbooks

                delegate: Item {
                    id: bookRow
                    required property var modelData

                    Layout.fillWidth: true
                    implicitHeight: 56
                    opacity: page.busy ? 0.6 : 1

                    Rectangle {
                        anchors.fill: parent
                        radius: 12
                        color: rowHover.hovered ? Palette.tint : "transparent"

                        Behavior on color {
                            ColorAnimation { duration: 150 }
                        }
                    }

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 8
                        anchors.rightMargin: 8
                        spacing: 12

                        Rectangle {
                            Layout.preferredWidth: 16
                            Layout.preferredHeight: 36
                            topLeftRadius: 3
                            topRightRadius: 3
                            color: JSON.parse(Core.bookLook(bookRow.modelData.color || "")).cloth
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 2

                            Body {
                                text: bookRow.modelData.name
                                bold: true
                                font.pixelSize: 16
                                elide: Text.ElideRight
                                maximumLineCount: 1
                                Layout.fillWidth: true
                            }

                            Body {
                                text: Core.plural(bookRow.modelData.recipeCount, "recipe")
                                muted: true
                                font.pixelSize: 13
                                font.weight: Font.DemiBold
                                Layout.fillWidth: true
                            }
                        }
                    }

                    HoverHandler {
                        id: rowHover
                        cursorShape: Qt.PointingHandCursor
                    }

                    TapHandler {
                        onTapped: if (!page.busy)
                            page.addToCookbook(bookRow.modelData)
                    }
                }
            }
        }
    }

    // ─── Delete confirmation ───
    Modal {
        id: deleteModal
        title: "Delete recipes?"
        description: Core.plural(page.selectedCount, "recipe") + " will be permanently deleted."
        footer: [
            CrumbButton {
                kind: "ghost"
                text: "Cancel"
                onClicked: deleteModal.close()
            },
            CrumbButton {
                kind: "danger"
                text: "Delete"
                enabled: !page.busy
                onClicked: page.deleteSelected()
            }
        ]
    }
}
