import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// Cook mode (web islands/CookPage.svelte, pages/shell/cook): one big step at a time with
// the section name, timers for any time a step mentions, the ingredients the step uses, and
// thumb-sized controls. Left and Right, Space and PageUp/PageDown move; the current step is
// remembered in the session. Read aloud and keeping the screen awake are browser APIs with
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
    readonly property var step: steps.length && index < steps.length ? steps[index] : null
    readonly property string stepKey: "crumb:cook:" + routeId
    readonly property var stepIngredients: {
        if (!step)
            return []
        return JSON.parse(Core.stepIngredients(step.text, step.section || "", recipeJson, scale))
    }
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

    function startTimer(label, seconds) {
        var list = parse(Store.read("crumb:timers", "[]"), [])
        var now = Date.now()
        list.push({ "id": now, "label": label, "total": seconds, "endsAt": now + seconds * 1000, "done": false })
        Store.write("crumb:timers", JSON.stringify(list))
    }

    function toggleChecked(key) {
        var next = Object.assign({}, checked)
        if (next[key])
            delete next[key]
        else
            next[key] = true
        checked = next
    }

    Keys.onPressed: (event) => {
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
                    tip: "Ingredients"
                    onClicked: ingredientsModal.open()
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
        anchors.right: parent.right
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

                    Flow {
                        visible: !!page.step && page.step.timers.length > 0
                        Layout.fillWidth: true
                        Layout.topMargin: 28
                        spacing: 10

                        Repeater {
                            model: page.step ? page.step.timers : []

                            delegate: CrumbButton {
                                id: timerButton
                                required property var modelData
                                text: "Start " + modelData.label + " timer"
                                iconName: "alarm-clock"
                                implicitHeight: 52
                                onClicked: page.startTimer("Step " + (page.index + 1) + ": " + modelData.label, modelData.seconds)

                                background: Rectangle {
                                    radius: 12
                                    color: Palette.tile
                                    opacity: timerButton.down ? 0.85 : 1
                                }
                                contentItem: RowLayout {
                                    spacing: 8

                                    Icon {
                                        name: "alarm-clock"
                                        size: 20
                                        color: Palette.onTile
                                    }

                                    Text {
                                        text: timerButton.text
                                        color: Palette.onTile
                                        font.family: Palette.fontSans
                                        font.pixelSize: 16
                                        font.weight: Font.Bold
                                    }
                                }
                            }
                        }
                    }

                    Card {
                        visible: page.stepIngredients.length > 0
                        Layout.fillWidth: true
                        Layout.topMargin: 28
                        implicitHeight: needCol.implicitHeight + 32

                        ColumnLayout {
                            id: needCol
                            anchors.fill: parent
                            anchors.margins: 16
                            spacing: 12

                            Text {
                                text: "You'll need"
                                color: Palette.textMuted
                                font.family: Palette.fontSans
                                font.pixelSize: 13
                                font.weight: Font.DemiBold
                                font.capitalization: Font.AllUppercase
                                font.letterSpacing: 1
                            }

                            Flow {
                                Layout.fillWidth: true
                                spacing: 8

                                Repeater {
                                    model: page.stepIngredients

                                    delegate: Rectangle {
                                        required property var modelData
                                        width: Math.min(needText.implicitWidth + 32, needCol.width)
                                        height: needText.implicitHeight + 16
                                        radius: 12
                                        color: Palette.tint

                                        Text {
                                            id: needText
                                            anchors.centerIn: parent
                                            width: parent.width - 32
                                            text: parent.modelData.scaled
                                            color: Palette.text
                                            font.family: Palette.fontSans
                                            font.pixelSize: 17
                                            font.weight: Font.DemiBold
                                            wrapMode: Text.WordWrap
                                        }
                                    }
                                }
                            }
                        }
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
        anchors.right: parent.right
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

    // ─── Ingredients ───
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

        Flickable {
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(ingredientColumn.implicitHeight, 420)
            contentHeight: ingredientColumn.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}

            ColumnLayout {
                id: ingredientColumn
                width: parent.width - 12
                spacing: 0

                Repeater {
                    model: page.recipe ? page.recipe.ingredients : []

                    delegate: ColumnLayout {
                        id: section
                        required property var modelData
                        required property int index
                        Layout.fillWidth: true
                        spacing: 0

                        Text {
                            visible: !!section.modelData.name
                            text: section.modelData.name || ""
                            color: Palette.primary
                            font.family: Palette.fontSans
                            font.pixelSize: 13
                            font.weight: Font.Bold
                            font.capitalization: Font.AllUppercase
                            font.letterSpacing: 0.5
                            Layout.topMargin: 16
                            Layout.bottomMargin: 4
                        }

                        Repeater {
                            model: section.modelData.items

                            delegate: MouseArea {
                                id: row
                                required property string modelData
                                required property int index
                                readonly property string rowKey: section.index + ":" + index
                                readonly property bool on: !!page.checked[rowKey]

                                Layout.fillWidth: true
                                implicitHeight: Math.max(48, label.implicitHeight + 20)
                                cursorShape: Qt.PointingHandCursor
                                onClicked: page.toggleChecked(rowKey)

                                Rectangle {
                                    anchors.fill: parent
                                    radius: 12
                                    color: Palette.tint
                                    opacity: row.pressed ? 1 : 0
                                }

                                Rectangle {
                                    id: box
                                    x: 4
                                    y: 12
                                    width: 24
                                    height: 24
                                    radius: 12
                                    color: row.on ? Palette.tile : "transparent"
                                    border.width: 2
                                    border.color: row.on ? Palette.tile : Palette.line

                                    Icon {
                                        visible: row.on
                                        anchors.centerIn: parent
                                        name: "check"
                                        size: 16
                                        color: Palette.onTile
                                    }
                                }

                                Text {
                                    id: label
                                    anchors.left: box.right
                                    anchors.leftMargin: 12
                                    anchors.right: parent.right
                                    anchors.rightMargin: 4
                                    y: 10
                                    text: Core.scaleIngredient(row.modelData, page.scale)
                                    color: row.on ? Palette.textMuted : Palette.text
                                    font.family: Palette.fontSans
                                    font.pixelSize: 18
                                    font.strikeout: row.on
                                    wrapMode: Text.WordWrap
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
