import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// Your shelf, as the web's islands/ShelfPage.svelte: every cookbook on the shelf, a book that
// flies off it and opens, and New book. The layout and colours come from Core.
Item {
    id: page

    property var session
    property int routeId: 0
    property var params: ({})

    property var cookbooks: []
    property bool loading: true
    property bool creating: false
    property var opened: null

    function startCreate() {
        var colors = JSON.parse(Core.bookColors())
        form.name = ""
        form.description = ""
        form.color = colors[Math.floor(Math.random() * colors.length)]
        createModal.open()
    }

    function create() {
        if (!form.name.trim() || page.creating)
            return
        page.creating = true
        requests.call("createCookbook", {
            "name": form.name.trim(),
            "description": form.description,
            "color": form.color
        }, function (book) {
            requests.call("cookbooks", {}, function (list) {
                page.cookbooks = list
                page.creating = false
                createModal.close()
                ApplicationWindow.window.toast({ "title": "“" + book.name + "” is on the shelf", "tone": "success" })
            }, function () {
                page.creating = false
            })
        }, function (error) {
            page.creating = false
            ApplicationWindow.window.toast({ "title": "Couldn't create cookbook", "description": error, "tone": "error" })
        })
    }

    Requests {
        id: requests
        onFailed: (error) => ApplicationWindow.window.toast({ "title": "Couldn't load the shelf", "description": error, "tone": "error" })
    }

    Component.onCompleted: requests.call("cookbooks", {}, function (list) {
        page.cookbooks = list
        page.loading = false
    }, function (error) {
        page.loading = false
        ApplicationWindow.window.toast({ "title": "Couldn't load the shelf", "description": error, "tone": "error" })
    })

    ScrollPage {
        anchors.fill: parent

        RowLayout {
            width: parent.width
            spacing: 12

            ColumnLayout {
                spacing: 8
                Layout.fillWidth: true

                Heading {
                    level: 1
                    text: "Your shelf"
                    font.pixelSize: 30
                    lineHeight: 1.05
                    Layout.fillWidth: true
                }

                Body {
                    text: "Pull a book off the shelf to open it."
                    muted: true
                    font.pixelSize: 14
                    Layout.fillWidth: true
                }
            }

            // An empty shelf has its own call to action
            CrumbButton {
                visible: page.cookbooks.length > 0
                kind: "primary"
                text: "New book"
                iconName: "plus"
                Layout.alignment: Qt.AlignBottom
                onClicked: page.startCreate()
            }
        }

        Item {
            width: parent.width
            height: 28
        }

        Rectangle {
            visible: page.loading
            width: parent.width
            height: 208
            radius: 16
            color: Palette.tint

            SequentialAnimation on opacity {
                running: page.loading
                loops: Animation.Infinite
                NumberAnimation { to: 0.55; duration: 800 }
                NumberAnimation { to: 1; duration: 800 }
            }
        }

        Item {
            visible: !page.loading && page.cookbooks.length === 0
            width: parent.width
            height: visible ? empty.implicitHeight + 96 : 0

            EmptyState {
                id: empty
                anchors.horizontalCenter: parent.horizontalCenter
                y: 48
                width: Math.min(parent.width, 480)
                iconName: "library-big"
                title: "An empty shelf"
                text: "Cookbooks group recipes, like “Weeknight dinners” or “Christmas baking”."
                actionText: "Make your first cookbook"
                onAction: page.startCreate()
            }
        }

        ShelfCard {
            visible: !page.loading && page.cookbooks.length > 0
            width: parent.width
            height: visible ? shelf.implicitHeight : 0

            Bookshelf {
                id: shelf
                width: parent.width
                books: page.cookbooks
                addable: true
                pulledId: page.opened ? page.opened.id : 0
                onOpenBook: (book, origin) => {
                    page.opened = book
                    openBook.show(book, origin)
                }
                onAdd: page.startCreate()
            }
        }
    }

    BookOpenModal {
        id: openBook
        onDismissed: page.opened = null
    }

    Modal {
        id: createModal
        title: "New book"
        onOpened: form.focusName()

        BookForm {
            id: form
            Layout.fillWidth: true
            onAccepted: page.create()
        }

        footer: [
            CrumbButton {
                kind: "ghost"
                text: "Cancel"
                onClicked: createModal.close()
            },
            CrumbButton {
                kind: "primary"
                text: "Put it on the shelf"
                enabled: !page.creating && form.name.trim() !== ""
                onClicked: page.create()
            }
        ]
    }
}
