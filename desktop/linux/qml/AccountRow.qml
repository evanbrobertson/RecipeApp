import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The web's `.list-row.settings-row`: a muted icon, a bold title with a small line under it,
// and buttons on the right (children). `clickable` makes the whole row a button. `confirming`
// swaps the buttons for a "Keep / danger" pair under the text, as AccountPage.svelte does;
// `confirmInline` puts that text in place of the title instead (Leave household).
Item {
    id: row

    property string iconName
    property string title
    property string subtitle
    property color titleColor: Palette.text
    property bool clickable: false
    property bool busy: false
    property bool confirming: false
    property bool confirmInline: false
    property string confirmText
    property string keepLabel: "Keep"
    property string dangerLabel
    default property alias trailing: trailingRow.data

    signal clicked()
    signal kept()
    signal confirmed()

    Layout.fillWidth: true
    implicitHeight: Math.max(56, content.implicitHeight + 20)
    opacity: clickable && !enabled ? 0.6 : 1

    Rectangle {
        anchors.left: parent.left
        anchors.leftMargin: 50
        anchors.right: parent.right
        height: 1
        color: Palette.line
        visible: row.y > 0
    }

    Rectangle {
        anchors.fill: parent
        color: Palette.tint
        opacity: row.clickable && hover.hovered && row.enabled ? 0.55 : 0
    }

    HoverHandler {
        id: hover
        cursorShape: row.clickable ? Qt.PointingHandCursor : Qt.ArrowCursor
    }

    TapHandler {
        enabled: row.clickable && row.enabled
        onTapped: row.clicked()
    }

    ColumnLayout {
        id: content
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        anchors.leftMargin: 16
        anchors.rightMargin: 16
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            spacing: 14

            Icon {
                name: row.iconName
                size: 20
                color: Palette.textMuted
                Layout.alignment: Qt.AlignVCenter
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter
                spacing: 0

                Text {
                    visible: !(row.confirming && row.confirmInline) && row.title !== ""
                    text: row.title
                    color: row.titleColor
                    font.family: Palette.fontSans
                    font.pixelSize: 15
                    font.weight: Font.Bold
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }

                Text {
                    visible: !(row.confirming && row.confirmInline) && row.subtitle !== ""
                    text: row.subtitle
                    color: Palette.textMuted
                    font.family: Palette.fontSans
                    font.pixelSize: 14
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }

                Text {
                    visible: row.confirming && row.confirmInline
                    text: row.confirmText
                    color: Palette.text
                    font.family: Palette.fontSans
                    font.pixelSize: 14
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
            }

            RowLayout {
                id: trailingRow
                visible: !row.confirming || !row.confirmInline
                spacing: 8
                Layout.alignment: Qt.AlignVCenter
            }

            RowLayout {
                visible: row.confirming && row.confirmInline
                spacing: 8
                Layout.alignment: Qt.AlignVCenter

                CrumbButton {
                    kind: "ghost"
                    text: row.keepLabel
                    onClicked: row.kept()
                }

                CrumbButton {
                    kind: "danger"
                    text: row.dangerLabel
                    enabled: !row.busy
                    onClicked: row.confirmed()
                }
            }
        }

        RowLayout {
            visible: row.confirming && !row.confirmInline
            spacing: 8
            Layout.fillWidth: true
            Layout.leftMargin: 34

            Text {
                text: row.confirmText
                color: Palette.textMuted
                font.family: Palette.fontSans
                font.pixelSize: 14
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }

            CrumbButton {
                kind: "ghost"
                text: row.keepLabel
                onClicked: row.kept()
            }

            CrumbButton {
                kind: "danger"
                text: row.dangerLabel
                enabled: !row.busy
                onClicked: row.confirmed()
            }
        }
    }
}
