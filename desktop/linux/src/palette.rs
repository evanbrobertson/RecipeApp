//! The active palette, mirroring Crumb's colour roles in QML: `Palette.bg`, `Palette.onButter`.
//!
//! The singleton owns the Omarchy theme watcher. It loads the active theme at construction,
//! then watches `current/` and applies any change on the Qt thread (so every property change
//! emits its signal and QML rebinds).
//!
//! This is a Rust `QML_SINGLETON` rather than `Palette.qml` on purpose. QML reserves
//! any `on<Capital>…` binding for signal handlers, so a QML file cannot declare both a
//! `tile` property and an `onTile` one (nor `butter` and `onButter`) — the runtime
//! compiler rejects it. A C++/Rust singleton exposes the exact same token names safely.

use std::path::PathBuf;

use core::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QColor, QString};

use crate::theme::{self, Rgb, Theme};
use crate::theme_watch::{self, ThemeWatch};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qcolor.h");
        type QColor = cxx_qt_lib::QColor;

        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
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
        #[qproperty(bool, dark)]
        #[qproperty(QString, font_sans, cxx_name = "fontSans")]
        #[qproperty(QString, font_serif, cxx_name = "fontSerif")]
        #[qproperty(QString, font_hand, cxx_name = "fontHand")]
        type Palette = super::PaletteRust;
    }

    impl cxx_qt::Threading for Palette {}
    impl cxx_qt::Initialize for Palette {}
}

fn rgb(hex: u32) -> QColor {
    QColor::from_rgb(
        ((hex >> 16) & 0xff) as i32,
        ((hex >> 8) & 0xff) as i32,
        (hex & 0xff) as i32,
    )
}

/// Builds a Qt colour from a theme role.
fn qcolor(color: Rgb) -> QColor {
    rgb(((color.r as u32) << 16) | ((color.g as u32) << 8) | color.b as u32)
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
    dark: bool,
    font_sans: QString,
    font_serif: QString,
    font_hand: QString,
    /// The last applied theme, so redundant signals are skipped.
    theme: Theme,
    /// Kept alive for the lifetime of the singleton.
    watch: Option<ThemeWatch>,
}

impl Default for PaletteRust {
    fn default() -> Self {
        let theme = Theme::crumb_dark();
        let fonts = crate::fonts::families();
        Self {
            bg: qcolor(theme.bg),
            paper: qcolor(theme.paper),
            tint: qcolor(theme.tint),
            text: qcolor(theme.text),
            text_muted: qcolor(theme.text_muted),
            line: qcolor(theme.line),
            primary: qcolor(theme.primary),
            tile: qcolor(theme.tile),
            on_tile_color: qcolor(theme.on_tile),
            butter: qcolor(theme.butter),
            on_butter_color: qcolor(theme.on_butter),
            nav: qcolor(theme.nav),
            error: qcolor(theme.error),
            dark: theme.dark,
            font_sans: QString::from(fonts.sans()),
            font_serif: QString::from(fonts.serif()),
            font_hand: QString::from(fonts.hand()),
            theme,
            watch: None,
        }
    }
}

/// Crumb's built-in palette for the given desktop preference.
fn builtin(preferred_dark: bool) -> Theme {
    if preferred_dark {
        Theme::crumb_dark()
    } else {
        Theme::crumb_light()
    }
}

impl qobject::Palette {
    /// Applies a theme, emitting every property's change signal. A no-op when unchanged.
    pub fn apply(mut self: Pin<&mut Self>, theme: &Theme) {
        if self.theme == *theme {
            return;
        }
        self.as_mut().set_bg(qcolor(theme.bg));
        self.as_mut().set_paper(qcolor(theme.paper));
        self.as_mut().set_tint(qcolor(theme.tint));
        self.as_mut().set_text(qcolor(theme.text));
        self.as_mut().set_text_muted(qcolor(theme.text_muted));
        self.as_mut().set_line(qcolor(theme.line));
        self.as_mut().set_primary(qcolor(theme.primary));
        self.as_mut().set_tile(qcolor(theme.tile));
        self.as_mut().set_on_tile_color(qcolor(theme.on_tile));
        self.as_mut().set_butter(qcolor(theme.butter));
        self.as_mut().set_on_butter_color(qcolor(theme.on_butter));
        self.as_mut().set_nav(qcolor(theme.nav));
        self.as_mut().set_error(qcolor(theme.error));
        self.as_mut().set_dark(theme.dark);
        self.as_mut().rust_mut().theme = *theme;
    }
}

impl cxx_qt::Initialize for qobject::Palette {
    fn initialize(mut self: Pin<&mut Self>) {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from);
        let preferred_dark = crate::fonts::preferred_dark();

        let initial = theme::load_omarchy(home.as_deref(), state.as_deref())
            .unwrap_or_else(|| builtin(preferred_dark));
        self.as_mut().apply(&initial);

        let Some(current) = theme::current_dir(home.as_deref(), state.as_deref()) else {
            return;
        };

        let qt_thread = self.as_mut().qt_thread();
        match theme_watch::watch(&current, move |reloaded| {
            let theme = reloaded.unwrap_or_else(|| builtin(preferred_dark));
            let _ = qt_thread.queue(move |mut object| {
                object.as_mut().apply(&theme);
            });
        }) {
            Ok(watch) => self.as_mut().rust_mut().watch = Some(watch),
            Err(err) => eprintln!("crumb-desktop: couldn't watch the Omarchy theme: {err}"),
        }
    }
}
