import QtQuick
import QtQuick.Controls

import app.crumb.desktop 1.0

// The kitchen timers dock (web TimerDock.svelte + lib/timers.svelte.ts). Timers live in
// `Store` under the web's key and shape, so they persist and every page shows them. Tap one
// to dismiss it. A finished timer shows "Done!" with a pulsing ring and a toast, and plays the
// web's chime (`Sound`, src/chime.rs).
Item {
    id: dock

    readonly property string key: "crumb:timers"
    readonly property bool cooking: {
        var w = ApplicationWindow.window
        return !!w && !!w.route && w.route.name === "cook"
    }
    property var timers: []
    property real now: Date.now()

    implicitHeight: timers.length ? row.height + (cooking ? 76 : 0) : 0

    function load() {
        try {
            var list = JSON.parse(Store.read(key, "[]"))
            timers = Array.isArray(list) ? list : []
        } catch (e) {
            timers = []
        }
    }

    function save(list) {
        timers = list
        Store.write(key, JSON.stringify(list))
    }

    function remove(id) {
        save(timers.filter(function (t) { return t.id !== id }))
    }

    function tick() {
        now = Date.now()
        var list = timers.slice()
        var changed = false
        for (var i = 0; i < list.length; i++) {
            var t = list[i]
            if (!t.done && t.endsAt <= now) {
                list[i] = { "id": t.id, "label": t.label, "total": t.total, "endsAt": t.endsAt, "done": true }
                changed = true
                var w = ApplicationWindow.window
                if (w)
                    w.toast({ "title": "⏰ " + t.label + " is up!", "tone": "primary", "duration": 15000 })
                Sound.chime()
            }
        }
        if (changed)
            save(list)
    }

    Component.onCompleted: load()

    Connections {
        target: Store

        // Another page started or dismissed a timer
        function onChanged(changedKey) {
            if (changedKey === dock.key)
                dock.load()
        }
    }

    Timer {
        interval: 500
        repeat: true
        running: true
        onTriggered: dock.tick()
    }

    Row {
        id: row
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: dock.cooking ? 76 : 0
        spacing: 8

        Repeater {
            model: dock.timers

            delegate: Item {
                id: pill
                required property var modelData
                readonly property bool done: !!modelData.done

                width: content.width
                height: 44

                Rectangle {
                    visible: pill.done
                    anchors.fill: parent
                    radius: 22
                    color: Palette.tile
                    transformOrigin: Item.Center

                    SequentialAnimation on scale {
                        running: pill.done
                        loops: Animation.Infinite
                        NumberAnimation { from: 1; to: 1.25; duration: 900; easing.type: Easing.OutQuad }
                        PauseAnimation { duration: 300 }
                    }
                    SequentialAnimation on opacity {
                        running: pill.done
                        loops: Animation.Infinite
                        NumberAnimation { from: 0.4; to: 0; duration: 900; easing.type: Easing.OutQuad }
                        PauseAnimation { duration: 300 }
                    }
                }

                Rectangle {
                    anchors.fill: parent
                    radius: 22
                    color: pill.done ? Palette.tile : Palette.paper
                    border.width: pill.done ? 0 : 1
                    border.color: Palette.line
                }

                Row {
                    id: content
                    height: parent.height
                    leftPadding: 14
                    rightPadding: 12
                    spacing: 8

                    Icon {
                        anchors.verticalCenter: parent.verticalCenter
                        name: "alarm-clock"
                        size: 18
                        color: pill.done ? Palette.onTile : Palette.primary
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: pill.done ? "Done!" : Core.formatClock(Math.max(0, (pill.modelData.endsAt - dock.now) / 1000))
                        color: pill.done ? Palette.onTile : Palette.text
                        font.family: Palette.fontSans
                        font.pixelSize: 15
                        font.weight: Font.Bold
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        width: Math.min(implicitWidth, 128)
                        text: pill.modelData.label
                        elide: Text.ElideRight
                        color: pill.done ? Palette.onTile : Palette.textMuted
                        font.family: Palette.fontSans
                        font.pixelSize: 13
                        font.weight: Font.DemiBold
                    }

                    Icon {
                        anchors.verticalCenter: parent.verticalCenter
                        name: "x"
                        size: 16
                        color: pill.done ? Palette.onTile : Palette.textMuted
                    }
                }

                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: dock.remove(pill.modelData.id)
                }
            }
        }
    }
}
