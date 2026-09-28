import QtQuick
import QtQuick.Controls

import app.crumb.desktop 1.0

// The editor's category select: a control-styled combo with a chevron (web `select.input`).
// `options` is [{label, value}]; `value` picks the current one.
ComboBox {
    id: select

    property var options: []
    property string value: ""

    model: options
    textRole: "label"
    valueRole: "value"
    currentIndex: {
        for (var i = 0; i < options.length; i++) {
            if (options[i].value === value)
                return i
        }
        return 0
    }
    signal picked(string value)
    onActivated: index => picked(options[index].value)

    implicitHeight: 44
    hoverEnabled: true
    font.family: Palette.fontSans
    font.pixelSize: 15

    background: Rectangle {
        radius: 12
        color: Palette.paper
        border.width: select.activeFocus ? 2 : 1
        border.color: select.activeFocus ? Palette.primary : Palette.line
    }

    contentItem: Text {
        leftPadding: 14
        rightPadding: 40
        text: select.displayText
        font: select.font
        color: Palette.text
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }

    indicator: Icon {
        x: select.width - width - 12
        y: (select.height - height) / 2
        name: "chevron-down"
        size: 16
        color: Palette.textMuted
    }

    delegate: ItemDelegate {
        required property var modelData
        required property int index
        width: select.width
        highlighted: select.highlightedIndex === index
        contentItem: Text {
            text: modelData.label
            color: Palette.text
            font: select.font
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
        }
        background: Rectangle {
            color: parent.highlighted ? Palette.tint : "transparent"
        }
    }

    popup: Popup {
        y: select.height + 4
        width: select.width
        implicitHeight: Math.min(contentItem.implicitHeight + 2, 320)
        padding: 1

        contentItem: ListView {
            clip: true
            implicitHeight: contentHeight
            model: select.popup.visible ? select.delegateModel : null
            currentIndex: select.highlightedIndex
            ScrollIndicator.vertical: ScrollIndicator {}
        }

        background: Rectangle {
            radius: 12
            color: Palette.paper
            border.width: 1
            border.color: Palette.line
        }
    }
}
