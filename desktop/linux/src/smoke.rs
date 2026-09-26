//! The `--smoke` support shim: a thin Rust binding to `src/smoke.cpp`.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("smoke.h");

        #[rust_name = "install_handler"]
        fn crumbSmokeInstallHandler();

        #[rust_name = "failed"]
        fn crumbSmokeFailed() -> bool;
    }
}

pub use qobject::{failed, install_handler};
