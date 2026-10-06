import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// A cook mode step's timer (web components/StepTimer.svelte): suggested from the times the
// step mentions, never trusted. The cook nudges or types the length before starting it, and
// any step can have one. `choices` is the step's `[{label, seconds}]` from `Core.cookSteps`
// (the first is suggested); `seconds` is the length the cook chose, -1 until they change it.
// `adjusted(seconds)` reports a new length and `started(seconds)` the Start tap.
ColumnLayout {
    id: timer

    property var choices: []
    property int seconds: -1
    // True while the minutes field is open, so the page can leave the arrow keys to it
    property bool typing: false
    readonly property int length: seconds >= 0 ? seconds : choices.length ? choices[0].seconds : -1

    signal adjusted(int seconds)
    signal started(int seconds)

    function commit() {
        if (!typing)
            return
        typing = false
        var chosen = Core.timerFromMinutes(Number(minutes.text.replace(",", ".")))
        if (chosen >= 0)
            adjusted(chosen)
    }

    // Tapping the length types it, in minutes
    function startTyping() {
        typing = true
        minutes.text = String(Math.round(length / 60 * 100) / 100)
        minutes.forceActiveFocus()
        minutes.selectAll()
    }

    function cancel() {
        typing = false
    }

    spacing: 12

    CrumbButton {
        visible: timer.length < 0
        text: "Set a timer"
        iconName: "alarm-clock-plus"
        implicitHeight: 52
        onClicked: timer.adjusted(Core.timerDefault())
    }

    Flow {
        visible: timer.length >= 0
        Layout.fillWidth: true
        spacing: 10

        Rectangle {
            width: group.implicitWidth + 8
            height: 52
            radius: 16
            color: Palette.tint
            Accessible.role: Accessible.Grouping
            Accessible.name: "Timer length"

            Row {
                id: group
                anchors.centerIn: parent
                spacing: 2

                CrumbButton {
                    kind: "ghost"
                    iconName: "minus"
                    iconSize: 22
                    width: 44
                    height: 44
                    enabled: timer.length > Core.timerMin()
                    Accessible.name: "Shorter"
                    onClicked: timer.adjusted(Core.nudgeTimer(timer.length, false))
                }

                Item {
                    width: Math.max(80, timer.typing ? typingRow.implicitWidth + 16 : words.implicitWidth + 16)
                    height: 44

                    MouseArea {
                        visible: !timer.typing
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        Accessible.role: Accessible.Button
                        Accessible.name: words.text + ". Tap to type a length"
                        Accessible.onPressAction: clicked(null)
                        onClicked: timer.startTyping()

                        Rectangle {
                            anchors.fill: parent
                            radius: 12
                            color: Palette.paper
                            opacity: parent.containsMouse ? 0.7 : 0
                        }

                        Text {
                            id: words
                            anchors.centerIn: parent
                            text: timer.length >= 0 ? Core.timerWords(timer.length) : ""
                            color: Palette.text
                            font.family: Palette.fontSans
                            font.pixelSize: 18
                            font.weight: Font.Bold
                        }
                    }

                    Row {
                        id: typingRow
                        visible: timer.typing
                        anchors.centerIn: parent
                        spacing: 6

                        TextField {
                            id: minutes
                            width: 80
                            height: 40
                            horizontalAlignment: TextInput.AlignHCenter
                            color: Palette.text
                            selectionColor: Palette.tile
                            selectedTextColor: Palette.onTile
                            font.family: Palette.fontSans
                            font.pixelSize: 18
                            font.weight: Font.Bold
                            inputMethodHints: Qt.ImhFormattedNumbersOnly
                            validator: RegularExpressionValidator { regularExpression: /[0-9]*[.,]?[0-9]*/ }
                            Accessible.name: "Minutes"
                            onAccepted: timer.commit()
                            onActiveFocusChanged: {
                                if (!activeFocus)
                                    timer.commit()
                            }
                            Keys.onEscapePressed: timer.cancel()

                            background: Rectangle {
                                radius: 12
                                color: Palette.paper
                                border.width: minutes.activeFocus ? 2 : 1
                                border.color: minutes.activeFocus ? Palette.primary : Palette.line
                            }
                        }

                        Text {
                            anchors.verticalCenter: minutes.verticalCenter
                            text: "min"
                            color: Palette.text
                            font.family: Palette.fontSans
                            font.pixelSize: 18
                            font.weight: Font.Bold
                        }
                    }
                }

                CrumbButton {
                    kind: "ghost"
                    iconName: "plus"
                    iconSize: 22
                    width: 44
                    height: 44
                    enabled: timer.length < Core.timerMax()
                    Accessible.name: "Longer"
                    onClicked: timer.adjusted(Core.nudgeTimer(timer.length, true))
                }
            }
        }

        CrumbButton {
            id: start
            text: "Start timer"
            implicitHeight: 52
            onClicked: {
                timer.commit()
                timer.started(timer.length)
            }

            background: Rectangle {
                radius: 12
                color: Palette.tile
                opacity: start.down ? 0.85 : 1
                border.width: start.visualFocus ? 2 : 0
                border.color: Palette.primary
            }
            contentItem: RowLayout {
                spacing: 8

                Icon {
                    name: "alarm-clock"
                    size: 20
                    color: Palette.onTile
                }

                Text {
                    text: start.text
                    color: Palette.onTile
                    font.family: Palette.fontSans
                    font.pixelSize: 16
                    font.weight: Font.Bold
                }
            }
        }
    }

    Flow {
        visible: timer.length >= 0 && timer.choices.length > 1
        Layout.fillWidth: true
        spacing: 8

        Text {
            height: 32
            verticalAlignment: Text.AlignVCenter
            text: "The step mentions"
            color: Palette.textMuted
            font.family: Palette.fontSans
            font.pixelSize: 13
            font.weight: Font.DemiBold
        }

        Repeater {
            model: timer.choices

            delegate: Rectangle {
                id: chip
                required property var modelData
                readonly property bool on: timer.length === modelData.seconds

                width: chipLabel.implicitWidth + 24
                height: 32
                radius: 16
                color: on ? Palette.tile : Palette.tint
                Accessible.role: Accessible.Button
                Accessible.name: modelData.label
                Accessible.checked: on
                Accessible.onPressAction: tap.tapped(null, Qt.LeftButton)

                Text {
                    id: chipLabel
                    anchors.centerIn: parent
                    text: chip.modelData.label
                    color: chip.on ? Palette.onTile : Palette.primary
                    font.family: Palette.fontSans
                    font.pixelSize: 14
                    font.weight: Font.DemiBold
                }

                HoverHandler {
                    cursorShape: Qt.PointingHandCursor
                }

                TapHandler {
                    id: tap
                    onTapped: timer.adjusted(chip.modelData.seconds)
                }
            }
        }
    }
}
