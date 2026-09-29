import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// The web's AccountSection.svelte: with accounts, who's signed in and their devices, then
// `between` (the account page's sign-in methods), then their household: members, invites,
// leaving and the other households they're in. Passkeys are left out: they need a browser.
ColumnLayout {
    id: section

    property var session
    property var status: ({})
    default property alias between: betweenSlot.data

    property var devices: []
    property var household: null
    property bool busy: false
    // The row asking to confirm: "remove:<member id>" or "leave"
    property string confirming: ""
    property bool inviting: false
    property string inviteEmail: ""
    // The last link made, to copy (self-hosted invites are only ever links)
    property string madeLink: ""
    property bool renaming: false
    property string newName: ""

    readonly property bool signedIn: status.mode !== "password" && !!status.user
    readonly property bool hosted: status.mode === "hosted"
    readonly property bool owner: !!household && household.role === "owner"
    readonly property var others: {
        if (!household)
            return []
        return household.households.filter(function (h) {
            return h.id !== household.id
        })
    }

    Layout.fillWidth: true
    spacing: 32

    function day(secs) {
        return Core.dateLabel(secs * 1000, -new Date(secs * 1000).getTimezoneOffset())
    }

    function toastError(title, error) {
        ApplicationWindow.window.toast({ "title": error || title, "tone": "error" })
    }

    function load() {
        if (!signedIn)
            return
        requests.call("devices", {}, function (list) {
            section.devices = list
        }, function () {
            section.devices = []
        })
        loadHousehold()
    }

    function loadHousehold() {
        requests.call("household", {}, function (h) {
            section.household = h
        }, function () {
            section.household = null
        })
    }

    // Runs a change, then reloads the household; toasts what went wrong.
    function change(op, args, failed, done) {
        busy = true
        requests.call(op, args, function () {
            section.busy = false
            section.confirming = ""
            section.loadHousehold()
            if (done)
                ApplicationWindow.window.toast({ "title": done })
        }, function (error) {
            section.busy = false
            section.toastError(failed, error)
        })
    }

    function signOutDevice(d) {
        busy = true
        requests.call("signOutDevice", { "id": d.id }, function () {
            section.busy = false
            section.load()
        }, function (error) {
            section.busy = false
            section.toastError("Couldn't sign that out", error)
        })
    }

    function signOutOthers() {
        busy = true
        requests.call("signOutOthers", {}, function () {
            section.busy = false
            section.load()
            ApplicationWindow.window.toast({ "title": "Signed out everywhere else" })
        }, function (error) {
            section.busy = false
            section.toastError("Couldn't sign out", error)
        })
    }

    function copyLink(url) {
        clipboard.copy(url)
        ApplicationWindow.window.toast({ "title": "Invite link copied" })
    }

    function invite() {
        busy = true
        var email = inviteEmail.trim()
        requests.call("invite", hosted ? { "email": email } : {}, function (made) {
            section.busy = false
            section.madeLink = made.url
            section.loadHousehold()
            if (section.hosted) {
                ApplicationWindow.window.toast({ "title": "Invite sent to " + email })
                section.inviteEmail = ""
                section.inviting = false
            } else {
                section.copyLink(made.url)
            }
        }, function (error) {
            section.busy = false
            section.toastError("Couldn't make an invite", error)
        })
    }

    function rename() {
        var name = newName.trim()
        if (!name || name === household.name) {
            renaming = false
            return
        }
        change("renameHousehold", { "name": name }, "Couldn't rename the household")
        renaming = false
    }

    // Leaving or switching changes whose recipes every page shows: start over from home.
    function leave() {
        busy = true
        var name = household.name
        requests.call("leaveHousehold", { "id": household.id }, function () {
            section.busy = false
            section.session.refreshStatus()
            ApplicationWindow.window.go("home", {}, true)
            ApplicationWindow.window.toast({ "title": "You left " + name })
        }, function (error) {
            section.busy = false
            section.toastError("Couldn't leave", error)
        })
    }

    function switchTo(h) {
        busy = true
        requests.call("switchHousehold", { "id": h.id }, function () {
            section.busy = false
            section.session.refreshStatus()
            ApplicationWindow.window.go("home", {}, true)
            ApplicationWindow.window.toast({ "title": "Now in " + h.name })
        }, function (error) {
            section.busy = false
            section.toastError("Couldn't switch", error)
        })
    }

    function inviteMeta(i) {
        var who = i.email || (i.createdBy ? "Link made by " + i.createdBy : "Invite link")
        return who + " · until " + day(i.expiresAt)
    }

    onSignedInChanged: load()
    Component.onCompleted: load()

    Requests {
        id: requests
    }

    RecipeClipboard {
        id: clipboard
    }

    // Account
    ColumnLayout {
        visible: section.signedIn
        Layout.fillWidth: true
        spacing: 0

        AccountHeading {
            text: "Account"
        }

        AccountList {
            AccountRow {
                iconName: "user-round"
                title: section.status.user ? section.status.user.name : ""
                subtitle: section.status.user ? section.status.user.email : ""
            }

            Repeater {
                model: section.devices

                AccountRow {
                    required property var modelData

                    iconName: "monitor-smartphone"
                    title: Core.deviceName(modelData.userAgent || "")
                    subtitle: modelData.current ? "This device" : "Last used " + section.day(modelData.lastSeenAt)

                    CrumbButton {
                        visible: !modelData.current
                        kind: "ghost"
                        iconName: "log-out"
                        enabled: !section.busy
                        ToolTip.visible: hovered
                        ToolTip.text: "Sign out"
                        Accessible.name: "Sign out " + Core.deviceName(modelData.userAgent || "")
                        onClicked: section.signOutDevice(modelData)
                    }
                }
            }

            AccountRow {
                visible: section.devices.length > 1
                iconName: "log-out"
                title: "Sign out everywhere else"
                clickable: true
                enabled: !section.busy
                onClicked: section.signOutOthers()
            }
        }
    }

    ColumnLayout {
        id: betweenSlot
        visible: section.signedIn
        Layout.fillWidth: true
        spacing: 0
    }

    // Household
    ColumnLayout {
        visible: section.signedIn && !!section.household
        Layout.fillWidth: true
        spacing: 0

        AccountHeading {
            text: "Household"
        }

        AccountList {
            AccountRow {
                visible: !section.renaming
                iconName: "house"
                title: section.household ? section.household.name : ""
                subtitle: !section.household ? "" : section.household.members.length === 1
                          ? "Just you" : section.household.members.length + " people share these recipes"

                CrumbButton {
                    visible: section.owner
                    kind: "ghost"
                    iconName: "pencil"
                    ToolTip.visible: hovered
                    ToolTip.text: "Rename"
                    Accessible.name: "Rename the household"
                    onClicked: {
                        section.newName = section.household.name
                        section.renaming = true
                    }
                }
            }

            AccountFormRow {
                visible: section.renaming
                iconName: "house"

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    StyledField {
                        id: nameField
                        Layout.fillWidth: true
                        maximumLength: 80
                        text: section.newName
                        onTextChanged: section.newName = text
                        onVisibleChanged: if (visible) forceActiveFocus()
                        Accessible.name: "Household name"
                        onAccepted: section.rename()
                    }

                    CrumbButton {
                        kind: "ghost"
                        text: "Save"
                        enabled: !section.busy && section.newName.trim() !== ""
                        onClicked: section.rename()
                    }
                }
            }

            Repeater {
                model: section.household ? section.household.members : []

                AccountRow {
                    id: member

                    required property var modelData
                    readonly property bool removable: section.owner && !modelData.you && modelData.role !== "owner"

                    iconName: "user-round"
                    title: modelData.you ? modelData.name + " (you)" : modelData.name
                    subtitle: (modelData.role === "owner" ? "Owner" : "Member") + " · " + modelData.email
                    confirming: section.confirming === "remove:" + modelData.id
                    confirmText: "They lose these recipes. Their account stays."
                    dangerLabel: "Remove"
                    busy: section.busy
                    onKept: section.confirming = ""
                    onConfirmed: section.change("removeMember", { "id": modelData.id },
                                                "Couldn't remove them", modelData.name + " was removed")

                    CrumbButton {
                        visible: member.removable
                        kind: "ghost"
                        iconName: "user-minus"
                        ToolTip.visible: hovered
                        ToolTip.text: "Remove"
                        Accessible.name: "Remove " + member.modelData.name + " from the household"
                        onClicked: section.confirming = "remove:" + member.modelData.id
                    }
                }
            }

            Repeater {
                model: section.owner ? section.household.invites : []

                AccountRow {
                    required property var modelData

                    iconName: modelData.email ? "mail" : "link"
                    title: "Invited"
                    subtitle: section.inviteMeta(modelData)

                    CrumbButton {
                        kind: "ghost"
                        text: "Cancel"
                        enabled: !section.busy
                        onClicked: section.change("cancelInvite", { "id": modelData.id },
                                                  "Couldn't cancel the invite", "Invite cancelled")
                    }
                }
            }

            AccountRow {
                visible: section.owner && section.madeLink !== ""
                iconName: "link"
                title: "New invite link"
                subtitle: section.hosted ? "Only works for the address it went to."
                                         : "Works once, for a week. Send it to who you're inviting."

                CrumbButton {
                    kind: "ghost"
                    iconName: "copy"
                    ToolTip.visible: hovered
                    ToolTip.text: "Copy link"
                    Accessible.name: "Copy the invite link"
                    onClicked: section.copyLink(section.madeLink)
                }
            }

            AccountFormRow {
                visible: section.owner && section.hosted && section.inviting
                iconName: "user-plus"

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    StyledField {
                        Layout.fillWidth: true
                        placeholderText: "Their email"
                        text: section.inviteEmail
                        onTextChanged: section.inviteEmail = text
                        onVisibleChanged: if (visible) forceActiveFocus()
                        Accessible.name: "Their email"
                        onAccepted: if (section.inviteEmail.trim() !== "") section.invite()
                    }

                    CrumbButton {
                        kind: "ghost"
                        text: "Send"
                        enabled: !section.busy && section.inviteEmail.trim() !== ""
                        onClicked: section.invite()
                    }
                }
            }

            AccountRow {
                visible: section.owner && !(section.hosted && section.inviting)
                iconName: "user-plus"
                title: "Invite someone"
                subtitle: section.hosted ? "By email" : "Makes a link to copy and send"
                clickable: true
                enabled: !section.busy
                onClicked: section.hosted ? section.inviting = true : section.invite()
            }

            Repeater {
                model: section.others

                AccountRow {
                    required property var modelData

                    iconName: "arrow-left-right"
                    title: "Switch to " + modelData.name
                    subtitle: "Another household you're in"
                    clickable: true
                    enabled: !section.busy
                    onClicked: section.switchTo(modelData)
                }
            }

            AccountRow {
                visible: !section.owner
                iconName: "door-open"
                title: "Leave household"
                clickable: !confirming
                confirming: section.confirming === "leave"
                confirmInline: true
                confirmText: section.household ? "You'll stop seeing " + section.household.name + "'s recipes." : ""
                keepLabel: "Stay"
                dangerLabel: "Leave"
                busy: section.busy
                onClicked: section.confirming = "leave"
                onKept: section.confirming = ""
                onConfirmed: section.leave()
            }
        }
    }
}
