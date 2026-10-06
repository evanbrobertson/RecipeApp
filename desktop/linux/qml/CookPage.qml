import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// Cook mode (web islands/CookPage.svelte, pages/shell/cook): one big step at a time with
// the section name, an adjustable timer (suggested from the times the step mentions), the
// ingredients pinned beside the steps on wide windows, and thumb-sized controls. Left and
// Right, Space and PageUp/PageDown move (not while the timer's minutes are being typed); the
// current step, a step's timer length and the ticked ingredients are remembered in the session. Read aloud and keeping the screen awake are browser APIs with
// no QML equivalent, so they are left out.
Item {
    id: page

    property var session
    property int routeId: 0
    property var params: ({})

    property var recipe: null
    property string recipeJson: ""
    property var steps: []
    property bool loading: true
    property string error: ""
    property real scale: 1
    property int index: 0
    property bool finished: false
    property int direction: 1
    property bool logged: false
    property var checked: ({})
    property var timerLengths: ({})
    property bool pinned: parse(Store.read(pinnedKey, "true"), true) !== false
    // Wide windows keep the ingredients beside the steps until unpinned; narrow ones open them over
    readonly property bool wide: width >= 768
    readonly property bool docked: pinned && wide
    readonly property int panelWidth: width >= 1280 ? 384 : width >= 1024 ? 320 : 288
    readonly property string pinnedKey: "crumb:cook-pinned"
    readonly property string timersKey: "crumb:cook-timers:" + routeId
    readonly property string checkedKey: "crumb:cook-checked:" + routeId
    // The ingredient lines in one list: a tick is a line's index here
    readonly property var ingredientRows: {
        var rows = []
        var sections = recipe ? recipe.ingredients : []
        for (var s = 0; s < sections.length; s++) {
            var name = sections[s].name || ""
            for (var i = 0; i < sections[s].items.length; i++) {
                var before = rows.length ? rows[rows.length - 1].section : ""
                rows.push({ "section": name, "head": name !== "" && name !== before, "raw": sections[s].items[i] })
            }
        }
        return rows
    }
    readonly property var step: steps.length && index < steps.length ? steps[index] : null
    readonly property string stepKey: "crumb:cook:" + routeId
    readonly property int textSize: {
        var len = step ? step.text.length : 0
        return len > 420 ? 24 : len > 240 ? 28 : 36
    }

    function parse(text, fallback) {
        try {
            var value = JSON.parse(text)
            return value === null || value === undefined ? fallback : value
        } catch (e) {
            return fallback
        }
    }

    function setScale(value) {
        scale = value
        Store.writeSession("crumb:scale:" + routeId, JSON.stringify(value))
    }

    function remember() {
        Store.writeSession(stepKey, JSON.stringify(index))
    }

    // Reaching the end logs a cook once per visit (the server also ignores repeats)
    function go(to) {
        if (to < 0)
            return
        if (to >= steps.length) {
            finished = true
            if (!logged) {
                logged = true
                markCooked()
            }
            return
        }
        direction = to > index ? 1 : -1
        finished = false
        index = to
        remember()
        slide.restart()
    }

    function next() {
        go(index + 1)
    }

    function prev() {
        if (finished)
            finished = false
        else
            go(index - 1)
    }

    function exit() {
        ApplicationWindow.window.go("recipe", { "id": routeId })
    }

    function markCooked() {
        var id = routeId
        requests.call("cooked", { "id": id }, function (result) {
            var w = ApplicationWindow.window
            if (!result || result.eventId === null || result.eventId === undefined) {
                w.toast({ "title": "Already marked as cooked", "description": "Logged in the last few hours." })
                return
            }
            w.toast({
                "title": "Marked as cooked",
                "description": "It'll sit out of Try next for a couple of weeks.",
                "tone": "success",
                "action": {
                    "label": "Undo",
                    "onselect": function () {
                        requests.call("undoCooked", { "id": id, "event": result.eventId }, function () {}, function (message) {
                            w.toast({ "title": "Couldn't undo", "description": message, "tone": "error" })
                        })
                    }
                }
            })
        }, function (message) {
            ApplicationWindow.window.toast({ "title": "Couldn't mark it as cooked", "description": message, "tone": "error" })
        })
    }

    function setPinned(value) {
        pinned = value
        Store.write(pinnedKey, JSON.stringify(value))
    }

    function setTimerLength(seconds) {
        var next = Object.assign({}, timerLengths)
        next[index] = seconds
        timerLengths = next
        Store.writeSession(timersKey, JSON.stringify(next))
    }

    function startTimer(label, seconds) {
        var list = parse(Store.read("crumb:timers", "[]"), [])
        var now = Date.now()
        list.push({ "id": now, "label": label, "total": seconds, "endsAt": now + seconds * 1000, "done": false })
        Store.write("crumb:timers", JSON.stringify(list))
    }

    function toggleChecked(line) {
        var next = Object.assign({}, checked)
        if (next[line])
            delete next[line]
        else
            next[line] = true
        checked = next
        Store.writeSession(checkedKey, JSON.stringify(Object.keys(next).map(Number)))
    }

    Keys.onPressed: (event) => {
        if (stepTimer.typing)
            return
        if (event.key === Qt.Key_Right || event.key === Qt.Key_Space || event.key === Qt.Key_PageDown) {
            next()
            event.accepted = true
        } else if (event.key === Qt.Key_Left || event.key === Qt.Key_PageUp) {
            prev()
            event.accepted = true
        }
    }

    focus: true

    Component.onCompleted: {
        scale = parse(Store.readSession("crumb:scale:" + routeId, "1"), 1)
        index = parse(Store.readSession(stepKey, "0"), 0)
        timerLengths = parse(Store.readSession(timersKey, "{}"), {})
        var ticked = {}
        parse(Store.readSession(checkedKey, "[]"), []).forEach(function (line) { ticked[line] = true })
        checked = ticked
        requests.call("recipe", { "id": routeId }, function (r) {
            recipe = r
            recipeJson = JSON.stringify(r)
            steps = JSON.parse(Core.cookSteps(recipeJson))
            // A remembered step past the end (the recipe was edited since) starts over
            if (index < 0 || index >= steps.length)
                index = 0
            loading = false
        }, function (message) {
            error = message
            loading = false
        })
    }

    Requests {
        id: requests
    }

    Rectangle {
        anchors.fill: parent
        color: Palette.bg
    }

    // ─── Header: the tile is the chrome ───
    Rectangle {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        height: headerColumn.implicitHeight + 4
        color: Palette.tile

        ColumnLayout {
            id: headerColumn
            width: Math.min(768, parent.width)
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 0

            RowLayout {
                Layout.fillWidth: true
                Layout.leftMargin: 16
                Layout.rightMargin: 16
                Layout.topMargin: 8
                spacing: 8

                CookTileButton {
                    iconName: "x"
                    tip: "Exit cook mode"
                    onClicked: page.exit()
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2

                    Text {
                        Layout.fillWidth: true
                        text: page.recipe ? page.recipe.title : ""
                        color: Palette.onTile
                        font.family: Palette.fontSerif
                        font.pixelSize: 20
                        elide: Text.ElideRight
                        horizontalAlignment: Text.AlignHCenter
                    }

                    Text {
                        Layout.fillWidth: true
                        text: page.finished ? "All done" : page.steps.length ? "Step " + (page.index + 1) + " of " + page.steps.length : ""
                        color: Palette.onTile
                        font.family: Palette.fontSans
                        font.pixelSize: 13
                        font.weight: Font.DemiBold
                        horizontalAlignment: Text.AlignHCenter
                    }
                }

                CookTileButton {
                    iconName: "list"
                    tip: page.wide ? "Pin ingredients" : "Ingredients"
                    active: page.wide && page.pinned
                    onClicked: page.wide ? page.setPinned(!page.pinned) : ingredientsModal.open()
                }
            }

            // Progress: click a segment to jump
            RowLayout {
                Layout.fillWidth: true
                Layout.leftMargin: 16
                Layout.rightMargin: 16
                spacing: 4

                Repeater {
                    model: page.steps.length

                    delegate: MouseArea {
                        id: segment
                        required property int index
                        readonly property bool current: index === page.index && !page.finished

                        Layout.fillWidth: true
                        Layout.preferredHeight: 44
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: page.go(index)

                        Rectangle {
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width
                            height: segment.current ? 10 : segment.containsMouse ? 8 : 6
                            radius: height / 2
                            color: Palette.onTile
                            opacity: segment.index <= page.index || page.finished ? 1 : 0.3

                            Behavior on height {
                                NumberAnimation { duration: 150 }
                            }
                        }
                    }
                }
            }
        }
    }

    // ─── The step ───
    Item {
        id: stage
        anchors.top: header.bottom
        anchors.bottom: footer.top
        anchors.left: parent.left
        anchors.right: panel.left
        clip: true

        DragHandler {
            xAxis.enabled: true
            yAxis.enabled: false
            target: null
            onActiveChanged: {
                if (active)
                    return
                if (Math.abs(translation.x) > 60) {
                    if (translation.x < 0)
                        page.next()
                    else
                        page.prev()
                }
            }
        }

        Rectangle {
            visible: page.loading
            width: Math.min(768, parent.width - 40)
            height: 40
            radius: 12
            color: Palette.tint
            anchors.centerIn: parent
        }

        Body {
            visible: !page.loading && page.error !== ""
            anchors.centerIn: parent
            text: page.error
            color: Palette.error
            font.pixelSize: 18
        }

        Body {
            visible: !page.loading && page.error === "" && page.steps.length === 0
            anchors.centerIn: parent
            text: "This recipe has no steps yet."
            muted: true
            font.pixelSize: 18
        }

        Flickable {
            id: flick
            visible: !page.loading && page.error === "" && page.steps.length > 0 && !page.finished
            anchors.fill: parent
            contentWidth: width
            contentHeight: Math.max(height, stepColumn.implicitHeight + 48)
            boundsBehavior: Flickable.StopAtBounds
            clip: true
            ScrollBar.vertical: ScrollBar {}

            // The step slides in from the direction of travel
            Item {
                id: slider
                width: flick.width
                height: flick.contentHeight

                ParallelAnimation {
                    id: slide
                    NumberAnimation { target: slider; property: "x"; from: 40 * page.direction; to: 0; duration: 280; easing.type: Easing.OutCubic }
                    NumberAnimation { target: slider; property: "opacity"; from: 0; to: 1; duration: 280 }
                }

                ColumnLayout {
                    id: stepColumn
                    width: Math.min(704, parent.width - 40)
                    x: (parent.width - width) / 2
                    y: Math.max(24, (flick.height - implicitHeight) / 2)
                    spacing: 0

                    Text {
                        visible: !!page.step && !!page.step.section
                        text: page.step && page.step.section ? page.step.section : ""
                        color: Palette.primary
                        font.family: Palette.fontSans
                        font.pixelSize: 13
                        font.weight: Font.Bold
                        font.capitalization: Font.AllUppercase
                        font.letterSpacing: 0.5
                        Layout.bottomMargin: 12
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 24

                        Text {
                            text: page.index + 1
                            color: Palette.primary
                            font.family: Palette.fontSerif
                            font.pixelSize: 60
                            Layout.alignment: Qt.AlignTop
                        }

                        Text {
                            text: page.step ? page.step.text : ""
                            color: Palette.text
                            font.family: Palette.fontSans
                            font.pixelSize: page.textSize
                            font.weight: Font.DemiBold
                            wrapMode: Text.WordWrap
                            lineHeight: 1.0
                            Layout.fillWidth: true
                            Layout.alignment: Qt.AlignTop
                        }
                    }

                    StepTimer {
                        id: stepTimer
                        Layout.fillWidth: true
                        Layout.topMargin: 28
                        choices: page.step ? page.step.timers : []
                        seconds: page.timerLengths[page.index] !== undefined ? page.timerLengths[page.index] : -1
                        onAdjusted: (chosen) => page.setTimerLength(chosen)
                        onStarted: (chosen) => page.startTimer("Step " + (page.index + 1), chosen)
                        onTypingChanged: if (!typing) page.forceActiveFocus()
                    }
                }
            }
        }

        // All done
        ColumnLayout {
            visible: page.finished
            anchors.centerIn: parent
            width: Math.min(672, parent.width - 40)
            spacing: 0

            Rectangle {
                Layout.alignment: Qt.AlignHCenter
                width: 96
                height: 96
                radius: 16
                color: Palette.tile

                Icon {
                    anchors.centerIn: parent
                    name: "chef-hat"
                    size: 44
                    color: Palette.onTile
                }
            }

            Heading {
                level: 1
                text: "Bon appétit!"
                font.pixelSize: 36
                horizontalAlignment: Text.AlignHCenter
                Layout.fillWidth: true
                Layout.topMargin: 20
            }

            Body {
                text: "That's every step. Enjoy it."
                muted: true
                font.pixelSize: 18
                horizontalAlignment: Text.AlignHCenter
                Layout.fillWidth: true
                Layout.topMargin: 8
            }

            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: 32
                spacing: 12

                CrumbButton {
                    text: "Back to recipe"
                    kind: "primary"
                    implicitHeight: 56
                    onClicked: page.exit()
                }

                CrumbButton {
                    text: "Start over"
                    kind: "soft"
                    implicitHeight: 56
                    onClicked: page.go(0)
                }
            }
        }
    }

    // ─── Big thumb-zone controls ───
    Rectangle {
        id: footer
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: panel.left
        height: 92
        color: Palette.paper

        Rectangle {
            anchors.top: parent.top
            width: parent.width
            height: 1
            color: Palette.line
        }

        RowLayout {
            width: Math.min(704, parent.width - 32)
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.top: parent.top
            anchors.topMargin: 12
            spacing: 12

            CrumbButton {
                iconName: "arrow-left"
                iconSize: 24
                enabled: !(page.index === 0 && !page.finished)
                implicitWidth: 64
                implicitHeight: 64
                Accessible.name: "Previous step"
                onClicked: page.prev()
            }

            CrumbButton {
                id: nextButton
                visible: !page.finished
                Layout.fillWidth: true
                kind: "primary"
                text: page.index + 1 >= page.steps.length ? "Finish" : "Next step"
                implicitHeight: 64
                enabled: page.steps.length > 0
                onClicked: page.next()

                // The label first, then its icon, as the web draws it
                contentItem: RowLayout {
                    spacing: 8

                    Item {
                        Layout.fillWidth: true
                    }

                    Text {
                        text: nextButton.text
                        color: nextButton.ink
                        font.family: Palette.fontSans
                        font.pixelSize: 18
                        font.weight: Font.Bold
                    }

                    Icon {
                        name: page.index + 1 >= page.steps.length ? "party-popper" : "arrow-right"
                        size: 20
                        color: nextButton.ink
                    }

                    Item {
                        Layout.fillWidth: true
                    }
                }
            }

            CrumbButton {
                visible: page.finished
                Layout.fillWidth: true
                kind: "outline"
                text: "Close"
                implicitHeight: 64
                onClicked: page.exit()
            }
        }
    }

    // ─── Ingredients: pinned beside the steps on wide windows, a modal on narrow ones ───
    Rectangle {
        id: panel
        anchors.top: header.bottom
        anchors.bottom: parent.bottom
        anchors.right: parent.right
        width: page.docked ? page.panelWidth : 0
        color: Palette.paper
        clip: true

        Behavior on width {
            NumberAnimation { duration: 220; easing.type: Easing.OutCubic }
        }

        Rectangle {
            width: 1
            height: parent.height
            color: Palette.line
        }

        // One width as the panel slides, so the list doesn't reflow
        ColumnLayout {
            x: 1
            width: page.panelWidth - 1
            height: parent.height
            spacing: 0

            Heading {
                level: 2
                text: "Ingredients"
                Layout.leftMargin: 20
                Layout.rightMargin: 20
                Layout.topMargin: 20
            }

            ScaleControl {
                value: page.scale
                Layout.leftMargin: 20
                Layout.topMargin: 12
                Layout.bottomMargin: 8
                onEdited: (v) => page.setScale(v)
            }

            CookIngredients {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.leftMargin: 20
                Layout.rightMargin: 8
                Layout.bottomMargin: 20
                rows: page.ingredientRows
                checked: page.checked
                scale: page.scale
                onToggled: (line) => page.toggleChecked(line)
            }
        }
    }

    // Running timers centre under the steps, not behind the panel
    Binding {
        target: ApplicationWindow.window
        property: "timerInset"
        value: panel.width
        restoreMode: Binding.RestoreValue
    }

    onWideChanged: {
        if (wide)
            ingredientsModal.close()
    }

    Modal {
        id: ingredientsModal
        title: "Ingredients"

        RowLayout {
            Layout.fillWidth: true

            Item {
                Layout.fillWidth: true
            }

            ScaleControl {
                value: page.scale
                onEdited: (v) => page.setScale(v)
            }
        }

        CookIngredients {
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(contentHeight, 420)
            rows: page.ingredientRows
            checked: page.checked
            scale: page.scale
            onToggled: (line) => page.toggleChecked(line)
        }
    }
}
