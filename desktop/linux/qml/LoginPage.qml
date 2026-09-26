import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

Item {
    id: page
    property var session

    function connectMode(url) {
        return !session || session.state === "setup"
                || session.serverUrl !== url || session.error !== ""
    }

    function submit() {
        if (!session)
            return
        var url = urlField.text.trim()
        if (connectMode(url))
            session.connect(url)
        else
            session.login(passwordField.text)
    }

    ColumnLayout {
        anchors.centerIn: parent
        width: Math.min(420, page.width - 64)
        spacing: 12

        Label {
            text: "Crumb"
            color: Palette.text
            font.pixelSize: 34
            font.bold: true
            Layout.alignment: Qt.AlignHCenter
        }

        Label {
            text: "Your private recipe box"
            color: Palette.textMuted
            font.pixelSize: 14
            Layout.alignment: Qt.AlignHCenter
            Layout.bottomMargin: 10
        }

        Label {
            text: "Server"
            color: Palette.textMuted
            font.pixelSize: 13
        }

        TextField {
            id: urlField
            Layout.fillWidth: true
            placeholderText: "https://crumb.example"
            text: session ? session.serverUrl : ""
            color: Palette.text
            font.pixelSize: 14
            enabled: session ? !session.busy : true
            onAccepted: if (passwordField.text.length > 0) page.submit()
        }

        Label {
            text: "Password"
            color: Palette.textMuted
            font.pixelSize: 13
        }

        TextField {
            id: passwordField
            Layout.fillWidth: true
            echoMode: TextInput.Password
            color: Palette.text
            font.pixelSize: 14
            enabled: session ? !session.busy : true
            onAccepted: page.submit()
        }

        Label {
            Layout.fillWidth: true
            visible: session ? session.error !== "" : false
            text: session ? session.error : ""
            color: Palette.error
            font.pixelSize: 13
            wrapMode: Text.WordWrap
        }

        Button {
            id: submitButton
            Layout.fillWidth: true
            Layout.preferredHeight: 44
            enabled: session ? !session.busy : false
            text: page.connectMode(urlField.text.trim()) ? "Connect" : "Sign in"
            onClicked: page.submit()

            background: Rectangle {
                radius: 12
                color: submitButton.enabled ? Palette.butter : Palette.tint
            }

            contentItem: Text {
                text: submitButton.text
                color: Palette.onButter
                font.pixelSize: 15
                font.bold: true
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
            }
        }
    }
}
