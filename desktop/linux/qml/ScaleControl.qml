import QtQuick
import QtQuick.Layouts

import app.crumb.desktop 1.0

// STUB, owned by the recipe-page agent (web ScaleControl.svelte): the ×½ … ×4 scale with
// − and +. Keep this interface: `value` (1 = as written) and `edited(value)`.
RowLayout {
    id: control
    property real value: 1
    signal edited(real value)

    Body {
        text: "×" + control.value
    }
}
