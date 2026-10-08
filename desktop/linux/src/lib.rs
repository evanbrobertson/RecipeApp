//! Crumb's native Linux desktop app: a Qt6/QML shell around `crumb-client`.
//!
//! The crate is a library so cargo's integration tests (and the `--smoke` test) can link
//! the cxx-qt generated symbols; `main.rs` is a thin wrapper calling [`run`].

mod api;
mod chime;
pub mod config;
mod core_bridge;
mod fonts;
mod native;
mod network;
mod palette;
mod runtime;
mod session;
mod smoke;
mod store;
#[cfg(test)]
mod test_server;
pub mod theme;
mod theme_watch;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQuickStyle, QString, QUrl};

/// The QML root, compiled into the binary's `app.crumb.desktop` module.
const MAIN_QML: &str = "qrc:/qt/qml/app/crumb/desktop/qml/Main.qml";

/// Starts the app (or runs the smoke check when `--smoke` is passed).
pub fn run() {
    // The Basic style is the neutral base the Green Tile palette sits on.
    QQuickStyle::set_style(&QString::from("Basic"));

    let smoke_mode = smoke::enabled();

    let mut app = QGuiApplication::new();
    if let Some(app) = app.as_mut() {
        app.set_application_name(&QString::from("Crumb"));
    }
    QGuiApplication::set_desktop_file_name(&QString::from("crumb-desktop"));

    if smoke_mode {
        smoke::install_handler();
    }

    // Register the bundled fonts before the QML engine builds any text.
    let fonts_ok = app.as_mut().is_some_and(fonts::install);

    static ROOT_PRESENT: AtomicBool = AtomicBool::new(false);
    static ENGINE_FAILED: AtomicBool = AtomicBool::new(false);

    let mut engine = QQmlApplicationEngine::new();
    if smoke_mode && let Some(mut engine) = engine.as_mut() {
        engine
            .as_mut()
            .on_object_created(|_, object, _| {
                if object.is_null() {
                    ENGINE_FAILED.store(true, Ordering::SeqCst);
                } else {
                    ROOT_PRESENT.store(true, Ordering::SeqCst);
                }
            })
            .release();
        engine
            .as_mut()
            .on_object_creation_failed(|_, _| {
                ENGINE_FAILED.store(true, Ordering::SeqCst);
            })
            .release();
    }
    if let Some(mut engine) = engine.as_mut() {
        // Smoke runs keep no photos on disk
        let photo_cache = if smoke_mode {
            QString::default()
        } else {
            QString::from(&*config::photo_cache_dir().to_string_lossy())
        };
        native::install_network(engine.as_mut(), &photo_cache);
        engine.load(&QUrl::from(MAIN_QML));
    }

    // A smoke page with `--shot` saves frames over a few seconds, then quits from QML
    if smoke_mode && smoke::shot().is_none() {
        // Pump the loop briefly so the engine can build the scene and flush bindings.
        let deadline = Instant::now() + Duration::from_millis(1000);
        while Instant::now() < deadline {
            if let Some(app) = app.as_ref() {
                app.process_events();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let failed = ENGINE_FAILED.load(Ordering::SeqCst)
            || !ROOT_PRESENT.load(Ordering::SeqCst)
            || !fonts_ok
            || smoke::failed();
        // Engine first, then the app: exiting with them alive leaves Qt's exit handlers to
        // deliver pending deletions to half-destroyed delegates (a double free now and then)
        drop(engine);
        drop(app);
        if failed {
            eprintln!("crumb-desktop: smoke test failed");
            std::process::exit(1);
        }
        std::process::exit(0);
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
    if smoke_mode && smoke::failed() {
        eprintln!("crumb-desktop: smoke test failed");
        std::process::exit(1);
    }
}
