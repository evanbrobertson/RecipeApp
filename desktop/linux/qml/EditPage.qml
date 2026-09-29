import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// Edits a recipe (`edit:<id>`) or starts a new one (`new`, `params.title` from the Add box's
// "From scratch"). Mirrors web islands/EditPage.svelte and NewRecipePage.svelte.
Item {
    id: page

    property var session
    property int routeId: 0
    property var params: ({})

    readonly property bool isNew: routeId <= 0
    readonly property string startTitle: params && params.title ? String(params.title).trim() : ""

    property bool loading: !isNew
    property var recipe: null
    property var flags: []
    property bool saving: false

    function goBack() {
        ApplicationWindow.window.back()
    }

    function openRecipe(id, replace) {
        ApplicationWindow.window.go("recipe", { "id": id }, replace === true)
    }

    function failed(error) {
        ApplicationWindow.window.toast({ "title": "Couldn't save", "description": error, "tone": "error" })
        saving = false
    }

    function save(fields) {
        saving = true
        if (isNew) {
            requests.call("createRecipe", { "fields": fields }, recipe => page.openRecipe(recipe.id, true), page.failed)
        } else {
            requests.call("patchRecipe", { "id": routeId, "patch": fields }, () => {
                ApplicationWindow.window.toast({ "title": "Saved", "tone": "success" })
                page.openRecipe(page.routeId, true)
            }, page.failed)
        }
    }

    // "Keep as is" is remembered, so Wee Chef won't raise that line again
    function dismiss(flag) {
        var before = flags
        flags = flags.filter(f => f.id !== flag.id)
        requests.call("dismissFlag", { "id": routeId, "flag": flag.id }, null, error => {
            page.flags = before
            ApplicationWindow.window.toast({ "title": "Couldn't save that", "description": error, "tone": "error" })
        })
    }

    Component.onCompleted: {
        if (isNew)
            return
        requests.call("recipe", { "id": routeId }, r => {
            page.recipe = r
            page.loading = false
            requests.call("recipeChecks", { "id": page.routeId }, checks => {
                page.flags = checks && checks.flags ? checks.flags : []
            }, () => {})
        }, () => {
            page.loading = false
        })
    }

    Requests {
        id: requests
    }

    ScrollPage {
        anchors.fill: parent
        visible: page.isNew || page.recipe !== null

        ColumnLayout {
            width: parent.width
            spacing: 0

            CrumbButton {
                visible: !page.isNew
                kind: "ghost"
                iconName: "arrow-left"
                text: page.recipe ? page.recipe.title : ""
                Layout.maximumWidth: parent.width
                Layout.leftMargin: -4
                leftPadding: 4
                onClicked: page.openRecipe(page.routeId)
            }

            Heading {
                level: 1
                text: page.isNew ? "New recipe" : "Edit recipe"
                font.pixelSize: 30
                Layout.topMargin: page.isNew ? 0 : 4
                Layout.bottomMargin: 28
                Layout.fillWidth: true
            }

            Loader {
                Layout.fillWidth: true
                active: page.isNew || page.recipe !== null
                sourceComponent: RecipeEditor {
                    initialDraft: page.isNew ? Core.newDraft(page.startTitle) : Core.draftFromRecipe(JSON.stringify(page.recipe))
                    saving: page.saving
                    submitLabel: page.isNew ? "Save recipe" : "Save"
                    flags: page.flags
                    canDismiss: !page.isNew
                    onSaved: fields => page.save(fields)
                    onCancelled: page.isNew ? page.goBack() : page.openRecipe(page.routeId)
                    onDismissed: flag => page.dismiss(flag)
                }
            }
        }
    }

    // The web's skeleton while the recipe loads
    Column {
        visible: page.loading
        x: 32
        y: 32
        width: Math.min(1200, parent.width - 64)
        spacing: 16

        Repeater {
            model: [{ "w": 160, "h": 20 }, { "w": 0.5, "h": 36 }, { "w": 1, "h": 256 }]

            delegate: Rectangle {
                required property var modelData
                width: modelData.w <= 1 ? parent.width * modelData.w : modelData.w
                height: modelData.h
                radius: 12
                color: Palette.tint
            }
        }
    }

    ColumnLayout {
        anchors.centerIn: parent
        visible: !page.isNew && !page.loading && page.recipe === null
        spacing: 10

        EmptyState {
            iconName: "file-question"
            title: "Recipe not found"
            Layout.alignment: Qt.AlignHCenter
        }

        CrumbButton {
            kind: "soft"
            text: "Back to recipes"
            Layout.alignment: Qt.AlignHCenter
            onClicked: ApplicationWindow.window.go("recipes", {}, true)
        }
    }
}
