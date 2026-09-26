use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    let builder = CxxQtBuilder::new_qml_module(
        QmlModule::new("app.crumb.desktop")
            .qml_file("qml/Main.qml")
            .qml_file("qml/LoginPage.qml")
            .qml_file("qml/RecipesPage.qml")
            // QColor properties need QtQuick for qmllint/qmlls.
            .depends(["QtQuick", "QtQuick.Controls", "QtQuick.Layouts"]),
    )
    .file("src/palette.rs")
    .file("src/session.rs")
    .file("src/recipes.rs")
    .file("src/smoke.rs");

    // A tiny C++ shim installs a Qt message handler so `--smoke` can fail on any
    // QML warning or error (the Qt API needs a native callback signature).
    let builder = unsafe {
        builder.cc_builder(|cc| {
            cc.file("src/smoke.cpp").include("src");
        })
    };

    builder.build();
}
