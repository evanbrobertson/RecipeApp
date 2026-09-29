import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The web's buttons. `kind`: "primary" (butter, the one main action), "soft" (tint),
// "outline" (paper with a border), "ghost" (text only) or "danger".
Button {
    id: button

    property string kind: "soft"
    property string iconName: ""
    property int iconSize: 18
    property bool small: false

    readonly property color fill: {
        if (!enabled)
            return Palette.tint
        switch (kind) {
        case "primary":
            return Palette.butter
        case "danger":
            return Palette.error
        case "outline":
            return Palette.paper
        case "ghost":
            return "transparent"
        default:
            return Palette.tint
        }
    }
    readonly property color ink: {
        if (!enabled)
            return Palette.textMuted
        switch (kind) {
        case "primary":
            return Palette.onButter
        case "danger":
            return "#ffffff"
        case "ghost":
            return Palette.primary
        default:
            return Palette.text
        }
    }

    implicitHeight: small ? 36 : 44
    leftPadding: text ? (small ? 12 : 16) : 10
    rightPadding: text ? (small ? 12 : 16) : 10
    hoverEnabled: true
    focusPolicy: Qt.StrongFocus

    HoverHandler {
        cursorShape: Qt.PointingHandCursor
    }

    background: Rectangle {
        radius: 12
        color: button.fill
        opacity: button.down ? 0.85 : 1
        border.width: button.kind === "outline" || button.visualFocus ? (button.visualFocus ? 2 : 1) : 0
        border.color: button.visualFocus ? Palette.primary : Palette.line

        Rectangle {
            anchors.fill: parent
            radius: parent.radius
            color: Palette.text
            opacity: button.hovered && button.enabled ? 0.06 : 0
        }
    }

    contentItem: RowLayout {
        spacing: 8

        Icon {
            visible: button.iconName !== ""
            name: button.iconName
            size: button.iconSize
            color: button.ink
            Layout.alignment: Qt.AlignVCenter
        }

        Text {
            visible: button.text !== ""
            text: button.text
            color: button.ink
            font.family: Palette.fontSans
            font.pixelSize: button.small ? 14 : 15
            font.weight: Font.Bold
            elide: Text.ElideRight
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
            horizontalAlignment: Text.AlignHCenter
        }
    }
}
