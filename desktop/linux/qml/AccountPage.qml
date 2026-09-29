import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts

import app.crumb.desktop 1.0

// More → Account (web/src/islands/AccountPage.svelte, pages/more/account.astro): who's signed
// in and how, their devices and household (AccountSection), the apps connected to Crumb, their
// data (download, delete) and signing out. With one password it's just the connected apps and
// signing out. Google and Apple can't be linked from here (they need a browser), but a linked
// one is listed and can be unlinked.
Item {
    id: page

    property var session
    property int routeId: 0
    property var params: ({})

    property var status: null
    property var methods: null
    property var apps: null
    property bool busy: false
    // The row asking to confirm: "app:<id>", "unlink:<provider>", "email" or "delete"
    property string confirming: ""
    property string password: ""
    property string typedEmail: ""
    property string deleteError: ""
    property string newEmail: ""
    property string newEmailAgain: ""
    property string emailPassword: ""
    property string emailError: ""

    readonly property bool signedIn: !!status && status.mode !== "password" && !!status.user
    readonly property bool hasPassword: !!methods && methods.password
    // Unlinking the last way in isn't allowed.
    readonly property int ways: (hasPassword ? 1 : 0) + (methods ? methods.linked.length : 0)
    readonly property var providerNames: ({ "google": "Google", "apple": "Apple" })

    function providerName(p) {
        return providerNames[p] || p
    }

    function day(secs) {
        return Core.dateLabel(secs * 1000, -new Date(secs * 1000).getTimezoneOffset())
    }

    function toast(t) {
        ApplicationWindow.window.toast(t)
    }

    function toastError(fallback, error) {
        toast({ "title": error || fallback, "tone": "error" })
    }

    function load() {
        requests.call("authStatus", {}, function (s) {
            page.status = s
            if (page.signedIn)
                page.loadMethods()
        }, function () {
            page.status = { "mode": "password" }
        })
        loadApps()
    }

    function loadMethods() {
        requests.call("signInMethods", {}, function (m) {
            page.methods = m
        }, function () {
            page.methods = null
        })
    }

    function loadApps() {
        requests.call("connectedApps", {}, function (list) {
            page.apps = list
        }, function () {
            page.apps = []
        })
    }

    function unlink(provider) {
        busy = true
        requests.call("unlink", { "provider": provider }, function () {
            page.busy = false
            page.confirming = ""
            page.loadMethods()
            page.toast({ "title": page.providerName(provider) + " unlinked" })
        }, function (error) {
            page.busy = false
            page.toastError("Couldn't unlink " + page.providerName(provider), error)
        })
    }

    function disconnect(app) {
        busy = true
        requests.call("disconnectApp", { "id": app.id }, function () {
            page.busy = false
            page.confirming = ""
            page.loadApps()
            page.toast({ "title": app.name + " disconnected", "description": "It has to be approved again." })
        }, function (error) {
            page.busy = false
            page.toastError("Couldn't disconnect it", error)
        })
    }

    function removeAccount() {
        busy = true
        deleteError = ""
        var args = hasPassword ? { "password": password } : { "confirm": typedEmail }
        requests.call("deleteAccount", args, function () {
            page.busy = false
            page.session.logout()
            page.toast({ "title": "Your account was deleted" })
        }, function (error) {
            page.busy = false
            page.deleteError = error || "Couldn't delete your account"
        })
    }

    function closeEmail() {
        confirming = ""
        newEmail = newEmailAgain = emailPassword = emailError = ""
    }

    function saveEmail() {
        var email = newEmail.trim()
        // Typed twice, because a typo here is the very thing this is for
        if (email.toLowerCase() !== newEmailAgain.trim().toLowerCase()) {
            emailError = "The two addresses don't match"
            return
        }
        busy = true
        emailError = ""
        requests.call("changeEmail", { "email": email, "password": emailPassword }, function (made) {
            page.busy = false
            if (made.pending) {
                var old = page.status.user.email
                page.closeEmail()
                page.toast({
                    "title": "Check " + made.email,
                    "description": "Open the link sent there to switch. Until then, sign in with " + old + "."
                })
            } else {
                page.closeEmail()
                page.session.refreshStatus()
                page.load()
                page.toast({ "title": "Email changed", "description": "Sign in with " + made.email + " from now on." })
            }
        }, function (error) {
            page.busy = false
            page.emailError = error || "Couldn't change your email"
        })
    }

    Component.onCompleted: load()

    Requests {
        id: requests
    }

    FolderDialog {
        id: folder
        title: "Save your data to"
        onAccepted: requests.call("exportAccount", { "dir": String(selectedFolder) }, function (saved) {
            page.toast({ "title": "Saved", "description": saved.path })
        }, function (error) {
            page.toastError("Couldn't save the file", error)
        })
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
                    text: "More"
                    iconName: "arrow-left"
                    Layout.leftMargin: -12
                    Layout.bottomMargin: 12
                    onClicked: ApplicationWindow.window.go("more")
                }

                Heading {
                    level: 1
                    text: "Account"
                    font.pixelSize: 30
                    Layout.fillWidth: true
                }

                Body {
                    text: "How you sign in, your household and connected apps."
                    muted: true
                    Layout.topMargin: 8
                    Layout.fillWidth: true
                }
            }

            // Loading
            ColumnLayout {
                visible: !page.status
                Layout.fillWidth: true
                spacing: 32

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 160
                    radius: 16
                    color: Palette.tint
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 112
                    radius: 16
                    color: Palette.tint
                }
            }

            ColumnLayout {
                visible: !!page.status
                Layout.fillWidth: true
                spacing: 32

                AccountSection {
                    visible: page.signedIn
                    session: page.session
                    status: page.status || ({})

                    // Sign-in methods
                    ColumnLayout {
                        visible: !!page.methods
                        Layout.fillWidth: true
                        spacing: 0

                        AccountHeading {
                            text: "Sign-in methods"
                        }

                        AccountList {
                            AccountRow {
                                visible: page.confirming !== "email"
                                iconName: "mail"
                                title: "Email"
                                subtitle: page.status && page.status.user ? page.status.user.email : ""

                                CrumbButton {
                                    kind: "ghost"
                                    iconName: "pencil"
                                    ToolTip.visible: hovered
                                    ToolTip.text: "Change"
                                    Accessible.name: "Change your email"
                                    onClicked: page.confirming = "email"
                                }
                            }

                            AccountFormRow {
                                visible: page.confirming === "email"
                                iconName: "mail"

                                Text {
                                    text: "Change your email"
                                    color: Palette.text
                                    font.family: Palette.fontSans
                                    font.pixelSize: 15
                                    font.weight: Font.Bold
                                }

                                Text {
                                    text: page.status && page.status.mode === "hosted"
                                          ? "We'll send the new address a link, and switch once it's opened."
                                          : "You'll sign in with the new address, and be signed out on other devices."
                                    color: Palette.textMuted
                                    font.family: Palette.fontSans
                                    font.pixelSize: 14
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }

                                AccountField {
                                    label: "New email"
                                    text: page.newEmail
                                    onTextChanged: page.newEmail = text
                                    onVisibleChanged: if (visible) input.forceActiveFocus()
                                    onAccepted: page.saveEmail()
                                }

                                AccountField {
                                    label: "New email again"
                                    text: page.newEmailAgain
                                    onTextChanged: page.newEmailAgain = text
                                    onAccepted: page.saveEmail()
                                }

                                AccountField {
                                    visible: page.hasPassword
                                    label: "Your password"
                                    echoMode: TextInput.Password
                                    text: page.emailPassword
                                    onTextChanged: page.emailPassword = text
                                    onAccepted: page.saveEmail()
                                }

                                Text {
                                    visible: page.emailError !== ""
                                    text: page.emailError
                                    color: Palette.error
                                    font.family: Palette.fontSans
                                    font.pixelSize: 14
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }

                                RowLayout {
                                    Layout.alignment: Qt.AlignRight
                                    spacing: 8

                                    CrumbButton {
                                        kind: "ghost"
                                        text: "Cancel"
                                        onClicked: page.closeEmail()
                                    }

                                    CrumbButton {
                                        kind: "primary"
                                        text: "Change email"
                                        iconName: page.busy ? "loader-circle" : ""
                                        enabled: !page.busy && page.newEmail.trim() !== ""
                                                 && page.newEmailAgain.trim() !== ""
                                                 && (!page.hasPassword || page.emailPassword !== "")
                                        onClicked: page.saveEmail()
                                    }
                                }
                            }

                            AccountRow {
                                visible: page.hasPassword
                                iconName: "key-round"
                                title: "Password"
                                subtitle: "With your email"
                            }

                            Repeater {
                                model: page.methods ? page.methods.linked : []

                                AccountRow {
                                    id: linked

                                    required property var modelData

                                    iconName: "link-2"
                                    title: page.providerName(modelData.provider)
                                    subtitle: modelData.email || "Linked " + page.day(modelData.createdAt)
                                    confirming: page.confirming === "unlink:" + modelData.provider
                                    confirmText: "You won't be able to sign in with it."
                                    dangerLabel: "Unlink"
                                    busy: page.busy
                                    onKept: page.confirming = ""
                                    onConfirmed: page.unlink(modelData.provider)

                                    CrumbButton {
                                        visible: page.ways > 1
                                        kind: "ghost"
                                        iconName: "unlink"
                                        ToolTip.visible: hovered
                                        ToolTip.text: "Unlink"
                                        Accessible.name: "Unlink " + page.providerName(linked.modelData.provider)
                                        onClicked: page.confirming = "unlink:" + linked.modelData.provider
                                    }
                                }
                            }
                        }
                    }
                }

                // Connected apps
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 0

                    AccountHeading {
                        text: "Connected apps"
                    }

                    AccountList {
                        AccountRow {
                            visible: page.apps === null
                            iconName: "loader-circle"
                            title: "Loading…"
                            titleColor: Palette.textMuted
                        }

                        AccountRow {
                            visible: !!page.apps && page.apps.length === 0
                            iconName: "plug"
                            title: "No apps connected"
                            subtitle: "Connect Claude to save and find recipes from a chat."
                            clickable: true
                            onClicked: ApplicationWindow.window.go("connect")
                        }

                        Repeater {
                            model: page.apps || []

                            AccountRow {
                                id: app

                                required property var modelData

                                iconName: "plug"
                                title: modelData.name
                                subtitle: [modelData.household && modelData.household.name,
                                           "connected " + page.day(modelData.connectedAt)].filter(Boolean).join(" · ")
                                confirming: page.confirming === "app:" + modelData.id
                                confirmText: "It stops working until it's approved again."
                                dangerLabel: "Disconnect"
                                busy: page.busy
                                onKept: page.confirming = ""
                                onConfirmed: page.disconnect(modelData)

                                CrumbButton {
                                    kind: "ghost"
                                    iconName: "unlink"
                                    ToolTip.visible: hovered
                                    ToolTip.text: "Disconnect"
                                    Accessible.name: "Disconnect " + app.modelData.name
                                    onClicked: page.confirming = "app:" + app.modelData.id
                                }
                            }
                        }
                    }
                }

                // Your data
                ColumnLayout {
                    visible: page.signedIn
                    Layout.fillWidth: true
                    spacing: 0

                    AccountHeading {
                        text: "Your data"
                    }

                    AccountList {
                        AccountRow {
                            iconName: "download"
                            title: "Download my data"
                            subtitle: "Your account and every household's recipes, as one JSON file"
                            clickable: true
                            onClicked: folder.open()
                        }

                        AccountRow {
                            visible: page.confirming !== "delete"
                            iconName: "trash-2"
                            title: "Delete account"
                            titleColor: Palette.error
                            subtitle: "And the households only you are in"
                            clickable: true
                            onClicked: page.confirming = "delete"
                        }

                        AccountFormRow {
                            visible: page.confirming === "delete"
                            iconName: "trash-2"

                            Text {
                                text: "Delete your account?"
                                color: Palette.text
                                font.family: Palette.fontSans
                                font.pixelSize: 15
                                font.weight: Font.Bold
                            }

                            Text {
                                text: "This can't be undone. Households you share pass to the next person in them; ones that are only yours are deleted with their recipes. Download your data first if you want a copy."
                                color: Palette.textMuted
                                font.family: Palette.fontSans
                                font.pixelSize: 14
                                wrapMode: Text.WordWrap
                                Layout.fillWidth: true
                            }

                            AccountField {
                                visible: page.hasPassword
                                label: "Your password"
                                echoMode: TextInput.Password
                                text: page.password
                                onTextChanged: page.password = text
                                onAccepted: page.removeAccount()
                            }

                            AccountField {
                                visible: !page.hasPassword
                                label: "Type " + (page.status && page.status.user ? page.status.user.email : "") + " to confirm"
                                text: page.typedEmail
                                onTextChanged: page.typedEmail = text
                                onAccepted: page.removeAccount()
                            }

                            Text {
                                visible: page.deleteError !== ""
                                text: page.deleteError
                                color: Palette.error
                                font.family: Palette.fontSans
                                font.pixelSize: 14
                                wrapMode: Text.WordWrap
                                Layout.fillWidth: true
                            }

                            RowLayout {
                                Layout.alignment: Qt.AlignRight
                                spacing: 8

                                CrumbButton {
                                    kind: "ghost"
                                    text: "Keep my account"
                                    onClicked: {
                                        page.confirming = ""
                                        page.deleteError = ""
                                    }
                                }

                                CrumbButton {
                                    kind: "danger"
                                    text: "Delete account"
                                    iconName: page.busy ? "loader-circle" : ""
                                    enabled: !page.busy && (page.hasPassword ? page.password !== "" : page.typedEmail !== "")
                                    onClicked: page.removeAccount()
                                }
                            }
                        }
                    }
                }

                // Sign out
                AccountList {
                    visible: page.signedIn || (!!page.status && !!page.status.passwordRequired)

                    AccountRow {
                        iconName: "log-out"
                        title: "Sign out"
                        clickable: true
                        onClicked: page.session.logout()
                    }
                }
            }
        }
    }
}
