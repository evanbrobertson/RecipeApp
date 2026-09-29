import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The trash (web pages/more/trash.astro and islands/TrashPage.svelte): recipes deleted in
// the last 30 days. Each can be put back, with its cookbooks and cook log, or deleted for
// good; so can all of them.
Item {
    id: page

    property var session
    property int routeId: 0
    property var params: ({})

    // null while loading
    property var items: null
    // The id being put back or deleted, "all" while emptying, else null
    property var busy: null

    function drop(id) {
        page.items = (page.items || []).filter(function (i) { return i.id !== id })
    }

    function restore(item) {
        page.busy = item.id
        requests.call("restoreRecipe", { "id": item.id }, function (recipe) {
            page.busy = null
            page.drop(item.id)
            var win = page.ApplicationWindow.window
            win.toast({
                "title": recipe.isNew ? "Put back" : "It's already in your box",
                "description": recipe.isNew ? item.title : "Its link was saved again since",
                "tone": "success",
                "action": { "label": "Open", "onselect": function () { win.go("recipe", { "id": recipe.id }) } }
            })
        }, function (error) {
            page.busy = null
            page.ApplicationWindow.window.toast({ "title": "Couldn't put it back", "description": error, "tone": "error" })
        })
    }

    function purge(item) {
        page.busy = item.id
        requests.call("purgeTrashed", { "id": item.id }, function () {
            page.busy = null
            page.drop(item.id)
        }, function (error) {
            page.busy = null
            page.ApplicationWindow.window.toast({ "title": "Couldn't delete it", "description": error, "tone": "error" })
        })
    }

    function emptyAll() {
        page.busy = "all"
        requests.call("emptyTrash", {}, function () {
            page.busy = null
            page.items = []
            confirmEmpty.close()
            page.ApplicationWindow.window.toast({ "title": "Trash emptied" })
        }, function (error) {
            page.busy = null
            page.ApplicationWindow.window.toast({ "title": "Couldn't empty the trash", "description": error, "tone": "error" })
        })
    }

    Requests {
        id: requests
    }

    Component.onCompleted: {
        requests.call("trash", {}, function (list) {
            page.items = list || []
        }, function (error) {
            page.items = []
            page.ApplicationWindow.window.toast({ "title": "Couldn't open the trash", "description": error, "tone": "error" })
        })
    }

    Modal {
        id: confirmEmpty

        readonly property int count: page.items ? page.items.length : 0

        title: "Empty the trash?"
        description: Core.emptyTrash(count)
        footer: [
            CrumbButton {
                kind: "ghost"
                text: "Cancel"
                onClicked: confirmEmpty.close()
            },
            CrumbButton {
                kind: "danger"
                text: "Empty the trash"
                enabled: page.busy === null
                onClicked: page.emptyAll()
            }
        ]
    }

    ScrollPage {
        anchors.fill: parent
        maxWidth: 600

        ColumnLayout {
            width: parent.width
            spacing: 32

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0

                CrumbButton {
                    kind: "ghost"
                    iconName: "arrow-left"
                    text: "More"
                    Layout.leftMargin: -12
                    Layout.bottomMargin: 12
                    onClicked: page.ApplicationWindow.window.go("more", {})
                }

                Heading {
                    level: 1
                    text: "Trash"
                    font.pixelSize: 30
                    Layout.fillWidth: true
                }

                Body {
                    text: "Deleted recipes stay here for 30 days, with their cookbooks and cook log, then go for good."
                    muted: true
                    Layout.topMargin: 8
                    Layout.fillWidth: true
                }
            }

            // The web's skeleton while the trash loads
            Rectangle {
                visible: page.items === null
                Layout.fillWidth: true
                Layout.preferredHeight: 160
                radius: 16
                color: Palette.tint
            }

            Card {
                visible: page.items !== null && page.items.length === 0
                Layout.fillWidth: true
                implicitHeight: emptyText.implicitHeight + 48

                Body {
                    id: emptyText
                    anchors.centerIn: parent
                    width: parent.width - 48
                    horizontalAlignment: Text.AlignHCenter
                    text: "Nothing in the trash."
                    muted: true
                }
            }

            ColumnLayout {
                visible: !!page.items && page.items.length > 0
                Layout.fillWidth: true
                spacing: 16

                Card {
                    implicitHeight: trashColumn.implicitHeight
                    clip: true
                    Layout.fillWidth: true

                    Column {
                        id: trashColumn
                        width: parent.width

                        Repeater {
                            model: page.items || []

                            delegate: Item {
                                id: row

                                required property var modelData
                                required property int index

                                width: trashColumn.width
                                implicitHeight: 76

                                Rectangle {
                                    visible: row.index > 0
                                    x: 16
                                    width: parent.width - 16
                                    height: 1
                                    color: Palette.line
                                }

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 16
                                    anchors.rightMargin: 16
                                    spacing: 14

                                    Photo {
                                        recipeId: row.modelData.id
                                        image: row.modelData.image || ""
                                        Layout.preferredWidth: 56
                                        Layout.preferredHeight: 56
                                    }

                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 0

                                        Text {
                                            Layout.fillWidth: true
                                            text: row.modelData.title
                                            color: Palette.text
                                            font.family: Palette.fontSans
                                            font.pixelSize: 16
                                            font.weight: Font.Bold
                                            elide: Text.ElideRight
                                        }
                                        Body {
                                            Layout.fillWidth: true
                                            text: Core.trashLeft(Date.parse(row.modelData.purgeAt), Date.now())
                                            muted: true
                                            font.pixelSize: 14
                                        }
                                    }

                                    CrumbButton {
                                        kind: "soft"
                                        iconName: "rotate-ccw"
                                        iconSize: 16
                                        text: "Put back"
                                        enabled: page.busy === null
                                        onClicked: page.restore(row.modelData)
                                    }

                                    CrumbButton {
                                        kind: "ghost"
                                        iconName: "trash-2"
                                        iconSize: 16
                                        enabled: page.busy === null
                                        Accessible.name: "Delete " + row.modelData.title + " for good"
                                        ToolTip.visible: hovered
                                        ToolTip.text: "Delete for good"
                                        onClicked: page.purge(row.modelData)
                                    }
                                }
                            }
                        }
                    }
                }

                CrumbButton {
                    kind: "ghost"
                    text: "Empty the trash"
                    enabled: page.busy === null
                    onClicked: confirmEmpty.open()
                }
            }
        }
    }
}
