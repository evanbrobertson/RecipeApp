import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Shapes

import app.crumb.desktop 1.0

// The Import page (web pages/import.astro and islands/ImportTools.svelte): paste links to
// import one after another, or drop files (Crumb backups, Paprika, Mealie, PDFs, web pages,
// text) and photos of a recipe. Left out because only a browser can do them: the "Read in
// Crumb" and "Crumb export" bookmarklets and the browser extension card.
Item {
    id: page

    property var session
    property int routeId: 0
    property var params: ({})

    readonly property int maxPhotos: 6
    // Link jobs: {url, state: waiting|working|saved|duplicate|failed, id, title, kind, message}
    property var jobs: []
    property bool running: false
    // File jobs, newest first: {name, state: working|done|failed, created, duplicates, skipped, message}
    property var fileJobs: []
    property bool dragging: false
    property var vision: null
    readonly property var parsedLinks: linksField.text.trim() ? JSON.parse(Core.linksInText(linksField.text)) : []
    readonly property var linkIcons: ({
        "waiting": "clock",
        "working": "loader-circle",
        "saved": "circle-check",
        "duplicate": "bookmark-check",
        "failed": "circle-x"
    })

    function go(name, params) {
        ApplicationWindow.window.go(name, params)
    }

    function isPhoto(name) {
        return /\.(jpe?g|png|webp|gif|heic|heif|avif)$/i.test(name)
    }

    function baseName(url) {
        var s = String(url)
        return decodeURIComponent(s.substring(s.lastIndexOf("/") + 1))
    }

    function patch(list, i, fields) {
        var next = list.slice()
        next[i] = Object.assign({}, next[i], fields)
        return next
    }

    // ─── Links ───
    property int cursor: 0
    property int active: 0

    function runLinks() {
        if (!parsedLinks.length || running)
            return
        running = true
        jobs = parsedLinks.map(function (url) {
            return { "url": url, "state": "waiting" }
        })
        cursor = 0
        // Two at a time: fast enough, and gentle on the server's headless browser
        active = 2
        nextLink()
        nextLink()
    }

    function nextLink() {
        if (cursor >= jobs.length) {
            active -= 1
            if (active === 0)
                finishLinks()
            return
        }
        var i = cursor++
        jobs = patch(jobs, i, { "state": "working" })
        // A cooking video waits its turn on the server; its place shows as the message
        requests.call("import", { "url": jobs[i].url }, function (res) {
            if (res.cookbook) {
                var b = res.cookbook
                jobs = patch(jobs, i, {
                    "state": b.added ? "saved" : "duplicate",
                    "id": b.id,
                    "kind": "cookbook",
                    "title": Core.bookImportedTitle(b.name, b.added, b.duplicates || 0, b.skipped === undefined ? -1 : b.skipped),
                    "message": ""
                })
            } else {
                jobs = patch(jobs, i, {
                    "state": res.isNew ? "saved" : "duplicate",
                    "id": res.recipe.id,
                    "kind": "recipe",
                    "title": res.recipe.title,
                    "message": res.droppedPhoto ? "Saved without its photo: the link doesn't work" : ""
                })
            }
            nextLink()
        }, function (err) {
            jobs = patch(jobs, i, { "state": "failed", "message": err })
            nextLink()
        }, function (line) {
            jobs = patch(jobs, i, { "message": line })
        })
    }

    function finishLinks() {
        running = false
        var saved = jobs.filter(function (j) { return j.state === "saved" }).length
        ApplicationWindow.window.toast({ "title": "Imported " + saved + " of " + jobs.length + " links", "tone": "success" })
    }

    // ─── Files ───
    function addFileJob(job) {
        fileJobs = [job].concat(fileJobs)
    }

    // The job is always the newest at the time it's made; later ones push it down, so
    // updates find it by its serial
    property int serial: 0

    function updateFileJob(key, fields) {
        for (var i = 0; i < fileJobs.length; i++) {
            if (fileJobs[i].key === key) {
                fileJobs = patch(fileJobs, i, fields)
                return
            }
        }
    }

    function checkVision(then) {
        if (vision !== null) {
            then()
            return
        }
        var known = ApplicationWindow.window.connector
        if (known && typeof known.vision === "boolean") {
            vision = known.vision
            then()
            return
        }
        requests.call("connector", {}, function (c) {
            page.vision = !!(c && c.vision)
            then()
        }, function () {
            page.vision = false
            then()
        })
    }

    // Photos chosen together are the pages of one recipe (up to six)
    function uploadPhotos(photos) {
        var n = Math.min(photos.length, maxPhotos)
        var key = ++serial
        addFileJob({
            "key": key, "name": n === 1 ? baseName(photos[0]) : n + " photos", "state": "working",
            "created": [], "duplicates": 0, "message": ""
        })
        if (photos.length > maxPhotos) {
            updateFileJob(key, { "state": "failed", "message": "Up to " + maxPhotos + " photos at a time, all pages of one recipe" })
            return
        }
        checkVision(function () {
            if (page.vision === false) {
                page.updateFileJob(key, { "state": "failed", "message": "Wee Chef isn't switched on for this Crumb, and this app can't read photos by itself." })
                return
            }
            page.updateFileJob(key, { "message": "Reading " + n + " photo" + (n === 1 ? "" : "s") + "…" })
            requests.call("importPhotos", { "paths": photos.map(String) }, function (res) {
                page.updateFileJob(key, {
                    "state": "done",
                    "created": res.isNew ? [{ "id": res.recipe.id, "title": res.recipe.title }] : [],
                    "duplicates": res.isNew ? 0 : 1,
                    "message": ""
                })
            }, function (err) {
                page.updateFileJob(key, { "state": "failed", "message": err })
            })
        })
    }

    function uploadFile(url) {
        var key = ++serial
        addFileJob({ "key": key, "name": baseName(url), "state": "working", "created": [], "duplicates": 0, "message": "" })
        requests.call("importFiles", { "paths": [String(url)] }, function (list) {
            var result = list && list[0]
            if (!result || result.error)
                updateFileJob(key, { "state": "failed", "message": result && result.error ? result.error : "Nothing imported" })
            else
                updateFileJob(key, { "state": "done", "created": result.created, "duplicates": result.duplicates, "skipped": result.skipped })
        }, function (err) {
            updateFileJob(key, { "state": "failed", "message": err })
        })
    }

    function uploadFiles(urls) {
        var photos = []
        var others = []
        for (var i = 0; i < urls.length; i++)
            (isPhoto(String(urls[i])) ? photos : others).push(urls[i])
        if (photos.length)
            uploadPhotos(photos)
        others.forEach(uploadFile)
    }

    function countLine(job) {
        var line = job.created.length + " added"
        if (job.duplicates)
            line += ", " + job.duplicates + " already saved"
        if (job.skipped)
            line += ", " + job.skipped + " skipped"
        return line
    }

    Component.onCompleted: {
        if (params.links)
            linksField.text = params.links
    }

    Requests {
        id: requests
    }

    FileDialog {
        id: picker
        title: "Choose files"
        fileMode: FileDialog.OpenFiles
        nameFilters: ["Recipes and photos (*.pdf *.paprikarecipes *.paprikarecipe *.json *.html *.htm *.txt *.md *.zip *.jpg *.jpeg *.png *.webp *.gif *.heic *.heif *.avif)", "All files (*)"]
        onAccepted: page.uploadFiles(selectedFiles)
    }

    ScrollPage {
        anchors.fill: parent
        maxWidth: 600

        ColumnLayout {
            width: parent.width
            spacing: 32

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0

                Heading {
                    level: 1
                    text: "Import recipes"
                    font.pixelSize: 30
                    Layout.fillWidth: true
                }

                Body {
                    text: "Paste links or upload files. Duplicates are skipped."
                    muted: true
                    Layout.topMargin: 8
                    Layout.fillWidth: true
                }
            }

            // Links
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0

                Text {
                    text: "LINKS"
                    color: Palette.textMuted
                    font.family: Palette.fontSans
                    font.pixelSize: 13
                    font.weight: Font.Bold
                    font.letterSpacing: 0.5
                    Layout.leftMargin: 16
                    Layout.bottomMargin: 8
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: Math.min(320, Math.max(80, linksField.implicitHeight + 4))
                    radius: 12
                    color: Palette.paper
                    border.width: linksField.activeFocus ? 2 : 1
                    border.color: linksField.activeFocus ? Palette.primary : Palette.line

                    ScrollView {
                        anchors.fill: parent
                        anchors.margins: 1
                        clip: true

                        TextArea {
                            id: linksField
                            wrapMode: TextArea.Wrap
                            leftPadding: 14
                            rightPadding: 14
                            topPadding: 10
                            bottomPadding: 10
                            color: Palette.text
                            placeholderText: "https://…\nhttps://…"
                            placeholderTextColor: Palette.textMuted
                            font.family: "monospace"
                            font.pixelSize: 14
                            selectionColor: Palette.tile
                            selectedTextColor: Palette.onTile
                            background: null
                            Accessible.name: "Links"
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: 12
                    spacing: 12

                    Text {
                        Layout.fillWidth: true
                        text: page.parsedLinks.length
                              ? page.parsedLinks.length + " link" + (page.parsedLinks.length === 1 ? "" : "s") + " found"
                              : "One per line, or any text with links in it"
                        elide: Text.ElideRight
                        color: Palette.textMuted
                        font.family: Palette.fontSans
                        font.pixelSize: 13
                        font.weight: Font.DemiBold
                    }

                    CrumbButton {
                        kind: "primary"
                        iconName: page.running ? "loader-circle" : "download"
                        text: page.running ? "Importing…" : "Import links"
                        enabled: page.parsedLinks.length > 0 && !page.running
                        onClicked: page.runLinks()
                    }
                }

                Card {
                    visible: page.jobs.length > 0
                    Layout.fillWidth: true
                    Layout.topMargin: 16
                    implicitHeight: jobColumn.implicitHeight
                    clip: true

                    Column {
                        id: jobColumn
                        width: parent.width

                        Repeater {
                            model: page.jobs

                            delegate: Item {
                                id: jobRow
                                required property var modelData
                                required property int index
                                width: jobColumn.width
                                implicitHeight: Math.max(68, jobInfo.implicitHeight + 24)
                                height: implicitHeight

                                Rectangle {
                                    visible: jobRow.index > 0
                                    x: 12
                                    width: parent.width - 24
                                    height: 1
                                    color: Palette.line
                                }

                                Icon {
                                    id: jobIcon
                                    x: 12
                                    y: 14
                                    name: page.linkIcons[jobRow.modelData.state]
                                    size: 20
                                    color: jobRow.modelData.state === "failed" ? Palette.error
                                           : jobRow.modelData.state === "working" || jobRow.modelData.state === "saved" ? Palette.primary
                                           : Palette.textMuted

                                    RotationAnimator on rotation {
                                        running: jobRow.modelData.state === "working"
                                        from: 0
                                        to: 360
                                        duration: 900
                                        loops: Animation.Infinite
                                    }
                                }

                                ColumnLayout {
                                    id: jobInfo
                                    anchors.left: parent.left
                                    anchors.leftMargin: 46
                                    anchors.right: parent.right
                                    anchors.rightMargin: 12
                                    anchors.verticalCenter: parent.verticalCenter
                                    spacing: 0

                                    Text {
                                        visible: !!jobRow.modelData.id
                                        Layout.fillWidth: true
                                        text: jobRow.modelData.title || ""
                                        elide: Text.ElideRight
                                        color: Palette.text
                                        font.family: Palette.fontSans
                                        font.pixelSize: 15
                                        font.weight: Font.Bold
                                        font.underline: titleHover.hovered

                                        HoverHandler {
                                            id: titleHover
                                            cursorShape: Qt.PointingHandCursor
                                        }

                                        TapHandler {
                                            onTapped: page.go(jobRow.modelData.kind, { "id": jobRow.modelData.id })
                                        }
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        text: jobRow.modelData.url
                                        elide: Text.ElideRight
                                        color: Palette.textMuted
                                        font.family: Palette.fontSans
                                        font.pixelSize: jobRow.modelData.id ? 13 : 14
                                        font.weight: jobRow.modelData.id ? Font.DemiBold : Font.Normal
                                    }

                                    Text {
                                        visible: jobRow.modelData.state === "duplicate" && jobRow.modelData.kind !== "cookbook"
                                        text: "Already saved"
                                        color: Palette.textMuted
                                        font.family: Palette.fontSans
                                        font.pixelSize: 13
                                        font.weight: Font.DemiBold
                                    }

                                    Text {
                                        visible: !!jobRow.modelData.message
                                        Layout.fillWidth: true
                                        text: jobRow.modelData.message || ""
                                        wrapMode: Text.WordWrap
                                        color: jobRow.modelData.state === "failed" ? Palette.error : Palette.textMuted
                                        font.family: Palette.fontSans
                                        font.pixelSize: 13
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Files
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0

                Text {
                    text: "FILES"
                    color: Palette.textMuted
                    font.family: Palette.fontSans
                    font.pixelSize: 13
                    font.weight: Font.Bold
                    font.letterSpacing: 0.5
                    Layout.leftMargin: 16
                    Layout.bottomMargin: 8
                }

                Rectangle {
                    id: zone
                    Layout.fillWidth: true
                    Layout.preferredHeight: zoneColumn.implicitHeight + 64
                    radius: 16
                    color: page.dragging ? Palette.tint : Palette.paper

                    Shape {
                        anchors.fill: parent
                        preferredRendererType: Shape.CurveRenderer

                        ShapePath {
                            fillColor: "transparent"
                            strokeWidth: 2
                            strokeColor: page.dragging || zoneHover.hovered ? Palette.tile : Palette.line
                            strokeStyle: ShapePath.DashLine
                            dashPattern: [4, 3]

                            PathRectangle {
                                x: 1
                                y: 1
                                width: zone.width - 2
                                height: zone.height - 2
                                radius: 15
                            }
                        }
                    }

                    ColumnLayout {
                        id: zoneColumn
                        anchors.centerIn: parent
                        width: parent.width - 48
                        spacing: 6

                        Icon {
                            name: "file-up"
                            size: 24
                            color: Palette.textMuted
                            Layout.alignment: Qt.AlignHCenter
                            Layout.bottomMargin: 4
                        }

                        Text {
                            text: "Drop files or tap to choose"
                            color: Palette.text
                            font.family: Palette.fontSans
                            font.pixelSize: 15
                            font.weight: Font.Bold
                            Layout.alignment: Qt.AlignHCenter
                        }

                        Text {
                            text: "PDF, Paprika, Mealie JSON, web pages, text, .zip, a Crumb backup, or photos of a recipe"
                            color: Palette.textMuted
                            font.family: Palette.fontSans
                            font.pixelSize: 13
                            font.weight: Font.DemiBold
                            wrapMode: Text.WordWrap
                            horizontalAlignment: Text.AlignHCenter
                            Layout.fillWidth: true
                        }
                    }

                    HoverHandler {
                        id: zoneHover
                        cursorShape: Qt.PointingHandCursor
                    }

                    TapHandler {
                        onTapped: picker.open()
                    }

                    DropArea {
                        anchors.fill: parent
                        onEntered: page.dragging = true
                        onExited: page.dragging = false
                        onDropped: function (drop) {
                            page.dragging = false
                            if (drop.hasUrls) {
                                drop.accepted = true
                                page.uploadFiles(drop.urls)
                            }
                        }
                    }

                    Accessible.role: Accessible.Button
                    Accessible.name: "Drop files or tap to choose"
                }

                Text {
                    Layout.fillWidth: true
                    Layout.topMargin: 8
                    Layout.leftMargin: 16
                    Layout.rightMargin: 16
                    text: "Up to 25 MB each. In text files, put --- between recipes. Photos chosen together are read as the pages of one recipe (up to " + page.maxPhotos + ")."
                    wrapMode: Text.WordWrap
                    color: Palette.textMuted
                    font.family: Palette.fontSans
                    font.pixelSize: 13
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: 16
                    spacing: 8

                    Repeater {
                        model: page.fileJobs

                        delegate: Card {
                            id: fileCard
                            required property var modelData
                            Layout.fillWidth: true
                            implicitHeight: fileColumn.implicitHeight + 24

                            ColumnLayout {
                                id: fileColumn
                                x: 12
                                y: 12
                                width: parent.width - 24
                                spacing: 0

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 8

                                    Icon {
                                        name: fileCard.modelData.state === "working" ? "loader-circle"
                                              : fileCard.modelData.state === "done" ? "circle-check" : "circle-x"
                                        size: 20
                                        color: fileCard.modelData.state === "failed" ? Palette.error : Palette.primary

                                        RotationAnimator on rotation {
                                            running: fileCard.modelData.state === "working"
                                            from: 0
                                            to: 360
                                            duration: 900
                                            loops: Animation.Infinite
                                        }
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        text: fileCard.modelData.name
                                        elide: Text.ElideRight
                                        color: Palette.text
                                        font.family: Palette.fontSans
                                        font.pixelSize: 15
                                        font.weight: Font.Bold
                                    }

                                    Text {
                                        visible: fileCard.modelData.state === "done"
                                        text: page.countLine(fileCard.modelData)
                                        color: Palette.textMuted
                                        font.family: Palette.fontSans
                                        font.pixelSize: 14
                                    }
                                }

                                Text {
                                    visible: !!fileCard.modelData.message
                                    Layout.fillWidth: true
                                    Layout.topMargin: 4
                                    text: fileCard.modelData.message || ""
                                    wrapMode: Text.WordWrap
                                    color: fileCard.modelData.state === "failed" ? Palette.error : Palette.textMuted
                                    font.family: Palette.fontSans
                                    font.pixelSize: 13
                                }

                                Flow {
                                    visible: fileCard.modelData.created.length > 0
                                    Layout.fillWidth: true
                                    Layout.topMargin: 10
                                    spacing: 8

                                    Repeater {
                                        model: fileCard.modelData.created.slice(0, 12)

                                        delegate: Rectangle {
                                            id: chipItem
                                            required property var modelData
                                            width: Math.min(chipText.implicitWidth + 32, fileColumn.width)
                                            height: 44
                                            radius: 22
                                            color: Palette.tint

                                            Text {
                                                id: chipText
                                                anchors.centerIn: parent
                                                width: Math.min(implicitWidth, parent.width - 32)
                                                elide: Text.ElideRight
                                                text: chipItem.modelData.title
                                                color: Palette.primary
                                                font.underline: chipHover.hovered
                                                font.family: Palette.fontSans
                                                font.pixelSize: 14
                                                font.weight: Font.Bold
                                            }

                                            HoverHandler {
                                                id: chipHover
                                                cursorShape: Qt.PointingHandCursor
                                            }

                                            TapHandler {
                                                onTapped: page.go("recipe", { "id": chipItem.modelData.id })
                                            }
                                        }
                                    }

                                    Text {
                                        visible: fileCard.modelData.created.length > 12
                                        height: 44
                                        verticalAlignment: Text.AlignVCenter
                                        text: "+" + (fileCard.modelData.created.length - 12) + " more"
                                        color: Palette.textMuted
                                        font.family: Palette.fontSans
                                        font.pixelSize: 13
                                        font.weight: Font.DemiBold
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Coming from Just the Recipe?
            Card {
                Layout.fillWidth: true
                implicitHeight: jtrColumn.implicitHeight + 8
                clip: true
                property bool open: false

                ColumnLayout {
                    id: jtrColumn
                    x: 16
                    y: 4
                    width: parent.width - 32
                    spacing: 0

                    Item {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 44

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "Coming from Just the Recipe?"
                            color: Palette.text
                            font.family: Palette.fontSans
                            font.pixelSize: 14
                            font.weight: Font.Bold
                        }

                        Icon {
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            name: "chevron-down"
                            size: 20
                            color: Palette.textMuted
                            rotation: jtrColumn.parent.open ? 180 : 0
                        }

                        HoverHandler {
                            cursorShape: Qt.PointingHandCursor
                        }

                        TapHandler {
                            onTapped: jtrColumn.parent.open = !jtrColumn.parent.open
                        }
                    }

                    Body {
                        visible: jtrColumn.parent.open
                        Layout.fillWidth: true
                        Layout.bottomMargin: 16
                        text: "Bring everything over at once, with your collections as cookbooks, using the Crumb export bookmark on Crumb's website: it reads your recipes in your own browser, so Crumb never sees your Just the Recipe password. It saves just-the-recipe.json. Upload that file above.\n\nFor a few recipes, paste each one's Share link above, upload PDFs made with Print → Save as PDF, or paste the text on Add."
                        muted: true
                        font.pixelSize: 14
                    }
                }
            }
        }
    }
}
