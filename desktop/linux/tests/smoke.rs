//! Runs the built binary with `--smoke` under the offscreen platform, so the QML is
//! checked end to end without a display or a server.

use std::path::Path;
use std::process::{Command, Output};

/// Runs the binary with `--smoke` (plus `args`) against a throwaway config dir.
///
/// `server` is `None` to leave `CRUMB_SERVER` unset: with an empty config dir the app starts
/// in the "setup" state, which is what the login-page check needs.
fn smoke(args: &[&str], server: Option<&str>, config: &Path) -> Output {
    let exe = env!("CARGO_BIN_EXE_crumb-desktop");
    let mut command = Command::new(exe);
    command
        .arg("--smoke")
        .args(args)
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("XDG_CONFIG_HOME", config)
        .env_remove("CRUMB_SERVER")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        // A GTK platform theme without a display makes GTK abort the process.
        .env_remove("QT_QPA_PLATFORMTHEME");
    if let Some(server) = server {
        command.env("CRUMB_SERVER", server);
    }
    command.output().expect("failed to run crumb-desktop")
}

fn assert_smoke_passes(output: &Output) {
    assert!(
        output.status.success(),
        "smoke test failed ({:?})\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn the_qml_shell_loads_offscreen() {
    let config = tempfile::tempdir().unwrap();
    let output = smoke(&[], Some("http://127.0.0.1:9"), config.path());
    assert_smoke_passes(&output);
}

/// The regression guard for `LoginPage { session: session }`: with no server configured the
/// page's `session` must be bound, so typing a URL enables the submit button.
#[test]
fn the_login_page_binds_its_session_offscreen() {
    let config = tempfile::tempdir().unwrap();
    let output = smoke(&[], None, config.path());
    assert_smoke_passes(&output);
}

/// `--smoke-page recipe` renders the detail page from a built-in fixture, no network.
#[test]
fn the_recipe_page_loads_offscreen() {
    let config = tempfile::tempdir().unwrap();
    let output = smoke(&["--smoke-page", "recipe"], None, config.path());
    assert_smoke_passes(&output);
}
