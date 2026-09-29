import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The web's toaster (lib/toast.ts): paper cards at the top centre, a coloured bar for a
// tone, an optional action (Undo), gone after 4 s (8 s for errors and actions) or a click.
Item {
    id: toaster

    // show({title, description, tone: default|success|error|primary, duration, action: {label, onselect}})
    function show(t) {
        toasts.append({
            "title": t.title || "",
            "description": t.description || "",
            "tone": t.tone || "default",
            "actionLabel": t.action ? t.action.label : "",
            "duration": t.duration || (t.tone === "error" || t.action ? 8000 : 4000),
            "key": ++serial
        })
        if (t.action)
            actions[serial] = t.action.onselect
    }

    property int serial: 0
    property var actions: ({})

    function dismiss(key) {
        for (var i = 0; i < toasts.count; i++) {
            if (toasts.get(i).key === key) {
                toasts.remove(i)
                break
            }
        }
        delete actions[key]
    }

    ListModel {
        id: toasts
    }

    Column {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        anchors.topMargin: 12
        spacing: 8
        width: Math.min(420, toaster.width - 32)

        Repeater {
            model: toasts

            delegate: Card {
                id: card
                required property string title
                required property string description
                required property string tone
                required property string actionLabel
                required property int duration
                required property int key

                width: parent.width
                height: row.implicitHeight + 24

                Timer {
                    running: true
                    interval: card.duration
                    onTriggered: toaster.dismiss(card.key)
                }

                TapHandler {
                    onTapped: toaster.dismiss(card.key)
                }

                Rectangle {
                    visible: card.tone !== "default"
                    x: 8
                    y: 12
                    width: 4
                    height: parent.height - 24
                    radius: 2
                    color: card.tone === "error" ? Palette.error
                           : card.tone === "primary" ? Palette.butter : Palette.tile
                }

                RowLayout {
                    id: row
                    x: card.tone !== "default" ? 20 : 16
                    y: 12
                    width: parent.width - x - 12
                    spacing: 12

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2

                        Body {
                            text: card.title
                            bold: true
                            Layout.fillWidth: true
                        }

                        Body {
                            visible: card.description !== ""
                            text: card.description
                            muted: true
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }
                    }

                    CrumbButton {
                        visible: card.actionLabel !== ""
                        kind: "ghost"
                        small: true
                        text: card.actionLabel
                        onClicked: {
                            var run = toaster.actions[card.key]
                            toaster.dismiss(card.key)
                            if (run)
                                run()
                        }
                    }
                }
            }
        }
    }
}
