//! The dark "Green Tile" palette, mirroring the `.dark` tokens in
//! `web/src/styles/app.css`. QML reads it as a singleton: `Palette.bg`, `Palette.onButter`.
//!
//! This is a Rust `QML_SINGLETON` rather than `Palette.qml` on purpose. QML reserves
//! any `on<Capital>…` binding for signal handlers, so a QML file cannot declare both a
//! `tile` property and an `onTile` one (nor `butter` and `onButter`) — the runtime
//! compiler rejects it. A C++/Rust singleton exposes the exact same token names safely.

use cxx_qt_lib::QColor;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qcolor.h");
        type QColor = cxx_qt_lib::QColor;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(QColor, bg)]
        #[qproperty(QColor, paper)]
        #[qproperty(QColor, tint)]
        #[qproperty(QColor, text)]
        #[qproperty(QColor, text_muted, cxx_name = "textMuted")]
        #[qproperty(QColor, line)]
        #[qproperty(QColor, primary)]
        #[qproperty(QColor, tile)]
        #[qproperty(QColor, on_tile_color, cxx_name = "onTile")]
        #[qproperty(QColor, butter)]
        #[qproperty(QColor, on_butter_color, cxx_name = "onButter")]
        #[qproperty(QColor, nav)]
        #[qproperty(QColor, error)]
        type Palette = super::PaletteRust;
    }
}

fn rgb(hex: u32) -> QColor {
    QColor::from_rgb(
        ((hex >> 16) & 0xff) as i32,
        ((hex >> 8) & 0xff) as i32,
        (hex & 0xff) as i32,
    )
}

/// The inner Rust struct behind the `Palette` singleton.
pub struct PaletteRust {
    bg: QColor,
    paper: QColor,
    tint: QColor,
    text: QColor,
    text_muted: QColor,
    line: QColor,
    primary: QColor,
    tile: QColor,
    on_tile_color: QColor,
    butter: QColor,
    on_butter_color: QColor,
    nav: QColor,
    error: QColor,
}

impl Default for PaletteRust {
    fn default() -> Self {
        Self {
            bg: rgb(0x141c17),
            paper: rgb(0x1d2721),
            tint: rgb(0x24322a),
            text: rgb(0xefe9da),
            text_muted: rgb(0xa8b3aa),
            line: rgb(0x2c3830),
            primary: rgb(0x93c4a3),
            tile: rgb(0x3a7859),
            on_tile_color: rgb(0xf5f0e2),
            butter: rgb(0xf0d582),
            on_butter_color: rgb(0x141c17),
            nav: rgb(0x0d130f),
            error: rgb(0xe59a83),
        }
    }
}
