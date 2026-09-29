import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// A recipe read from a site before it's saved (the web's /preview: src/preview.rs and
// islands/PreviewActions.svelte), for a link from Popular. `params.url` is the page. "Add to
// my Crumb" saves it like the Add box does (the server keeps the reading it just showed);
// the other states are the page before that: reading, or why it couldn't be read. A link
// already in the box opens its recipe, and a cooking video or shared cookbook is imported as
// the Add box would, since there's nothing to preview.
Item {
    id: page

    property var session
    property int routeId: 0
    property var params: ({})

    readonly property string url: params.url || ""
    readonly property string host: Core.hostOf(url).replace(/^www\./, "") || "the site"

    // "reading", "ready" or "failed"
    property string step: "reading"
    property var recipe: null
    property string message: ""
    // Why it failed, when the server says: `site_blocked` or `site_terms`
    property string code: ""
    property string site: ""
    property bool saving: false
    property string progress: ""
    property real scale: 1

    readonly property var window: page.ApplicationWindow.window
    readonly property bool wide: width >= 784
    readonly property string image: recipe && recipe.image ? Core.webLink(recipe.image) : ""
    readonly property string sub: recipe ? Core.kicker(recipe.recipeCategory || "", recipe.recipeCuisine || "") : ""
    readonly property var metaItems: {
        if (!recipe)
            return []
        var all = [
            {"icon": "timer", "label": "Prep", "value": recipe.prepTime},
            {"icon": "flame", "label": "Cook", "value": recipe.cookTime},
            {"icon": "snowflake", "label": "Extra", "value": recipe.freezeTime},
            {"icon": "clock", "label": "Total", "value": recipe.totalTime}
        ]
        var shown = []
        for (var i = 0; i < all.length; i++) {
            var value = Core.displayDuration(all[i].value || "")
            if (value)
                shown.push({"icon": all[i].icon, "label": all[i].label, "value": value})
        }
        var y = recipe.recipeYield || ""
        if (y)
            shown.push({"icon": "users", "label": "Serves", "value": scale !== 1 ? Core.scaleIngredient(y, scale) : y})
        return shown
    }

    function read() {
        page.step = "reading"
        page.recipe = null
        requests.call("preview", {"url": page.url}, function (preview) {
            if (preview.status === "saved") {
                page.window.go("recipe", {"id": preview.id}, true)
            } else if (preview.status === "import") {
                page.add()
            } else {
                page.recipe = preview.recipe
                page.step = "ready"
            }
        }, function (error, info) {
            page.message = error
            page.code = info.code || ""
            page.site = info.site || ""
            page.step = "failed"
        })
    }

    function add() {
        if (page.saving)
            return
        page.saving = true
        page.progress = ""
        requests.call("import", {"url": page.url}, function (res) {
            if (res.cookbook) {
                var b = res.cookbook
                page.window.toast({
                    "title": Core.bookImportedTitle(b.name, b.added, b.duplicates || 0, b.skipped === undefined ? -1 : b.skipped),
                    "tone": b.added ? "success" : "default"
                })
                page.window.go("cookbook", {"id": b.id}, true)
                return
            }
            page.window.toast(res.isNew ? {"title": "Added to your box", "tone": "success"} : {"title": "Already in your box"})
            page.window.go("recipe", {"id": res.recipe.id}, true)
        }, function (error, info) {
            page.saving = false
            page.progress = ""
            // A video or cookbook went straight to saving: show why here, as a failed read
            if (page.step !== "ready") {
                page.message = error
                page.code = info.code || ""
                page.site = info.site || ""
                page.step = "failed"
                return
            }
            page.window.toast({"title": "Couldn't add it", "description": error, "tone": "error"})
        }, function (line) {
            page.progress = line
        })
    }

    Requests {
        id: requests
    }

    Component.onCompleted: {
        if (page.url)
            page.read()
        else
            page.window.back()
    }

    ScrollPage {
        id: scroller
        anchors.fill: parent
        maxWidth: 1420

        ColumnLayout {
            width: scroller.contentWidthAvailable
            spacing: page.wide ? 64 : 48

            GridLayout {
                id: header
                Layout.fillWidth: true
                columns: page.wide && page.image !== "" ? 2 : 1
                columnSpacing: 48
                rowSpacing: 24

                Photo {
                    visible: page.image !== ""
                    direct: true
                    image: page.image
                    Layout.preferredWidth: page.wide ? Math.max(0, Math.min(560, header.width - 48 - 256)) : header.width
                    Layout.preferredHeight: Layout.preferredWidth * 3 / 4
                    Layout.alignment: Qt.AlignTop
                    Accessible.role: Accessible.Graphic
                    Accessible.name: page.recipe ? page.recipe.title : ""
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignTop
                    Layout.topMargin: page.wide ? 8 : 0
                    spacing: 20

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Text {
                            readonly property string line: page.step === "ready" ? page.sub : "From " + page.host
                            visible: line !== ""
                            text: line
                            color: Palette.primary
                            font.family: Palette.fontSans
                            font.pixelSize: 13
                            font.weight: Font.Bold
                            font.capitalization: Font.AllUppercase
                            font.letterSpacing: 0.5
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                        Heading {
                            level: 1
                            text: page.step === "ready" && page.recipe ? page.recipe.title
                                  : page.step === "failed" ? "Couldn't read that recipe" : "Reading the recipe…"
                            font.pixelSize: 30
                            lineHeight: 0.95
                            elide: Text.ElideNone
                            Layout.fillWidth: true
                            Accessible.role: Accessible.Heading
                        }
                        Body {
                            visible: page.step === "ready" && !!(page.recipe && page.recipe.author)
                            text: page.recipe && page.recipe.author ? "By " + page.recipe.author : ""
                            muted: true
                            Layout.fillWidth: true
                        }
                        Body {
                            readonly property string line: page.step === "failed" ? page.message
                                                           : page.step === "ready" && page.recipe ? (page.recipe.description || "") : ""
                            visible: line !== ""
                            text: line
                            Layout.fillWidth: true
                        }
                    }

                    // Time and yield
                    Card {
                        id: facts
                        visible: page.step === "ready" && page.metaItems.length > 0
                        Layout.fillWidth: true
                        readonly property int cols: Math.max(1, Math.min(page.metaItems.length, Math.floor((width - 8) / 88)))
                        implicitHeight: factGrid.implicitHeight + 12

                        GridLayout {
                            id: factGrid
                            x: 4
                            y: 6
                            width: parent.width - 8
                            columns: facts.cols
                            columnSpacing: 0
                            rowSpacing: 0

                            Repeater {
                                model: page.metaItems

                                delegate: ColumnLayout {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 1
                                    Layout.alignment: Qt.AlignTop
                                    Layout.margins: 14
                                    Layout.topMargin: 12
                                    Layout.bottomMargin: 12
                                    spacing: 4

                                    Row {
                                        spacing: 6

                                        Icon {
                                            anchors.verticalCenter: parent.verticalCenter
                                            name: modelData.icon
                                            size: 16
                                            color: Palette.primary
                                        }
                                        Text {
                                            anchors.verticalCenter: parent.verticalCenter
                                            text: modelData.label
                                            color: Palette.textMuted
                                            font.family: Palette.fontSans
                                            font.pixelSize: 13
                                            font.weight: Font.DemiBold
                                        }
                                    }
                                    Body {
                                        text: modelData.value
                                        bold: true
                                        Layout.fillWidth: true
                                    }
                                }
                            }
                        }
                    }

                    // The actions for each state
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        CrumbButton {
                            visible: page.step === "ready"
                            kind: "primary"
                            iconName: page.saving ? "loader-circle" : "plus"
                            text: page.saving ? "Adding…" : "Add to my Crumb"
                            enabled: !page.saving
                            onClicked: page.add()
                        }
                        Body {
                            visible: page.step === "ready"
                            text: page.progress || "Not in your box yet."
                            muted: true
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }

                        CrumbButton {
                            visible: page.step === "reading"
                            kind: "primary"
                            iconName: "loader-circle"
                            text: page.saving && page.progress ? page.progress : "Reading " + page.host + "…"
                            enabled: false
                        }

                        BlockedNudge {
                            visible: page.step === "failed" && (page.code === "site_terms" || page.code === "site_blocked")
                            url: page.url
                            why: page.code === "site_terms" ? "terms" : "bot"
                            name: page.site
                            Layout.fillWidth: true
                            onPaste: page.window.go("add", {})
                        }

                        Flow {
                            visible: page.step === "failed" && page.code !== "site_terms"
                            Layout.fillWidth: true
                            spacing: 8

                            CrumbButton {
                                kind: page.code === "site_blocked" ? "ghost" : "primary"
                                iconName: "rotate-cw"
                                text: "Try again"
                                onClicked: page.read()
                            }
                            CrumbButton {
                                visible: page.code !== "site_blocked"
                                kind: "soft"
                                text: "Paste it into Add"
                                onClicked: page.window.go("add", {})
                            }
                        }

                        CrumbButton {
                            visible: page.step !== "ready"
                            kind: "ghost"
                            iconName: "external-link"
                            text: "Back to " + page.host
                            Layout.leftMargin: -12
                            onClicked: Qt.openUrlExternally(page.url)
                        }
                    }
                }
            }

            // The web's skeleton lines while the site is read
            Column {
                visible: page.step === "reading"
                Layout.fillWidth: true
                spacing: 8

                Repeater {
                    model: 6

                    Rectangle {
                        width: parent.width
                        height: 44
                        radius: 12
                        color: Palette.tint
                    }
                }
            }

            // Ingredients beside the method on wide screens, as the recipe page
            GridLayout {
                visible: page.step === "ready"
                Layout.fillWidth: true
                columns: page.wide ? 2 : 1
                columnSpacing: 56
                rowSpacing: 48

                RecipeIngredients {
                    recipe: page.recipe
                    scale: page.scale
                    Layout.fillWidth: true
                    Layout.preferredWidth: page.wide ? 2 : 1
                    Layout.alignment: Qt.AlignTop
                    onScaleEdited: value => page.scale = value
                }

                RecipeMethod {
                    recipe: page.recipe
                    Layout.fillWidth: true
                    Layout.preferredWidth: page.wide ? 3 : 1
                    Layout.alignment: Qt.AlignTop
                }
            }
        }
    }
}
