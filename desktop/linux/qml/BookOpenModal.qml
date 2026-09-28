import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Shapes

import app.crumb.desktop 1.0

// A cookbook pulled off the shelf, as the web's OpenBook.svelte: it lands in the centre, the
// cover swings open and the right-hand page is the table of contents. Call `show(book)` with
// a shelf book {id, name, description, color, recipeCount}; `dismissed` fires once it has closed.
Popup {
    id: modal

    property var book: null
    property var details: null
    property bool loading: false
    // The cover is swung open
    property bool flipped: false
    signal dismissed()

    readonly property var look: JSON.parse(Core.bookLook(book && book.color ? book.color : ""))
    readonly property real pageWidth: Math.min(380, width * 0.44)

    function show(next) {
        book = next
        details = null
        loading = true
        flipped = false
        open()
        // Let the closed book land before the cover swings open
        openTimer.restart()
        requests.call("cookbook", { "id": next.id }, function (d) {
            if (modal.book && modal.book.id === next.id)
                modal.details = d
            modal.loading = false
        }, function () {
            modal.details = null
            modal.loading = false
        })
    }

    function dismiss() {
        if (!visible || closeTimer.running)
            return
        openTimer.stop()
        flipped = false
        closeTimer.restart()
    }

    parent: Overlay.overlay
    x: 0
    y: 0
    width: parent ? parent.width : 900
    height: parent ? parent.height : 700
    padding: 0
    modal: true
    focus: true
    closePolicy: Popup.NoAutoClose
    enter: Transition {
        NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 300 }
    }
    exit: Transition {
        NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 200 }
    }

    Overlay.modal: Rectangle {
        color: Qt.rgba(13 / 255, 19 / 255, 15 / 255, 0.6)
    }

    background: Item {}

    Requests {
        id: requests
    }

    Timer {
        id: openTimer
        interval: 380
        onTriggered: modal.flipped = true
    }

    Timer {
        id: closeTimer
        interval: 520
        onTriggered: {
            modal.close()
            modal.dismissed()
        }
    }

    contentItem: Item {
        focus: true
        Keys.onEscapePressed: modal.dismiss()

        TapHandler {
            onTapped: modal.dismiss()
        }

        Item {
            id: stage

            width: modal.pageWidth * 2
            height: Math.min(560, modal.height * 0.8)
            // Closed: centre the cover (the right half)
            x: (parent.width - width) / 2 + (modal.flipped ? 0 : -modal.pageWidth / 2)
            y: (parent.height - height) / 2
            scale: modal.flipped ? 1 : 0.92
            opacity: modal.visible ? 1 : 0

            Behavior on x {
                NumberAnimation { duration: 600; easing.type: Easing.OutCubic }
            }
            Behavior on scale {
                NumberAnimation { duration: 600; easing.type: Easing.OutCubic }
            }

            // Swallow taps on the book itself
            TapHandler {}

            // Right page: the contents
            Rectangle {
                x: modal.pageWidth
                width: modal.pageWidth
                height: parent.height
                color: Palette.paper
                topRightRadius: 16
                bottomRightRadius: 16

                Rectangle {
                    width: parent.width * 0.08
                    height: parent.height
                    gradient: Gradient {
                        orientation: Gradient.Horizontal
                        GradientStop { position: 0; color: Qt.rgba(0, 0, 0, 0.08) }
                        GradientStop { position: 1; color: "transparent" }
                    }
                }

                ColumnLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 26
                    anchors.rightMargin: 26
                    anchors.topMargin: 28
                    anchors.bottomMargin: 20
                    spacing: 0

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.bottomMargin: 16
                        spacing: 8

                        Heading {
                            level: 2
                            text: "Contents"
                            Layout.fillWidth: true
                        }

                        Body {
                            text: (modal.book ? modal.book.recipeCount : 0) + " recipes"
                            muted: true
                            font.pixelSize: 13
                            font.weight: Font.DemiBold
                        }
                    }

                    ColumnLayout {
                        visible: modal.loading
                        spacing: 12
                        Layout.fillWidth: true

                        Repeater {
                            model: 5

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 16
                                radius: 12
                                color: Palette.tint
                            }
                        }
                    }

                    Body {
                        visible: !modal.loading && !(modal.details && modal.details.recipes.length)
                        text: "Blank pages, for now. Add recipes from any recipe page or with “Select” on the recipes list."
                        muted: true
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }

                    ListView {
                        id: toc

                        visible: !modal.loading && modal.details && modal.details.recipes.length > 0
                        model: modal.details ? modal.details.recipes : []
                        clip: true
                        boundsBehavior: Flickable.StopAtBounds
                        ScrollBar.vertical: ScrollBar {}
                        Layout.fillWidth: true
                        Layout.fillHeight: true

                        delegate: Item {
                            id: entry

                            required property var modelData
                            required property int index

                            width: toc.width - 8
                            height: 44

                            HoverHandler {
                                id: entryHover
                                cursorShape: Qt.PointingHandCursor
                            }

                            TapHandler {
                                onTapped: {
                                    modal.close()
                                    modal.dismissed()
                                    ApplicationWindow.window.go("recipe", { "id": entry.modelData.id })
                                }
                            }

                            RowLayout {
                                anchors.fill: parent
                                spacing: 6

                                Text {
                                    text: entry.modelData.title
                                    color: entryHover.hovered ? Palette.primary : Palette.text
                                    font.family: Palette.fontSerif
                                    font.pixelSize: 16
                                    elide: Text.ElideRight
                                    Layout.maximumWidth: entry.width - 60
                                }

                                // The dotted leader
                                Shape {
                                    Layout.fillWidth: true
                                    Layout.minimumWidth: 16
                                    Layout.preferredHeight: 2
                                    Layout.alignment: Qt.AlignBottom
                                    Layout.bottomMargin: 14
                                    ShapePath {
                                        strokeWidth: 2
                                        strokeColor: Palette.line
                                        strokeStyle: ShapePath.DashLine
                                        dashPattern: [1, 1.5]
                                        capStyle: ShapePath.RoundCap
                                        startX: 0
                                        startY: 1
                                        PathLine { x: 400; y: 1 }
                                    }
                                }

                                Body {
                                    text: entry.index + 1
                                    muted: true
                                    font.pixelSize: 14
                                    Layout.alignment: Qt.AlignBottom
                                    Layout.bottomMargin: 10
                                }
                            }
                        }
                    }

                    Item {
                        Layout.fillHeight: true
                        visible: !toc.visible
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.topMargin: 16

                        Item {
                            Layout.fillWidth: true
                        }

                        CrumbButton {
                            kind: "soft"
                            text: "Open cookbook"
                            iconName: "arrow-right"
                            onClicked: {
                                var id = modal.book.id
                                modal.close()
                                modal.dismissed()
                                ApplicationWindow.window.go("cookbook", { "id": id })
                            }
                        }
                    }
                }
            }

            // The cover: front outside, endpaper inside
            Item {
                id: cover

                x: modal.pageWidth
                y: -6
                width: modal.pageWidth + 6
                height: stage.height + 12
                property real angle: modal.flipped ? -180 : 0

                transform: Rotation {
                    origin.x: 0
                    origin.y: cover.height / 2
                    axis { x: 0; y: 1; z: 0 }
                    angle: cover.angle
                }

                Behavior on angle {
                    NumberAnimation { duration: 900; easing.type: Easing.OutCubic }
                }

                Rectangle {
                    visible: cover.angle > -90
                    anchors.fill: parent
                    color: modal.look.cloth
                    topRightRadius: 16
                    bottomRightRadius: 16

                    Rectangle {
                        width: parent.width * 0.07
                        height: parent.height
                        gradient: Gradient {
                            orientation: Gradient.Horizontal
                            GradientStop { position: 0; color: Qt.rgba(0, 0, 0, 0.22) }
                            GradientStop { position: 1; color: "transparent" }
                        }
                    }

                    Rectangle {
                        anchors.fill: parent
                        topRightRadius: 16
                        bottomRightRadius: 16
                        color: "transparent"
                        border.width: modal.look.edge ? 1 : 0
                        border.color: Qt.rgba(0.11, 0.17, 0.13, 0.22)
                    }

                    // The foil frame, doubled
                    Rectangle {
                        anchors.fill: parent
                        anchors.margins: 28
                        color: "transparent"
                        radius: 3
                        border.width: 1.5
                        border.color: modal.look.foil

                        Rectangle {
                            anchors.fill: parent
                            anchors.margins: -6
                            color: "transparent"
                            radius: 3
                            border.width: 1
                            border.color: modal.look.foil
                        }

                        ColumnLayout {
                            anchors.centerIn: parent
                            width: parent.width - 40
                            spacing: 14

                            Icon {
                                name: "chef-hat"
                                size: 32
                                color: modal.look.foil
                                Layout.alignment: Qt.AlignHCenter
                            }

                            Text {
                                text: modal.book ? modal.book.name : ""
                                color: modal.look.foil
                                font.family: Palette.fontSerif
                                font.pixelSize: 30
                                lineHeight: 1.1
                                wrapMode: Text.WordWrap
                                horizontalAlignment: Text.AlignHCenter
                                Layout.fillWidth: true
                            }

                            Rectangle {
                                color: modal.look.foil
                                Layout.preferredWidth: parent.width * 0.4
                                Layout.preferredHeight: 1
                                Layout.alignment: Qt.AlignHCenter
                            }

                            Text {
                                text: "RECIPES"
                                color: modal.look.foil
                                font.family: Palette.fontSans
                                font.pixelSize: 13
                                font.weight: Font.Bold
                                font.letterSpacing: 2.6
                                Layout.alignment: Qt.AlignHCenter
                            }
                        }
                    }
                }

                Rectangle {
                    visible: cover.angle <= -90
                    anchors.fill: parent
                    // Endpaper: paper faintly tinted with the cover colour
                    color: Qt.tint(Palette.paper, Qt.rgba(Qt.color(modal.look.cloth).r, Qt.color(modal.look.cloth).g,
                                                          Qt.color(modal.look.cloth).b, 0.1))
                    topLeftRadius: 16
                    bottomLeftRadius: 16
                    border.width: 1
                    border.color: Palette.line
                    transform: Scale {
                        origin.x: cover.width / 2
                        xScale: -1
                    }

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 30
                        anchors.topMargin: 34
                        anchors.bottomMargin: 34
                        spacing: 0

                        Text {
                            text: "EX LIBRIS"
                            color: Palette.textMuted
                            font.family: Palette.fontSans
                            font.pixelSize: 13
                            font.weight: Font.Bold
                            font.letterSpacing: 3.9
                        }

                        Text {
                            text: modal.book ? modal.book.name : ""
                            color: Palette.text
                            font.family: Palette.fontSerif
                            font.pixelSize: 24
                            lineHeight: 1.1
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                            Layout.topMargin: 12
                        }

                        Body {
                            visible: modal.book && !!modal.book.description
                            text: modal.book && modal.book.description ? modal.book.description : ""
                            font.pixelSize: 14
                            Layout.fillWidth: true
                            Layout.topMargin: 12
                        }

                        Item {
                            Layout.fillHeight: true
                        }

                        Body {
                            text: "A cookbook of " + (modal.book ? modal.book.recipeCount : 0) + " recipes"
                            muted: true
                            font.pixelSize: 13
                            font.weight: Font.DemiBold
                            Layout.fillWidth: true
                        }
                    }
                }
            }

            Rectangle {
                x: parent.width - 44
                y: -52
                width: 44
                height: 44
                radius: 22
                color: closeHover.hovered ? Qt.rgba(1, 253 / 255, 248 / 255, 0.26) : Qt.rgba(1, 253 / 255, 248 / 255, 0.16)
                activeFocusOnTab: true
                Accessible.role: Accessible.Button
                Accessible.name: "Close book"
                Accessible.onPressAction: modal.dismiss()
                Keys.onReturnPressed: modal.dismiss()
                Keys.onSpacePressed: modal.dismiss()

                HoverHandler {
                    id: closeHover
                    cursorShape: Qt.PointingHandCursor
                }

                TapHandler {
                    onTapped: modal.dismiss()
                }

                Icon {
                    anchors.centerIn: parent
                    name: "x"
                    size: 22
                    color: "#fffdf8"
                }
            }
        }
    }
}
