import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// A cookbook, as the web's islands/CookbookPage.svelte: its spine, name and recipes; Rename
// (name, description, cover colour), Share (link, text, file), Delete, and a remove button on
// each recipe. Cover cloth comes from Core.bookLook, the same in light and dark.
Item {
    id: page

    property var session
    property int routeId: 0
    property var params: ({})
    property int cookbookId: routeId

    property var cookbook: null
    property bool loading: true
    property bool editing: false
    // Its share link, if it has one
    property var share: null

    readonly property var look: JSON.parse(Core.bookLook(cookbook && cookbook.color ? cookbook.color : ""))
    readonly property int viewport: ApplicationWindow.window ? ApplicationWindow.window.width : 1280
    readonly property int columns: viewport >= 1280 ? 5 : viewport >= 1024 ? 4 : viewport >= 640 ? 3 : 2

    function refresh() {
        requests.call("cookbook", { "id": page.cookbookId }, function (book) {
            page.cookbook = book
            page.loading = false
        }, function () {
            page.loading = false
        })
    }

    function startEdit() {
        form.name = cookbook.name
        form.description = cookbook.description || ""
        form.color = JSON.parse(Core.bookLook(cookbook.color || "")).name
        page.editing = true
        form.focusName()
    }

    function saveEdit() {
        requests.call("patchCookbook", {
            "id": page.cookbookId,
            "patch": { "name": form.name, "description": form.description, "color": form.color }
        }, function () {
            page.editing = false
            page.refresh()
        }, function (error) {
            ApplicationWindow.window.toast({ "title": "Couldn't save", "description": error, "tone": "error" })
        })
    }

    function removeRecipe(recipeId) {
        requests.call("removeFromCookbook", { "id": page.cookbookId, "recipeId": recipeId }, function () {
            page.refresh()
        }, function (error) {
            ApplicationWindow.window.toast({ "title": "Couldn't remove recipe", "description": error, "tone": "error" })
        })
    }

    function deleteCookbook() {
        requests.call("deleteCookbook", { "id": page.cookbookId }, function () {
            ApplicationWindow.window.go("shelf", {}, true)
            ApplicationWindow.window.toast({ "title": "Cookbook deleted" })
        }, function (error) {
            deleteModal.close()
            ApplicationWindow.window.toast({ "title": "Couldn't delete", "description": error, "tone": "error" })
        })
    }

    Requests {
        id: requests
    }

    Component.onCompleted: {
        refresh()
        requests.call("shares", {}, function (list) {
            for (var i = 0; i < list.length; i++) {
                if (list[i].kind === "cookbook" && list[i].id === page.cookbookId)
                    page.share = list[i]
            }
        }, function () {})
    }

    ScrollPage {
        anchors.fill: parent

        // Loading
        ColumnLayout {
            visible: page.loading
            width: parent.width
            spacing: 28

            RowLayout {
                spacing: 16
                Layout.alignment: Qt.AlignBottom

                Rectangle {
                    Layout.preferredWidth: 48
                    Layout.preferredHeight: 128
                    topLeftRadius: 3
                    topRightRadius: 3
                    color: Palette.tint
                }

                ColumnLayout {
                    spacing: 10

                    Rectangle {
                        Layout.preferredWidth: 280
                        Layout.preferredHeight: 32
                        radius: 12
                        color: Palette.tint
                    }

                    Rectangle {
                        Layout.preferredWidth: 140
                        Layout.preferredHeight: 16
                        radius: 12
                        color: Palette.tint
                    }
                }
            }
        }

        // Not found
        ColumnLayout {
            visible: !page.loading && !page.cookbook
            width: parent.width
            spacing: 0

            EmptyState {
                iconName: "file-question"
                title: "Cookbook not found"
                actionText: "All cookbooks"
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: 48
                onAction: ApplicationWindow.window.go("shelf", {})
            }
        }

        ColumnLayout {
            visible: !page.loading && !!page.cookbook
            width: parent.width
            spacing: 0

            CrumbButton {
                kind: "ghost"
                text: "Shelf"
                iconName: "arrow-left"
                Layout.leftMargin: -12
                Layout.bottomMargin: 12
                onClicked: ApplicationWindow.window.go("shelf", {})
            }

            // Rename
            Card {
                visible: page.editing
                Layout.fillWidth: true
                Layout.maximumWidth: 672
                Layout.preferredHeight: editColumn.implicitHeight + 40

                ColumnLayout {
                    id: editColumn
                    x: 20
                    y: 20
                    width: parent.width - 40
                    spacing: 16

                    BookForm {
                        id: form
                        large: true
                        Layout.fillWidth: true
                        onAccepted: page.saveEdit()
                    }

                    RowLayout {
                        spacing: 8
                        Layout.topMargin: 4

                        CrumbButton {
                            kind: "primary"
                            text: "Save"
                            onClicked: page.saveEdit()
                        }

                        CrumbButton {
                            kind: "ghost"
                            text: "Cancel"
                            onClicked: page.editing = false
                        }
                    }
                }
            }

            RowLayout {
                visible: !page.editing
                spacing: 12
                Layout.fillWidth: true

                RowLayout {
                    spacing: 16
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignBottom

                    // The book's spine, standing on a strip of the shelf plank
                    Item {
                        Layout.preferredWidth: 48
                        Layout.preferredHeight: 136
                        Layout.alignment: Qt.AlignBottom
                        Accessible.ignored: true

                        Rectangle {
                            width: 48
                            height: 128
                            topLeftRadius: 3
                            topRightRadius: 3
                            color: page.look.cloth
                            clip: true

                            // A darker fore-edge
                            Rectangle {
                                x: parent.width * 0.78
                                width: parent.width - x
                                height: parent.height
                                gradient: Gradient {
                                    orientation: Gradient.Horizontal
                                    GradientStop { position: 0; color: "transparent" }
                                    GradientStop { position: 1; color: Qt.rgba(Qt.color(page.look.shade).r, Qt.color(page.look.shade).g, Qt.color(page.look.shade).b, 0.55) }
                                }
                            }

                            // Foil bands near the top and bottom
                            Rectangle {
                                y: 10
                                width: parent.width
                                height: 2
                                color: page.look.foil
                                opacity: 0.85
                            }
                            Rectangle {
                                y: parent.height - 12
                                width: parent.width
                                height: 2
                                color: page.look.foil
                                opacity: 0.85
                            }

                            // A faint inset line so pale cloths still read against the page
                            Rectangle {
                                anchors.fill: parent
                                topLeftRadius: 3
                                topRightRadius: 3
                                color: "transparent"
                                border.width: 1
                                border.color: Palette.dark ? Qt.rgba(1, 1, 1, 0.14) : Qt.rgba(28 / 255, 43 / 255, 34 / 255, 0.22)
                            }

                            Item {
                                anchors.fill: parent
                                anchors.topMargin: 14
                                anchors.bottomMargin: 14

                                Text {
                                    anchors.centerIn: parent
                                    width: parent.height
                                    horizontalAlignment: Text.AlignHCenter
                                    text: page.cookbook ? page.cookbook.name : ""
                                    color: page.look.foil
                                    font.family: Palette.fontSerif
                                    font.pixelSize: 14
                                    elide: Text.ElideRight
                                    rotation: -90
                                }
                            }
                        }

                        Rectangle {
                            x: -6
                            y: 128
                            width: 60
                            height: 8
                            radius: 4
                            color: Palette.tile
                        }
                    }

                    ColumnLayout {
                        spacing: 6
                        Layout.fillWidth: true
                        Layout.alignment: Qt.AlignBottom
                        Layout.bottomMargin: 8

                        Heading {
                            level: 1
                            text: page.cookbook ? page.cookbook.name : ""
                            font.pixelSize: 30
                            lineHeight: 1.05
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }

                        Body {
                            text: page.cookbook ? page.cookbook.recipes.length + " recipe" + (page.cookbook.recipes.length === 1 ? "" : "s") : ""
                            muted: true
                            font.pixelSize: 13
                            font.weight: Font.DemiBold
                            Layout.fillWidth: true
                        }
                    }
                }

                RowLayout {
                    spacing: 8
                    Layout.alignment: Qt.AlignBottom
                    Layout.bottomMargin: 6

                    CrumbButton {
                        kind: "outline"
                        iconName: "share"
                        Accessible.name: "Share"
                        onClicked: shareSheet.open()

                        ToolTip.visible: hovered
                        ToolTip.text: "Share"
                        ToolTip.delay: 600
                    }

                    CrumbButton {
                        id: more
                        kind: "outline"
                        iconName: "ellipsis"
                        Accessible.name: "More"
                        onClicked: menu.popup(more)
                    }
                }
            }

            Body {
                visible: !page.editing && !!page.cookbook && !!page.cookbook.description
                text: page.cookbook && page.cookbook.description ? page.cookbook.description : ""
                muted: true
                font.pixelSize: 16
                Layout.topMargin: 16
                Layout.fillWidth: true
                Layout.maximumWidth: 672
            }

            Item {
                Layout.preferredHeight: 28
            }

            // Empty
            EmptyState {
                visible: !!page.cookbook && page.cookbook.recipes.length === 0
                iconName: "book-open"
                title: "This cookbook is empty"
                text: "Open a recipe and tap the cookbook name, or use Select on the recipes page."
                actionText: "Browse recipes"
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: 32
                onAction: ApplicationWindow.window.go("recipes", {})
            }

            Grid {
                id: grid

                readonly property real gap: page.viewport >= 640 ? 16 : 12

                visible: !!page.cookbook && page.cookbook.recipes.length > 0
                columns: page.columns
                columnSpacing: gap
                rowSpacing: 20
                Layout.fillWidth: true

                Repeater {
                    model: page.cookbook ? page.cookbook.recipes : []

                    delegate: Item {
                        id: cell

                        required property var modelData

                        width: (grid.width - (grid.columns - 1) * grid.gap) / grid.columns
                        height: card.implicitHeight

                        RecipeCard {
                            id: card
                            width: parent.width
                            recipe: cell.modelData
                        }

                        Rectangle {
                            x: parent.width - 52
                            y: 8
                            width: 44
                            height: 44
                            radius: 12
                            color: removeHover.hovered ? Palette.tint : Palette.paper
                            border.width: 1
                            border.color: Palette.line
                            activeFocusOnTab: true
                            Accessible.role: Accessible.Button
                            Accessible.name: "Remove " + cell.modelData.title + " from cookbook"
                            Accessible.onPressAction: page.removeRecipe(cell.modelData.id)
                            Keys.onReturnPressed: page.removeRecipe(cell.modelData.id)
                            Keys.onSpacePressed: page.removeRecipe(cell.modelData.id)

                            HoverHandler {
                                id: removeHover
                                cursorShape: Qt.PointingHandCursor
                            }

                            TapHandler {
                                onTapped: page.removeRecipe(cell.modelData.id)
                            }

                            ToolTip.visible: removeHover.hovered
                            ToolTip.text: "Remove from cookbook"
                            ToolTip.delay: 600

                            Icon {
                                anchors.centerIn: parent
                                name: "x"
                                size: 20
                                color: Palette.text
                            }

                            Rectangle {
                                visible: parent.activeFocus
                                anchors.fill: parent
                                radius: 12
                                color: "transparent"
                                border.width: 2
                                border.color: Palette.primary
                            }
                        }
                    }
                }
            }
        }
    }

    CrumbMenu {
        id: menu
        groups: [
            [{ "label": "Rename", "icon": "pencil", "onselect": function () { page.startEdit() } }],
            [{ "label": "Delete cookbook", "icon": "trash-2", "danger": true, "onselect": function () { deleteModal.open() } }]
        ]
    }

    CookbookShareSheet {
        id: shareSheet
        cookbook: page.cookbook || ({})
        share: page.share
        onLinkChanged: (link) => page.share = link
    }

    Modal {
        id: deleteModal
        title: "Delete this cookbook?"
        description: page.share ? "Recipes in it are kept. Its share link stops working." : "Recipes in it are kept."

        footer: [
            CrumbButton {
                kind: "ghost"
                text: "Cancel"
                onClicked: deleteModal.close()
            },
            CrumbButton {
                kind: "danger"
                text: "Delete"
                onClicked: page.deleteCookbook()
            }
        ]
    }
}
