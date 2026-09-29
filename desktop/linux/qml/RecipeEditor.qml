import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The recipe form, for editing and for a new recipe (web components/RecipeEditor.svelte).
// `initialDraft` is `Core.draftFromRecipe(...)` or `Core.newDraft(...)`; the form keeps the
// draft as an object, and saving hands `Core.draftToFields` of it to `saved`. Everything about
// markers, blank lines and nutrition happens in Core, not here.
ColumnLayout {
    id: editor

    property string initialDraft: "{}"
    property bool saving: false
    property string submitLabel: "Save"
    // Wee Chef's flags for this recipe (`recipeChecks.flags`)
    property var flags: []
    // Whether "Keep as is" is offered (it needs a saved recipe)
    property bool canDismiss: false

    property var draft: JSON.parse(initialDraft)
    property string error: ""
    // Bumped on each keystroke, so the hints follow the text without rebinding the fields
    property int revision: 0

    readonly property var metaFields: JSON.parse(Core.metaFields())
    readonly property var categories: JSON.parse(Core.categories())
    // A category from before the fixed list stays selected (marked old) until it's changed
    readonly property string oldCategory: Core.legacyCategory(JSON.parse(initialDraft).recipeCategory)
    readonly property var categoryOptions: {
        var list = [{ "label": "None", "value": "" }]
        if (oldCategory !== "" && draft.recipeCategory === oldCategory)
            list.push({ "label": "(old) " + oldCategory, "value": oldCategory })
        for (var i = 0; i < categories.length; i++)
            list.push({ "label": categories[i], "value": categories[i] })
        return list
    }
    // A photo link the site refuses: shown while the draft still has that link
    readonly property var photoFlag: {
        revision
        for (var i = 0; i < flags.length; i++) {
            var f = flags[i]
            if (f.field === "image" && f.state === "review" && f.itemText === draft.image)
                return f
        }
        return null
    }

    readonly property bool wide: width >= 940
    readonly property bool sm: width >= 576

    signal saved(var fields)
    signal cancelled()
    signal dismissed(var flag)

    spacing: 28

    function copy(value) {
        return JSON.parse(JSON.stringify(value))
    }

    function setField(key, value) {
        draft[key] = value
        revision += 1
    }

    function sectionValue(kind, i, key) {
        var s = draft[kind][i]
        return s ? s[key] : ""
    }

    function setSection(kind, i, key, value) {
        draft[kind][i][key] = value
        revision += 1
    }

    function addSection(kind) {
        var next = copy(draft)
        next[kind].push({ "name": "", "text": "" })
        draft = next
    }

    function removeSection(kind, i) {
        var next = copy(draft)
        next[kind].splice(i, 1)
        draft = next
    }

    // The review flags whose line is still in this section
    function hintsFor(kind, i) {
        revision
        var s = draft[kind][i]
        if (!s)
            return []
        var here = s.text.split("\n").map(l => l.trim())
        return flags.filter(f => f.field === kind && f.state === "review" && here.indexOf(f.itemText) >= 0)
    }

    // `{label, draft}` for the flag's one-tap fix, or null when none fits
    function fixFor(kind, i, flag) {
        var json = Core.fixFor(JSON.stringify(draft), kind, i, flag.kind, flag.itemText)
        return json === "" ? null : JSON.parse(json)
    }

    function applyFix(fix) {
        draft = fix.draft
    }

    function dismissFlag(flag) {
        editor.dismissed(flag)
    }

    function submit() {
        if (saving)
            return
        error = ""
        var body = JSON.parse(Core.draftToFields(JSON.stringify(draft)))
        if (body.error) {
            error = body.error
            return
        }
        saved(body)
    }

    Shortcut {
        sequence: "Ctrl+S"
        onActivated: editor.submit()
    }

    EditorHint {
        visible: editor.photoFlag !== null
        Layout.fillWidth: true
        lead: editor.photoFlag ? Core.reviewText(editor.photoFlag.kind) + "." : ""
        rest: "Remove it, or paste a new one under Image link."
        fixLabel: "Remove photo"
        onFix: {
            var next = editor.copy(editor.draft)
            next.image = ""
            editor.draft = next
        }
        onDismiss: editor.dismissFlag(editor.photoFlag)
    }

    // ─── Title and description ───
    Card {
        Layout.fillWidth: true
        implicitHeight: topColumn.implicitHeight + 40

        ColumnLayout {
            id: topColumn
            anchors.fill: parent
            anchors.margins: 20
            spacing: 16

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 6

                Body {
                    text: "Title <font color=\"" + Palette.error + "\">*</font>"
                    textFormat: Text.StyledText
                    bold: true
                }

                StyledField {
                    Layout.fillWidth: true
                    implicitHeight: 56
                    font.pixelSize: 16
                    text: editor.draft.title
                    placeholderText: "Grandma's lasagne"
                    Accessible.name: "Title"
                    onTextEdited: editor.setField("title", text)
                    onAccepted: editor.submit()
                }

                Body {
                    visible: editor.error !== ""
                    text: editor.error
                    color: Palette.error
                    bold: true
                    font.pixelSize: 14
                    Layout.topMargin: 2
                    Layout.fillWidth: true
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 6

                Body {
                    text: "Description"
                    bold: true
                }

                EditorArea {
                    Layout.fillWidth: true
                    rows: 2
                    value: editor.draft.description
                    accessibleName: "Description"
                    onEdited: text => editor.setField("description", text)
                }
            }
        }
    }

    // ─── Details ───
    ColumnLayout {
        Layout.fillWidth: true
        spacing: 14

        Heading {
            level: 2
            text: "Details"
            Layout.fillWidth: true
        }

        Card {
            Layout.fillWidth: true
            implicitHeight: metaGrid.implicitHeight + 40

            GridLayout {
                id: metaGrid
                anchors.fill: parent
                anchors.margins: 20
                columns: editor.sm ? 4 : 2
                columnSpacing: 12
                rowSpacing: 16
                uniformCellWidths: true

                Repeater {
                    model: editor.metaFields

                    delegate: ColumnLayout {
                        id: metaCell

                        required property var modelData
                        readonly property string key: modelData[0]

                        Layout.fillWidth: true
                        Layout.alignment: Qt.AlignTop
                        spacing: 6

                        Body {
                            text: metaCell.modelData[1]
                            bold: true
                        }

                        EditorSelect {
                            visible: metaCell.key === "recipeCategory"
                            Layout.fillWidth: true
                            options: editor.categoryOptions
                            value: editor.draft.recipeCategory
                            Accessible.name: "Category"
                            onPicked: v => editor.setField("recipeCategory", v)
                        }

                        StyledField {
                            visible: metaCell.key !== "recipeCategory"
                            Layout.fillWidth: true
                            implicitHeight: 44
                            text: editor.draft[metaCell.key] === undefined ? "" : editor.draft[metaCell.key]
                            placeholderText: metaCell.modelData[2]
                            Accessible.name: metaCell.modelData[1]
                            onTextEdited: editor.setField(metaCell.key, text)
                        }
                    }
                }
            }
        }
    }

    // ─── Ingredients and instructions ───
    GridLayout {
        id: sectionsGrid
        Layout.fillWidth: true
        columns: editor.wide ? 2 : 1
        columnSpacing: 40
        rowSpacing: 28

        EditorSections {
            editor: editor
            kind: "ingredients"
            Layout.alignment: Qt.AlignTop
            Layout.fillWidth: true
            Layout.preferredWidth: editor.wide ? (sectionsGrid.width - 40) * 2 / 5 : sectionsGrid.width
        }

        EditorSections {
            editor: editor
            kind: "instructions"
            Layout.alignment: Qt.AlignTop
            Layout.fillWidth: true
            Layout.preferredWidth: editor.wide ? (sectionsGrid.width - 40) * 3 / 5 : sectionsGrid.width
        }
    }

    // ─── Notes and sources ───
    ColumnLayout {
        Layout.fillWidth: true
        spacing: 14

        Heading {
            level: 2
            text: "Notes and sources"
            Layout.fillWidth: true
        }

        Card {
            Layout.fillWidth: true
            implicitHeight: sourcesGrid.implicitHeight + 40

            GridLayout {
                id: sourcesGrid
                anchors.fill: parent
                anchors.margins: 20
                columns: editor.sm ? 2 : 1
                columnSpacing: 16
                rowSpacing: 16
                uniformCellWidths: true

                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignTop
                    spacing: 6

                    Body {
                        text: "Notes"
                        bold: true
                    }

                    EditorArea {
                        Layout.fillWidth: true
                        hand: true
                        rows: 3
                        value: editor.draft.notes
                        placeholder: "Less sugar next time…"
                        accessibleName: "Notes"
                        onEdited: text => editor.setField("notes", text)
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignTop
                    spacing: 6

                    Body {
                        text: "Nutrition"
                        bold: true
                    }

                    EditorArea {
                        Layout.fillWidth: true
                        rows: 3
                        value: editor.draft.nutrition
                        accessibleName: "Nutrition"
                        onEdited: text => editor.setField("nutrition", text)
                    }

                    Body {
                        text: "One per line, e.g. calories: 320"
                        muted: true
                        font.pixelSize: 13
                        Layout.topMargin: -2
                        Layout.fillWidth: true
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignTop
                    spacing: 6

                    Body {
                        text: "Source link"
                        bold: true
                    }

                    StyledField {
                        Layout.fillWidth: true
                        implicitHeight: 44
                        text: editor.draft.url
                        placeholderText: "https://…"
                        Accessible.name: "Source link"
                        onTextEdited: editor.setField("url", text)
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignTop
                    spacing: 6

                    Body {
                        text: "Image link"
                        bold: true
                    }

                    StyledField {
                        Layout.fillWidth: true
                        implicitHeight: 44
                        text: editor.draft.image
                        placeholderText: "https://…"
                        Accessible.name: "Image link"
                        onTextEdited: editor.setField("image", text)
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignTop
                    Layout.columnSpan: editor.sm ? 2 : 1
                    spacing: 6

                    Body {
                        text: "Video link"
                        bold: true
                    }

                    StyledField {
                        Layout.fillWidth: true
                        implicitHeight: 44
                        text: editor.draft.video
                        placeholderText: "https://youtu.be/…"
                        Accessible.name: "Video link"
                        onTextEdited: editor.setField("video", text)
                    }

                    Body {
                        text: "YouTube, Vimeo, TikTok and Instagram play on the page"
                        muted: true
                        font.pixelSize: 13
                        Layout.topMargin: -2
                        Layout.fillWidth: true
                    }
                }
            }
        }
    }

    // ─── Actions ───
    Rectangle {
        Layout.fillWidth: true
        Layout.preferredHeight: 1
        color: Palette.line
    }

    RowLayout {
        Layout.fillWidth: true
        Layout.topMargin: -8
        spacing: 8

        Item {
            Layout.fillWidth: true
        }

        CrumbButton {
            kind: "ghost"
            text: "Cancel"
            implicitHeight: 48
            onClicked: editor.cancelled()
        }

        CrumbButton {
            kind: "primary"
            iconName: editor.saving ? "loader-circle" : "check"
            text: editor.submitLabel
            implicitHeight: 48
            enabled: !editor.saving
            onClicked: editor.submit()
        }
    }
}
