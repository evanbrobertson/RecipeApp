//! The `--smoke` support shim: a thin Rust binding to `src/smoke.cpp`, plus a tiny QML
//! singleton so a page can report its own failure and know which smoke scenario it is in.
//!
//! Two scenarios ride on `--smoke`:
//!
//! * the login page check: with no configured server (state "setup"), QML verifies the
//!   page's `session` is bound and its submit button enables once a URL is typed. This is
//!   the regression guard for the page binding its own `session` property to itself.
//! * `--smoke-page recipe` (or `CRUMB_SMOKE_PAGE=recipe`): loads `RecipePage` with a
//!   built-in fixture, no network, so the detail page's QML is exercised headless.

use std::sync::atomic::{AtomicBool, Ordering};

use core::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

/// Whether this run is a smoke check: `--smoke`, or a named `--smoke-page`.
pub fn enabled() -> bool {
    std::env::args().any(|arg| arg == "--smoke") || page().is_some()
}

/// The page named by `--smoke-page <name>` (or `CRUMB_SMOKE_PAGE`), if any.
pub fn page() -> Option<String> {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == "--smoke-page" {
            return args.next();
        }
        if let Some(value) = arg.strip_prefix("--smoke-page=") {
            return Some(value.to_string());
        }
    }
    std::env::var("CRUMB_SMOKE_PAGE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Records a failed smoke assertion on the Rust side, alongside the Qt message handler.
static FAILED: AtomicBool = AtomicBool::new(false);

/// Whether any Qt warning was seen or any [`fail`] assertion fired.
pub fn failed() -> bool {
    FAILED.load(Ordering::SeqCst) || qobject::native_failed()
}

/// Fails the smoke run, printing why.
pub fn fail(reason: &str) {
    eprintln!("crumb-desktop: smoke check failed: {reason}");
    FAILED.store(true, Ordering::SeqCst);
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("smoke.h");

        #[rust_name = "install_handler"]
        fn crumbSmokeInstallHandler();

        #[rust_name = "native_failed"]
        fn crumbSmokeFailed() -> bool;
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(bool, enabled)]
        #[qproperty(QString, page)]
        type Smoke = super::SmokeRust;

        /// Fails the smoke run with a reason; a no-op outside a smoke run.
        #[qinvokable]
        fn fail(self: Pin<&mut Smoke>, reason: QString);
    }
}

/// The inner Rust struct behind the `Smoke` singleton.
pub struct SmokeRust {
    enabled: bool,
    page: QString,
}

impl Default for SmokeRust {
    fn default() -> Self {
        Self {
            enabled: enabled(),
            page: QString::from(&page().unwrap_or_default()),
        }
    }
}

impl qobject::Smoke {
    pub fn fail(self: Pin<&mut Self>, reason: QString) {
        if self.rust().enabled {
            crate::smoke::fail(&reason.to_string());
        }
    }
}

pub use qobject::install_handler;
