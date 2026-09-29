import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Shapes

import app.crumb.desktop 1.0

// "What should I cook next?": four recipes worth cooking soon, with Shuffle, and some days a
// last card with an idea from Wee Chef for a dish that isn't in the box yet. Mirrors
// web/src/components/TryNext.svelte (and its data, which HomeFeed.svelte loads).
Item {
    id: tryNext

    readonly property int count: 4
    readonly property string key: "crumb:suggest"
    readonly property string today: new Date().toDateString()
    readonly property int viewport: ApplicationWindow.window ? ApplicationWindow.window.width : 1280

    // Suggestions: {items, ai, idea}; null until the first answer
    property var result: null
    // Recipes in the whole box: Shuffle needs a few to choose from
    property int total: 0
    property bool loading: false
    // This session's shuffles today: {date, seed, shown}
    property var saved: ({ "date": today, "seed": 0, "shown": [] })
    property int tries: 0
    property int waiting: 2

    readonly property bool ready: result !== null && !loading
    readonly property int columns: viewport >= 640 ? 4 : 2
    readonly property real gap: viewport >= 640 ? 16 : 12

    implicitHeight: column.implicitHeight

    function ids() {
        return tryNext.result.items.map(function (i) { return i.recipe.id })
    }

    function load(seed, exclude, ok, fail) {
        requests.call("suggestions", { "limit": tryNext.count, "seed": seed, "exclude": exclude }, ok, fail)
    }

    function unique(list) {
        return list.filter(function (id, i) { return list.indexOf(id) === i })
    }

    function shuffle() {
        tryNext.loading = true
        var shown = unique(saved.shown.concat(ids()))
        var seed = saved.seed + 1

        function done() {
            tryNext.loading = false
        }
        function finish(next, kept) {
            if (next.items.length < 2) {
                ApplicationWindow.window.toast({
                    "title": "That's everything for now",
                    "description": "Cook something and check back."
                })
                done()
                return
            }
            tryNext.result = next
            tryNext.saved = { "date": tryNext.today, "seed": seed, "shown": kept }
            Store.writeSession(tryNext.key, JSON.stringify(tryNext.saved))
            done()
        }
        function fail(error) {
            ApplicationWindow.window.toast({ "title": "Couldn't shuffle", "description": error, "tone": "error" })
            done()
        }

        load(seed, shown, function (next) {
            // Seen everything: start again from the whole box
            if (next.items.length < 2)
                load(seed, ids(), function (again) { finish(again, []) }, fail)
            else
                finish(next, shown)
        }, fail)
    }

    // Both first answers are in: show that day's shuffled set again, or wait for Wee Chef's blurbs
    function start() {
        if (saved.seed > 0) {
            loading = true
            load(saved.seed, saved.shown, function (d) {
                if (d.items.length >= 2)
                    tryNext.result = d
                tryNext.loading = false
            }, function () { tryNext.loading = false })
        } else if (result.ai === "pending") {
            poller.start()
        }
    }

    function opened() {
        waiting -= 1
        if (waiting === 0)
            start()
    }

    Component.onCompleted: {
        var stored = JSON.parse(Store.readSession(key, "null"))
        if (stored && stored.date === today)
            saved = stored
        load(0, [], function (d) {
            tryNext.result = d
            tryNext.opened()
        }, function () {})
        requests.call("recipes", {}, function (list) {
            tryNext.total = list.length
            tryNext.opened()
        }, function () { tryNext.opened() })
    }

    Requests {
        id: requests
    }

    // Wee Chef is still writing blurbs: pick them up when they're ready
    Timer {
        id: poller
        interval: 4000
        repeat: true
        onTriggered: {
            tryNext.tries += 1
            tryNext.load(0, [], function (d) {
                if (d.ai === "ready" && tryNext.saved.seed === 0)
                    tryNext.result = d
                if (d.ai !== "pending" || tryNext.tries >= 3)
                    poller.stop()
            }, function () { poller.stop() })
        }
    }

    ColumnLayout {
        id: column
        width: tryNext.width
        spacing: 0

        // Loading
        Grid {
            id: skeleton

            readonly property int cols: tryNext.viewport >= 1280 ? 5 : tryNext.viewport >= 1024 ? 4 : tryNext.viewport >= 640 ? 3 : 2
            readonly property real cardWidth: (width - (cols - 1) * tryNext.gap) / cols

            visible: tryNext.result === null || tryNext.loading
            columns: cols
            columnSpacing: tryNext.gap
            rowSpacing: 20
            Layout.fillWidth: true

            Repeater {
                model: tryNext.count

                Item {
                    width: skeleton.cardWidth
                    height: Math.round(width * 3 / 4) + 10 + 16 + 8 + 14

                    Rectangle {
                        width: parent.width
                        height: Math.round(width * 3 / 4)
                        radius: 12
                        color: Palette.tint
                    }

                    Rectangle {
                        y: Math.round(parent.width * 3 / 4) + 10
                        width: parent.width * 0.8
                        height: 16
                        radius: 12
                        color: Palette.tint
                    }

                    Rectangle {
                        y: Math.round(parent.width * 3 / 4) + 10 + 16 + 8
                        width: parent.width * 0.4
                        height: 14
                        radius: 12
                        color: Palette.tint
                    }

                    SequentialAnimation on opacity {
                        running: skeleton.visible
                        loops: Animation.Infinite
                        NumberAnimation { to: 0.55; duration: 800 }
                        NumberAnimation { to: 1; duration: 800 }
                    }
                }
            }
        }

        // The picks, and Wee Chef's idea
        Grid {
            id: picks

            readonly property real cardWidth: (width - (tryNext.columns - 1) * tryNext.gap) / tryNext.columns

            visible: tryNext.ready && tryNext.result.items.length >= 2
            columns: tryNext.columns
            columnSpacing: tryNext.gap
            rowSpacing: 24
            Layout.fillWidth: true
            Layout.preferredWidth: tryNext.width

            Repeater {
                model: tryNext.ready ? tryNext.result.items : []

                RecipeCard {
                    required property var modelData

                    width: picks.cardWidth
                    recipe: modelData.recipe
                    reason: modelData.reason
                    aiReason: modelData.ai
                }
            }

            Item {
                id: idea

                readonly property var value: tryNext.ready ? tryNext.result.idea : null

                visible: !!value
                width: picks.cardWidth
                height: ideaColumn.implicitHeight
                Accessible.role: Accessible.Link
                Accessible.name: value ? value.title + ": an idea from Wee Chef, not in your box yet. Search the web for a recipe" : ""
                Accessible.onPressAction: Qt.openUrlExternally(value.searchUrl)

                HoverHandler {
                    id: ideaHover
                    cursorShape: Qt.PointingHandCursor
                }

                TapHandler {
                    onTapped: Qt.openUrlExternally(idea.value.searchUrl)
                }

                ColumnLayout {
                    id: ideaColumn
                    width: parent.width
                    spacing: 0

                    Item {
                        id: ideaBox

                        Layout.fillWidth: true
                        Layout.preferredHeight: Math.round(width * 3 / 4)

                        Rectangle {
                            anchors.fill: parent
                            radius: 12
                            color: Palette.tint
                        }

                        Shape {
                            anchors.fill: parent
                            preferredRendererType: Shape.CurveRenderer

                            ShapePath {
                                fillColor: "transparent"
                                strokeWidth: 1
                                strokeColor: Palette.line
                                strokeStyle: ShapePath.DashLine
                                dashPattern: [4, 4]

                                PathRectangle {
                                    x: 0.5
                                    y: 0.5
                                    width: ideaBox.width - 1
                                    height: ideaBox.height - 1
                                    radius: 11.5
                                }
                            }
                        }

                        ColumnLayout {
                            anchors.centerIn: parent
                            spacing: 6

                            Icon {
                                name: "chef-hat"
                                size: 44
                                color: Palette.primary
                                Layout.alignment: Qt.AlignHCenter
                                rotation: ideaHover.hovered ? -6 : 0

                                Behavior on rotation {
                                    NumberAnimation { duration: 500; easing.type: Easing.OutCubic }
                                }
                            }

                            RowLayout {
                                spacing: 4
                                Layout.alignment: Qt.AlignHCenter

                                Icon {
                                    name: "sparkles"
                                    size: 14
                                    color: Palette.primary
                                }

                                Body {
                                    text: "Idea from Wee Chef"
                                    color: Palette.primary
                                    font.pixelSize: 13
                                    font.weight: Font.Bold
                                    wrapMode: Text.NoWrap
                                }
                            }
                        }
                    }

                    Body {
                        text: idea.value ? idea.value.title : ""
                        bold: true
                        font.pixelSize: 16
                        maximumLineCount: 2
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                        Layout.topMargin: 10
                    }

                    Body {
                        text: idea.value ? idea.value.why : ""
                        muted: true
                        font.pixelSize: 14
                        maximumLineCount: 2
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                        Layout.topMargin: 4
                    }

                    RowLayout {
                        spacing: 4
                        Layout.topMargin: 4

                        Body {
                            text: "Find it"
                            color: Palette.primary
                            bold: true
                            font.pixelSize: 14
                            font.underline: ideaHover.hovered
                        }

                        Icon {
                            name: "external-link"
                            size: 14
                            color: Palette.primary
                        }
                    }
                }
            }
        }

        Body {
            visible: tryNext.ready && tryNext.result.items.length < 2
            text: "Add a few more recipes and Crumb will start suggesting some."
            muted: true
            font.pixelSize: 14
            Layout.fillWidth: true
        }

        CrumbButton {
            visible: tryNext.result !== null && tryNext.result.items.length >= 2 && tryNext.total > tryNext.count
            enabled: !tryNext.loading
            iconName: "shuffle"
            text: "Shuffle"
            Layout.alignment: Qt.AlignHCenter
            Layout.topMargin: 16
            onClicked: tryNext.shuffle()
        }
    }
}
