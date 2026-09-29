import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// Suggestions: Wee Chef's checks (status, Check all with progress), the recipes with lines to
// review, and the staples tip. Mirrors web/src/islands/SuggestionsPage.svelte.
Item {
    id: page
    property var session
    property int routeId: 0
    property var params: ({})

    // null until the first answers arrive
    property var recipes: null
    // Wee Chef's status, or null when checks aren't set up on the server
    property var checks: null
    property var staples: null
    property string loadError: ""
    property bool starting: false
    // The most waiting at once in this run: progress is how many of those are through
    property int run: 0
    property real pollDelay: 1500
    property bool watching: false

    readonly property bool running: !!checks && checks.pending > 0
    readonly property int progress: checks && run > 0 ? Math.round((run - checks.pending) / run * 100) : 0
    readonly property int topStaple: {
        var top = 1
        if (staples)
            for (var i = 0; i < staples.staples.length; i++)
                top = Math.max(top, staples.staples[i].recipes)
        return top
    }

    // The web's summary line for a recipe: "2 steps · 1 ingredient"
    readonly property var names: ({
        "ingredients": "ingredient",
        "instructions": "step",
        "notes": "note",
        "totalTime": "time",
        "image": "broken photo link"
    })

    function summary(fields) {
        var entries = Object.keys(fields || {}).map(function (f) { return [f, fields[f]] })
        entries.sort(function (a, b) { return b[1] - a[1] })
        return entries.map(function (e) {
            return Core.plural(e[1], page.names[e[0]] || "thing")
        }).join(" · ")
    }

    function setChecks(status) {
        checks = status && status.enabled ? status : null
        var pending = checks ? checks.pending : 0
        if (pending === 0)
            run = 0
        else if (pending > run)
            run = pending
        if (pending > 0)
            watch()
    }

    function setRecipes(list) {
        recipes = list || []
        ApplicationWindow.window.refreshNav()
    }

    function load() {
        var waiting = 3
        function done() {
            waiting -= 1
            if (waiting === 0 && page.recipes === null)
                page.recipes = []
        }
        requests.call("checksReview", {}, function (list) {
            page.setRecipes(list)
            done()
        }, function (error) {
            page.loadError = error
            done()
        })
        requests.call("staples", {}, function (s) {
            page.staples = s
            done()
        }, function () { done() })
        requests.call("checksStatus", {}, function (c) {
            page.setChecks(c)
            done()
        }, function () { done() })
    }

    // While a Check all runs: polls its progress with backoff (quicker again whenever a
    // recipe finishes) and reloads the list as results arrive.
    function watch() {
        if (watching)
            return
        watching = true
        poller.interval = pollDelay
        poller.restart()
    }

    function poll() {
        var before = checks
        requests.call("checksStatus", {}, function (next) {
            page.checks = next && next.enabled ? next : null
            var moved = !before || !page.checks
                || page.checks.checked !== before.checked || page.checks.toCheck !== before.toCheck
            if (moved || (page.checks && page.checks.pending === 0))
                page.refreshList()
            page.pollDelay = moved ? 1500 : Math.min(page.pollDelay * 1.5, 8000)
            page.pollDone()
        }, function () {
            page.pollDelay = Math.min(page.pollDelay * 2, 10000)
            page.pollDone()
        })
    }

    function pollDone() {
        watching = false
        if (checks && checks.pending > 0)
            watch()
        else
            run = 0
    }

    function refreshList() {
        requests.call("checksReview", {}, function (list) {
            page.setRecipes(list)
        }, function () {})
    }

    function checkAll() {
        if (starting || running || !checks || !checks.due)
            return
        starting = true
        requests.call("checkAll", {}, function (status) {
            page.starting = false
            page.setChecks(status)
            if (!status.queued)
                ApplicationWindow.window.toast({ "title": "Every recipe has been checked" })
        }, function (error) {
            page.starting = false
            ApplicationWindow.window.toast({
                "title": "Couldn't start the checks",
                "description": error,
                "tone": "error"
            })
        })
    }

    Component.onCompleted: load()

    Requests {
        id: requests
    }

    Timer {
        id: poller
        repeat: false
        onTriggered: page.poll()
    }

    ScrollPage {
        anchors.fill: parent
        maxWidth: 640

        ColumnLayout {
            width: parent.width
            spacing: 24

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 4

                Heading {
                    level: 1
                    text: "Suggestions"
                    Layout.fillWidth: true
                }

                Body {
                    muted: true
                    text: "Wee Chef reads your recipes and flags anything worth a look. Nothing changes until you say so."
                    Layout.fillWidth: true
                }
            }

            // Loading
            Rectangle {
                visible: page.recipes === null
                Layout.fillWidth: true
                Layout.preferredHeight: 112
                radius: 12
                color: Palette.tint
            }
            Rectangle {
                visible: page.recipes === null
                Layout.fillWidth: true
                Layout.preferredHeight: 192
                radius: 12
                color: Palette.tint
            }

            Body {
                visible: page.recipes !== null && page.loadError !== "" && page.recipes.length === 0
                muted: true
                text: page.loadError
                Layout.fillWidth: true
            }

            // Wee Chef checks
            Card {
                visible: page.recipes !== null && !!page.checks
                Layout.fillWidth: true
                Layout.preferredHeight: checksLayout.implicitHeight + 40

                GridLayout {
                    id: checksLayout
                    x: 20
                    y: 20
                    width: parent.width - 40
                    columns: parent.width >= 560 ? 2 : 1
                    columnSpacing: 16
                    rowSpacing: 16

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        Rectangle {
                            Layout.preferredWidth: 48
                            Layout.preferredHeight: 48
                            Layout.alignment: Qt.AlignTop
                            radius: 12
                            color: Palette.tint

                            Icon {
                                anchors.centerIn: parent
                                name: "chef-hat"
                                size: 24
                                color: Palette.primary
                            }
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 2

                            Body {
                                text: "Wee Chef checks"
                                bold: true
                                Layout.fillWidth: true
                            }

                            Body {
                                muted: true
                                font.pixelSize: 14
                                text: page.checks ? Core.checksStatusText(JSON.stringify(page.checks), page.run) : ""
                                Layout.fillWidth: true
                            }

                            Rectangle {
                                visible: page.running
                                Layout.fillWidth: true
                                Layout.topMargin: 8
                                Layout.preferredHeight: 4
                                radius: 2
                                color: Palette.tint
                                clip: true

                                Rectangle {
                                    width: parent.width * page.progress / 100
                                    height: parent.height
                                    radius: 2
                                    color: Palette.tile
                                }
                            }
                        }
                    }

                    CrumbButton {
                        kind: "primary"
                        enabled: !page.running && !page.starting && !!page.checks && page.checks.due > 0
                        iconName: page.running || page.starting ? "loader-circle" : "clipboard-check"
                        text: page.running || page.starting ? "Checking…" : "Check all with Wee Chef"
                        Layout.fillWidth: checksLayout.columns === 1
                        Layout.alignment: Qt.AlignVCenter
                        onClicked: page.checkAll()
                    }
                }
            }

            // Nothing to review
            EmptyState {
                visible: page.recipes !== null && page.recipes.length === 0 && page.loadError === ""
                iconName: "sparkles"
                title: "Nothing to review"
                text: page.checks ? "Wee Chef will flag anything it isn't sure about."
                                  : "Wee Chef will flag any photo link that stops working."
                Layout.fillWidth: true
            }

            // Lines to review
            Card {
                visible: page.recipes !== null && page.recipes.length > 0
                Layout.fillWidth: true
                Layout.preferredHeight: rows.implicitHeight
                clip: true

                Column {
                    id: rows
                    width: parent.width

                    Repeater {
                        model: page.recipes || []

                        SuggestionRow {
                            required property var modelData
                            required property int index
                            width: rows.width
                            recipe: modelData
                            first: index === 0
                            summary: page.summary(modelData.fields)
                            onOpened: ApplicationWindow.window.go("edit", { "id": modelData.id })
                        }
                    }
                }
            }

            // The staples tip
            Card {
                visible: page.recipes !== null && !!page.staples && page.staples.staples.length > 0
                Layout.fillWidth: true
                Layout.preferredHeight: tip.implicitHeight + 40

                ColumnLayout {
                    id: tip
                    x: 20
                    y: 20
                    width: parent.width - 40
                    spacing: 16

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        Rectangle {
                            Layout.preferredWidth: 48
                            Layout.preferredHeight: 48
                            Layout.alignment: Qt.AlignTop
                            radius: 12
                            color: Palette.tint

                            Icon {
                                anchors.centerIn: parent
                                name: "shopping-basket"
                                size: 24
                                color: Palette.primary
                            }
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0

                            Body {
                                text: "Wee Chef tip"
                                color: Palette.primary
                                font.pixelSize: 13
                                font.weight: Font.Bold
                                font.letterSpacing: 0.5
                                font.capitalization: Font.AllUppercase
                            }

                            Body {
                                text: "Your most used ingredients. Essentials like Salt & Pepper are skipped."
                                bold: true
                                Layout.fillWidth: true
                            }
                        }
                    }

                    GridLayout {
                        columns: 3
                        columnSpacing: 16
                        rowSpacing: 16
                        Layout.fillWidth: true

                        Repeater {
                            model: page.staples ? page.staples.staples : []

                            Rectangle {
                                id: staple
                                required property var modelData
                                Layout.preferredWidth: (tip.width - 32) / 3
                                Layout.preferredHeight: stapleColumn.implicitHeight + 28
                                radius: 12
                                color: "transparent"
                                border.width: 1
                                border.color: Palette.line

                                HoverHandler {
                                    id: stapleHover
                                }

                                ColumnLayout {
                                    id: stapleColumn
                                    x: 16
                                    y: 14
                                    width: parent.width - 32
                                    spacing: 10

                                    Body {
                                        text: staple.modelData.name.charAt(0).toUpperCase() + staple.modelData.name.slice(1)
                                        bold: true
                                        lineHeight: 1.1
                                        wrapMode: Text.WrapAnywhere
                                        Layout.fillWidth: true
                                    }

                                    StapleBar {
                                        share: staple.modelData.recipes / page.topStaple
                                        Layout.fillWidth: true
                                    }
                                }

                                Rectangle {
                                    // In N of M recipes, on hover as the web's title tooltip
                                    visible: stapleHover.hovered
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    anchors.bottom: parent.top
                                    anchors.bottomMargin: 4
                                    width: hint.implicitWidth + 16
                                    height: hint.implicitHeight + 8
                                    radius: 8
                                    color: Palette.paper
                                    border.width: 1
                                    border.color: Palette.line
                                    z: 5

                                    Body {
                                        id: hint
                                        anchors.centerIn: parent
                                        muted: true
                                        font.pixelSize: 13
                                        text: "In " + staple.modelData.recipes + " of " + page.staples.recipes + " recipes"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
