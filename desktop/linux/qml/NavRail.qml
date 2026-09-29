import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The web's left nav rail (Layout.astro): Crumb, the sections, and Add recipe.
// Suggestions shows only while Wee Chef checks are on (or on its page), with a badge for
// lines to review.
Rectangle {
    id: rail

    property string section: ""
    property bool showSuggestions: false
    property int reviewCount: 0
    signal navigate(string route)

    color: Palette.nav
    implicitWidth: 240

    ColumnLayout {
        anchors.fill: parent
        anchors.leftMargin: 16
        anchors.rightMargin: 16
        anchors.topMargin: 24
        spacing: 4

        Text {
            text: "Crumb"
            color: Palette.text
            font.family: Palette.fontSerif
            font.pixelSize: 30
            Layout.leftMargin: 12
            Layout.bottomMargin: 24

            TapHandler {
                onTapped: rail.navigate("home")
            }
        }

        Repeater {
            model: [
                { "key": "kitchen", "route": "home", "label": "Home", "icon": "house" },
                { "key": "recipes", "route": "recipes", "label": "Recipes", "icon": "book-open-text" },
                { "key": "shelf", "route": "shelf", "label": "Shelf", "icon": "library-big" },
                { "key": "suggestions", "route": "suggestions", "label": "Suggestions", "icon": "sparkles" },
                { "key": "more", "route": "more", "label": "More", "icon": "ellipsis" }
            ]

            delegate: Rectangle {
                id: item
                required property var modelData
                readonly property bool active: rail.section === modelData.key

                visible: modelData.key !== "suggestions" || rail.showSuggestions || active
                Layout.fillWidth: true
                Layout.preferredHeight: 48
                radius: 12
                color: active ? Palette.butter : hover.hovered ? Qt.alpha(Palette.text, 0.06) : "transparent"

                HoverHandler {
                    id: hover
                    cursorShape: Qt.PointingHandCursor
                }

                TapHandler {
                    onTapped: rail.navigate(item.modelData.route)
                }

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    spacing: 12

                    Icon {
                        name: item.modelData.icon
                        size: 22
                        color: item.active ? Palette.onButter : Palette.textMuted
                    }

                    Text {
                        text: item.modelData.label
                        color: item.active ? Palette.onButter : Palette.text
                        font.family: Palette.fontSans
                        font.pixelSize: 15
                        font.weight: item.active ? Font.Bold : Font.DemiBold
                        Layout.fillWidth: true
                    }

                    Rectangle {
                        visible: item.modelData.key === "suggestions" && rail.reviewCount > 0 && !item.active
                        width: 22
                        height: 22
                        radius: 11
                        color: Palette.butter

                        Text {
                            anchors.centerIn: parent
                            text: rail.reviewCount > 99 ? "99+" : rail.reviewCount
                            color: Palette.onButter
                            font.family: Palette.fontSans
                            font.pixelSize: 13
                            font.weight: Font.Bold
                        }
                    }
                }
            }
        }

        CrumbButton {
            kind: rail.section === "add" ? "primary" : "primary"
            iconName: "plus"
            text: "Add recipe"
            Layout.fillWidth: true
            Layout.topMargin: 24
            onClicked: rail.navigate("add")
        }

        Item {
            Layout.fillHeight: true
        }
    }
}
