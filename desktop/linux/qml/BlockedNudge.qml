import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import app.crumb.desktop 1.0

// Shown instead of an error when a recipe site's bot check turned Crumb's server away
// (`site_blocked`), or the site's terms ask for no automated copying (`site_terms`), as the
// web's components/BlockedNudge.svelte. The Crumb browser extension reads the page from the
// cook's own browser; this app can't hold an extension, so the main action opens the recipe
// in the browser, where the extension's toolbar button reads it.
Card {
    id: nudge

    // The recipe page that couldn't be read
    property string url
    // "bot": the site's bot check turned Crumb away. "terms": its terms ask us not to fetch.
    property string why: "bot"
    // With "terms": the site's name as the server knows it ("Allrecipes"); else its host
    property string name: ""
    // Shows the Dismiss button
    property bool dismissable: false
    property bool opened: false

    readonly property bool terms: why === "terms"
    readonly property string site: name !== "" ? name : (Core.hostOf(url).replace(/^www\./, "") || "This site")
    readonly property string extensionUrl: "https://github.com/evanbrobertson/RecipeApp/releases?q=extension"

    signal paste
    signal dismissed

    onUrlChanged: opened = false

    implicitHeight: column.implicitHeight + 32

    ColumnLayout {
        id: column
        x: 16
        y: 16
        width: parent.width - 32
        spacing: 12

        RowLayout {
            Layout.fillWidth: true
            spacing: 12

            HomeWell {
                iconName: nudge.terms ? "scroll-text" : "shield-check"
                Layout.alignment: Qt.AlignTop
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignTop
                spacing: 4

                Body {
                    text: nudge.terms ? "Their terms ask us not to" : "This site wants a human"
                    bold: true
                    font.pixelSize: 17
                    Layout.fillWidth: true
                    Accessible.role: Accessible.Heading
                }
                Body {
                    muted: true
                    Layout.fillWidth: true
                    text: nudge.opened
                          ? "Opened in your browser. Click Crumb in your browser's toolbar there, and the extension reads the recipe from that page and opens it in Crumb, ready to add."
                          : nudge.terms
                            ? nudge.site + "'s terms don't allow automated copying, so Crumb didn't fetch it. Open it on their site, then click Crumb in your toolbar: the extension reads the recipe from your own browser."
                            : "It asked for a human check, so Crumb couldn't read it from here. Open the recipe, then click Crumb in your toolbar: the extension reads it from your browser, which is already past the check."
                }
            }

            CrumbButton {
                visible: nudge.dismissable
                kind: "ghost"
                small: true
                iconName: "x"
                Layout.alignment: Qt.AlignTop
                Accessible.name: "Dismiss"
                onClicked: nudge.dismissed()
            }
        }

        Flow {
            Layout.fillWidth: true
            spacing: 8

            CrumbButton {
                kind: "primary"
                iconName: "external-link"
                text: "Read it with the extension"
                onClicked: {
                    Qt.openUrlExternally(nudge.url)
                    nudge.opened = true
                }
            }
            CrumbButton {
                kind: "soft"
                iconName: "clipboard-paste"
                text: "Paste the text instead"
                onClicked: nudge.paste()
            }
        }

        Body {
            muted: true
            font.pixelSize: 13
            Layout.fillWidth: true
            textFormat: Text.StyledText
            text: "No extension yet? <a href=\"" + nudge.extensionUrl + "\">Get it</a>, then open the recipe page and click Crumb in your toolbar."
            onLinkActivated: link => Qt.openUrlExternally(link)

            HoverHandler {
                cursorShape: parent.hoveredLink ? Qt.PointingHandCursor : Qt.ArrowCursor
            }
        }
    }
}
