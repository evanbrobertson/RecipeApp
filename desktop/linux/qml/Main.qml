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

    Behavior on color {
        ColorAnimation { duration: 150 }
    }

    Session {
        id: appSession
    }

    Component.onCompleted: {
        if (appSession.state === "checking" && appSession.serverUrl !== "")
            appSession.connect(appSession.serverUrl)
    }

    Loader {
        anchors.fill: parent
        sourceComponent: {
            if (appSession.state === "ready")
                return recipesPage
            if (appSession.state === "checking")
                return checkingPage
            return loginPage
        }
    }

    Component {
        id: loginPage
        LoginPage {
            session: appSession
        }
    }

    Component {
        id: recipesPage
        RecipesPage {
            session: appSession
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
