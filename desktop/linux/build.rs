use std::path::PathBuf;

use cxx_qt_build::{CxxQtBuilder, QResource, QResources, QmlModule};
use qt_build_utils::QResourceFile;

/// The bundled fonts, by file stem, in `web/src/assets/fonts`.
const FONTS: &[&str] = &["nunito-sans", "dm-serif-display", "caveat"];

fn main() {
    let builder = CxxQtBuilder::new_qml_module(
        QmlModule::new("app.crumb.desktop")
            .qml_file("qml/Main.qml")
            .qml_file("qml/LoginPage.qml")
            .qml_file("qml/RecipesPage.qml")
            .qml_file("qml/RecipePage.qml")
            .qml_file("qml/StyledField.qml")
            // QColor properties need QtQuick for qmllint/qmlls.
            .depends([
                "QtQuick",
                "QtQuick.Controls",
                "QtQuick.Layouts",
                "QtQuick.Effects",
            ]),
    )
    .file("src/palette.rs")
    .file("src/session.rs")
    .file("src/recipes.rs")
    .file("src/recipe.rs")
    .file("src/smoke.rs")
    .file("src/native.rs")
    .qrc_resources(resources());

    // Tiny C++ shims: one makes `--smoke` fail on any QML warning, the other wraps the
    // QFontDatabase/QStyleHints calls cxx-qt-lib does not expose.
    let builder = unsafe {
        builder.cc_builder(|cc| {
            cc.file("src/smoke.cpp")
                .file("src/native.cpp")
                .include("src");
        })
    };

    builder.build();
}

/// Bundles the WOFF2 fonts from `web/src/assets/fonts` into `:/fonts`, and the local image
/// the `--smoke-page recipe` fixture uses into `:/img` (so that smoke run stays offline).
///
/// Qt links FreeType with Brotli, so `addApplicationFont` reads WOFF2 directly; there is no
/// decompressed-TTF fallback, which would only make local and CI builds differ.
fn resources() -> QResources {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let fonts = manifest.join("../../web/src/assets/fonts");

    let mut font_resource = QResource::new().prefix("/fonts");
    for stem in FONTS {
        let woff2 = fonts.join(format!("{stem}.woff2"));
        if woff2.is_file() {
            font_resource =
                font_resource.file(QResourceFile::new(&woff2).alias(format!("{stem}.woff2")));
        }
    }

    let fixture = manifest.join("assets/fixture.png");
    let mut resources = QResources::new().resource(font_resource);
    if fixture.is_file() {
        let image_resource = QResource::new()
            .prefix("/img")
            .file(QResourceFile::new(&fixture).alias("fixture.png"));
        resources = resources.resource(image_resource);
    }
    resources
}
