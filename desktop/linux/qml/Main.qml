import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

ApplicationWindow {
    id: window
    visible: true
    width: 1100
    height: 760
    minimumWidth: 480
    minimumHeight: 400
    title: "Crumb"
    color: Palette.bg

    Session {
        id: session
    }

    Component.onCompleted: {
        if (session.state === "checking" && session.serverUrl !== "")
            session.connect(session.serverUrl)
    }

    Loader {
        anchors.fill: parent
        sourceComponent: {
            if (session.state === "ready")
                return recipesPage
            if (session.state === "checking")
                return checkingPage
            return loginPage
        }
    }

    Component {
        id: loginPage
        LoginPage {
            session: session
        }
    }

    Component {
        id: recipesPage
        RecipesPage {
            session: session
        }
    }

    Component {
        id: checkingPage
        Item {
            ColumnLayout {
                anchors.centerIn: parent
                spacing: 12

                BusyIndicator {
                    Layout.alignment: Qt.AlignHCenter
                    running: true
                }

                Label {
                    text: "Connecting…"
                    color: Palette.text
                    font.pixelSize: 15
                    Layout.alignment: Qt.AlignHCenter
                }
            }
        }
    }
}
