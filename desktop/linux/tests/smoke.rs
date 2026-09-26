//! Runs the built binary with `--smoke` under the offscreen platform, so the QML is
//! checked end to end without a display or a server.

use std::process::Command;

#[test]
fn the_qml_shell_loads_offscreen() {
    let exe = env!("CARGO_BIN_EXE_crumb-desktop");
    // Keep the run away from the user's real settings and session.
    let config = tempfile::tempdir().unwrap();
    let output = Command::new(exe)
        .arg("--smoke")
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("CRUMB_SERVER", "http://127.0.0.1:9")
        .env("XDG_CONFIG_HOME", config.path())
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        // A GTK platform theme without a display makes GTK abort the process.
        .env_remove("QT_QPA_PLATFORMTHEME")
        .output()
        .expect("failed to run crumb-desktop");

    assert!(
        output.status.success(),
        "smoke test failed ({:?})\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
