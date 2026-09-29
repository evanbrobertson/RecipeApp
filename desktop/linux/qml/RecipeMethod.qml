import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The Method panel of the recipe page (web RecipePage.svelte and RecipeVideo.svelte): the
// video link, numbered steps by section, the cook's notes in Kalam, and nutrition. A desktop
// app can't embed the player, so the video opens on its site.
ColumnLayout {
    id: panel

    property var recipe: null
    property bool secondary: false

    readonly property var embed: recipe && recipe.video ? JSON.parse(Core.videoEmbed(recipe.video)) : null
    readonly property string videoHost: recipe && recipe.video ? Core.hostOf(recipe.video) : ""
    readonly property var nutritionKeys: recipe && recipe.nutrition ? Object.keys(recipe.nutrition) : []
    readonly property var stepStarts: {
        var starts = []
        var n = 0
        var sections = recipe ? recipe.instructions : []
        for (var i = 0; i < sections.length; i++) {
            starts.push(n)
            n += sections[i].items.length
        }
        return starts
    }

    spacing: 0

    ColumnLayout {
        visible: !!(panel.recipe && panel.recipe.video)
        Layout.fillWidth: true
        Layout.bottomMargin: 40
        spacing: 16

        Flow {
            Layout.fillWidth: true
            spacing: 12

            Heading {
                text: "Video"
                height: 44
                verticalAlignment: Text.AlignVCenter
            }

            Text {
                id: watch
                readonly property bool named: !!panel.embed && panel.embed.provider !== "file"
                                              && panel.embed.provider !== "jwplayer"
                text: (named ? "Watch on " + panel.embed.label : (panel.videoHost || "Watch"))
                color: Palette.primary
                font.family: Palette.fontSans
                font.pixelSize: 14
                font.weight: Font.Bold
                font.underline: watchHover.hovered
                height: 44
                verticalAlignment: Text.AlignVCenter
                rightPadding: 18
                Accessible.role: Accessible.Link
                Accessible.name: text

                Icon {
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    name: "external-link"
                    size: 14
                    color: Palette.primary
                }
                HoverHandler {
                    id: watchHover
                    cursorShape: Qt.PointingHandCursor
                }
                TapHandler {
                    onTapped: panel.openVideo()
                }
            }
        }

        // The stand-in for the player: a tint tile with a play button
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 56
            radius: 16
            color: Palette.paper
            border.width: 1
            border.color: Palette.line
            Accessible.role: Accessible.Button
            Accessible.name: "Watch the video on " + (panel.videoHost || "its site")
            Accessible.onPressAction: panel.openVideo()

            Rectangle {
                anchors.fill: parent
                radius: parent.radius
                color: Palette.tint
                opacity: tileHover.hovered ? 0.55 : 0
            }
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 16
                spacing: 12

                Rectangle {
                    Layout.preferredWidth: 32
                    Layout.preferredHeight: 32
                    radius: 16
                    color: Palette.tile

                    Icon {
                        anchors.centerIn: parent
                        anchors.horizontalCenterOffset: 1
                        name: "play"
                        size: 16
                        color: Palette.onTile
                    }
                }
                Body {
                    text: "Watch the video on " + (panel.videoHost || "its site")
                    bold: true
                    elide: Text.ElideRight
                    wrapMode: Text.NoWrap
                    Layout.fillWidth: true
                }
            }
            HoverHandler {
                id: tileHover
                cursorShape: Qt.PointingHandCursor
            }
            TapHandler {
                onTapped: panel.openVideo()
            }
        }
    }

    function openVideo() {
        Qt.openUrlExternally(embed && embed.watchUrl ? embed.watchUrl : recipe.video)
    }

    Heading {
        text: "Method"
        color: panel.secondary ? Palette.textMuted : Palette.text
        Layout.preferredHeight: 44
        Layout.bottomMargin: 20
        verticalAlignment: Text.AlignVCenter

        Behavior on color {
            ColorAnimation { duration: 250 }
        }
    }

    Body {
        visible: !!panel.recipe && panel.recipe.instructions.length === 0
        text: "No steps listed."
        muted: true
        font.pixelSize: 14
    }

    ColumnLayout {
        Layout.fillWidth: true
        spacing: 32

        Repeater {
            model: panel.recipe ? panel.recipe.instructions : []

            delegate: ColumnLayout {
                id: section
                required property var modelData
                required property int index
                Layout.fillWidth: true
                spacing: 16

                Text {
                    visible: !!section.modelData.name
                    text: section.modelData.name || ""
                    color: Palette.primary
                    font.family: Palette.fontSans
                    font.pixelSize: 13
                    font.weight: Font.Bold
                    font.capitalization: Font.AllUppercase
                    font.letterSpacing: 0.5
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 24

                    Repeater {
                        model: section.modelData.items

                        delegate: RowLayout {
                            id: step
                            required property string modelData
                            required property int index
                            Layout.fillWidth: true
                            spacing: 16

                            Text {
                                text: panel.stepStarts[section.index] + step.index + 1
                                color: Palette.primary
                                font.family: Palette.fontSans
                                font.pixelSize: 28
                                font.weight: Font.ExtraBold
                                horizontalAlignment: Text.AlignRight
                                Layout.preferredWidth: 36
                                Layout.alignment: Qt.AlignTop
                                Accessible.ignored: true
                            }
                            Body {
                                text: step.modelData
                                font.pixelSize: 17
                                lineHeight: 1.2
                                Layout.fillWidth: true
                                Layout.topMargin: 2
                                Accessible.name: "Step " + (panel.stepStarts[section.index] + step.index + 1) + ": " + text
                            }
                        }
                    }
                }
            }
        }
    }

    // The cook's own notes are the one place for handwriting
    Card {
        visible: !!(panel.recipe && panel.recipe.notes)
        Layout.fillWidth: true
        Layout.topMargin: 40
        implicitHeight: notesColumn.implicitHeight + 48

        ColumnLayout {
            id: notesColumn
            anchors.fill: parent
            anchors.margins: 24
            spacing: 8

            RowLayout {
                spacing: 8

                Icon {
                    name: "sticky-note"
                    size: 18
                    color: Palette.primary
                }
                Body {
                    text: "Notes"
                    color: Palette.primary
                    bold: true
                }
            }
            Text {
                text: panel.recipe && panel.recipe.notes ? panel.recipe.notes : ""
                color: Palette.text
                font.family: Palette.fontNotes
                font.pixelSize: 21
                lineHeight: 1.1
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
        }
    }

    ColumnLayout {
        visible: panel.nutritionKeys.length > 0
        Layout.fillWidth: true
        Layout.topMargin: 40
        spacing: 20

        Heading {
            text: "Nutrition"
        }

        GridLayout {
            id: grid
            readonly property int cols: Math.max(1, Math.floor((width + 12) / (136 + 12)))
            Layout.fillWidth: true
            columns: cols
            columnSpacing: 12
            rowSpacing: 12

            Repeater {
                model: panel.nutritionKeys

                delegate: Card {
                    id: fact
                    required property string modelData
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    Layout.alignment: Qt.AlignTop
                    implicitHeight: factColumn.implicitHeight + 24

                    ColumnLayout {
                        id: factColumn
                        anchors.fill: parent
                        anchors.leftMargin: 16
                        anchors.rightMargin: 16
                        anchors.topMargin: 12
                        spacing: 2

                        Text {
                            text: Core.nutritionLabel(fact.modelData)
                            color: Palette.textMuted
                            font.family: Palette.fontSans
                            font.pixelSize: 13
                            font.weight: Font.DemiBold
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                        Body {
                            text: String(panel.recipe.nutrition[fact.modelData])
                            bold: true
                            Layout.fillWidth: true
                        }
                    }
                }
            }
        }
    }
}
