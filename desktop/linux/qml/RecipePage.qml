import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Effects

import app.crumb.desktop 1.0

// A recipe's detail page. Content, order and wording follow the web's
// web/src/islands/RecipePage.svelte; all parsing/scaling/formatting happens in Rust.
Item {
    id: page

    property var session
    property int recipeId: 0
    signal goBack()

    readonly property int gutter: width >= 1000 ? 32 : 20
    readonly property bool wide: width >= 1000
    readonly property real contentWidth: Math.max(0, width - 2 * gutter)
    readonly property real heroHeight: Math.min(420, Math.round(contentWidth * 9 / 16))

    // Struck-through ingredient lines are in-memory only, keyed by section/item position.
    property var checkedKeys: []
    property var ingredientSections: page.parseJson(view.ingredientsJson, [])
    property var instructionSections: page.parseJson(view.instructionsJson, [])
    property var metaItems: page.parseJson(view.meta, [])
    property var nutrition: page.parseJson(view.nutritionJson, ({}))
    property var nutritionEntries: page.entries(nutrition)

    readonly property var nutritionLabels: ({
            "calories": "Calories",
            "fatContent": "Fat",
            "saturatedFatContent": "Saturated fat",
            "unsaturatedFatContent": "Unsaturated fat",
            "transFatContent": "Trans fat",
            "carbohydrateContent": "Carbs",
            "sugarContent": "Sugar",
            "fiberContent": "Fiber",
            "proteinContent": "Protein",
            "cholesterolContent": "Cholesterol",
            "sodiumContent": "Sodium",
            "servingSize": "Serving size"
        })

    function parseJson(text, fallback) {
        if (!text || text.length === 0)
            return fallback
        try {
            return JSON.parse(text)
        } catch (e) {
            return fallback
        }
    }

    function entries(nutrition) {
        var out = []
        var keys = Object.keys(nutrition)
        for (var i = 0; i < keys.length; i++) {
            var name = keys[i]
            out.push({
                "label": page.nutritionLabels[name] !== undefined ? page.nutritionLabels[name] : name.replace(/Content$/, ""),
                "value": nutrition[name]
            })
        }
        return out
    }

    function scaleLabel(value) {
        if (value === 0.5)
            return "½"
        var whole = Math.floor(value)
        if (value - whole === 0.5)
            return (whole > 0 ? whole : "") + "½"
        return value.toString()
    }

    function stepOffset(sectionIndex) {
        var total = 0
        for (var i = 0; i < sectionIndex && i < page.instructionSections.length; i++)
            total += page.instructionSections[i].items.length
        return total
    }

    function ingredientKey(sectionIndex, itemIndex) {
        return sectionIndex + "-" + itemIndex
    }

    function isChecked(sectionIndex, itemIndex) {
        return page.checkedKeys.indexOf(page.ingredientKey(sectionIndex, itemIndex)) !== -1
    }

    function toggleIngredient(sectionIndex, itemIndex) {
        var key = page.ingredientKey(sectionIndex, itemIndex)
        var next = page.checkedKeys.slice()
        var at = next.indexOf(key)
        if (at === -1)
            next.push(key)
        else
            next.splice(at, 1)
        page.checkedKeys = next
    }

    RecipeView {
        id: view

        onUnauthorized: if (page.session)
            page.session.requireLogin()
        onToastChanged: if (view.toast !== "")
            toastTimer.restart()
    }

    Timer {
        id: toastTimer
        interval: 3000
        onTriggered: view.toast = ""
    }

    Component.onCompleted: view.load(page.recipeId)

    // The `--smoke-page recipe` assertion: the fixture must have loaded and populated the page.
    Timer {
        running: Smoke.enabled && Smoke.page === "recipe"
        interval: 100
        onTriggered: {
            if (view.title.length === 0)
                Smoke.fail("the recipe fixture did not load")
        }
    }

    // ─── Loading ───
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: page.gutter
        spacing: 16
        visible: view.loading

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: page.heroHeight
            radius: 12
            color: Palette.tint
        }
        Rectangle {
            Layout.preferredWidth: Math.min(420, page.contentWidth)
            Layout.preferredHeight: 36
            radius: 8
            color: Palette.tint
        }
        Rectangle {
            Layout.preferredWidth: Math.min(300, page.contentWidth)
            Layout.preferredHeight: 18
            radius: 8
            color: Palette.tint
        }
    }

    // ─── Error ───
    ColumnLayout {
        anchors.centerIn: parent
        width: Math.min(420, page.width - 80)
        spacing: 14
        visible: !view.loading && view.error !== ""

        Text {
            Layout.fillWidth: true
            text: "Couldn't open this recipe"
            color: Palette.text
            font.family: Palette.fontSerif
            font.pixelSize: 24
            horizontalAlignment: Text.AlignHCenter
        }

        Text {
            Layout.fillWidth: true
            text: view.error
            color: Palette.error
            font.pixelSize: 14
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
        }

        Button {
            id: retryButton
            Layout.alignment: Qt.AlignHCenter
            implicitHeight: 44
            leftPadding: 22
            rightPadding: 22
            onClicked: view.retry()

            background: Rectangle {
                radius: 12
                color: Palette.butter
            }

            contentItem: Text {
                text: "Retry"
                color: Palette.onButter
                font.pixelSize: 15
                font.weight: Font.DemiBold
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
            }
        }
    }

    // ─── The recipe ───
    ScrollView {
        id: scroller
        anchors.fill: parent
        clip: true
        contentWidth: availableWidth
        visible: !view.loading && view.error === "" && view.title !== ""

        ColumnLayout {
            id: contentColumn
            width: scroller.availableWidth
            spacing: 0

            // Full-width header.
            ColumnLayout {
                Layout.fillWidth: true
                Layout.margins: page.gutter
                spacing: 14

                Button {
                    id: backButton
                    flat: true
                    implicitHeight: 32
                    leftPadding: 0
                    rightPadding: 0
                    onClicked: page.goBack()

                    background: Item {}

                    contentItem: Text {
                        text: "‹  Back"
                        color: Palette.textMuted
                        font.pixelSize: 14
                        font.weight: Font.DemiBold
                        verticalAlignment: Text.AlignVCenter
                    }
                }

                Rectangle {
                    id: hero
                    Layout.fillWidth: true
                    Layout.preferredHeight: page.heroHeight
                    radius: 12
                    color: Palette.tint
                    visible: view.heroUrl !== ""

                    Image {
                        id: heroImage
                        anchors.fill: parent
                        source: view.heroUrl
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                        sourceSize.width: 1280
                        layer.enabled: hero.visible
                        layer.effect: MultiEffect {
                            maskEnabled: true
                            maskSource: heroMask
                        }
                    }

                    Rectangle {
                        id: heroMask
                        anchors.fill: parent
                        radius: 12
                        color: "black"
                        visible: false
                        layer.enabled: true
                    }
                }

                Text {
                    Layout.fillWidth: true
                    visible: view.kicker !== ""
                    text: view.kicker.toUpperCase()
                    color: Palette.textMuted
                    font.pixelSize: 13
                    font.weight: Font.DemiBold
                    font.letterSpacing: 1.4
                    wrapMode: Text.WordWrap
                }

                Text {
                    Layout.fillWidth: true
                    text: view.title
                    color: Palette.text
                    font.family: Palette.fontSerif
                    font.pixelSize: 36
                    wrapMode: Text.WordWrap
                }

                Text {
                    Layout.fillWidth: true
                    visible: view.description !== ""
                    text: view.description
                    color: Palette.textMuted
                    font.pixelSize: 17
                    lineHeight: 1.35
                    wrapMode: Text.WordWrap
                }

                Flow {
                    Layout.fillWidth: true
                    spacing: 8
                    visible: page.metaItems.length > 0

                    Repeater {
                        model: page.metaItems

                        delegate: Rectangle {
                            required property var modelData

                            height: 34
                            width: metaRow.implicitWidth + 24
                            radius: 999
                            color: Palette.tint

                            RowLayout {
                                id: metaRow
                                anchors.centerIn: parent
                                spacing: 6

                                Text {
                                    text: modelData.label
                                    color: Palette.textMuted
                                    font.pixelSize: 13
                                }

                                Text {
                                    text: modelData.value
                                    color: Palette.text
                                    font.pixelSize: 14
                                    font.weight: Font.DemiBold
                                }
                            }
                        }
                    }
                }

                Flow {
                    Layout.fillWidth: true
                    Layout.topMargin: 2
                    spacing: 10

                    Button {
                        id: cookedButton
                        implicitHeight: 44
                        leftPadding: 20
                        rightPadding: 20
                        onClicked: view.markCooked()

                        background: Rectangle {
                            radius: 12
                            color: cookedButton.enabled ? Palette.butter : Palette.tint

                            Behavior on color {
                                ColorAnimation { duration: 150 }
                            }
                        }

                        contentItem: Text {
                            text: "Cooked it"
                            color: cookedButton.enabled ? Palette.onButter : Palette.textMuted
                            font.pixelSize: 15
                            font.weight: Font.DemiBold
                            horizontalAlignment: Text.AlignHCenter
                            verticalAlignment: Text.AlignVCenter
                        }
                    }

                    Button {
                        id: copyButton
                        implicitHeight: 44
                        leftPadding: 18
                        rightPadding: 18
                        onClicked: view.copyText()

                        background: Rectangle {
                            radius: 12
                            color: Palette.paper
                            border.width: 1
                            border.color: Palette.line
                        }

                        contentItem: Text {
                            text: "Copy"
                            color: Palette.text
                            font.pixelSize: 14
                            font.weight: Font.DemiBold
                            horizontalAlignment: Text.AlignHCenter
                            verticalAlignment: Text.AlignVCenter
                        }
                    }

                    Button {
                        id: sourceButton
                        implicitHeight: 44
                        leftPadding: 18
                        rightPadding: 18
                        visible: view.sourceUrl !== ""
                        onClicked: view.openSource()

                        background: Rectangle {
                            radius: 12
                            color: Palette.paper
                            border.width: 1
                            border.color: Palette.line
                        }

                        contentItem: Text {
                            text: "Source · " + view.sourceHost
                            color: Palette.text
                            font.pixelSize: 14
                            font.weight: Font.DemiBold
                            horizontalAlignment: Text.AlignHCenter
                            verticalAlignment: Text.AlignVCenter
                        }
                    }

                    // "Serves ×1" with −/+ and a preset menu.
                    Rectangle {
                        implicitHeight: 44
                        implicitWidth: scaleRow.implicitWidth + 18
                        radius: 12
                        color: Palette.tint

                        RowLayout {
                            id: scaleRow
                            anchors.centerIn: parent
                            spacing: 4

                            Text {
                                text: "Serves"
                                color: Palette.textMuted
                                font.pixelSize: 13
                            }

                            Text {
                                text: "×" + page.scaleLabel(view.scale)
                                color: Palette.text
                                font.pixelSize: 15
                                font.weight: Font.DemiBold
                            }

                            Button {
                                id: minusButton
                                implicitWidth: 30
                                implicitHeight: 30
                                leftPadding: 0
                                rightPadding: 0
                                onClicked: view.setScaleValue(view.scale - 0.5)

                                background: Rectangle {
                                    radius: 12
                                    color: Palette.paper
                                    border.width: 1
                                    border.color: Palette.line
                                }

                                contentItem: Text {
                                    text: "−"
                                    color: Palette.text
                                    font.pixelSize: 16
                                    font.weight: Font.Bold
                                    horizontalAlignment: Text.AlignHCenter
                                    verticalAlignment: Text.AlignVCenter
                                }
                            }

                            Button {
                                id: plusButton
                                implicitWidth: 30
                                implicitHeight: 30
                                leftPadding: 0
                                rightPadding: 0
                                onClicked: view.setScaleValue(view.scale + 0.5)

                                background: Rectangle {
                                    radius: 12
                                    color: Palette.paper
                                    border.width: 1
                                    border.color: Palette.line
                                }

                                contentItem: Text {
                                    text: "+"
                                    color: Palette.text
                                    font.pixelSize: 16
                                    font.weight: Font.Bold
                                    horizontalAlignment: Text.AlignHCenter
                                    verticalAlignment: Text.AlignVCenter
                                }
                            }

                            Button {
                                id: presetButton
                                implicitWidth: 30
                                implicitHeight: 30
                                leftPadding: 0
                                rightPadding: 0
                                onClicked: presetMenu.popup()

                                background: Rectangle {
                                    radius: 12
                                    color: Palette.paper
                                    border.width: 1
                                    border.color: Palette.line
                                }

                                contentItem: Text {
                                    text: "▾"
                                    color: Palette.text
                                    font.pixelSize: 13
                                    horizontalAlignment: Text.AlignHCenter
                                    verticalAlignment: Text.AlignVCenter
                                }
                            }
                        }

                        Menu {
                            id: presetMenu

                            Repeater {
                                model: [0.5, 1, 1.5, 2, 3, 4]

                                MenuItem {
                                    required property var modelData
                                    text: page.scaleLabel(modelData) + "×"
                                    font.pixelSize: 14
                                    onTriggered: view.setScaleValue(modelData)
                                }
                            }
                        }
                    }
                }
            }

            // Ingredients left, method right on wide windows; stacked otherwise.
            GridLayout {
                Layout.fillWidth: true
                Layout.leftMargin: page.gutter
                Layout.rightMargin: page.gutter
                Layout.topMargin: 18
                columns: page.wide ? 2 : 1
                columnSpacing: page.wide ? 32 : 0
                rowSpacing: 28

                ColumnLayout {
                    id: ingredientsPanel
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignTop
                    Layout.preferredWidth: page.wide ? 360 : -1
                    spacing: 14

                    Text {
                        text: "Ingredients"
                        color: Palette.text
                        font.family: Palette.fontSerif
                        font.pixelSize: 22
                    }

                    Text {
                        Layout.fillWidth: true
                        visible: page.ingredientSections.length === 0
                        text: "No ingredients listed."
                        color: Palette.textMuted
                        font.pixelSize: 14
                    }

                    Repeater {
                        model: page.ingredientSections

                        delegate: ColumnLayout {
                            id: sectionBlock
                            required property var modelData
                            required property int index

                            Layout.fillWidth: true
                            spacing: 8

                            Text {
                                Layout.fillWidth: true
                                visible: sectionBlock.modelData.name !== null && sectionBlock.modelData.name !== ""
                                text: sectionBlock.modelData.name !== null ? sectionBlock.modelData.name : ""
                                color: Palette.text
                                font.family: Palette.fontSerif
                                font.pixelSize: 18
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                implicitHeight: itemsColumn.implicitHeight
                                radius: 16
                                color: Palette.paper

                                ColumnLayout {
                                    id: itemsColumn
                                    anchors.left: parent.left
                                    anchors.right: parent.right
                                    anchors.top: parent.top
                                    spacing: 0

                                    Repeater {
                                        model: sectionBlock.modelData.items

                                        delegate: Rectangle {
                                            id: ingredientRow
                                            required property string modelData
                                            required property int index

                                            Layout.fillWidth: true
                                            implicitHeight: ingredientLine.implicitHeight + 24
                                            color: "transparent"

                                            readonly property bool struck: page.isChecked(sectionBlock.index, ingredientRow.index)

                                            Rectangle {
                                                anchors.top: parent.top
                                                anchors.left: parent.left
                                                anchors.right: parent.right
                                                anchors.leftMargin: 12
                                                anchors.rightMargin: 12
                                                height: 1
                                                color: Palette.line
                                                visible: ingredientRow.index > 0
                                            }

                                            RowLayout {
                                                id: ingredientLine
                                                anchors.left: parent.left
                                                anchors.right: parent.right
                                                anchors.top: parent.top
                                                anchors.margins: 12
                                                spacing: 12

                                                Rectangle {
                                                    Layout.alignment: Qt.AlignTop
                                                    Layout.preferredWidth: 22
                                                    Layout.preferredHeight: 22
                                                    radius: 11
                                                    border.width: 2
                                                    border.color: ingredientRow.struck ? Palette.tile : Palette.line
                                                    color: ingredientRow.struck ? Palette.tile : "transparent"

                                                    Text {
                                                        anchors.centerIn: parent
                                                        visible: ingredientRow.struck
                                                        text: "✓"
                                                        color: Palette.onTile
                                                        font.pixelSize: 13
                                                        font.weight: Font.Bold
                                                    }
                                                }

                                                Text {
                                                    Layout.fillWidth: true
                                                    text: ingredientRow.modelData
                                                    color: ingredientRow.struck ? Palette.textMuted : Palette.text
                                                    font.pixelSize: 15
                                                    font.strikeout: ingredientRow.struck
                                                    wrapMode: Text.WordWrap
                                                }
                                            }

                                            MouseArea {
                                                anchors.fill: parent
                                                cursorShape: Qt.PointingHandCursor
                                                onClicked: page.toggleIngredient(sectionBlock.index, ingredientRow.index)
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                ColumnLayout {
                    id: methodPanel
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignTop
                    spacing: 16

                    Text {
                        text: "Method"
                        color: Palette.text
                        font.family: Palette.fontSerif
                        font.pixelSize: 22
                    }

                    Text {
                        Layout.fillWidth: true
                        visible: page.instructionSections.length === 0
                        text: "No steps listed."
                        color: Palette.textMuted
                        font.pixelSize: 14
                    }

                    Repeater {
                        model: page.instructionSections

                        delegate: ColumnLayout {
                            id: stepBlock
                            required property var modelData
                            required property int index

                            Layout.fillWidth: true
                            spacing: 18

                            Text {
                                Layout.fillWidth: true
                                visible: stepBlock.modelData.name !== null && stepBlock.modelData.name !== ""
                                text: stepBlock.modelData.name !== null ? stepBlock.modelData.name : ""
                                color: Palette.text
                                font.family: Palette.fontSerif
                                font.pixelSize: 18
                            }

                            Repeater {
                                model: stepBlock.modelData.items

                                delegate: RowLayout {
                                    id: stepRow
                                    required property string modelData
                                    required property int index

                                    Layout.fillWidth: true
                                    spacing: 16

                                    Text {
                                        Layout.alignment: Qt.AlignTop
                                        text: (page.stepOffset(stepBlock.index) + stepRow.index + 1).toString()
                                        color: Palette.primary
                                        font.pixelSize: 26
                                        font.weight: Font.Bold
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        Layout.topMargin: 3
                                        text: stepRow.modelData
                                        color: Palette.text
                                        font.pixelSize: 17
                                        lineHeight: 1.35
                                        wrapMode: Text.WordWrap
                                    }
                                }
                            }
                        }
                    }

                    // The cook's own notes are the one place for handwriting.
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.topMargin: 10
                        visible: view.notes !== ""
                        implicitHeight: notesColumn.implicitHeight + 32
                        radius: 16
                        color: Palette.tint

                        ColumnLayout {
                            id: notesColumn
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.margins: 16
                            spacing: 6

                            Text {
                                text: "Notes"
                                color: Palette.primary
                                font.pixelSize: 15
                                font.weight: Font.Bold
                            }

                            Text {
                                Layout.fillWidth: true
                                text: view.notes
                                color: Palette.text
                                font.family: Palette.fontHand
                                font.pixelSize: 22
                                wrapMode: Text.WordWrap
                            }
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        Layout.topMargin: 10
                        visible: page.nutritionEntries.length > 0
                        spacing: 10

                        Text {
                            text: "Nutrition"
                            color: Palette.text
                            font.family: Palette.fontSerif
                            font.pixelSize: 22
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0

                            Repeater {
                                model: page.nutritionEntries

                                delegate: Rectangle {
                                    id: nutritionRow
                                    required property var modelData
                                    required property int index

                                    Layout.fillWidth: true
                                    implicitHeight: nutritionLine.implicitHeight + 16
                                    radius: index === 0 || index === page.nutritionEntries.length - 1 ? 12 : 0
                                    color: nutritionRow.index % 2 === 0 ? Palette.paper : Palette.tint
                                    border.width: 1
                                    border.color: Palette.line

                                    RowLayout {
                                        id: nutritionLine
                                        anchors.left: parent.left
                                        anchors.right: parent.right
                                        anchors.top: parent.top
                                        anchors.margins: 8
                                        spacing: 12

                                        Text {
                                            Layout.fillWidth: true
                                            text: nutritionRow.modelData.label
                                            color: Palette.textMuted
                                            font.pixelSize: 14
                                        }

                                        Text {
                                            text: nutritionRow.modelData.value
                                            color: Palette.text
                                            font.pixelSize: 14
                                            font.weight: Font.DemiBold
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Item {
                Layout.preferredHeight: 48
            }
        }
    }

    // ─── Toast ───
    Rectangle {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 24
        visible: view.toast !== ""
        implicitWidth: toastText.implicitWidth + 36
        implicitHeight: toastText.implicitHeight + 20
        radius: 12
        color: Palette.tile

        Text {
            id: toastText
            anchors.centerIn: parent
            text: view.toast
            color: Palette.onTile
            font.pixelSize: 14
            font.weight: Font.DemiBold
        }
    }
}
