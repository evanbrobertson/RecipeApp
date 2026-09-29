import QtQuick

// A lucide icon (`assets/icons/<name>.svg`) in a palette colour, as the web's `Icon`.
Image {
    id: icon
    property string name
    property color color: "black"
    property int size: 20

    width: size
    height: size
    sourceSize.width: Math.ceil(size * Screen.devicePixelRatio)
    sourceSize.height: Math.ceil(size * Screen.devicePixelRatio)
    source: name ? "image://icon/" + name + "/" + encodeURIComponent(String(color)) : ""
    fillMode: Image.PreserveAspectFit
    smooth: true
    cache: true
}
