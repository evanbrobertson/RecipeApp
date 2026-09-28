import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// Mise en place (web islands/PrepPage.svelte): everything prepped, measured and in its place
// before the heat goes on. Each ingredient gets a vessel sized to its quantity (PrepBowl);
// tap it once it's ready. Groups, amounts, vessels and the "Get out" line come from Core.
// Keeping the screen awake is a browser API, so that link is left out.
Item {
    id: page

    property var session
    property int routeId: 0
    property var params: ({})

    property var recipe: null
    property var groups: []
    property var counts: []
    property var ready: []
    property real scale: 1
    property bool loading: true
    property string error: ""
    readonly property string readyKey: "crumb:prep:" + routeId
    readonly property bool wide: width >= 640
    property int total: 0
    readonly property bool allReady: total > 0 && ready.length === total

    function parse(text, fallback) {
        try {
            var value = JSON.parse(text)
            return value === null || value === undefined ? fallback : value
        } catch (e) {
            return fallback
        }
    }

    function lines() {
        var out = []
        var sections = recipe ? recipe.ingredients : []
        for (var i = 0; i < sections.length; i++)
            for (var j = 0; j < sections[i].items.length; j++)
                out.push(sections[i].items[j])
        return out
    }

    function refresh() {
        var json = JSON.stringify(lines())
        groups = JSON.parse(Core.prepGroups(json, scale))
        counts = JSON.parse(Core.vesselCounts(json))
        var n = 0
        for (var i = 0; i < groups.length; i++)
            n += groups[i].items.length
        total = n
    }

    function setScale(value) {
        scale = value
        Store.writeSession("crumb:scale:" + routeId, JSON.stringify(value))
        refresh()
    }

    function toggle(key) {
        var at = ready.indexOf(key)
        ready = at === -1 ? ready.concat([key]) : ready.filter(function (k) { return k !== key })
        Store.writeSession(readyKey, JSON.stringify(ready))
    }

    function iconFor(kind) {
        return kind === "chop" ? "slice" : kind === "measure" ? "soup" : "hand"
    }

    function backToRecipe() {
        ApplicationWindow.window.go("recipe", { "id": routeId })
    }

    Component.onCompleted: {
        scale = parse(Store.readSession("crumb:scale:" + routeId, "1"), 1)
        ready = parse(Store.readSession(readyKey, "[]"), [])
        requests.call("recipe", { "id": routeId }, function (r) {
            recipe = r
            refresh()
            loading = false
        }, function (message) {
            error = message
            loading = false
        })
    }

    Requests {
        id: requests
    }

    Flickable {
        id: flick
        anchors.fill: parent
        clip: true
        contentWidth: width
        contentHeight: column.implicitHeight + 32 + 96
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}

        // The progress bar sticks to the top once its place has scrolled away
        Rectangle {
            id: sticky
            z: 5
            width: flick.width
            height: stickyRow.implicitHeight + 24
            y: Math.max(column.y + slot.y, flick.contentY)
            color: Qt.rgba(Palette.bg.r, Palette.bg.g, Palette.bg.b, 0.92)

            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                height: 1
                color: Palette.line
            }

            RowLayout {
                id: stickyRow
                width: column.width
                x: column.x
                y: 12
                spacing: 12

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 10
                    radius: 5
                    color: Palette.tint

                    Rectangle {
                        width: page.total ? parent.width * page.ready.length / page.total : 0
                        height: parent.height
                        radius: 5
                        color: Palette.tile

                        Behavior on width {
                            NumberAnimation { duration: 500 }
                        }
                    }
                }

                Text {
                    text: page.ready.length + "/" + page.total + " ready"
                    color: Palette.text
                    font.family: Palette.fontSans
                    font.pixelSize: 15
                    font.weight: Font.Bold
                }
            }
        }

        ColumnLayout {
            id: column
            x: Math.max(32, (flick.width - width) / 2)
            y: 32
            width: Math.min(1024, flick.width - 64)
            spacing: 0

            // ─── Header ───
            RowLayout {
                Layout.fillWidth: true
                Layout.bottomMargin: 24
                spacing: 12

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 0

                    MouseArea {
                        id: back
                        implicitWidth: Math.min(backRow.implicitWidth + 8, 480)
                        implicitHeight: 44
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: page.backToRecipe()

                        Row {
                            id: backRow
                            x: 0
                            spacing: 4
                            anchors.verticalCenter: parent.verticalCenter

                            Icon {
                                anchors.verticalCenter: parent.verticalCenter
                                name: "arrow-left"
                                size: 18
                                color: back.containsMouse ? Palette.primary : Palette.textMuted
                            }

                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                width: Math.min(implicitWidth, 440)
                                text: page.recipe ? page.recipe.title : "Recipe"
                                elide: Text.ElideRight
                                color: back.containsMouse ? Palette.primary : Palette.textMuted
                                font.family: Palette.fontSans
                                font.pixelSize: 15
                                font.weight: Font.DemiBold
                            }
                        }
                    }

                    Heading {
                        level: 1
                        text: "Mise en place"
                        Layout.fillWidth: true
                        Layout.topMargin: 4
                    }

                    Body {
                        textFormat: Text.StyledText
                        text: "<i>“Everything in its place.”</i> Prep and measure it all before you start cooking. Tap each one when it's ready."
                        muted: true
                        Layout.fillWidth: true
                        Layout.maximumWidth: 672
                        Layout.topMargin: 8
                    }
                }

                ScaleControl {
                    visible: page.wide
                    Layout.alignment: Qt.AlignTop
                    value: page.scale
                    onEdited: (v) => page.setScale(v)
                }
            }

            // ─── What to get out ───
            Card {
                visible: page.counts.length > 0
                Layout.fillWidth: true
                Layout.bottomMargin: 24
                implicitHeight: getOut.implicitHeight + 24

                Flow {
                    id: getOut
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.margins: 16
                    spacing: 20

                    Text {
                        text: "Get out:"
                        color: Palette.text
                        font.family: Palette.fontSans
                        font.pixelSize: 15
                        font.weight: Font.Bold
                    }

                    Repeater {
                        model: page.counts

                        delegate: Text {
                            required property var modelData
                            textFormat: Text.StyledText
                            text: "<b><font color=\"" + Palette.text + "\">" + modelData.count + "</font></b>" + modelData.label.substring(String(modelData.count).length)
                            color: Palette.textMuted
                            font.family: Palette.fontSans
                            font.pixelSize: 15
                        }
                    }

                    ScaleControl {
                        visible: !page.wide
                        value: page.scale
                        onEdited: (v) => page.setScale(v)
                    }
                }
            }

            // Where the progress bar sits before it sticks
            Item {
                id: slot
                Layout.fillWidth: true
                Layout.preferredHeight: sticky.height
                Layout.bottomMargin: 24
            }

            Rectangle {
                visible: page.loading
                Layout.fillWidth: true
                Layout.preferredHeight: 256
                radius: 12
                color: Palette.tint
            }

            Body {
                visible: !page.loading && page.error !== ""
                text: page.error
                color: Palette.error
                Layout.fillWidth: true
            }

            EmptyState {
                visible: !page.loading && page.error === "" && page.total === 0
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: 32
                iconName: "soup"
                title: "No ingredients to prep"
                actionText: "Back to recipe"
                onAction: page.backToRecipe()
            }

            // ─── The counter ───
            Rectangle {
                id: counter
                visible: !page.loading && page.total > 0
                Layout.fillWidth: true
                implicitHeight: groupColumn.implicitHeight + 2 * pad
                readonly property int pad: page.wide ? 28 : 16
                radius: 16
                color: Palette.tint
                border.width: 1
                border.color: Palette.line

                ColumnLayout {
                    id: groupColumn
                    x: counter.pad
                    y: counter.pad
                    width: parent.width - 2 * counter.pad
                    spacing: 28

                    Repeater {
                        model: page.groups

                        delegate: ColumnLayout {
                            id: group
                            required property var modelData
                            Layout.fillWidth: true
                            spacing: 14

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10

                                Rectangle {
                                    Layout.preferredWidth: 44
                                    Layout.preferredHeight: 44
                                    radius: 12
                                    color: Palette.paper

                                    Icon {
                                        anchors.centerIn: parent
                                        name: page.iconFor(group.modelData.kind)
                                        size: 22
                                        color: Palette.primary
                                    }
                                }

                                Heading {
                                    level: 2
                                    text: group.modelData.title
                                }

                                Body {
                                    visible: page.wide
                                    text: "· " + group.modelData.hint
                                    muted: true
                                    font.pixelSize: 14
                                    Layout.fillWidth: true
                                    elide: Text.ElideRight
                                    wrapMode: Text.NoWrap
                                }

                                Item {
                                    visible: !page.wide
                                    Layout.fillWidth: true
                                }
                            }

                            GridLayout {
                                Layout.fillWidth: true
                                columns: counter.width < 560 ? 2 : counter.width < 900 ? 3 : 4
                                columnSpacing: 12
                                rowSpacing: 12
                                uniformCellWidths: true

                                Repeater {
                                    model: group.modelData.items

                                    delegate: MouseArea {
                                        id: tile
                                        required property var modelData
                                        readonly property bool done: page.ready.indexOf(modelData.key) !== -1

                                        Layout.fillWidth: true
                                        Layout.fillHeight: true
                                        implicitHeight: info.implicitHeight + 28
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: page.toggle(modelData.key)

                                        Rectangle {
                                            anchors.fill: parent
                                            radius: 16
                                            color: tile.done ? Palette.tint : Palette.paper
                                            border.width: 1
                                            border.color: tile.done ? "transparent" : tile.containsMouse ? Palette.textMuted : Palette.line
                                            scale: tile.pressed ? 0.97 : 1

                                            Behavior on scale {
                                                NumberAnimation { duration: 100 }
                                            }
                                        }

                                        Rectangle {
                                            anchors.top: parent.top
                                            anchors.right: parent.right
                                            anchors.margins: 8
                                            width: 28
                                            height: 28
                                            radius: 14
                                            color: tile.done ? Palette.tile : "transparent"
                                            border.width: tile.done ? 0 : 2
                                            border.color: Palette.line
                                            scale: tile.done ? 1 : 0.75

                                            Behavior on scale {
                                                NumberAnimation { duration: 150 }
                                            }

                                            Icon {
                                                visible: tile.done
                                                anchors.centerIn: parent
                                                name: "check"
                                                size: 16
                                                color: Palette.onTile
                                            }
                                        }

                                        ColumnLayout {
                                            id: info
                                            x: 8
                                            y: 16
                                            width: parent.width - 16
                                            spacing: 0

                                            Item {
                                                Layout.alignment: Qt.AlignHCenter
                                                Layout.preferredWidth: 116
                                                Layout.preferredHeight: 112

                                                PrepBowl {
                                                    anchors.horizontalCenter: parent.horizontalCenter
                                                    anchors.bottom: parent.bottom
                                                    anchors.bottomMargin: tile.containsMouse ? 2 : 0
                                                    vessel: tile.modelData.vessel
                                                    filled: tile.done
                                                    fillColor: tile.modelData.color
                                                }
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                Layout.topMargin: 8
                                                text: tile.modelData.amount || " "
                                                color: Palette.text
                                                font.family: Palette.fontSans
                                                font.pixelSize: 18
                                                font.weight: Font.Bold
                                                horizontalAlignment: Text.AlignHCenter
                                                wrapMode: Text.WordWrap
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                Layout.topMargin: 2
                                                text: tile.modelData.name
                                                color: tile.done ? Palette.textMuted : Palette.text
                                                font.family: Palette.fontSans
                                                font.pixelSize: 16
                                                font.strikeout: tile.done
                                                horizontalAlignment: Text.AlignHCenter
                                                wrapMode: Text.WordWrap
                                                maximumLineCount: 2
                                                elide: Text.ElideRight
                                            }

                                            Rectangle {
                                                visible: !!tile.modelData.task
                                                Layout.alignment: Qt.AlignHCenter
                                                Layout.topMargin: 6
                                                implicitWidth: taskText.implicitWidth + 20
                                                implicitHeight: taskText.implicitHeight + 4
                                                radius: height / 2
                                                color: Palette.dark ? Qt.rgba(0.647, 0.353, 0.251, 0.26) : Qt.rgba(0.647, 0.353, 0.251, 0.12)

                                                Text {
                                                    id: taskText
                                                    anchors.centerIn: parent
                                                    text: tile.modelData.task || ""
                                                    color: Palette.dark ? "#e6ad95" : "#8f4b34"
                                                    font.family: Palette.fontSans
                                                    font.pixelSize: 13
                                                    font.weight: Font.Bold
                                                    font.capitalization: Font.Capitalize
                                                }
                                            }

                                            Text {
                                                visible: !tile.modelData.task && !!tile.modelData.prep
                                                Layout.fillWidth: true
                                                Layout.topMargin: 4
                                                text: tile.modelData.prep || ""
                                                color: Palette.textMuted
                                                font.family: Palette.fontSans
                                                font.pixelSize: 13
                                                horizontalAlignment: Text.AlignHCenter
                                                elide: Text.ElideRight
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                Layout.topMargin: 6
                                                text: tile.modelData.vesselLabel
                                                color: Palette.textMuted
                                                font.family: Palette.fontSans
                                                font.pixelSize: 13
                                                font.weight: Font.DemiBold
                                                font.capitalization: Font.AllUppercase
                                                font.letterSpacing: 0.5
                                                horizontalAlignment: Text.AlignHCenter
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
    }

    // ─── All ready ───
    CrumbButton {
        id: start
        kind: "primary"
        text: "Everything's in its place. Start cooking"
        iconName: "flame"
        implicitHeight: 56
        leftPadding: 24
        rightPadding: 24
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 32 + (page.allReady ? 0 : -24)
        visible: opacity > 0
        opacity: page.allReady ? 1 : 0
        onClicked: ApplicationWindow.window.go("cook", { "id": page.routeId })

        Behavior on opacity {
            NumberAnimation { duration: 300 }
        }

        Behavior on anchors.bottomMargin {
            NumberAnimation { duration: 300; easing.type: Easing.OutCubic }
        }
    }
}
