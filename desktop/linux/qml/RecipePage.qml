import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// A recipe's page. Content, order and wording follow the web's islands/RecipePage.svelte;
// the wording and arithmetic come from Core, the calls from Api (via Requests).
Item {
    id: page

    property var session
    property int routeId: 0
    property var params: ({})
    readonly property int recipeId: routeId

    readonly property bool smoke: Smoke.page === "recipe"
    readonly property var window: ApplicationWindow.window

    // ─── What the page shows ───
    property bool loading: true
    property var recipe: null
    property var cookbooks: []
    property var inCookbooks: []
    property var cookStats: null
    property var checks: null
    property bool weeChefChecks: false
    property var share: null
    property real scale: 1

    // ─── Wee Chef's check, asked for from the menu ───
    property int polls: 0
    property bool asked: false
    property double askedAt: 0
    property var fixedBefore: []

    // ─── Ingredients | Method split ───
    readonly property string lockKey: "crumb:recipe-split-locked"
    property string primary: "method"
    property bool locked: false

    readonly property bool fromRandom: params.from === "random"
    readonly property bool wide: width >= 784
    readonly property bool hasHero: !!(recipe && recipe.image)
    readonly property string sourceUrl: recipe ? (Core.webLink(recipe.originalUrl || "")
                                                  || Core.webLink(recipe.url || "")) : ""
    readonly property string sourceHost: Core.hostOf(sourceUrl)
    readonly property string sub: recipe ? Core.kicker(recipe.recipeCategory || "", recipe.recipeCuisine || "") : ""
    readonly property string cooked: cookStats && cookStats.count > 0
                                     ? Core.cookedLine(cookStats.count,
                                                       cookStats.lastCookedAt ? Date.parse(cookStats.lastCookedAt) : -1,
                                                       Date.now())
                                     : ""
    readonly property string scaledYield: {
        var y = recipe ? recipe.recipeYield : ""
        return y && scale !== 1 ? Core.scaleIngredient(y, scale) : (y || "")
    }
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
        return shown
    }

    function toast(t) {
        window.toast(t)
    }

    // ─── Storage, as the web's lib/storage.ts ───
    function readList(key, session) {
        try {
            var text = session ? Store.readSession(key, "[]") : Store.read(key, "[]")
            var list = JSON.parse(text)
            return Array.isArray(list) ? list : []
        } catch (e) {
            return []
        }
    }

    function rememberViewed(r) {
        var steps = 0
        for (var i = 0; i < r.instructions.length; i++)
            steps += r.instructions[i].items.length
        var rest = readList("crumb:recent", false).filter(v => v.id !== r.id)
        Store.write("crumb:recent", JSON.stringify([{"id": r.id, "title": r.title, "image": r.image || null,
                                                       "at": Date.now(), "steps": steps}].concat(rest).slice(0, 6)))
    }

    function forgetViewed(id) {
        Store.write("crumb:recent", JSON.stringify(readList("crumb:recent", false).filter(v => v.id !== id)))
    }

    function rememberRandom(id) {
        var rest = readList("crumb:random:seen", true).filter(v => v !== id)
        Store.writeSession("crumb:random:seen", JSON.stringify([id].concat(rest).slice(0, 40)))
    }

    function setScale(value) {
        scale = value
        Store.writeSession("crumb:scale:" + recipeId, JSON.stringify(value))
    }

    function setLocked(value) {
        locked = value
        Store.write(lockKey, JSON.stringify(value))
    }

    // ─── Loading ───
    function load() {
        if (smoke) {
            recipe = JSON.parse(Api.smokeRecipe())
            loading = false
            return
        }
        requests.call("recipe", {"id": recipeId}, function (r) {
            recipe = r
            loading = false
            rememberViewed(r)
            requests.call("viewed", {"id": r.id}, null, function () {})
            requests.call("cookStats", {"id": r.id}, function (stats) {
                cookStats = stats
            }, function () {})
        }, function () {
            loading = false
        })
        requests.call("cookbooks", {}, function (list) {
            cookbooks = list
        }, function () {})
        requests.call("recipeCookbooks", {"id": recipeId}, function (ids) {
            inCookbooks = ids
        }, function () {})
        requests.call("recipeChecks", {"id": recipeId}, function (c) {
            checks = c
        }, function () {})
        requests.call("connector", {}, function (c) {
            weeChefChecks = !!c.weeChefChecks
        }, function () {})
        requests.call("shares", {}, function (list) {
            var mine = list.filter(s => s.kind === "recipe" && s.id === recipeId)
            share = mine.length ? mine[0] : null
        }, function () {})
    }

    Component.onCompleted: {
        var remembered = parseFloat(Store.readSession("crumb:scale:" + routeId, "1"))
        scale = remembered > 0 ? remembered : 1
        locked = Store.read(lockKey, "false") === "true"
        if (fromRandom)
            rememberRandom(routeId)
        Qt.callLater(load)
    }

    Requests {
        id: requests
        onFailed: error => page.toast({"title": error, "tone": "error"})
    }

    RecipeClipboard {
        id: clipboard
    }

    // ─── Actions ───
    function markCooked() {
        requests.call("cooked", {"id": recipeId}, function (res) {
            cookStats = {"count": res.count, "lastCookedAt": res.lastCookedAt}
            // Already logged a moment ago: no Undo, which would take away that earlier cook
            if (res.eventId === null || res.eventId === undefined) {
                toast({"title": "Already marked as cooked", "description": "Logged in the last few hours."})
                return
            }
            toast({
                "title": "Marked as cooked",
                "description": "It'll sit out of Try next for a couple of weeks.",
                "tone": "success",
                "action": {"label": "Undo", "onselect": function () {
                    requests.call("undoCooked", {"id": recipeId, "event": res.eventId}, function (stats) {
                        cookStats = stats
                    }, function (error) {
                        page.toast({"title": "Couldn't undo", "description": error, "tone": "error"})
                    })
                }}
            })
        }, function (error) {
            toast({"title": "Couldn't mark as cooked", "description": error, "tone": "error"})
        })
    }

    function surprise() {
        var seen = readList("crumb:random:seen", true)
        var recent = readList("crumb:recent", false).map(v => v.id)
        var exclude = seen.concat(recent.filter(id => seen.indexOf(id) < 0))
        requests.call("random", {"current": recipeId, "exclude": exclude}, function (pick) {
            window.go("recipe", {"id": pick.id, "from": "random"})
        }, function (error) {
            toast({"title": "Couldn't pick a recipe", "description": error, "tone": "error"})
        })
    }

    function deleteRecipe() {
        requests.call("deleteRecipe", {"id": recipeId}, function () {
            forgetViewed(recipeId)
            toast({"title": "Recipe deleted"})
            window.back()
        }, function (error) {
            deleteModal.close()
            toast({"title": "Couldn't delete", "description": error, "tone": "error"})
        })
    }

    function toggleCookbook(bookId) {
        var inBook = inCookbooks.indexOf(bookId) >= 0
        var done = function () {
            inCookbooks = inBook ? inCookbooks.filter(b => b !== bookId) : inCookbooks.concat([bookId])
        }
        var failed = function (error) {
            page.toast({"title": "Couldn't update cookbook", "description": error, "tone": "error"})
        }
        if (inBook)
            requests.call("removeFromCookbook", {"id": bookId, "recipeId": recipeId}, done, failed)
        else
            requests.call("addToCookbook", {"id": bookId, "recipeIds": [recipeId]}, done, failed)
    }

    function createCookbook(name) {
        requests.call("createCookbook", {"name": name}, function (book) {
            cookbooks = cookbooks.concat([book])
            requests.call("addToCookbook", {"id": book.id, "recipeIds": [recipeId]}, function () {
                inCookbooks = inCookbooks.concat([book.id])
            }, function (error) {
                page.toast({"title": "Couldn't update cookbook", "description": error, "tone": "error"})
            })
        }, function (error) {
            toast({"title": "Couldn't create the cookbook", "description": error, "tone": "error"})
        })
    }

    // ─── Wee Chef's check ───
    function checkNow() {
        var before = checks && checks.flags ? checks.flags : []
        requests.call("checkRecipe", {"id": recipeId}, function (c) {
            fixedBefore = before.filter(f => f.state === "fixed").map(f => f.id)
            polls = 0
            asked = true
            askedAt = Date.now()
            checks = c
            toast({"title": "Wee Chef is checking…"})
        }, function (error) {
            toast({"title": "Couldn't start the check", "description": error, "tone": "error"})
        })
    }

    onChecksChanged: {
        if (checks && checks.status === "pending") {
            poll.interval = !asked ? 1500 : polls < 10 ? 3000 : 10000
            poll.restart()
        } else {
            poll.stop()
        }
    }

    // Wee Chef's check runs in the background for a second or so after an import (or when
    // asked for): pick up its result, and the tidied recipe, without a reload.
    Timer {
        id: poll
        onTriggered: {
            if (page.asked ? Date.now() - page.askedAt > 5 * 60000 : ++page.polls > 20) {
                page.asked = false
                return
            }
            if (page.asked)
                page.polls++
            requests.call("recipeChecks", {"id": page.recipeId}, function (c) {
                if (!c)
                    return
                var pending = c.status === "pending"
                var finish = function () {
                    page.checks = pending ? Object.assign({}, c) : c
                    if (page.asked && !pending) {
                        page.asked = false
                        page.checkDone(c)
                    }
                }
                if (!pending && c.flags.some(f => f.state === "fixed"))
                    requests.call("recipe", {"id": page.recipeId}, function (fresh) {
                        page.recipe = fresh
                        finish()
                    }, finish)
                else
                    finish()
            }, function () {})
        }
    }

    // Says how the check went: this check's fixes only, the tidy counting each small thing
    function checkDone(c) {
        var review = c.flags.filter(f => f.state === "review" && f.field !== "image").length
        var fixed = 0
        for (var i = 0; i < c.flags.length; i++) {
            var f = c.flags[i]
            if (f.state === "fixed" && fixedBefore.indexOf(f.id) < 0)
                fixed += Core.fixWeight(JSON.stringify(f.detail || {}))
        }
        var t = JSON.parse(Core.checkDoneToast(c.status, fixed, review))
        toast({"title": t[0], "description": t[1], "tone": c.status === "failed" ? "error" : "default"})
    }

    // ─── The ⋯ menu ───
    CrumbMenu {
        id: menu
        groups: {
            var list = [[
                {"label": "Mark as cooked", "icon": "chef-hat", "onselect": page.markCooked},
                {"label": "Edit", "icon": "pencil", "onselect": () => page.window.go("edit", {"id": page.recipeId})},
                {"label": "Surprise me", "icon": "dices", "onselect": page.surprise}
            ]]
            if (page.weeChefChecks)
                list.push([{"label": "Check with Wee Chef", "icon": "clipboard-check", "onselect": page.checkNow}])
            list.push([{"label": "Delete", "icon": "trash-2", "danger": true, "onselect": () => deleteModal.open()}])
            return list
        }
    }

    Modal {
        id: deleteModal
        title: "Delete this recipe?"
        description: "This can't be undone."
        footer: [
            CrumbButton {
                text: "Cancel"
                kind: "ghost"
                onClicked: deleteModal.close()
            },
            CrumbButton {
                text: "Delete"
                kind: "danger"
                onClicked: page.deleteRecipe()
            }
        ]
    }

    Modal {
        id: bookModal
        title: "New cookbook"
        footer: [
            CrumbButton {
                text: "Cancel"
                kind: "ghost"
                onClicked: bookModal.close()
            },
            CrumbButton {
                text: "Create"
                kind: "primary"
                enabled: bookName.text.trim() !== ""
                onClicked: bookModal.create()
            }
        ]
        onOpened: {
            bookName.text = ""
            bookName.forceActiveFocus()
        }

        function create() {
            if (bookName.text.trim() === "")
                return
            page.createCookbook(bookName.text.trim())
            close()
        }

        StyledField {
            id: bookName
            placeholderText: "Name"
            Layout.fillWidth: true
            onAccepted: bookModal.create()
        }
    }

    ShareSheet {
        id: shareSheet
        recipe: page.recipe
        share: page.share
        onUpdated: value => page.share = value
    }

    // ─── Not found ───
    ColumnLayout {
        visible: !page.loading && !page.recipe
        anchors.centerIn: parent
        spacing: 16

        EmptyState {
            iconName: "file-question"
            title: "Recipe not found"
            text: "It may have been deleted."
            Layout.alignment: Qt.AlignHCenter
        }
        CrumbButton {
            text: "Back to recipes"
            Layout.alignment: Qt.AlignHCenter
            onClicked: page.window.go("recipes", {})
        }
    }

    // ─── Loading ───
    ColumnLayout {
        visible: page.loading
        x: 32
        y: 32
        width: Math.min(640, page.width - 64)
        spacing: 20

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: width * 3 / 4
            radius: 12
            color: Palette.tint
        }
        Rectangle {
            Layout.preferredWidth: parent.width * 2 / 3
            Layout.preferredHeight: 36
            radius: 12
            color: Palette.tint
        }
        Rectangle {
            Layout.preferredWidth: parent.width / 2
            Layout.preferredHeight: 16
            radius: 12
            color: Palette.tint
        }
    }

    ScrollPage {
        id: scroller
        anchors.fill: parent
        visible: !!page.recipe
        maxWidth: 1420

        ColumnLayout {
            width: scroller.contentWidthAvailable
            spacing: page.wide ? 64 : 48

            // ─── Hero: photo left, title and actions right on wide screens ───
            GridLayout {
                id: header
                Layout.fillWidth: true
                columns: page.wide && page.hasHero ? 2 : 1
                columnSpacing: 48
                rowSpacing: 24

                Photo {
                    visible: page.hasHero
                    recipeId: page.recipeId
                    image: page.recipe && page.recipe.image ? page.recipe.image : ""
                    Layout.preferredWidth: page.wide ? Math.max(0, Math.min(640, header.width - 48 - 256)) : header.width
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

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        ColumnLayout {
                            Layout.fillWidth: true
                            Layout.alignment: Qt.AlignTop
                            spacing: 8

                            Text {
                                visible: page.sub !== ""
                                text: page.sub
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
                                text: page.recipe ? page.recipe.title : ""
                                font.pixelSize: 30
                                lineHeight: 0.95
                                elide: Text.ElideNone
                                Layout.fillWidth: true
                                Accessible.role: Accessible.Heading
                            }
                        }

                        RowLayout {
                            Layout.alignment: Qt.AlignTop
                            Layout.topMargin: -4
                            spacing: 8

                            CrumbButton {
                                kind: "outline"
                                iconName: "share"
                                Accessible.name: "Share"
                                ToolTip.visible: hovered
                                ToolTip.text: "Share"
                                onClicked: shareSheet.open()
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

                    // Cooked, by, and where it came from
                    Flow {
                        visible: page.cooked !== "" || !!(page.recipe && page.recipe.author) || page.sourceHost !== ""
                        Layout.fillWidth: true
                        Layout.topMargin: -8
                        spacing: 6

                        Row {
                            visible: page.cooked !== ""
                            spacing: 6
                            height: 44

                            Icon {
                                anchors.verticalCenter: parent.verticalCenter
                                name: "chef-hat"
                                size: 16
                                color: Palette.primary
                            }
                            Body {
                                anchors.verticalCenter: parent.verticalCenter
                                text: page.cooked
                                bold: true
                                font.pixelSize: 14
                            }
                            Body {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: !!(page.recipe && page.recipe.author) || page.sourceHost !== ""
                                text: "·"
                                muted: true
                                font.pixelSize: 14
                            }
                        }
                        Body {
                            visible: !!(page.recipe && page.recipe.author)
                            text: page.recipe && page.recipe.author ? "By " + page.recipe.author : ""
                            muted: true
                            font.pixelSize: 14
                            height: 44
                            verticalAlignment: Text.AlignVCenter
                        }
                        Body {
                            visible: !!(page.recipe && page.recipe.author) && page.sourceHost !== ""
                            text: "·"
                            muted: true
                            font.pixelSize: 14
                            height: 44
                            verticalAlignment: Text.AlignVCenter
                        }
                        Text {
                            visible: page.sourceHost !== ""
                            text: page.sourceHost
                            color: Palette.primary
                            font.family: Palette.fontSans
                            font.pixelSize: 14
                            font.weight: Font.Bold
                            font.underline: sourceHover.hovered
                            height: 44
                            rightPadding: 18
                            verticalAlignment: Text.AlignVCenter
                            Accessible.role: Accessible.Link
                            Accessible.name: text

                            Icon {
                                anchors.right: parent.right
                                anchors.verticalCenter: parent.verticalCenter
                                name: "external-link"
                                size: 14
                                color: Palette.primary
                            }
                            HoverHandler {
                                id: sourceHover
                                cursorShape: Qt.PointingHandCursor
                            }
                            TapHandler {
                                onTapped: Qt.openUrlExternally(page.sourceUrl)
                            }
                        }
                    }

                    Body {
                        visible: !!(page.recipe && page.recipe.description)
                        text: page.recipe && page.recipe.description ? page.recipe.description : ""
                        muted: true
                        font.pixelSize: 17
                        lineHeight: 1.2
                        Layout.fillWidth: true
                        Layout.maximumWidth: 65 * 9
                    }

                    WeeChefCard {
                        visible: !!page.checks && (fixed.length > 0 || review.length > 0 || !!photo)
                        Layout.fillWidth: true
                        recipeId: page.recipeId
                        checks: page.checks
                        onUndone: (r, c) => {
                            page.recipe = r
                            page.checks = c
                        }
                        onEditRequested: page.window.go("edit", {"id": page.recipeId})
                    }

                    // Time and yield
                    Card {
                        id: facts
                        visible: page.metaItems.length > 0 || page.scaledYield !== ""
                        Layout.fillWidth: true
                        readonly property int count: page.metaItems.length + (page.scaledYield !== "" ? 1 : 0)
                        readonly property int cols: Math.max(1, Math.min(count, Math.floor((width - 8) / 88)))
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
                                model: page.metaItems.concat(page.scaledYield !== ""
                                                             ? [{"icon": "users", "label": "Serves", "value": page.scaledYield}] : [])

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

                    // Cooking mode is the one main action here
                    GridLayout {
                        Layout.fillWidth: true
                        Layout.topMargin: 4
                        columns: page.wide && width < 560 ? 1 : 2
                        columnSpacing: 12
                        rowSpacing: 12

                        BigButton {
                            text: "Cooking mode"
                            iconName: "flame"
                            onClicked: page.window.go("cook", {"id": page.recipeId})
                        }
                        BigButton {
                            text: "Mise en place"
                            iconName: "soup"
                            tile: true
                            onClicked: page.window.go("prep", {"id": page.recipeId})
                        }
                        CrumbButton {
                            visible: page.fromRandom
                            text: "Shuffle again"
                            iconName: "dices"
                            Layout.columnSpan: parent.columns
                            Layout.fillWidth: true
                            Layout.preferredHeight: 48
                            onClicked: page.surprise()
                        }
                    }
                }
            }

            // ─── Ingredients | Method ───
            Item {
                id: split
                Layout.fillWidth: true
                implicitHeight: page.wide ? Math.max(ingredients.implicitHeight, method.implicitHeight)
                                          : ingredients.implicitHeight + 48 + method.implicitHeight

                readonly property real avail: width - 44 - 2 * 28
                readonly property real share: {
                    var mid = page.width < 1040 // small desktops swing less
                    if (page.locked)
                        return 0.42
                    if (page.primary === "ingredients")
                        return mid ? 0.54 : 0.56
                    return mid ? 0.42 : 0.38
                }

                RecipeIngredients {
                    id: ingredients
                    recipe: page.recipe
                    scale: page.scale
                    secondary: page.wide && !page.locked && page.primary !== "ingredients"
                    x: 0
                    y: 0
                    width: page.wide ? split.avail * split.share : split.width
                    onScaleEdited: value => page.setScale(value)
                    onEngaged: if (page.wide && !page.locked) page.primary = "ingredients"

                    Behavior on width {
                        enabled: page.wide
                        NumberAnimation { duration: 250; easing.type: Easing.InOutQuad }
                    }
                }

                // The toggle between the panels, with a hairline under it
                Item {
                    visible: page.wide
                    x: ingredients.width + 28
                    width: 44
                    height: parent.height

                    Rectangle {
                        x: 22
                        y: 68
                        width: 1
                        height: parent.height - 68
                        color: Palette.line
                    }
                    Rectangle {
                        id: lockButton
                        width: 44
                        height: 44
                        radius: 12
                        color: page.locked ? Palette.tile : Palette.paper
                        border.width: page.locked ? 0 : 1
                        border.color: Palette.line
                        Accessible.role: Accessible.CheckBox
                        Accessible.checked: page.locked
                        Accessible.name: "Keep the panels from resizing"
                        Accessible.onPressAction: page.setLocked(!page.locked)
                        ToolTip.visible: lockHover.hovered
                        ToolTip.text: "Keep the panels from resizing"

                        Icon {
                            anchors.centerIn: parent
                            name: "columns-2"
                            size: 20
                            color: page.locked ? Palette.onTile : Palette.textMuted
                        }
                        HoverHandler {
                            id: lockHover
                            cursorShape: Qt.PointingHandCursor
                        }
                        TapHandler {
                            onTapped: page.setLocked(!page.locked)
                        }
                    }
                }

                RecipeMethod {
                    id: method
                    recipe: page.recipe
                    secondary: page.wide && !page.locked && page.primary !== "method"
                    x: page.wide ? ingredients.width + 28 + 44 + 28 : 0
                    y: page.wide ? 0 : ingredients.implicitHeight + 48
                    width: page.wide ? split.avail - ingredients.width : split.width

                    TapHandler {
                        onTapped: if (page.wide && !page.locked) page.primary = "method"
                    }
                }
            }

            // ─── On the shelf in ───
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 20

                Heading {
                    text: "On the shelf in"
                }

                Flow {
                    Layout.fillWidth: true
                    spacing: 8

                    Repeater {
                        model: page.cookbooks

                        delegate: Rectangle {
                            id: chip
                            required property var modelData
                            readonly property bool inBook: page.inCookbooks.indexOf(modelData.id) >= 0

                            width: chipRow.implicitWidth + 24
                            height: 44
                            radius: 22
                            color: inBook ? Palette.tint : Palette.paper
                            border.width: inBook ? 0 : 1
                            border.color: Palette.line
                            Accessible.role: Accessible.CheckBox
                            Accessible.checked: inBook
                            Accessible.name: modelData.name
                            Accessible.onPressAction: page.toggleCookbook(modelData.id)

                            Row {
                                id: chipRow
                                anchors.centerIn: parent
                                spacing: 6

                                Rectangle {
                                    width: 10
                                    height: 24
                                    topLeftRadius: 3
                                    topRightRadius: 3
                                    color: JSON.parse(Core.bookLook(chip.modelData.color || "")).cloth
                                    border.width: 1
                                    border.color: Qt.rgba(0, 0, 0, 0.22)
                                    anchors.verticalCenter: parent.verticalCenter
                                }
                                Text {
                                    text: chip.modelData.name
                                    color: chip.inBook ? Palette.primary : (chipHover.hovered ? Palette.text : Palette.textMuted)
                                    font.family: Palette.fontSans
                                    font.pixelSize: 14
                                    font.weight: Font.Bold
                                    anchors.verticalCenter: parent.verticalCenter
                                }
                                Icon {
                                    name: chip.inBook ? "check" : "plus"
                                    size: 16
                                    color: chip.inBook ? Palette.primary : Palette.textMuted
                                    anchors.verticalCenter: parent.verticalCenter
                                }
                            }
                            HoverHandler {
                                id: chipHover
                                cursorShape: Qt.PointingHandCursor
                            }
                            TapHandler {
                                onTapped: page.toggleCookbook(chip.modelData.id)
                            }
                        }
                    }

                    Rectangle {
                        width: newRow.implicitWidth + 24
                        height: 44
                        radius: 22
                        color: Palette.paper
                        border.width: 1
                        border.color: Palette.line
                        Accessible.role: Accessible.Button
                        Accessible.name: "New cookbook"
                        Accessible.onPressAction: bookModal.open()

                        Row {
                            id: newRow
                            anchors.centerIn: parent
                            spacing: 6

                            Icon {
                                name: "plus"
                                size: 16
                                color: Palette.textMuted
                                anchors.verticalCenter: parent.verticalCenter
                            }
                            Text {
                                text: "New cookbook"
                                color: Palette.textMuted
                                font.family: Palette.fontSans
                                font.pixelSize: 14
                                font.weight: Font.Bold
                                anchors.verticalCenter: parent.verticalCenter
                            }
                        }
                        HoverHandler {
                            cursorShape: Qt.PointingHandCursor
                        }
                        TapHandler {
                            onTapped: bookModal.open()
                        }
                    }
                }
            }
        }
    }

    // The web's .btn-xl: 56px tall, butter for the one main action, solid tile green otherwise
    component BigButton: Rectangle {
        id: big
        property string text
        property string iconName
        property bool tile: false
        signal clicked

        Layout.fillWidth: true
        Layout.preferredHeight: 56
        radius: 12
        color: tile ? Palette.tile : Palette.butter
        Accessible.role: Accessible.Button
        Accessible.name: text
        Accessible.onPressAction: big.clicked()
        activeFocusOnTab: true
        Keys.onReturnPressed: big.clicked()
        Keys.onSpacePressed: big.clicked()

        Rectangle {
            anchors.fill: parent
            radius: parent.radius
            color: Palette.text
            opacity: bigHover.hovered ? 0.06 : 0
        }
        Rectangle {
            anchors.fill: parent
            radius: parent.radius
            color: "transparent"
            border.width: 2
            border.color: Palette.primary
            visible: big.activeFocus
        }
        Row {
            anchors.centerIn: parent
            spacing: 8

            Icon {
                name: big.iconName
                size: 20
                color: big.tile ? Palette.onTile : Palette.onButter
                anchors.verticalCenter: parent.verticalCenter
            }
            Text {
                text: big.text
                color: big.tile ? Palette.onTile : Palette.onButter
                font.family: Palette.fontSans
                font.pixelSize: 17
                font.weight: Font.Bold
                anchors.verticalCenter: parent.verticalCenter
            }
        }
        HoverHandler {
            id: bigHover
            cursorShape: Qt.PointingHandCursor
        }
        TapHandler {
            onTapped: big.clicked()
        }
    }
}
