import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

Item {
    id: page
    property var session

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
                font.pixelSize: 28
                font.bold: true
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
                    color: Palette.primary
                    font.pixelSize: 14
                    verticalAlignment: Text.AlignVCenter
                }
            }
        }

        TextField {
            id: searchField
            Layout.fillWidth: true
            placeholderText: "Search recipes…"
            color: Palette.text
            font.pixelSize: 14
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
                    width: grid.cellWidth - 12
                    height: grid.cellHeight - 12
                    radius: 16
                    color: Palette.paper
                    border.color: Palette.line
                    border.width: 1

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 0

                        Image {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 150
                            source: model.imageUrl
                            asynchronous: true
                            fillMode: Image.PreserveAspectCrop
                            sourceSize.width: 320
                            visible: model.imageUrl !== ""
                        }

                        Label {
                            Layout.fillWidth: true
                            Layout.leftMargin: 12
                            Layout.rightMargin: 12
                            Layout.topMargin: 10
                            text: model.title
                            color: Palette.text
                            font.pixelSize: 15
                            font.bold: true
                            elide: Text.ElideRight
                            maximumLineCount: 2
                            wrapMode: Text.WordWrap
                        }

                        Label {
                            Layout.fillWidth: true
                            Layout.leftMargin: 12
                            Layout.rightMargin: 12
                            visible: model.kicker !== ""
                            text: model.kicker
                            color: Palette.textMuted
                            font.pixelSize: 13
                            elide: Text.ElideRight
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
