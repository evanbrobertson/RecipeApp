import QtQuick
import QtQuick.Effects

import app.crumb.desktop 1.0

// A recipe photo, as the web's Photo.svelte: the server's resized WebP at the width shown,
// 12px corners (or `radius`), a tint while it loads or when there's none.
Item {
    id: photo

    property int recipeId: 0
    property string image: ""
    property int radius: 12
    // Only the top corners, for a card whose photo runs edge to edge.
    property bool topOnly: false
    // The site's own photo, not the server's resized copy: a preview isn't saved, so it isn't
    // in the resizer yet (as the web's preview page shows it).
    property bool direct: false
    readonly property bool loaded: img.status === Image.Ready
    // Masks are shaders: the software scene graph (no GPU, or offscreen screenshots) shows
    // the photo square-cornered instead of not at all.
    readonly property bool shaders: GraphicsInfo.api !== GraphicsInfo.Software

    Rectangle {
        anchors.fill: parent
        color: Palette.tint
        topLeftRadius: photo.radius
        topRightRadius: photo.radius
        bottomLeftRadius: photo.topOnly ? 0 : photo.radius
        bottomRightRadius: photo.topOnly ? 0 : photo.radius
    }

    Icon {
        anchors.centerIn: parent
        visible: !photo.loaded
        name: "cooking-pot"
        size: Math.max(20, Math.min(40, photo.height / 4))
        color: Palette.textMuted
        opacity: 0.6
    }

    Image {
        id: img
        anchors.fill: parent
        visible: !photo.shaders && photo.loaded
        asynchronous: true
        fillMode: Image.PreserveAspectCrop
        sourceSize.width: Math.ceil(photo.width * Screen.devicePixelRatio)
        source: photo.image && photo.width > 0
                ? (photo.image.indexOf("qrc:") === 0 || photo.direct ? photo.image
                   : Api.photoUrl(photo.recipeId, Math.ceil(photo.width * Screen.devicePixelRatio), photo.image))
                : ""
        layer.enabled: photo.shaders
    }

    Rectangle {
        id: mask
        anchors.fill: parent
        visible: false
        layer.enabled: true
        color: "black"
        topLeftRadius: photo.radius
        topRightRadius: photo.radius
        bottomLeftRadius: photo.topOnly ? 0 : photo.radius
        bottomRightRadius: photo.topOnly ? 0 : photo.radius
    }

    MultiEffect {
        anchors.fill: parent
        visible: photo.shaders && photo.loaded
        source: img
        maskEnabled: true
        maskSource: mask
    }
}
