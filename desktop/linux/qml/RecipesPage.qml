import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Effects

import app.crumb.desktop 1.0

Item {
    id: page
    property var session
    signal openRecipe(int recipeId)

    function focusSearch() {
        searchField.forceActiveFocus()
    }

    RecipeList {
        id: list
        onUnauthorized: if (page.session) page.session.requireLogin()
    }

    Timer {
        id: debounce
        interval: 250
        repeat: false
        onTriggered: list.refresh(searchField.text)
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 24
        spacing: 16

        RowLayout {
            Layout.fillWidth: true
            spacing: 12

            Label {
                text: "Recipes"
                color: Palette.text
                font.family: Palette.fontSerif
                font.pixelSize: 28
                Layout.fillWidth: true
            }

            BusyIndicator {
                running: list.loading
                visible: list.loading
                Layout.preferredWidth: 24
                Layout.preferredHeight: 24
            }

            Button {
                flat: true
                text: "Sign out"
                onClicked: if (page.session) page.session.logout()

                background: Item {}

                contentItem: Text {
                    text: "Sign out"
                    color: Palette.textMuted
                    font.pixelSize: 14
                    verticalAlignment: Text.AlignVCenter
                }
            }
        }

        StyledField {
            id: searchField
            Layout.fillWidth: true
            placeholderText: "Search recipes…"
            onTextChanged: debounce.restart()
        }

        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            GridView {
                id: grid
                anchors.fill: parent
                clip: true
                model: list
                cellWidth: 232
                cellHeight: 252

                delegate: Rectangle {
                    id: card
                    width: grid.cellWidth - 12
                    height: layout.implicitHeight
                    radius: 16
                    color: Palette.paper

                    Behavior on color {
                        ColorAnimation { duration: 150 }
                    }

                    ColumnLayout {
                        id: layout
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        spacing: 0

                        Item {
                            id: photoFrame
                            Layout.fillWidth: true
                            Layout.preferredHeight: 150

                            Rectangle {
                                id: photo
                                anchors.fill: parent
                                color: Palette.tint
                                topLeftRadius: 16
                                topRightRadius: 16

                                // Striped placeholder, shown until a photo loads over it.
                                Canvas {
                                    id: stripes
                                    anchors.fill: parent
                                    visible: model.imageUrl === ""
                                    property color stripeColor: Palette.line
                                    onStripeColorChanged: requestPaint()
                                    onWidthChanged: requestPaint()
                                    onHeightChanged: requestPaint()
                                    Component.onCompleted: requestPaint()
                                    onPaint: {
                                        var ctx = getContext("2d")
                                        ctx.clearRect(0, 0, width, height)
                                        ctx.strokeStyle = stripeColor
                                        ctx.lineWidth = 1
                                        var step = 12
                                        for (var x = 0; x < width + height; x += step) {
                                            ctx.beginPath()
                                            ctx.moveTo(x, 0)
                                            ctx.lineTo(x - height, height)
                                            ctx.stroke()
                                        }
                                    }
                                }

                                Image {
                                    anchors.fill: parent
                                    source: model.imageUrl
                                    asynchronous: true
                                    fillMode: Image.PreserveAspectCrop
                                    sourceSize.width: 320
                                    visible: model.imageUrl !== ""
                                }

                                // The photo fills the card edge to edge and is masked to the
                                // card's 16px top corners (the bottom stays square).
                                layer.enabled: true
                                layer.effect: MultiEffect {
                                    maskEnabled: true
                                    maskSource: photoMask
                                }
                            }

                            Rectangle {
                                id: photoMask
                                anchors.fill: parent
                                color: "black"
                                visible: false
                                layer.enabled: true
                                topLeftRadius: 16
                                topRightRadius: 16
                            }
                        }

                        ColumnLayout {
                            id: textBlock
                            Layout.fillWidth: true
                            Layout.margins: 12
                            spacing: 2

                            Label {
                                Layout.fillWidth: true
                                text: model.title
                                color: Palette.text
                                font.pixelSize: 16
                                font.weight: Font.DemiBold
                                elide: Text.ElideRight
                                maximumLineCount: 2
                                wrapMode: Text.WordWrap
                            }

                            Label {
                                Layout.fillWidth: true
                                visible: model.kicker !== ""
                                text: model.kicker
                                color: Palette.textMuted
                                font.pixelSize: 13
                                elide: Text.ElideRight
                            }
                        }
                    }

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: page.openRecipe(model.recipeId)
                    }

                    // Card border, drawn over the photo so the 1px line stays crisp.
                    Rectangle {
                        anchors.fill: parent
                        radius: 16
                        color: "transparent"
                        border.color: Palette.line
                        border.width: 1
                        z: 1

                        Behavior on border.color {
                            ColorAnimation { duration: 150 }
                        }
                    }
                }
            }

            Label {
                anchors.centerIn: parent
                visible: !list.loading && list.count === 0
                text: "No recipes yet"
                color: Palette.textMuted
                font.pixelSize: 16
            }
        }
    }

    Component.onCompleted: list.refresh("")
}
