import QtQuick

// Puts text on the clipboard (QML has no direct call): `clipboard.copy("text")`.
TextEdit {
    visible: false
    function copy(text) {
        clipboard.text = text
        clipboard.selectAll()
        clipboard.copy()
    }
    id: clipboard
}
