import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// Signing in: the server, then the form for how it signs people in (web/src/islands/
// LoginForm.svelte, InviteForm.svelte, ResetForm.svelte and pages/{login,signup,setup}.astro).
// One password, or an email and password with sign-up, first-account set-up, joining a
// household from an invite link and (hosted) a password reset link. Google, Apple and
// passkeys need a browser's cookie, so they aren't here; links from emails (choosing the new
// password, confirming an email change) open on the web.
Item {
    id: page

    property var session

    // A server that answered, so the sign-in form applies (also true after signing out)
    property bool connected: false
    property bool connecting: false
    // What the person picked from the sign-in form: login, signup, reset or invite
    property string choice: "login"
    property bool checkEmail: false
    property bool joining: false
    property string localError: ""
    property string emailSent: ""
    // Joining a household: the pasted link's token, what it's for, and how far along
    property string inviteToken: ""
    property var invitePreview: null
    property string inviteStage: "token"
    property string inviteTab: "new"

    readonly property var status: {
        try {
            return JSON.parse(session.statusJson)
        } catch (e) {
            return ({})
        }
    }
    readonly property bool accounts: !!session && (session.mode === "accounts" || session.mode === "hosted")
    readonly property bool hosted: !!session && session.mode === "hosted"
    readonly property string view: {
        if (!connected)
            return "server"
        if (!accounts)
            return "login"
        if (status.setupNeeded)
            return "setup"
        if (choice === "signup" && !status.signupOpen)
            return "login"
        if (choice === "reset" && !hosted)
            return "login"
        return choice
    }
    readonly property bool working: !!session && (session.busy || requests.busy > 0 || joining)
    readonly property string error: localError !== "" ? localError : (session ? session.error : "")
    readonly property string greeting: view === "setup" ? "let's get cooking"
                                       : view === "signup" ? "pull up a chair" : "welcome back, cook"
    readonly property string intro: {
        if (view === "setup")
            return "Make the first account. It owns the recipes already here."
        if (view === "signup")
            return "Make an account and a recipe box of your own."
        return accounts ? "Sign in to open your recipe box." : "Enter your password to open your recipe box."
    }
    readonly property bool needsName: view === "signup" || view === "setup"
    readonly property bool needsEmail: accounts && (view === "login" || needsName)
    readonly property bool canSubmit: {
        if (working)
            return false
        var short = passwordField.text.length < 8
        if (view === "login")
            return accounts ? emailField.text.trim() !== "" && passwordField.text !== "" : passwordField.text !== ""
        if (needsName)
            return nameField.text.trim() !== "" && emailField.text.trim() !== "" && !short
                    && (!status.setupNeedsAppPassword || view !== "setup" || appPasswordField.text !== "")
        return false
    }
    // Connect needs a URL; the sign-in form has its own rules (`canSubmit`).
    readonly property bool canConnect: !!session && !session.busy && urlField.text.trim().length > 0

    function connect() {
        if (!canConnect)
            return
        localError = ""
        checkEmail = false
        connecting = true
        session.connect(urlField.text.trim())
    }

    function submit() {
        if (!canSubmit)
            return
        localError = ""
        if (view === "login" && accounts)
            session.signIn(emailField.text.trim(), passwordField.text)
        else if (view === "login")
            session.login(passwordField.text)
        else
            session.signUp(nameField.text.trim(), emailField.text.trim(), passwordField.text, appPasswordField.text)
    }

    function show(next) {
        localError = ""
        emailSent = ""
        checkEmail = false
        inviteStage = "token"
        invitePreview = null
        choice = next
    }

    function sendReset() {
        var email = emailField.text.trim()
        if (email === "" || working)
            return
        localError = ""
        requests.call("requestReset", { "email": email }, function () {
            page.emailSent = email
        }, function (error) {
            page.localError = error || "Something went wrong"
        })
    }

    // The token in a pasted invite link (`…/invite#<token>`), or the token itself.
    function tokenOf(text) {
        var t = text.trim()
        var at = t.lastIndexOf("#")
        return decodeURIComponent(at >= 0 ? t.slice(at + 1) : t)
    }

    function readInvite() {
        var token = tokenOf(inviteField.text)
        if (token === "" || working)
            return
        localError = ""
        inviteToken = token
        // Hosted invites can only be read by the person they're for, once signed in
        if (hosted) {
            inviteStage = "form"
            return
        }
        requests.call("previewInvite", { "token": token }, function (preview) {
            page.invitePreview = preview
            page.inviteStage = preview ? "form" : "gone"
        }, function (error) {
            page.inviteStage = "gone"
        })
    }

    function join() {
        if (!canJoin)
            return
        localError = ""
        joining = true
        var email = emailField.text.trim()
        var args = { "token": inviteToken, "email": email, "password": passwordField.text }
        if (inviteTab === "new")
            args.name = nameField.text.trim()
        requests.call("acceptInvite", args, function (done) {
            if (done && done.verify) {
                page.joining = false
                page.checkEmail = true
                return
            }
            var name = page.invitePreview && page.invitePreview.householdName
            ApplicationWindow.window.toast({ "title": name ? "Welcome to " + name : "You're in" })
            // Joined: sign in to keep the session
            page.joining = false
            page.session.signIn(email, passwordField.text)
        }, function (error) {
            page.joining = false
            page.localError = error || "Couldn't join"
            // A refused sign-in drops the session; pick the server up again
            Qt.callLater(function () {
                var message = page.localError
                page.connecting = true
                page.session.connect(page.session.serverUrl)
                page.localError = message
            })
        })
    }

    readonly property bool canJoin: !working && emailField.text.trim() !== "" && passwordField.text !== ""
                                    && (inviteTab !== "new" || (nameField.text.trim() !== "" && passwordField.text.length >= 8))

    Component.onCompleted: connected = !!session && session.serverUrl !== "" && session.error === ""

    // The `--smoke` regression check: the page's `session` must be bound (it once bound to
    // itself, so the button never enabled), and typing a URL must enable the button.
    function smokeCheck() {
        if (!session) {
            Smoke.fail("LoginPage.session is not bound")
            return
        }
        urlField.text = ""
        if (submitButton.enabled)
            Smoke.fail("the submit button enabled with no server")
        urlField.text = "https://crumb.example"
        if (!submitButton.enabled)
            Smoke.fail("the submit button never enabled")
    }

    Connections {
        target: page.session

        function onVerifyChanged() {
            if (page.session.verify)
                page.checkEmail = true
        }

        function onBusyChanged() {
            // A connect that came back with a server (or without) decides which step shows
            if (!page.session.busy && page.connecting) {
                page.connecting = false
                page.connected = page.session.serverUrl !== "" && page.session.error === ""
            }
        }
    }

    Requests {
        id: requests
    }

    Flickable {
        anchors.fill: parent
        clip: true
        contentWidth: width
        contentHeight: column.implicitHeight
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}

        ColumnLayout {
            id: column
            width: parent.width
            spacing: 0

            // The web's bare header: the wordmark and a line of handwriting on the tile
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 214
                color: Palette.tile

                ColumnLayout {
                    id: header
                    x: Math.max(20, (parent.width - width) / 2)
                    y: 74
                    width: Math.min(344, parent.width - 40)
                    spacing: 8

                    Text {
                        text: "Crumb"
                        color: Palette.onTile
                        font.family: Palette.fontSerif
                        font.pixelSize: 30
                    }

                    Text {
                        text: page.greeting
                        color: Palette.onTile
                        font.family: Palette.fontHand
                        font.pixelSize: 38
                        lineHeight: 0.8
                        Layout.topMargin: -6
                    }
                }
            }

            ColumnLayout {
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: 28
                Layout.bottomMargin: 40
                Layout.preferredWidth: Math.min(344, page.width - 40)
                Layout.maximumWidth: 344
                spacing: 16

                // Which server
                ColumnLayout {
                    visible: page.view === "server"
                    Layout.fillWidth: true
                    spacing: 16

                    Body {
                        text: "Enter the address of your Crumb server."
                        muted: true
                        Layout.fillWidth: true
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 6

                        Text {
                            text: "Server"
                            color: Palette.text
                            font.family: Palette.fontSans
                            font.pixelSize: 15
                            font.weight: Font.Bold
                        }

                        StyledField {
                            id: urlField
                            Layout.fillWidth: true
                            Layout.preferredHeight: 56
                            font.pixelSize: 16
                            placeholderText: "https://crumb.example"
                            text: page.session ? page.session.serverUrl : ""
                            enabled: page.session ? !page.session.busy : true
                            onAccepted: page.connect()
                        }
                    }

                    Text {
                        visible: page.view === "server" && page.error !== ""
                        text: page.error
                        color: Palette.error
                        font.family: Palette.fontSans
                        font.pixelSize: 14
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }

                    CrumbButton {
                        id: submitButton
                        kind: "primary"
                        text: "Connect"
                        iconName: page.session && page.session.busy ? "loader-circle" : ""
                        Layout.fillWidth: true
                        Layout.preferredHeight: 56
                        enabled: page.canConnect
                        onClicked: page.connect()
                    }
                }

                // Check your email
                ColumnLayout {
                    visible: page.view !== "server" && page.checkEmail
                    Layout.fillWidth: true
                    spacing: 12

                    Body {
                        text: "Check your email"
                        bold: true
                        Layout.fillWidth: true
                    }

                    Body {
                        text: page.view === "invite"
                              ? "We sent a link to " + emailField.text.trim() + ". Open it to confirm your address and join, then sign in here."
                              : "We sent a link to " + emailField.text.trim() + ". Open it to confirm your address and your recipe box is ready."
                        muted: true
                        Layout.fillWidth: true
                    }

                    CrumbButton {
                        kind: "ghost"
                        text: "Back to sign in"
                        Layout.leftMargin: -16
                        onClicked: page.show("login")
                    }
                }

                // Sign in, sign up, first account
                ColumnLayout {
                    visible: (page.view === "login" || page.view === "signup" || page.view === "setup") && !page.checkEmail
                    Layout.fillWidth: true
                    spacing: 16

                    Body {
                        text: page.intro
                        muted: true
                        Layout.fillWidth: true
                    }
                }

                // Joining a household
                ColumnLayout {
                    visible: page.view === "invite" && !page.checkEmail
                    Layout.fillWidth: true
                    spacing: 16

                    Body {
                        visible: page.inviteStage === "token"
                        text: "Paste the invite link you were sent to join a household."
                        muted: true
                        Layout.fillWidth: true
                    }

                    AccountField {
                        id: inviteField
                        visible: page.inviteStage === "token"
                        large: true
                        label: "Invite link"
                        onAccepted: page.readInvite()
                    }

                    Text {
                        visible: page.inviteStage === "token" && page.error !== ""
                        text: page.error
                        color: Palette.error
                        font.family: Palette.fontSans
                        font.pixelSize: 14
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }

                    CrumbButton {
                        visible: page.inviteStage === "token"
                        kind: "primary"
                        text: "Continue"
                        Layout.fillWidth: true
                        Layout.preferredHeight: 56
                        enabled: !page.working && page.tokenOf(inviteField.text) !== ""
                        onClicked: page.readInvite()
                    }

                    // This invite can't be used
                    Body {
                        visible: page.inviteStage === "gone"
                        text: "This invite can't be used"
                        bold: true
                        Layout.fillWidth: true
                    }

                    Body {
                        visible: page.inviteStage === "gone"
                        text: "It has expired, was cancelled or was already used. Ask for a new one."
                        muted: true
                        Layout.fillWidth: true
                    }

                    Text {
                        visible: page.inviteStage === "form"
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        textFormat: Text.StyledText
                        color: Palette.textMuted
                        font.family: Palette.fontSans
                        font.pixelSize: 15
                        text: page.invitePreview && page.invitePreview.householdName
                              ? (page.invitePreview.invitedBy || "Someone") + " invited you to <b><font color=\""
                                + Palette.text + "\">" + page.invitePreview.householdName
                                + "</font></b> on Crumb, to share their recipes."
                              : "You've been invited to share a recipe box on Crumb. Use the email address the invite was sent to."
                    }
                }

                // Sign in, sign up and invite share their fields
                ColumnLayout {
                    visible: page.view !== "server" && !page.checkEmail
                             && (page.view === "login" || page.view === "signup" || page.view === "setup"
                                 || (page.view === "invite" && page.inviteStage === "form")
                                 || (page.view === "reset" && page.emailSent === ""))
                    Layout.fillWidth: true
                    spacing: 16

                    RowLayout {
                        visible: page.view === "invite"
                        Layout.fillWidth: true
                        spacing: 8

                        CrumbButton {
                            Layout.fillWidth: true
                            kind: page.inviteTab === "new" ? "soft" : "ghost"
                            text: "I'm new here"
                            onClicked: page.inviteTab = "new"
                        }

                        CrumbButton {
                            Layout.fillWidth: true
                            kind: page.inviteTab === "existing" ? "soft" : "ghost"
                            text: "I have an account"
                            onClicked: page.inviteTab = "existing"
                        }
                    }

                    AccountField {
                        id: nameField
                        visible: page.needsName || (page.view === "invite" && page.inviteTab === "new")
                        large: true
                        label: "Your name"
                        maximumLength: 80
                        onAccepted: emailField.input.forceActiveFocus()
                    }

                    AccountField {
                        id: emailField
                        visible: page.needsEmail || page.view === "invite" || page.view === "reset"
                        large: true
                        label: "Email"
                        onAccepted: page.view === "reset" ? page.sendReset() : passwordField.input.forceActiveFocus()
                    }

                    AccountField {
                        id: passwordField
                        visible: page.view !== "reset" && page.view !== "server"
                        large: true
                        label: "Password"
                        echoMode: TextInput.Password
                        hint: (page.needsName || (page.view === "invite" && page.inviteTab === "new")) ? "At least 8 characters." : ""
                        onAccepted: page.view === "invite" ? page.join() : page.submit()
                    }

                    AccountField {
                        id: appPasswordField
                        visible: page.view === "setup" && !!page.status.setupNeedsAppPassword
                        large: true
                        label: "Current app password"
                        echoMode: TextInput.Password
                        hint: "The one this box used before accounts."
                        onAccepted: page.submit()
                    }

                    Text {
                        visible: page.error !== ""
                        text: page.error
                        color: Palette.error
                        font.family: Palette.fontSans
                        font.pixelSize: 14
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }

                    CrumbButton {
                        visible: page.view === "login" || page.view === "signup" || page.view === "setup"
                        kind: "primary"
                        text: page.view === "login" ? "Sign in" : "Create account"
                        iconName: page.working ? "loader-circle" : ""
                        Layout.fillWidth: true
                        Layout.preferredHeight: 56
                        enabled: page.canSubmit
                        onClicked: page.submit()
                    }

                    CrumbButton {
                        visible: page.view === "invite"
                        kind: "primary"
                        text: page.inviteTab === "new" ? "Create account and join" : "Sign in and join"
                        iconName: page.working ? "loader-circle" : ""
                        Layout.fillWidth: true
                        Layout.preferredHeight: 56
                        enabled: page.canJoin
                        onClicked: page.join()
                    }

                    CrumbButton {
                        visible: page.view === "reset" && page.emailSent === ""
                        kind: "primary"
                        text: "Send the link"
                        iconName: page.working ? "loader-circle" : ""
                        Layout.fillWidth: true
                        Layout.preferredHeight: 56
                        enabled: !page.working && emailField.text.trim() !== ""
                        onClicked: page.sendReset()
                    }
                }

                // Forgot your password (hosted)
                Text {
                    visible: page.view === "login" && page.hosted && !page.checkEmail
                    Layout.alignment: Qt.AlignHCenter
                    textFormat: Text.StyledText
                    text: "<b><a href=\"reset\">Forgot your password?</a></b>"
                    color: Palette.text
                    linkColor: Palette.primary
                    font.family: Palette.fontSans
                    font.pixelSize: 15
                    onLinkActivated: page.show("reset")
                }

                // Password reset
                ColumnLayout {
                    visible: page.view === "reset" && page.emailSent !== ""
                    Layout.fillWidth: true
                    spacing: 12

                    Body {
                        text: "Check your email"
                        bold: true
                        Layout.fillWidth: true
                    }

                    Body {
                        text: "If " + page.emailSent + " has an account, a link to choose a new password is on its way."
                        muted: true
                        Layout.fillWidth: true
                    }
                }

                Body {
                    visible: page.view === "reset" && page.emailSent === ""
                    text: "Enter your email and we'll send you a link to choose a new password."
                    muted: true
                    Layout.fillWidth: true
                    Layout.topMargin: -8
                }

                // Where to go next
                Text {
                    visible: page.view === "login" && page.accounts && !page.checkEmail
                    Layout.alignment: Qt.AlignHCenter
                    Layout.topMargin: 8
                    textFormat: Text.StyledText
                    text: (page.status.signupOpen ? "New here? <b><a href=\"signup\">Create an account</a></b><br>" : "")
                          + "Invited to a household? <b><a href=\"invite\">Join with a link</a></b>"
                    horizontalAlignment: Text.AlignHCenter
                    lineHeight: 1.5
                    color: Palette.textMuted
                    linkColor: Palette.primary
                    font.family: Palette.fontSans
                    font.pixelSize: 15
                    onLinkActivated: (link) => page.show(link)
                }

                Text {
                    visible: (page.view === "signup" || page.view === "invite" || page.view === "reset") && !page.checkEmail
                    Layout.alignment: Qt.AlignHCenter
                    Layout.topMargin: 8
                    textFormat: Text.StyledText
                    text: page.view === "signup" ? "Have an account? <b><a href=\"login\">Sign in</a></b>"
                                                 : "<b><a href=\"login\">Back to sign in</a></b>"
                    color: Palette.textMuted
                    linkColor: Palette.primary
                    font.family: Palette.fontSans
                    font.pixelSize: 15
                    onLinkActivated: page.show("login")
                }

                // The server, and a way to pick another
                Text {
                    visible: page.view !== "server"
                    Layout.alignment: Qt.AlignHCenter
                    Layout.topMargin: 16
                    textFormat: Text.StyledText
                    text: (page.session ? page.session.serverUrl : "") + " · <a href=\"change\">Change server</a>"
                    color: Palette.textMuted
                    linkColor: Palette.primary
                    font.family: Palette.fontSans
                    font.pixelSize: 13
                    onLinkActivated: {
                        page.localError = ""
                        page.connected = false
                        urlField.forceActiveFocus()
                    }
                }
            }
        }
    }
}
