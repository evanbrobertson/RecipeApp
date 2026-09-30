import QtQuick

// The photos shown most recently, held decoded so a page that comes back (Home after a
// recipe) shows them at once instead of the tint. Every navigation rebuilds the page, and Qt
// keeps only 2 MB of images nothing shows, dropping a quarter every 30 seconds. A kept Image
// shares the shown one's decoded copy (same source, width and fill), so it costs nothing
// extra while that one is on screen. Newest first, up to `budget` bytes.
Item {
    id: keep
    visible: false

    property real budget: 64 * 1024 * 1024
    property real used: 0

    function hold(url, width, bytes) {
        for (var i = 0; i < kept.count; i++) {
            var entry = kept.get(i)
            if (entry.photoUrl === url && entry.photoWidth === width) {
                if (i > 0)
                    kept.move(i, 0, 1)
                return
            }
        }
        kept.insert(0, { "photoUrl": url, "photoWidth": width, "bytes": bytes })
        used += bytes
        while (used > budget && kept.count > 1) {
            used -= kept.get(kept.count - 1).bytes
            kept.remove(kept.count - 1)
        }
    }

    function clear() {
        kept.clear()
        used = 0
    }

    ListModel {
        id: kept
    }

    Repeater {
        model: kept

        Image {
            required property string photoUrl
            required property int photoWidth
            asynchronous: true
            fillMode: Image.PreserveAspectCrop
            sourceSize.width: photoWidth
            source: photoUrl
        }
    }
}
