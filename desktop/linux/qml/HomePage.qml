import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// Home, as the web's pages/index.astro and islands/HomeFeed.svelte: a tile header with the
// greeting and the Add box, then Pick up where you left off, Can't decide?, the shelf and
// Fresh in the box. Wording and dates come from Core.
Item {
    id: page

    property var session
    property int routeId: 0
    property var params: ({})

    property var recipes: []
    property var cookbooks: []
    property bool loading: true
    property var opened: null
    // Recently viewed: {id, title, image, at, steps}
    property var recent: []

    readonly property real now: Date.now()
    readonly property int offset: -new Date().getTimezoneOffset()
    readonly property int viewport: ApplicationWindow.window ? ApplicationWindow.window.width : 1280
    readonly property int columns: viewport >= 1280 ? 6 : viewport >= 640 ? 3 : 2

    function loadRecent() {
        var all = JSON.parse(Store.read("crumb:recent", "[]"))
        page.recent = all.slice(0, 3).map(function (v) {
            var step = JSON.parse(Store.readSession("crumb:cook:" + v.id, "null"))
            var progress = JSON.parse(Core.cookProgress(step === null ? -1 : step, v.steps || -1))
            return {
                "id": v.id,
                "title": v.title,
                "image": v.image || "",
                "line": Core.viewedLine(v.at || 0, page.now, page.offset),
                "step": progress ? progress[0] : 0,
                "of": progress ? progress[1] : 0
            }
        })
    }

    // Surprise me: one recipe at random, skipping what this session served and what was opened lately
    function surprise() {
        var seen = JSON.parse(Store.readSession("crumb:random:seen", "[]"))
        var exclude = seen.slice()
        JSON.parse(Store.read("crumb:recent", "[]")).forEach(function (v) {
            if (exclude.indexOf(v.id) < 0)
                exclude.push(v.id)
        })
        requests.call("random", { "exclude": exclude }, function (recipe) {
            var next = [recipe.id].concat(seen.filter(function (s) { return s !== recipe.id })).slice(0, 40)
            Store.writeSession("crumb:random:seen", JSON.stringify(next))
            ApplicationWindow.window.go("recipe", { "id": recipe.id })
        })
    }

    Requests {
        id: requests
        onFailed: (error) => ApplicationWindow.window.toast({ "title": "Couldn't do that", "description": error, "tone": "error" })
    }

    Component.onCompleted: {
        loadRecent()
        requests.call("recipes", { "limit": 8 }, function (list) {
            page.recipes = list
            page.loading = false
        }, function () {
            page.loading = false
        })
        requests.call("cookbooks", {}, function (list) {
            page.cookbooks = list
        }, function () {})
    }

    Flickable {
        id: flick

        anchors.fill: parent
        clip: true
        contentWidth: width
        contentHeight: column.implicitHeight
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}

        Column {
            id: column
            width: flick.width

            // The glossy backsplash tile, with the greeting and the Add box
            Rectangle {
                id: hero

                width: parent.width
                height: heroColumn.implicitHeight + 40 + 36
                color: Palette.tile
                clip: true

                Repeater {
                    model: Math.ceil(hero.width / 32) + 1
                    Rectangle {
                        required property int index
                        x: index * 32 - 1
                        width: 2
                        height: hero.height
                        color: Qt.rgba(0, 0, 0, 0.16)
                    }
                }

                Repeater {
                    model: Math.ceil(hero.height / 32) + 1
                    Rectangle {
                        required property int index
                        y: index * 32 - 1
                        width: hero.width
                        height: 2
                        color: Qt.rgba(0, 0, 0, 0.16)
                    }
                }

                ColumnLayout {
                    id: heroColumn

                    x: Math.max(32, (parent.width - width) / 2)
                    y: 40
                    width: Math.min(1200, parent.width - 64)
                    spacing: 14

                    Text {
                        text: Core.greeting(new Date().getHours())
                        color: Palette.onTile
                        font.family: Palette.fontHand
                        font.pixelSize: 48
                        font.weight: Font.Bold
                        lineHeight: 1
                        Layout.bottomMargin: 6
                    }

                    TopBox {
                        tabs: true
                        onTile: true
                        Layout.fillWidth: true
                        Layout.maximumWidth: 640
                    }
                }
            }

            Item {
                width: parent.width
                height: body.implicitHeight + 36 + 64

                ColumnLayout {
                    id: body

                    x: Math.max(32, (parent.width - width) / 2)
                    y: 36
                    width: Math.min(1200, parent.width - 64)
                    spacing: 36

                    GridLayout {
                        columns: page.recent.length && page.width >= 1024 ? 2 : 1
                        columnSpacing: 32
                        rowSpacing: 32
                        Layout.fillWidth: true

                        ColumnLayout {
                            visible: page.recent.length > 0
                            spacing: 14
                            Layout.fillWidth: true
                            Layout.alignment: Qt.AlignTop

                            Heading {
                                level: 2
                                text: "Pick up where you left off"
                                Layout.fillWidth: true
                            }

                            HomeListCard {
                                Layout.fillWidth: true

                                Repeater {
                                    model: page.recent

                                    delegate: HomeRow {
                                        id: recentRow

                                        required property var modelData
                                        required property int index

                                        first: index === 0
                                        label: modelData.title
                                        onClicked: ApplicationWindow.window.go(modelData.step > 0 ? "cook" : "recipe", { "id": modelData.id })

                                        Photo {
                                            recipeId: recentRow.modelData.id
                                            image: recentRow.modelData.image
                                            Layout.preferredWidth: 64
                                            Layout.preferredHeight: 64
                                        }

                                        ColumnLayout {
                                            spacing: 2
                                            Layout.fillWidth: true
                                            Layout.alignment: Qt.AlignVCenter

                                            Body {
                                                text: recentRow.modelData.title
                                                bold: true
                                                font.pixelSize: 17
                                                lineHeight: 1.05
                                                maximumLineCount: 2
                                                elide: Text.ElideRight
                                                Layout.fillWidth: true
                                            }

                                            RowLayout {
                                                visible: recentRow.modelData.step > 0
                                                spacing: 12
                                                Layout.fillWidth: true
                                                Layout.topMargin: 6

                                                Rectangle {
                                                    Layout.fillWidth: true
                                                    Layout.preferredHeight: 4
                                                    radius: 2
                                                    color: Palette.tint

                                                    Rectangle {
                                                        width: parent.width * recentRow.modelData.step / Math.max(1, recentRow.modelData.of)
                                                        height: parent.height
                                                        radius: 2
                                                        color: Palette.tile
                                                    }
                                                }

                                                Body {
                                                    text: "Step " + recentRow.modelData.step + " of " + recentRow.modelData.of
                                                    muted: true
                                                    font.pixelSize: 13
                                                    font.weight: Font.DemiBold
                                                }
                                            }

                                            Body {
                                                visible: recentRow.modelData.step === 0
                                                text: recentRow.modelData.line
                                                muted: true
                                                font.pixelSize: 14
                                                Layout.fillWidth: true
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        ColumnLayout {
                            spacing: 14
                            Layout.fillWidth: true
                            Layout.alignment: Qt.AlignTop

                            Heading {
                                level: 2
                                text: "Can't decide?"
                                Layout.fillWidth: true
                            }

                            HomeListCard {
                                Layout.fillWidth: true

                                HomeRow {
                                    first: true
                                    label: "Surprise me"
                                    onClicked: page.surprise()

                                    HomeWell {
                                        iconName: "shuffle"
                                    }

                                    ColumnLayout {
                                        spacing: 0
                                        Layout.fillWidth: true

                                        Body {
                                            text: "Surprise me"
                                            bold: true
                                            font.pixelSize: 17
                                            lineHeight: 1.05
                                            Layout.fillWidth: true
                                        }

                                        Body {
                                            text: "One recipe, picked at random"
                                            muted: true
                                            font.pixelSize: 14
                                            Layout.fillWidth: true
                                        }
                                    }

                                    Icon {
                                        name: "chevron-right"
                                        size: 20
                                        color: Palette.textMuted
                                    }
                                }

                                HomeRow {
                                    label: "What should I cook next?"
                                    onClicked: ApplicationWindow.window.go("suggestions", {})

                                    HomeWell {
                                        iconName: "layout-grid"
                                    }

                                    ColumnLayout {
                                        spacing: 0
                                        Layout.fillWidth: true

                                        Body {
                                            text: "What should I cook next?"
                                            bold: true
                                            font.pixelSize: 17
                                            lineHeight: 1.05
                                            Layout.fillWidth: true
                                        }

                                        Body {
                                            text: "Four ideas to choose from"
                                            muted: true
                                            font.pixelSize: 14
                                            Layout.fillWidth: true
                                        }
                                    }

                                    Icon {
                                        name: "chevron-right"
                                        size: 20
                                        color: Palette.textMuted
                                    }
                                }
                            }
                        }
                    }

                    ColumnLayout {
                        visible: page.cookbooks.length > 0
                        spacing: 14
                        Layout.fillWidth: true

                        HomeSectionHeader {
                            title: "The shelf"
                            onSeeAll: ApplicationWindow.window.go("shelf", {})
                        }

                        ShelfCard {
                            Layout.fillWidth: true
                            Layout.preferredHeight: shelf.implicitHeight + 20

                            Bookshelf {
                                id: shelf
                                width: parent.width
                                books: page.cookbooks.slice(0, 14)
                                single: true
                                pulledId: page.opened ? page.opened.id : 0
                                onOpenBook: (book) => {
                                    page.opened = book
                                    openBook.show(book)
                                }
                            }
                        }
                    }

                    ColumnLayout {
                        visible: page.loading || page.recipes.length > 0
                        spacing: 14
                        Layout.fillWidth: true

                        HomeSectionHeader {
                            title: "Fresh in the box"
                            onSeeAll: ApplicationWindow.window.go("recipes", {})
                        }

                        Grid {
                            id: fresh

                            readonly property real gap: page.viewport >= 640 ? 16 : 12
                            readonly property real cardWidth: (width - (columns - 1) * gap) / columns

                            columns: page.columns
                            columnSpacing: gap
                            rowSpacing: 20
                            Layout.fillWidth: true

                            Repeater {
                                model: page.loading ? 4 : Math.min(6, page.recipes.length)

                                delegate: Item {
                                    id: freshCell

                                    required property int index
                                    readonly property var recipe: page.loading ? null : page.recipes[index]

                                    // Below the small breakpoint only four show
                                    visible: index < 4 || page.viewport >= 640
                                    width: fresh.cardWidth
                                    height: visible ? (recipe ? card.implicitHeight : skeleton.height) : 0

                                    Rectangle {
                                        id: skeleton
                                        visible: freshCell.recipe === null
                                        width: parent.width
                                        height: Math.round(width * 3 / 4) + 60
                                        radius: 12
                                        color: Palette.tint

                                        SequentialAnimation on opacity {
                                            running: skeleton.visible
                                            loops: Animation.Infinite
                                            NumberAnimation { to: 0.55; duration: 800 }
                                            NumberAnimation { to: 1; duration: 800 }
                                        }
                                    }

                                    RecipeCard {
                                        id: card
                                        visible: freshCell.recipe !== null
                                        width: parent.width
                                        recipe: freshCell.recipe || ({})
                                        meta: freshCell.recipe
                                            ? Core.freshMeta(Date.parse(freshCell.recipe.createdAt), freshCell.recipe.source || "", page.now, page.offset)
                                            : ""
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    BookOpenModal {
        id: openBook
        onDismissed: page.opened = null
    }
}
