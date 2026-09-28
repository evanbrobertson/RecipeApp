//! Rust bindings to `src/native.cpp` (fonts and colour scheme).

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("native.h");

        #[rust_name = "add_application_font"]
        fn crumbAddApplicationFont(path: &QString) -> i32;

        #[rust_name = "application_font_family"]
        fn crumbApplicationFontFamily(id: i32) -> QString;

        #[rust_name = "prefers_dark"]
        fn crumbPrefersDark() -> bool;

        #[rust_name = "set_clipboard_text"]
        fn crumbSetClipboardText(text: &QString);

        #[rust_name = "open_url"]
        fn crumbOpenUrl(url: &QString) -> bool;
    }
}

pub use qobject::{
    add_application_font, application_font_family, open_url, prefers_dark, set_clipboard_text,
};
