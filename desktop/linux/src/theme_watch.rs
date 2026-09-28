//! Watches Omarchy's `current/` directory for theme changes. Pure Rust and callback-based,
//! so the same code is unit-testable and usable from `Palette`.
//!
//! `omarchy-theme-set` replaces the whole `current/theme` directory with `mv` and then
//! rewrites `current/theme.name`, so watching `current/` and debouncing a short burst of
//! events is enough.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use notify::{Event, RecursiveMode, Watcher};

use crate::theme::{self, Theme};

/// How long to collect events before re-reading the theme.
const DEBOUNCE: Duration = Duration::from_millis(150);
/// How often a blocked watcher wakes to notice a stop request.
const POLL: Duration = Duration::from_millis(250);

/// A running watcher. Dropping it stops the thread.
pub struct ThemeWatch {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for ThemeWatch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Starts watching `current`; `callback` runs on the watcher thread with the newly loaded
/// theme (`None` when the theme file is gone or invalid, so the caller can fall back).
pub fn watch<F>(current: &Path, callback: F) -> notify::Result<ThemeWatch>
where
    F: Fn(Option<Theme>) + Send + 'static,
{
    let (sender, receiver) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher = notify::recommended_watcher(move |result| {
        let _ = sender.send(result);
    })?;
    watcher.watch(current, RecursiveMode::Recursive)?;

    let root = current.to_path_buf();
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    let handle = thread::spawn(move || {
        // Keep the watcher alive for as long as the thread runs.
        let _watcher = watcher;
        while !thread_stop.load(Ordering::SeqCst) {
            match receiver.recv_timeout(POLL) {
                Ok(Ok(event)) => {
                    if !relevant(&event, &root) {
                        continue;
                    }
                    // Collect the rest of the burst before re-reading.
                    let deadline = Instant::now() + DEBOUNCE;
                    while let Some(now) = deadline.checked_duration_since(Instant::now()) {
                        if receiver.recv_timeout(now).is_ok() {
                            continue;
                        }
                        break;
                    }
                    callback(read(&root));
                }
                Ok(Err(_)) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });

    Ok(ThemeWatch {
        stop,
        handle: Some(handle),
    })
}

/// Re-reads the theme from a `current/` directory.
fn read(current: &Path) -> Option<Theme> {
    let text = std::fs::read_to_string(theme::theme_file(current)).ok()?;
    theme::from_omarchy(&text)
}

/// Only `theme.name` and files under `theme/` matter; the `background` symlink is noise.
fn relevant(event: &Event, current: &Path) -> bool {
    let theme_dir: PathBuf = current.join("theme");
    event.paths.iter().any(|path| {
        path.file_name().is_some_and(|name| name == "theme.name") || path.starts_with(&theme_dir)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRST: &str = "mode = \"dark\"\naccent = \"#3a7859\"\nbackground = \"#141c17\"\n";
    const SECOND: &str = "mode = \"dark\"\naccent = \"#798186\"\nbackground = \"#101315\"\n";

    #[test]
    fn a_theme_change_is_delivered_within_two_seconds() {
        let temp = tempfile::tempdir().unwrap();
        let current = temp.path().join("omarchy/current");
        std::fs::create_dir_all(current.join("theme")).unwrap();
        std::fs::write(current.join("theme/colors.toml"), FIRST).unwrap();
        std::fs::write(current.join("theme.name"), "first").unwrap();

        let (sender, receiver) = mpsc::channel();
        let _watch = watch(&current, move |theme| {
            let _ = sender.send(theme);
        })
        .expect("watcher starts");

        // Let inotify register before the change we care about.
        thread::sleep(Duration::from_millis(200));

        std::fs::write(current.join("theme/colors.toml"), SECOND).unwrap();
        std::fs::write(current.join("theme.name"), "second").unwrap();

        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match receiver.recv_timeout(remaining) {
                Ok(Some(theme)) if theme.tile.hex() == "#798186" => break,
                Ok(_) => continue,
                Err(_) => panic!("timed out waiting for the new mapping"),
            }
        }
    }
}
