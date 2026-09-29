import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The ingredient or instruction sections of the recipe editor: a title with "+ Section", then
// one textarea per section with Wee Chef's hints under the section that still has the line.
// Mirrors the `sections` snippet in RecipeEditor.svelte.
ColumnLayout {
    id: sections

    property var editor
    // "ingredients" or "instructions"
    property string kind: "ingredients"
    readonly property bool isIngredients: kind === "ingredients"

    spacing: 0

    RowLayout {
        Layout.fillWidth: true
        Layout.bottomMargin: 14
        spacing: 12

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 0

            Heading {
                level: 2
                text: sections.isIngredients ? "Ingredients" : "Instructions"
                Layout.fillWidth: true
            }

            Body {
                text: "One " + (sections.isIngredients ? "ingredient" : "step") + " per line."
                muted: true
                font.pixelSize: 13
                Layout.topMargin: 4
                Layout.fillWidth: true
            }
        }

        CrumbButton {
            kind: "soft"
            iconName: "plus"
            text: "Section"
            onClicked: sections.editor.addSection(sections.kind)
        }
    }

    ColumnLayout {
        Layout.fillWidth: true
        spacing: 16

        Repeater {
            model: sections.editor.draft[sections.kind].length

            delegate: ColumnLayout {
                id: section

                required property int index
                readonly property var hints: sections.editor.hintsFor(sections.kind, index)

                Layout.fillWidth: true
                spacing: 8

                RowLayout {
                    visible: sections.editor.draft[sections.kind].length > 1 || nameField.text !== ""
                    Layout.fillWidth: true
                    spacing: 8

                    StyledField {
                        id: nameField
                        Layout.fillWidth: true
                        implicitHeight: 44
                        font.weight: Font.Bold
                        font.family: Palette.fontSans
                        font.pixelSize: 15
                        text: sections.editor.sectionValue(sections.kind, section.index, "name")
                        placeholderText: sections.isIngredients ? "Section name, e.g. For the sauce" : "Section name, e.g. Make the icing"
                        Accessible.name: "Section name"
                        onTextEdited: sections.editor.setSection(sections.kind, section.index, "name", text)
                    }

                    CrumbButton {
                        kind: "ghost"
                        Accessible.name: "Remove section"
                        onClicked: sections.editor.removeSection(sections.kind, section.index)

                        // The web's `text-error` icon
                        contentItem: Icon {
                            name: "trash-2"
                            size: 18
                            color: Palette.error
                        }
                    }
                }

                EditorArea {
                    Layout.fillWidth: true
                    rows: sections.isIngredients ? 6 : 8
                    value: sections.editor.sectionValue(sections.kind, section.index, "text")
                    accessibleName: sections.isIngredients ? "Ingredients" : "Steps"
                    placeholder: sections.isIngredients ? "2 cups flour\n1 tsp salt" : "Preheat the oven to 180°C.\nMix the dry ingredients."
                    onEdited: text => sections.editor.setSection(sections.kind, section.index, "text", text)
                }

                Repeater {
                    model: section.hints

                    delegate: EditorHint {
                        id: hintBar

                        required property var modelData
                        readonly property var fix: sections.editor.fixFor(sections.kind, section.index, modelData)

                        Layout.fillWidth: true
                        lead: Core.quote(modelData.itemText, 60)
                        rest: Core.reviewText(modelData.kind)
                        fixLabel: fix ? fix.label : ""
                        onFix: sections.editor.applyFix(hintBar.fix)
                        onDismiss: sections.editor.dismissFlag(modelData)
                    }
                }
            }
        }
    }
}
