import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The top box (web islands/TopBox.svelte): Search and Add tabs, like the dividers in a recipe
// box. Add is one field; Core.detect works out what was pasted and the chip shows it, and opens
// a menu to pick the mode by hand. Links, recipe text, names to start from scratch, files and
// photos (read by Wee Chef) all save from here.
// Left out because only a browser can do them: the extension's video hand-over (`fromBrowser`),
// pasting an image from the clipboard, and the PWA share target's prefilled fields. Photos are
// only read by Wee Chef: the web's fallback reads them in the browser with tesseract.js.
Item {
    id: box

    // Show the Search/Add tabs (Home); without them it's just the Add block (the Add page)
    property bool tabs: true
    // Sits on the tile header; off it, the block gets a border
    property bool onTile: true
    property bool autofocus: false

    readonly property var modes: [
        { "mode": "auto", "label": "Auto-detect", "hint": "Crumb works out what you pasted" },
        { "mode": "link", "label": "Link", "hint": "Import from a website" },
        { "mode": "text", "label": "Recipe text", "hint": "Paste the whole recipe" },
        { "mode": "photo", "label": "Photo", "hint": "Snap or upload the pages of a recipe" },
        { "mode": "file", "label": "File", "hint": "PDF, saved web page, text or backup" },
        { "mode": "claude", "label": "Claude", "hint": "Send a recipe from a Claude chat" },
        { "mode": "scratch", "label": "From scratch", "hint": "Write it out yourself" },
        { "mode": "apps", "label": "Other apps", "hint": "Import from another recipe app" }
    ]
    // Handwritten hints beside Add; `short` fits a narrow tab row, on two lines
    readonly property var tips: [
        { "long": "Try pasting the whole recipe", "short": "paste the\nwhole recipe" },
        { "long": "Try uploading a photo of your recipe", "short": "or snap a\nphoto of it" },
        { "long": "Try uploading a PDF of your recipe", "short": "PDFs\nwork too" },
        { "long": "Paste a link from any recipe site", "short": "links from\nany site" },
        { "long": "Drop in a Crumb or Just the Recipe backup", "short": "backups\nwork too" },
        { "long": "Paste a few links at once", "short": "a few links\nat once" },
        { "long": "Paste a TikTok, Reel or YouTube video", "short": "cooking\nvideos too" }
    ]
    readonly property int maxPhotos: 6

    property string tab: tabs ? "search" : "add"
    property string input: ""
    // A chosen file's path, and the pages of a recipe photographed: [{path, name}]
    property string filePath: ""
    property string fileName: ""
    property var pages: []
    // Whether Wee Chef reads photos here; null until known
    property var vision: null
    property string progress: ""
    // The mode picked by hand ("" hands the choice back to detection)
    property string manual: ""
    property bool saving: false
    // The link a site's bot check (or its terms) kept the server from reading: a nudge, not
    // an error toast. `blockedWhy` is "bot" or "terms"; `blockedSite` the listed site's name.
    property string blocked: ""
    property string blockedWhy: "bot"
    property string blockedSite: ""
    property int recipeCount: -1
    property var detected: ({ "mode": "auto", "summary": "" })
    property int tipIndex: -1
    property bool fieldFocused: false

    readonly property string mode: manual !== "" ? manual : detected.mode
    readonly property string modeLabel: {
        for (var i = 0; i < modes.length; i++) {
            if (modes[i].mode === mode)
                return modes[i].label
        }
        return "Auto-detect"
    }
    readonly property var links: input.trim() ? JSON.parse(Core.linksIn(input)) : []
    readonly property int linkCount: links.length
    // What Add would do right now (the chip lags by the debounce; this doesn't)
    readonly property string now: manual !== "" ? manual : pages.length ? "photo" : filePath ? "file" : JSON.parse(Core.detect(input)).mode
    readonly property bool canAdd: !saving && (now === "claude" || now === "apps" || now === "file"
                                               || now === "photo" || (now === "link" && linkCount > 0)
                                               || (now === "scratch" && input.trim().length > 0)
                                               || ((now === "text" || now === "auto") && input.trim().length >= 10))
    readonly property string summary: {
        if (saving && mode === "photo")
            return progress
        if (saving && mode === "link" && progress)
            return progress
        if (saving)
            return mode === "link" ? "Fetching… tricky sites take ~20s" : "Reading…"
        if (mode === "photo")
            return pages.length ? detected.summary : "Up to " + maxPhotos + " pages of one recipe"
        if (mode === "claude")
            return "Opens the connector setup"
        if (mode === "apps")
            return "Opens the importer"
        if (mode === "file")
            return filePath ? fileName : "Choose a file"
        return manual !== "" && manual !== detected.mode ? "" : detected.summary
    }
    readonly property string buttonLabel: mode === "claude" || mode === "apps" ? "Open" : mode === "link" && linkCount > 1 ? "Import all" : "Add"
    readonly property bool photoNote: mode === "photo" && pages.length > 0
    readonly property bool hideField: photoNote && vision === false && !input.trim() && !fieldFocused
    readonly property var tip: tabs && onTile && tab === "add" && tipIndex >= 0 ? tips[tipIndex] : null
    readonly property color ring: Qt.rgba(Palette.butter.r, Palette.butter.g, Palette.butter.b, 0.8)

    implicitHeight: content.implicitHeight

    function go(name, params) {
        ApplicationWindow.window.go(name, params)
    }

    function toast(t) {
        ApplicationWindow.window.toast(t)
    }

    // A different tip each time Add is opened
    function pickTip() {
        var i = Math.floor(Math.random() * tips.length)
        if (i === tipIndex)
            i = (i + 1) % tips.length
        tipIndex = i
    }

    function switchTab(next) {
        if (next === "add" && tab !== "add")
            pickTip()
        tab = next
        menu.close()
        Qt.callLater(function () {
            (next === "add" ? addField : searchField).forceActiveFocus()
        })
    }

    function search() {
        var q = searchField.text.trim()
        go("recipes", q ? { "q": q } : {})
    }

    function isPhoto(name) {
        return /\.(jpe?g|png|webp|gif|heic|heif|avif)$/i.test(name)
    }

    function baseName(url) {
        var s = String(url)
        return decodeURIComponent(s.substring(s.lastIndexOf("/") + 1))
    }

    function checkVision() {
        if (vision !== null)
            return
        var known = ApplicationWindow.window.connector
        if (known && typeof known.vision === "boolean") {
            vision = known.vision
            return
        }
        requests.call("connector", {}, function (c) {
            box.vision = !!(c && c.vision)
        }, function () {
            box.vision = false
        })
    }

    function choose(m) {
        manual = m === "auto" ? "" : m
        menu.close()
        if (m !== "file") {
            filePath = ""
            fileName = ""
        }
        if (m !== "photo")
            pages = []
        if (m === "file") {
            filePicker.open()
        } else if (m === "photo") {
            checkVision()
            if (!pages.length)
                photoPicker.open()
        } else {
            addField.forceActiveFocus()
        }
    }

    // Pasted or dropped photos only take over an empty field: text already there stays what
    // it is, unless Photo was picked by hand (then it's the note)
    function photosWelcome() {
        if (saving)
            return false
        if (!input.trim() || manual === "photo")
            return true
        toast({ "title": "Clear the text to add a photo", "description": "Or pick Photo from the menu to keep it as a note." })
        return false
    }

    // Adds photos as pages (up to six)
    function addPhotos(urls) {
        if (saving)
            return
        var room = maxPhotos - pages.length
        if (urls.length > room)
            toast({ "title": "Up to " + maxPhotos + " photos", "description": "They should all be one recipe." })
        var added = urls.slice(0, Math.max(0, room))
        if (!added.length)
            return
        manual = "photo"
        filePath = ""
        fileName = ""
        checkVision()
        var next = pages.slice()
        for (var i = 0; i < added.length; i++)
            next.push({ "path": String(added[i]), "name": baseName(added[i]) })
        pages = next
    }

    function removePage(i) {
        var next = pages.slice()
        next.splice(i, 1)
        pages = next
        if (!pages.length && !input.trim())
            manual = "photo"
    }

    function movePage(i, by) {
        var j = i + by
        if (j < 0 || j >= pages.length)
            return
        var next = pages.slice()
        var tmp = next[i]
        next[i] = next[j]
        next[j] = tmp
        pages = next
    }

    // Photos become pages; any other file is imported on its own
    function takeFiles(urls) {
        if (saving)
            return
        var images = []
        for (var i = 0; i < urls.length; i++) {
            if (isPhoto(String(urls[i])))
                images.push(urls[i])
        }
        if (images.length) {
            if (photosWelcome())
                addPhotos(images)
            return
        }
        if (urls.length) {
            pages = []
            filePath = String(urls[0])
            fileName = baseName(urls[0])
            manual = "file"
        }
    }

    function fail(title, message) {
        toast({ "title": title, "description": message, "tone": "error" })
        saving = false
    }

    function readPhotos() {
        if (vision === false) {
            fail("Couldn't read that photo", "Wee Chef isn't switched on for this Crumb, and this app can't read photos by itself.")
            return
        }
        saving = true
        progress = "Reading " + pages.length + " photo" + (pages.length === 1 ? "" : "s") + "…"
        var paths = pages.map(function (p) { return p.path })
        var note = input.trim()
        requests.call("importPhotos", { "paths": paths, "hint": vision && note ? note : undefined }, function (res) {
            if (!res.isNew)
                toast({ "title": "Already in your recipes" })
            go("recipe", { "id": res.recipe.id })
        }, function (err) {
            fail("Couldn't read that photo", err)
        })
    }

    function importFile() {
        saving = true
        requests.call("importFiles", { "paths": [filePath] }, function (list) {
            var result = list && list[0]
            if (!result || result.error) {
                fail("Couldn't read that file", result && result.error ? result.error : "Nothing in it looked like a recipe")
                return
            }
            if (result.created.length === 1) {
                go("recipe", { "id": result.created[0].id })
                return
            }
            toast({
                "title": result.created.length ? "Imported " + result.created.length + " recipes" : "Already in your recipes",
                "tone": result.created.length ? "success" : "default"
            })
            go("recipes", {})
        }, function (err) {
            fail("Couldn't read that file", err)
        })
    }

    // From the nudge: the link is left behind and the field waits for the recipe's text.
    function pasteInstead() {
        blocked = ""
        input = ""
        manual = "text"
        addField.forceActiveFocus()
    }

    function importBody(body, what) {
        saving = true
        blocked = ""
        progress = ""
        requests.call("import", body, function (res) {
            // Another Crumb's shared cookbook: every recipe in it, into a cookbook of that name
            if (res.cookbook) {
                var b = res.cookbook
                toast({
                    "title": Core.bookImportedTitle(b.name, b.added, b.duplicates || 0, b.skipped === undefined ? -1 : b.skipped),
                    "tone": b.added ? "success" : "default"
                })
                go("cookbook", { "id": b.id })
                return
            }
            if (!res.isNew)
                toast({ "title": "Already in your recipes" })
            else if (res.droppedPhoto)
                toast({ "title": "Saved without its photo", "description": "The site's photo link doesn't work. You can add one in Edit." })
            else if (res.fromVideo)
                toast({ "title": "Saved from the video", "tone": "success" })
            go("recipe", { "id": res.recipe.id })
        }, function (err, info) {
            if (body.url && (info.code === "site_blocked" || info.code === "site_terms")) {
                box.blockedWhy = info.code === "site_terms" ? "terms" : "bot"
                box.blockedSite = info.site || ""
                box.blocked = body.url
                box.saving = false
                return
            }
            fail("Couldn't " + what, err)
        }, function (line) {
            box.progress = line
        })
    }

    function add() {
        if (!canAdd)
            return
        var text = input.trim()
        // Don't wait for the debounced chip: a paste-and-submit uses what's there now
        switch (now) {
        case "claude":
            go("connect", {})
            return
        case "apps":
            go("import", {})
            return
        case "scratch":
            go("new", { "title": text })
            return
        case "photo":
            if (!pages.length) {
                photoPicker.open()
                return
            }
            readPhotos()
            return
        case "file":
            if (!filePath) {
                filePicker.open()
                return
            }
            if (/\.(docx?|pages|rtf|mp4|mov|mkv|webm|avi|mp3|m4a|wav|ogg|flac)$/i.test(fileName)) {
                toast({ "title": "Crumb can't read that kind of file yet", "description": "Open it, copy the recipe text, and paste it here instead." })
                return
            }
            importFile()
            return
        case "link":
            if (links.length > 1) {
                go("import", { "links": links.join("\n") })
                return
            }
            importBody({ "url": links[0] }, "get that recipe")
            return
        default:
            importBody({ "text": input }, "read that recipe")
        }
    }

    // Debounced detection (about 250ms), so the chip doesn't flicker while typing
    onInputChanged: {
        if (pages.length) {
            detected = { "mode": "photo", "summary": pages.length + " photo" + (pages.length === 1 ? "" : "s") }
        } else if (filePath) {
            detected = { "mode": "file", "summary": fileName }
        } else if (input.trim()) {
            detect.restart()
        } else {
            detect.stop()
            detected = { "mode": "auto", "summary": "" }
            // Clearing the field hands the choice back to detection
            manual = ""
        }
    }
    onPagesChanged: {
        if (pages.length)
            detected = { "mode": "photo", "summary": pages.length + " photo" + (pages.length === 1 ? "" : "s") }
        else
            detected = JSON.parse(Core.detect(input))
    }
    onFilePathChanged: {
        if (filePath)
            detected = { "mode": "file", "summary": fileName }
        else if (!pages.length)
            detected = JSON.parse(Core.detect(input))
    }

    Component.onCompleted: {
        if (tabs && onTile) {
            requests.call("recipes", {}, function (list) {
                box.recipeCount = list.length
            }, function () {})
        }
        if (autofocus)
            (tab === "add" ? addField : searchField).forceActiveFocus()
    }

    Requests {
        id: requests
    }

    Timer {
        id: detect
        interval: 250
        onTriggered: box.detected = JSON.parse(Core.detect(box.input))
    }

    FileDialog {
        id: filePicker
        title: "Choose a file"
        fileMode: FileDialog.OpenFiles
        nameFilters: ["Recipes and photos (*.pdf *.txt *.md *.markdown *.html *.htm *.json *.jpg *.jpeg *.png *.webp *.gif *.heic *.heif *.avif)", "All files (*)"]
        onAccepted: box.takeFiles(selectedFiles)
    }

    FileDialog {
        id: photoPicker
        title: "Choose photos"
        fileMode: FileDialog.OpenFiles
        nameFilters: ["Photos (*.jpg *.jpeg *.png *.webp *.gif *.heic *.heif *.avif)"]
        onAccepted: box.addPhotos(selectedFiles)
    }

    ColumnLayout {
        id: content
        width: box.width
        spacing: 0

        // The tabs, and the handwritten tip beside Add
        Item {
            visible: box.tabs
            Layout.fillWidth: true
            Layout.preferredHeight: 44
            z: 1

            Row {
                x: 28
                spacing: 12

                Repeater {
                    model: [
                        { "name": "search", "label": "Search", "icon": "search" },
                        { "name": "add", "label": "Add", "icon": "plus" }
                    ]

                    delegate: Item {
                        id: tabItem
                        required property var modelData
                        readonly property bool selected: box.tab === modelData.name

                        width: tabRow.implicitWidth + 32
                        height: selected ? 46 : 44
                        clip: true

                        // Rounded on top only: the bottom corners are pushed out of the clip
                        Rectangle {
                            width: parent.width
                            height: parent.height + 12
                            radius: 12
                            color: tabItem.selected ? Palette.paper
                                                    : box.onTile ? Qt.rgba(Palette.onTile.r, Palette.onTile.g, Palette.onTile.b, tabHover.hovered ? 0.24 : 0.16)
                                                                 : Palette.tint
                        }

                        Row {
                            id: tabRow
                            anchors.centerIn: parent
                            spacing: 8

                            Icon {
                                name: tabItem.modelData.icon
                                size: 18
                                color: tabText.color
                                anchors.verticalCenter: parent.verticalCenter
                            }

                            Text {
                                id: tabText
                                text: tabItem.modelData.label
                                color: tabItem.selected ? Palette.text : box.onTile ? Palette.onTile : Palette.textMuted
                                font.family: Palette.fontSans
                                font.pixelSize: 15
                                font.weight: tabItem.selected ? Font.Bold : Font.DemiBold
                                anchors.verticalCenter: parent.verticalCenter
                            }
                        }

                        HoverHandler {
                            id: tabHover
                            cursorShape: Qt.PointingHandCursor
                        }

                        TapHandler {
                            onTapped: box.switchTab(tabItem.modelData.name)
                        }

                        Accessible.role: Accessible.PageTab
                        Accessible.name: modelData.label
                        activeFocusOnTab: true
                        Keys.onPressed: function (event) {
                            if (event.key === Qt.Key_Right || event.key === Qt.Key_Left) {
                                box.switchTab(box.tab === "search" ? "add" : "search")
                                event.accepted = true
                            } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                                box.switchTab(modelData.name)
                                event.accepted = true
                            }
                        }

                        Rectangle {
                            visible: tabItem.activeFocus
                            anchors.fill: parent
                            radius: 12
                            color: "transparent"
                            border.width: 2
                            border.color: Palette.butter
                        }
                    }
                }
            }

            Text {
                visible: box.tip !== null
                anchors.right: parent.right
                anchors.rightMargin: 34
                anchors.verticalCenter: parent.verticalCenter
                width: Math.min(implicitWidth, parent.width - 320)
                horizontalAlignment: Text.AlignRight
                elide: Text.ElideLeft
                text: box.tip ? (box.width >= 576 ? box.tip.long : box.tip.short) : ""
                color: Qt.rgba(Palette.onTile.r, Palette.onTile.g, Palette.onTile.b, 0.88)
                font.family: Palette.fontHand
                font.pixelSize: box.width >= 416 ? 20 : 16
                lineHeight: 0.95
            }
        }

        // Search
        Item {
            visible: box.tab === "search"
            Layout.fillWidth: true
            Layout.preferredHeight: 56

            Rectangle {
                anchors.fill: parent
                anchors.margins: -3
                radius: 19
                color: box.onTile ? box.ring : Palette.tint
                visible: searchField.activeFocus
            }

            Rectangle {
                anchors.fill: parent
                radius: 16
                color: Palette.paper
                border.width: box.onTile ? 0 : 1
                border.color: searchField.activeFocus ? Palette.tile : Palette.line
            }

            TextField {
                id: searchField
                anchors.fill: parent
                leftPadding: 16
                rightPadding: 16
                verticalAlignment: TextInput.AlignVCenter
                color: Palette.text
                placeholderText: box.recipeCount > 0 ? "Search " + box.recipeCount + " recipes" : "Search recipes"
                placeholderTextColor: Palette.textMuted
                font.family: Palette.fontSans
                font.pixelSize: 16
                selectionColor: Palette.tile
                selectedTextColor: Palette.onTile
                background: null
                Accessible.name: "Search recipes by title, ingredient or category"
                onAccepted: box.search()
            }
        }

        // Add
        Item {
            id: addPanel
            visible: box.tab === "add"
            Layout.fillWidth: true
            implicitHeight: addCard.implicitHeight

            Rectangle {
                anchors.fill: addCard
                anchors.margins: -3
                radius: 19
                color: box.onTile ? box.ring : Palette.tint
                visible: addField.activeFocus
            }

            Rectangle {
                id: addCard
                width: parent.width
                implicitHeight: addColumn.implicitHeight + 2
                radius: 16
                color: Palette.paper
                border.width: box.onTile ? 0 : 1
                border.color: addField.activeFocus ? Palette.tile : Palette.line

                DropArea {
                    anchors.fill: parent
                    onDropped: function (drop) {
                        if (drop.hasUrls) {
                            drop.accepted = true
                            box.takeFiles(drop.urls)
                        }
                    }
                }

                ColumnLayout {
                    id: addColumn
                    x: 1
                    y: 1
                    width: parent.width - 2
                    spacing: 0

                    // The photographed pages, in order
                    ListView {
                        id: tray
                        visible: box.mode === "photo" && box.pages.length > 0
                        Layout.fillWidth: true
                        Layout.preferredHeight: 7 * 16 + 44 + 12 + 4
                        Layout.topMargin: 12
                        leftMargin: 12
                        rightMargin: 12
                        spacing: 10
                        orientation: ListView.Horizontal
                        clip: true
                        model: box.pages.length + (box.pages.length < box.maxPhotos ? 1 : 0)
                        Accessible.name: "Photos, in page order"

                        delegate: Item {
                            id: pageItem
                            required property int index
                            readonly property bool isAdd: index >= box.pages.length
                            readonly property var page: isAdd ? null : box.pages[index]

                            width: 88
                            height: tray.height

                            // The add-another-page button
                            Rectangle {
                                visible: pageItem.isAdd
                                width: 88
                                height: 112
                                radius: 12
                                color: addPageHover.hovered && !box.saving ? Palette.tint : "transparent"
                                border.width: 2
                                border.color: addPageHover.hovered && !box.saving ? Palette.tile : Palette.line
                                opacity: box.saving ? 0.5 : 1

                                Column {
                                    anchors.centerIn: parent
                                    width: parent.width - 16
                                    spacing: 6

                                    Icon {
                                        name: "image-plus"
                                        size: 24
                                        color: Palette.primary
                                        anchors.horizontalCenter: parent.horizontalCenter
                                    }

                                    Text {
                                        width: parent.width
                                        text: "Add another page"
                                        horizontalAlignment: Text.AlignHCenter
                                        wrapMode: Text.WordWrap
                                        color: Palette.primary
                                        font.family: Palette.fontSans
                                        font.pixelSize: 13
                                        font.weight: Font.Bold
                                    }
                                }

                                HoverHandler {
                                    id: addPageHover
                                    cursorShape: Qt.PointingHandCursor
                                }

                                TapHandler {
                                    enabled: !box.saving
                                    onTapped: photoPicker.open()
                                }
                            }

                            Column {
                                visible: !pageItem.isAdd
                                width: parent.width
                                spacing: 0

                                Rectangle {
                                    width: 88
                                    height: 112
                                    radius: 12
                                    color: Palette.tint
                                    border.width: 1
                                    border.color: Palette.line
                                    clip: true

                                    Image {
                                        id: thumb
                                        anchors.fill: parent
                                        anchors.margins: 1
                                        source: pageItem.page ? pageItem.page.path : ""
                                        sourceSize.width: 176
                                        fillMode: Image.PreserveAspectCrop
                                        autoTransform: true
                                        asynchronous: true
                                    }

                                    Column {
                                        visible: thumb.status === Image.Error
                                        anchors.centerIn: parent
                                        width: parent.width - 12
                                        spacing: 4

                                        Icon {
                                            name: "image-off"
                                            size: 20
                                            color: Palette.textMuted
                                            anchors.horizontalCenter: parent.horizontalCenter
                                        }

                                        Text {
                                            width: parent.width
                                            text: pageItem.page ? pageItem.page.name : ""
                                            elide: Text.ElideRight
                                            horizontalAlignment: Text.AlignHCenter
                                            color: Palette.textMuted
                                            font.family: Palette.fontSans
                                            font.pixelSize: 13
                                        }
                                    }

                                    Rectangle {
                                        x: 6
                                        y: 6
                                        width: Math.max(24, pageNo.implicitWidth + 12)
                                        height: 24
                                        radius: 12
                                        color: Palette.tile

                                        Text {
                                            id: pageNo
                                            anchors.centerIn: parent
                                            text: pageItem.index + 1
                                            color: Palette.onTile
                                            font.family: Palette.fontSans
                                            font.pixelSize: 13
                                            font.weight: Font.Bold
                                        }
                                    }
                                }

                                Item {
                                    width: 88
                                    height: 44

                                    Rectangle {
                                        id: moveBtn
                                        visible: box.pages.length > 1
                                        x: 0
                                        width: 44
                                        height: 44
                                        radius: 22
                                        color: moveHover.hovered && !box.saving ? Palette.tint : "transparent"

                                        Icon {
                                            anchors.centerIn: parent
                                            name: pageItem.index ? "chevron-left" : "chevron-right"
                                            size: 20
                                            color: Palette.textMuted
                                        }

                                        HoverHandler {
                                            id: moveHover
                                            cursorShape: Qt.PointingHandCursor
                                        }

                                        TapHandler {
                                            enabled: !box.saving
                                            onTapped: box.movePage(pageItem.index, pageItem.index ? -1 : 1)
                                        }

                                        Accessible.role: Accessible.Button
                                        Accessible.name: pageItem.index ? "Move page " + (pageItem.index + 1) + " earlier" : "Move page 1 later"
                                    }

                                    Rectangle {
                                        anchors.right: parent.right
                                        width: 44
                                        height: 44
                                        radius: 22
                                        color: removeHover.hovered && !box.saving ? Palette.tint : "transparent"

                                        Icon {
                                            anchors.centerIn: parent
                                            name: "x"
                                            size: 20
                                            color: Palette.textMuted
                                        }

                                        HoverHandler {
                                            id: removeHover
                                            cursorShape: Qt.PointingHandCursor
                                        }

                                        TapHandler {
                                            enabled: !box.saving
                                            onTapped: box.removePage(pageItem.index)
                                        }

                                        Accessible.role: Accessible.Button
                                        Accessible.name: "Remove page " + (pageItem.index + 1)
                                    }
                                }
                            }
                        }
                    }

                    ScrollView {
                        id: fieldScroll
                        visible: !box.hideField
                        Layout.fillWidth: true
                        Layout.preferredHeight: Math.min(320, Math.max(56, addField.implicitHeight))
                        clip: true

                        TextArea {
                            id: addField
                            padding: 16
                            wrapMode: TextArea.Wrap
                            enabled: !box.saving
                            color: Palette.text
                            placeholderText: !box.photoNote ? "Paste a link, a recipe, or type a name"
                                             : box.vision === false ? "Photos need Wee Chef, so a note isn't used"
                                             : "Add a note for Wee Chef, like which recipe on the page (optional)"
                            placeholderTextColor: Palette.textMuted
                            font.family: Palette.fontSans
                            font.pixelSize: 16
                            selectionColor: Palette.tile
                            selectedTextColor: Palette.onTile
                            background: null
                            Accessible.name: !box.photoNote ? "Recipe link, text or name"
                                             : box.vision === false ? "Note (not used: photos need Wee Chef)"
                                             : "A note for Wee Chef about the photos"
                            onTextChanged: box.input = text
                            onActiveFocusChanged: box.fieldFocused = activeFocus
                            Keys.onPressed: function (event) {
                                var enter = event.key === Qt.Key_Return || event.key === Qt.Key_Enter
                                if (!enter)
                                    return
                                if (event.modifiers & Qt.ControlModifier || event.modifiers & Qt.MetaModifier) {
                                    box.add()
                                    event.accepted = true
                                } else if (!(event.modifiers & Qt.ShiftModifier) && (box.now === "link" || box.now === "scratch")) {
                                    // Links and names submit on Enter; pasted text needs Shift+Enter for new lines anyway
                                    box.add()
                                    event.accepted = true
                                }
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: Palette.line
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.margins: 4
                        spacing: 10

                        Rectangle {
                            id: chip
                            Layout.preferredHeight: 44
                            Layout.preferredWidth: chipRow.implicitWidth + 24
                            radius: 12
                            color: chipHover.hovered ? Qt.darker(Palette.tint, 1.04) : Palette.tint

                            Row {
                                id: chipRow
                                x: 14
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: 4

                                Text {
                                    text: box.modeLabel
                                    color: Palette.primary
                                    font.family: Palette.fontSans
                                    font.pixelSize: 14
                                    font.weight: Font.Bold
                                    anchors.verticalCenter: parent.verticalCenter
                                }

                                Icon {
                                    name: "chevron-down"
                                    size: 16
                                    color: Palette.primary
                                    rotation: menu.opened ? 180 : 0
                                    anchors.verticalCenter: parent.verticalCenter

                                    Behavior on rotation {
                                        NumberAnimation { duration: 150 }
                                    }
                                }
                            }

                            HoverHandler {
                                id: chipHover
                                cursorShape: Qt.PointingHandCursor
                            }

                            TapHandler {
                                onTapped: menu.opened ? menu.close() : menu.open()
                            }

                            activeFocusOnTab: true
                            Accessible.role: Accessible.Button
                            Accessible.name: box.modeLabel
                            Keys.onPressed: function (event) {
                                if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                                    menu.opened ? menu.close() : menu.open()
                                    event.accepted = true
                                }
                            }

                            Rectangle {
                                visible: chip.activeFocus
                                anchors.fill: parent
                                radius: 12
                                color: "transparent"
                                border.width: 2
                                border.color: Palette.primary
                            }
                        }

                        Text {
                            Layout.fillWidth: true
                            text: box.summary
                            elide: Text.ElideRight
                            color: Palette.textMuted
                            font.family: Palette.fontSans
                            font.pixelSize: 14
                            Accessible.role: Accessible.StaticText
                        }

                        Rectangle {
                            id: addButton
                            Layout.preferredHeight: 44
                            Layout.preferredWidth: addRow.implicitWidth + 40
                            radius: 12
                            color: box.canAdd ? Palette.butter : Palette.tint
                            opacity: addPress.pressed ? 0.85 : 1

                            Row {
                                id: addRow
                                anchors.centerIn: parent
                                spacing: 6

                                Icon {
                                    visible: box.saving
                                    name: "loader-circle"
                                    size: 16
                                    color: addLabel.color
                                    anchors.verticalCenter: parent.verticalCenter

                                    RotationAnimator on rotation {
                                        running: box.saving
                                        from: 0
                                        to: 360
                                        duration: 900
                                        loops: Animation.Infinite
                                    }
                                }

                                Text {
                                    id: addLabel
                                    text: box.buttonLabel
                                    color: box.canAdd ? Palette.onButter : Palette.textMuted
                                    font.family: Palette.fontSans
                                    font.pixelSize: 15
                                    font.weight: Font.Bold
                                    anchors.verticalCenter: parent.verticalCenter
                                }
                            }

                            HoverHandler {
                                cursorShape: box.canAdd ? Qt.PointingHandCursor : Qt.ArrowCursor
                            }

                            TapHandler {
                                id: addPress
                                enabled: box.canAdd
                                onTapped: box.add()
                            }

                            activeFocusOnTab: true
                            Accessible.role: Accessible.Button
                            Accessible.name: box.buttonLabel
                            Keys.onPressed: function (event) {
                                if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                                    box.add()
                                    event.accepted = true
                                }
                            }

                            Rectangle {
                                visible: addButton.activeFocus
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

        BlockedNudge {
            visible: box.blocked !== ""
            url: box.blocked
            why: box.blockedWhy
            name: box.blockedSite
            dismissable: true
            Layout.fillWidth: true
            Layout.topMargin: 12
            onPaste: box.pasteInstead()
            onDismissed: box.blocked = ""
        }
    }

    // The mode menu, under the box
    Popup {
        id: menu
        parent: box
        y: box.height + 8
        width: box.width
        padding: 4
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

        background: Rectangle {
            radius: 16
            color: Palette.paper
            border.width: 1
            border.color: Palette.line

            // Menus are the one place with a shadow
            Rectangle {
                z: -1
                anchors.fill: parent
                anchors.topMargin: 4
                anchors.leftMargin: 1
                anchors.rightMargin: -1
                anchors.bottomMargin: -6
                radius: 18
                color: Qt.rgba(0, 0, 0, 0.12)
            }
        }

        contentItem: Column {
            spacing: 0
            Accessible.role: Accessible.PopupMenu
            Accessible.name: "How to add"

            Repeater {
                model: box.modes

                delegate: Rectangle {
                    id: menuRow
                    required property var modelData
                    readonly property bool current: box.mode === modelData.mode

                    width: menu.availableWidth
                    height: 56
                    radius: 12
                    color: rowHover.hovered ? Palette.tint : "transparent"

                    HoverHandler {
                        id: rowHover
                        cursorShape: Qt.PointingHandCursor
                    }

                    TapHandler {
                        onTapped: box.choose(menuRow.modelData.mode)
                    }

                    Accessible.role: Accessible.MenuItem
                    Accessible.name: modelData.label
                    Accessible.checked: current

                    Column {
                        anchors.left: parent.left
                        anchors.leftMargin: 12
                        anchors.right: parent.right
                        anchors.rightMargin: 44
                        anchors.verticalCenter: parent.verticalCenter

                        Text {
                            width: parent.width
                            text: menuRow.modelData.label
                            elide: Text.ElideRight
                            color: Palette.text
                            font.family: Palette.fontSans
                            font.pixelSize: 15
                            font.weight: Font.Bold
                        }

                        Text {
                            width: parent.width
                            text: menuRow.modelData.hint
                            elide: Text.ElideRight
                            color: Palette.textMuted
                            font.family: Palette.fontSans
                            font.pixelSize: 13
                        }
                    }

                    Icon {
                        visible: menuRow.current
                        name: "check"
                        size: 20
                        color: Palette.primary
                        anchors.right: parent.right
                        anchors.rightMargin: 12
                        anchors.verticalCenter: parent.verticalCenter
                    }
                }
            }
        }
    }
}
