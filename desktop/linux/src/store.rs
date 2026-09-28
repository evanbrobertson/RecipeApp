//! What the web keeps in the browser's storage, for the desktop: `Store.read(key, fallback)`
//! and `Store.write(key, json)` from QML, with the web's own keys (`crumb:recent`,
//! `crumb:timers`, `crumb:scale:7`, ...).
//!
//! Like the browser, each server gets its own store. `local` values last (a JSON file in the
//! app's state folder); `session` values last until the app quits, as a tab's
//! `sessionStorage` does. Nothing secret goes here: the session cookie is in the keyring.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use core::pin::Pin;

use cxx_qt_lib::QString;
use serde_json::{Map, Value};

use crate::session::app_core;

/// Where local values are saved: `$XDG_STATE_HOME/crumb-desktop/store.json`.
fn state_file() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("crumb-desktop/store.json")
}

/// Both kinds of storage, by server, then by key.
#[derive(Default)]
pub struct Stores {
    path: Option<PathBuf>,
    local: Map<String, Value>,
    session: HashMap<String, HashMap<String, Value>>,
}

impl Stores {
    /// Local values from `path` (nothing when it's missing or unreadable).
    pub fn open(path: &Path) -> Self {
        let local = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<Map<String, Value>>(&text).ok())
            .unwrap_or_default();
        Self {
            path: Some(path.to_path_buf()),
            local,
            session: HashMap::new(),
        }
    }

    pub fn read(&self, server: &str, key: &str, session: bool) -> Option<&Value> {
        if session {
            self.session.get(server)?.get(key)
        } else {
            self.local.get(server)?.get(key)
        }
    }

    /// Sets a value (`Null` removes it) and saves local values.
    pub fn write(&mut self, server: &str, key: &str, value: Value, session: bool) {
        if session {
            let store = self.session.entry(server.to_string()).or_default();
            if value.is_null() {
                store.remove(key);
            } else {
                store.insert(key.to_string(), value);
            }
            return;
        }
        let store = self
            .local
            .entry(server.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(map) = store.as_object_mut() {
            if value.is_null() {
                map.remove(key);
            } else {
                map.insert(key.to_string(), value);
            }
        }
        self.save();
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let text = serde_json::to_string(&self.local).unwrap_or_default();
        // Write then rename, so a crash never leaves half a file
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }
}

fn stores() -> &'static Mutex<Stores> {
    static STORES: OnceLock<Mutex<Stores>> = OnceLock::new();
    STORES.get_or_init(|| Mutex::new(Stores::open(&state_file())))
}

/// The signed-in server, which keys the store.
fn server() -> String {
    app_core()
        .try_lock()
        .map(|core| core.server().to_string())
        .unwrap_or_default()
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        type Store = super::StoreRust;

        /// A stored value changed (so, for example, every page's timer dock updates).
        #[qsignal]
        fn changed(self: Pin<&mut Store>, key: QString);

        /// The JSON stored at `key`, or `fallback` (JSON) when there's none.
        #[qinvokable]
        fn read(self: &Store, key: QString, fallback: QString) -> QString;

        /// Stores `json` at `key`; "null" removes it.
        #[qinvokable]
        fn write(self: Pin<&mut Store>, key: QString, json: QString);

        /// Like `read`, but gone when the app quits (the web's `sessionStorage`).
        #[qinvokable]
        #[cxx_name = "readSession"]
        fn read_session(self: &Store, key: QString, fallback: QString) -> QString;

        #[qinvokable]
        #[cxx_name = "writeSession"]
        fn write_session(self: Pin<&mut Store>, key: QString, json: QString);
    }
}

#[derive(Default)]
pub struct StoreRust;

fn read(key: &QString, fallback: QString, session: bool) -> QString {
    let stores = stores().lock().unwrap();
    match stores.read(&server(), &key.to_string(), session) {
        Some(value) => QString::from(&value.to_string()),
        None => fallback,
    }
}

fn write(key: &QString, json: &QString, session: bool) {
    let value = serde_json::from_str(&json.to_string()).unwrap_or(Value::Null);
    stores()
        .lock()
        .unwrap()
        .write(&server(), &key.to_string(), value, session);
}

impl qobject::Store {
    pub fn read(&self, key: QString, fallback: QString) -> QString {
        read(&key, fallback, false)
    }

    pub fn write(self: Pin<&mut Self>, key: QString, json: QString) {
        write(&key, &json, false);
        self.changed(key);
    }

    pub fn read_session(&self, key: QString, fallback: QString) -> QString {
        read(&key, fallback, true)
    }

    pub fn write_session(self: Pin<&mut Self>, key: QString, json: QString) {
        write(&key, &json, true);
        self.changed(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn values_are_kept_per_server_and_local_ones_last() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state/store.json");
        let mut stores = Stores::open(&path);
        stores.write("https://a", "crumb:recent", json!([1, 2]), false);
        stores.write("https://a", "crumb:scale:7", json!(2), true);
        assert_eq!(stores.read("https://b", "crumb:recent", false), None);
        assert_eq!(
            stores.read("https://a", "crumb:scale:7", true),
            Some(&json!(2))
        );

        let reopened = Stores::open(&path);
        assert_eq!(
            reopened.read("https://a", "crumb:recent", false),
            Some(&json!([1, 2]))
        );
        // Session values are gone, as they are when a tab closes
        assert_eq!(reopened.read("https://a", "crumb:scale:7", true), None);

        let mut stores = reopened;
        stores.write("https://a", "crumb:recent", Value::Null, false);
        assert_eq!(stores.read("https://a", "crumb:recent", false), None);
    }
}
