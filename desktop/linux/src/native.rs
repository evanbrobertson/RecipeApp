//! Rust bindings to `src/native.cpp` (fonts, colour scheme, and the photo loader's session
//! and icons).

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qqmlapplicationengine.h");
        type QQmlApplicationEngine = cxx_qt_lib::QQmlApplicationEngine;

        include!("native.h");

        #[rust_name = "add_application_font"]
        fn crumbAddApplicationFont(path: &QString) -> i32;

        #[rust_name = "application_font_family"]
        fn crumbApplicationFontFamily(id: i32) -> QString;

        #[rust_name = "prefers_dark"]
        fn crumbPrefersDark() -> bool;

        #[rust_name = "install_network"]
        fn crumbInstallNetwork(engine: Pin<&mut QQmlApplicationEngine>);

        #[rust_name = "set_photo_session"]
        fn crumbSetPhotoSession(origin: &QString, cookie: &QString);
    }
}

pub use qobject::{
    add_application_font, application_font_family, install_network, prefers_dark, set_photo_session,
};
