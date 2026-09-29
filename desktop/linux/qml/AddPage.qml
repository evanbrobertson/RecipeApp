import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The Add page (web pages/add.astro): the Add box, what can be pasted, and other ways in.
Item {
    id: page

    property var session
    property int routeId: 0
    property var params: ({})

    // Popular (the server's src/popular.rs): links several other households saved that this
    // one hasn't. Nothing shows when the server has no Popular or nothing is popular yet.
    property var popular: []

    function go(name) {
        ApplicationWindow.window.go(name, {})
    }

    Requests {
        id: requests
    }

    Component.onCompleted: {
        requests.call("popular", {}, function (res) {
            if (res && res.enabled)
                page.popular = res.items.slice(0, 8)
        }, function () {
            // Popular is a nice-to-have: the Add page works the same without it
        })
    }

    ScrollPage {
        anchors.fill: parent
        maxWidth: 768

        ColumnLayout {
            width: parent.width
            spacing: 28

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0

                Heading {
                    level: 1
                    text: "Add a recipe"
                    font.pixelSize: 30
                    Layout.fillWidth: true
                }

                Body {
                    text: "Paste a link or the recipe itself."
                    muted: true
                    Layout.topMargin: 8
                    Layout.fillWidth: true
                }

                TopBox {
                    tabs: false
                    onTile: false
                    autofocus: true
                    Layout.topMargin: 20
                    Layout.fillWidth: true
                }
            }

            ColumnLayout {
                visible: page.popular.length > 0
                Layout.fillWidth: true
                spacing: 0

                Heading {
                    level: 2
                    text: "Popular in Crumb"
                    Layout.fillWidth: true
                }

                Body {
                    text: "Saved by several other households."
                    muted: true
                    font.pixelSize: 14
                    Layout.topMargin: 4
                    Layout.bottomMargin: 14
                    Layout.fillWidth: true
                }

                Card {
                    Layout.fillWidth: true
                    implicitHeight: popularColumn.implicitHeight
                    clip: true

                    Column {
                        id: popularColumn
                        width: parent.width

                        Repeater {
                            model: page.popular

                            delegate: AddRow {
                                required property var modelData
                                required property int index
                                width: popularColumn.width
                                first: index === 0
                                iconName: "flame"
                                title: modelData.title
                                text: modelData.host + " · in " + modelData.households + " boxes"
                                chevron: true
                                onClicked: page.ApplicationWindow.window.go("preview", { "url": modelData.url })
                            }
                        }
                    }
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 14

                Heading {
                    level: 2
                    text: "What you can paste"
                    Layout.fillWidth: true
                }

                Card {
                    Layout.fillWidth: true
                    implicitHeight: pasteColumn.implicitHeight
                    clip: true

                    Column {
                        id: pasteColumn
                        width: parent.width

                        Repeater {
                            model: [
                                { "icon": "link", "title": "A link", "text": "Most recipe sites work. If one blocks us, a real browser has a go." },
                                { "icon": "clipboard-type", "title": "Any recipe text", "text": "From an email, notes, a PDF or a photo's text. Headings help but aren't required." },
                                { "icon": "list", "title": "Lots of links", "text": "Paste several at once and they're imported one after another." }
                            ]

                            delegate: AddRow {
                                required property var modelData
                                required property int index
                                width: pasteColumn.width
                                first: index === 0
                                iconName: modelData.icon
                                title: modelData.title
                                text: modelData.text
                            }
                        }
                    }
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 14

                Heading {
                    level: 2
                    text: "Other ways in"
                    Layout.fillWidth: true
                }

                Card {
                    Layout.fillWidth: true
                    implicitHeight: waysColumn.implicitHeight
                    clip: true

                    Column {
                        id: waysColumn
                        width: parent.width

                        AddRow {
                            width: waysColumn.width
                            iconName: "pen-line"
                            title: "Write from scratch"
                            text: "Grandma's card, your own creation"
                            chevron: true
                            onClicked: page.go("new")
                        }

                        AddRow {
                            width: waysColumn.width
                            first: false
                            iconName: "file-down"
                            title: "Import from another app"
                            text: "Just the Recipe, Paprika, Mealie…"
                            chevron: true
                            onClicked: page.go("import")
                        }
                    }
                }
            }
        }
    }
}
