//! Session logic, in two layers:
//!
//! * [`SessionCore`] is plain Rust: a small state machine over `crumb-client`. Tests
//!   drive it directly with an in-memory cookie store.
//! * `Session` (the cxx-qt bridge below) is the thin QObject wrapper QML sees. It owns
//!   a shared `SessionCore` behind a tokio mutex and moves work onto the runtime.

use std::sync::{Arc, OnceLock};

#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::Mutex as StdMutex;

use core::pin::Pin;

use crumb_client::{Client, Error};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use tokio::sync::Mutex;

/// Where the app is in signing in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// No server URL is configured yet.
    Setup,
    /// Checking a server and restoring a saved session.
    Checking,
    /// A server is known, but a password (or a fresh sign-in) is needed.
    Login,
    /// Signed in: the app can list recipes.
    Ready,
}

impl State {
    /// The value QML's `state` property shows.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Checking => "checking",
            Self::Login => "login",
            Self::Ready => "ready",
        }
    }
}

/// Persistence for the session cookie, keyed by server URL.
///
/// Kept behind a trait so tests use [`MemoryStore`], never the real keyring.
pub trait CookieStore: Send + Sync {
    fn load(&self, server: &str) -> Option<String>;
    fn save(&self, server: &str, cookie: &str);
    fn clear(&self, server: &str);
}

/// Secret Service (`gnome-keyring`) storage. A failure only costs persistence: the client
/// keeps the cookie in memory for this run, and a warning is logged. Nothing is ever
/// written to a plain file.
pub struct KeyringStore;

const SERVICE: &str = "crumb-desktop";

impl CookieStore for KeyringStore {
    fn load(&self, server: &str) -> Option<String> {
        match keyring::Entry::new(SERVICE, server) {
            Ok(entry) => match entry.get_password() {
                Ok(cookie) => Some(cookie),
                Err(keyring::Error::NoEntry) => None,
                Err(err) => {
                    eprintln!("crumb-desktop: couldn't read the saved session: {err}");
                    None
                }
            },
            Err(err) => {
                eprintln!("crumb-desktop: couldn't open the keyring: {err}");
                None
            }
        }
    }

    fn save(&self, server: &str, cookie: &str) {
        let stored =
            keyring::Entry::new(SERVICE, server).and_then(|entry| entry.set_password(cookie));
        if let Err(err) = stored {
            eprintln!("crumb-desktop: keeping the session in memory only: {err}");
        }
    }

    fn clear(&self, server: &str) {
        if let Ok(entry) = keyring::Entry::new(SERVICE, server) {
            let _ = entry.delete_credential();
        }
    }
}

/// An in-memory store for tests (and a safe fallback that never touches disk).
#[cfg(test)]
#[derive(Default)]
pub struct MemoryStore(StdMutex<HashMap<String, String>>);

#[cfg(test)]
impl CookieStore for MemoryStore {
    fn load(&self, server: &str) -> Option<String> {
        self.0.lock().unwrap().get(server).cloned()
    }

    fn save(&self, server: &str, cookie: &str) {
        self.0
            .lock()
            .unwrap()
            .insert(server.to_string(), cookie.to_string());
    }

    fn clear(&self, server: &str) {
        self.0.lock().unwrap().remove(server);
    }
}

/// The plain-Rust session state machine: health-check a server, restore a cookie,
/// sign in, sign out. No Qt.
pub struct SessionCore {
    store: Arc<dyn CookieStore>,
    client: Option<Client>,
    server: String,
    state: State,
    error: String,
    reachable: bool,
}

impl SessionCore {
    pub fn new(store: Arc<dyn CookieStore>) -> Self {
        Self {
            store,
            client: None,
            server: String::new(),
            state: State::Setup,
            error: String::new(),
            reachable: false,
        }
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn error(&self) -> &str {
        &self.error
    }

    pub fn server(&self) -> &str {
        &self.server
    }

    pub fn reachable(&self) -> bool {
        self.reachable
    }

    /// The signed-in client, for the recipe list to borrow.
    pub fn client(&self) -> Option<Client> {
        self.client.clone()
    }

    /// Seeds the start-up server URL without doing any I/O.
    pub fn set_server(&mut self, server: &str) {
        self.server = server.trim().trim_end_matches('/').to_string();
        self.state = if self.server.is_empty() {
            State::Setup
        } else {
            State::Checking
        };
    }

    /// Validates the URL, checks the server is up, and tries to restore a saved session.
    /// A success is either [`State::Ready`] (no password, or a good cookie) or
    /// [`State::Login`] (a password is needed). Everything else leaves a message in
    /// [`Self::error`].
    pub async fn connect(&mut self, url: &str) {
        self.error.clear();
        self.reachable = false;
        let trimmed = url.trim();
        if trimmed.is_empty() {
            self.server.clear();
            self.client = None;
            self.state = State::Setup;
            return;
        }
        self.server = trimmed.trim_end_matches('/').to_string();
        self.state = State::Checking;

        let cookie = self.store.load(&self.server);
        let client = match cookie {
            Some(cookie) => Client::with_session(trimmed, &cookie),
            None => Client::new(trimmed),
        };
        let client = match client {
            Ok(client) => client,
            Err(err) => {
                self.client = None;
                self.state = State::Login;
                self.error = err.to_string();
                return;
            }
        };

        if let Err(err) = client.health().await {
            self.client = None;
            self.state = State::Login;
            self.error = err.to_string();
            return;
        }
        self.reachable = true;

        match client.recipes(None, Some(1)).await {
            Ok(_) => self.adopt(client),
            Err(Error::Unauthorized) => {
                self.store.clear(&self.server);
                self.client = Some(client);
                self.state = State::Login;
            }
            Err(err) => {
                self.client = Some(client);
                self.state = State::Login;
                self.error = err.to_string();
            }
        }
    }

    /// Signs in with a password; the server's message is shown on a wrong one.
    pub async fn login(&mut self, password: &str) {
        let Some(client) = self.client.clone() else {
            self.state = State::Login;
            self.error = "Connect to a Crumb server first.".to_string();
            return;
        };
        self.error.clear();
        match client.login(password).await {
            Ok(()) => self.adopt(client),
            Err(err) => {
                self.state = State::Login;
                self.error = err.to_string();
            }
        }
    }

    /// Signs out: best-effort server call, then forget the local cookie.
    pub async fn logout(&mut self) {
        if let Some(client) = self.client.take() {
            let _ = client.logout().await;
        }
        self.store.clear(&self.server);
        self.state = State::Login;
        self.error.clear();
    }

    /// A later authenticated call returned 401: go back to login.
    pub fn require_login(&mut self) {
        self.client = None;
        self.store.clear(&self.server);
        self.state = State::Login;
        self.error.clear();
    }

    fn adopt(&mut self, client: Client) {
        if let Some(cookie) = client.session_cookie() {
            self.store.save(&self.server, &cookie);
        }
        self.client = Some(client);
        self.state = State::Ready;
        self.error.clear();
    }
}

/// The process-wide core shared by the `Session` and `RecipeList` QObjects.
pub fn app_core() -> Arc<Mutex<SessionCore>> {
    static CORE: OnceLock<Arc<Mutex<SessionCore>>> = OnceLock::new();
    CORE.get_or_init(|| Arc::new(Mutex::new(SessionCore::new(Arc::new(KeyringStore)))))
        .clone()
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
        #[qproperty(QString, server_url, cxx_name = "serverUrl")]
        #[qproperty(QString, state)]
        #[qproperty(QString, error)]
        #[qproperty(bool, busy)]
        type Session = super::SessionRust;

        #[qinvokable]
        fn connect(self: Pin<&mut Session>, url: QString);

        #[qinvokable]
        fn login(self: Pin<&mut Session>, password: QString);

        #[qinvokable]
        fn logout(self: Pin<&mut Session>);

        #[qinvokable]
        fn require_login(self: Pin<&mut Session>);
    }

    impl cxx_qt::Threading for Session {}
}

/// The inner Rust struct behind the `Session` QObject.
pub struct SessionRust {
    core: Arc<Mutex<SessionCore>>,
    server_url: QString,
    state: QString,
    error: QString,
    busy: bool,
}

impl Default for SessionRust {
    fn default() -> Self {
        let core = app_core();
        let config = crate::config::apply_env(
            crate::config::load(&crate::config::config_dir()),
            std::env::var(crate::config::ENV_SERVER).ok(),
        );
        let server_url = config.server_url.unwrap_or_default();
        let state = if server_url.trim().is_empty() {
            State::Setup
        } else {
            State::Checking
        };
        if let Ok(mut core) = core.try_lock() {
            core.set_server(&server_url);
        }
        Self {
            core,
            server_url: QString::from(&server_url),
            state: QString::from(state.as_str()),
            error: QString::default(),
            busy: false,
        }
    }
}

impl qobject::Session {
    pub fn connect(mut self: Pin<&mut Self>, url: QString) {
        let url = url.to_string();
        self.as_mut().set_busy(true);
        self.as_mut().set_error(QString::default());
        self.as_mut()
            .set_state(QString::from(State::Checking.as_str()));
        self.as_mut().set_server_url(QString::from(&url));

        let qt_thread = self.as_mut().qt_thread();
        let core = self.rust().core.clone();
        drop(crate::runtime::spawn(async move {
            let (state, error, server, reachable) = {
                let mut core = core.lock().await;
                core.connect(&url).await;
                (
                    core.state(),
                    core.error().to_string(),
                    core.server().to_string(),
                    core.reachable(),
                )
            };
            if reachable
                && std::env::var(crate::config::ENV_SERVER).is_err()
                && let Err(err) = crate::config::save(&crate::config::config_dir(), &server)
            {
                eprintln!("crumb-desktop: couldn't save settings: {err}");
            }
            let _ = qt_thread.queue(move |mut object| {
                object.as_mut().set_state(QString::from(state.as_str()));
                object.as_mut().set_error(QString::from(&error));
                object.as_mut().set_busy(false);
            });
        }));
    }

    pub fn login(mut self: Pin<&mut Self>, password: QString) {
        let password = password.to_string();
        self.as_mut().set_busy(true);
        self.as_mut().set_error(QString::default());

        let qt_thread = self.as_mut().qt_thread();
        let core = self.rust().core.clone();
        drop(crate::runtime::spawn(async move {
            let (state, error) = {
                let mut core = core.lock().await;
                core.login(&password).await;
                (core.state(), core.error().to_string())
            };
            let _ = qt_thread.queue(move |mut object| {
                object.as_mut().set_state(QString::from(state.as_str()));
                object.as_mut().set_error(QString::from(&error));
                object.as_mut().set_busy(false);
            });
        }));
    }

    pub fn logout(mut self: Pin<&mut Self>) {
        self.as_mut().set_busy(true);
        self.as_mut().set_error(QString::default());

        let qt_thread = self.as_mut().qt_thread();
        let core = self.rust().core.clone();
        drop(crate::runtime::spawn(async move {
            let (state, error) = {
                let mut core = core.lock().await;
                core.logout().await;
                (core.state(), core.error().to_string())
            };
            let _ = qt_thread.queue(move |mut object| {
                object.as_mut().set_state(QString::from(state.as_str()));
                object.as_mut().set_error(QString::from(&error));
                object.as_mut().set_busy(false);
            });
        }));
    }

    pub fn require_login(mut self: Pin<&mut Self>) {
        self.as_mut()
            .set_state(QString::from(State::Login.as_str()));
        self.as_mut().set_error(QString::default());

        let core = self.rust().core.clone();
        drop(crate::runtime::spawn(async move {
            core.lock().await.require_login();
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crumb::{AppState, app, browser::Browser, config::Config, db};

    /// A running Crumb server on a random local port, with a throwaway database.
    struct TestServer {
        origin: String,
        _dist: tempfile::TempDir,
    }

    impl TestServer {
        async fn start(password: Option<&str>) -> Self {
            let dist = tempfile::tempdir().unwrap();
            for (rel, title) in [
                ("index.html", "home"),
                ("recipes/index.html", "recipes"),
                ("shell/recipe/index.html", "recipe"),
                ("add/index.html", "add"),
                ("login/index.html", "login"),
                ("404.html", "missing"),
            ] {
                let path = dist.path().join(rel);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(
                    &path,
                    format!(
                        "<!doctype html><title>{title}</title>{}",
                        crumb::web::MARKER
                    ),
                )
                .unwrap();
            }

            let config = Config {
                app_password: password.map(String::from),
                web_dist: dist.path().to_path_buf(),
                ..Config::default()
            };
            let state = AppState::new(db::open_in_memory().unwrap(), config, Browser::disabled());

            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let origin = format!("http://{}", listener.local_addr().unwrap());
            tokio::spawn(async move {
                axum::serve(listener, app(state)).await.unwrap();
            });

            Self {
                origin,
                _dist: dist,
            }
        }
    }

    fn core(store: &Arc<MemoryStore>) -> SessionCore {
        SessionCore::new(store.clone())
    }

    #[tokio::test]
    async fn a_server_without_a_password_connects_straight_to_ready() {
        let server = TestServer::start(None).await;
        let store = Arc::new(MemoryStore::default());
        let mut core = core(&store);

        core.connect(&server.origin).await;
        assert_eq!(core.state(), State::Ready);
        assert_eq!(core.error(), "");
    }

    #[tokio::test]
    async fn a_password_server_needs_login_and_reports_a_wrong_one() {
        let server = TestServer::start(Some("secret")).await;
        let store = Arc::new(MemoryStore::default());
        let mut core = core(&store);

        core.connect(&server.origin).await;
        assert_eq!(core.state(), State::Login);
        assert_eq!(core.error(), "");

        core.login("wrong").await;
        assert_eq!(core.state(), State::Login);
        assert_eq!(core.error(), "Incorrect password");

        core.login("secret").await;
        assert_eq!(core.state(), State::Ready);
        assert_eq!(core.error(), "");
    }

    #[tokio::test]
    async fn a_saved_cookie_skips_login() {
        let server = TestServer::start(Some("secret")).await;
        let store = Arc::new(MemoryStore::default());

        let mut first = core(&store);
        first.connect(&server.origin).await;
        first.login("secret").await;
        assert_eq!(first.state(), State::Ready);

        let mut second = core(&store);
        second.connect(&server.origin).await;
        assert_eq!(second.state(), State::Ready);
    }

    #[tokio::test]
    async fn an_invalid_cookie_returns_to_login() {
        let server = TestServer::start(Some("secret")).await;
        let store = Arc::new(MemoryStore::default());
        store.save(&server.origin, "not-a-real-session");

        let mut core = core(&store);
        core.connect(&server.origin).await;
        assert_eq!(core.state(), State::Login);
        assert_eq!(core.error(), "");
        assert_eq!(store.load(&server.origin), None);
    }

    #[tokio::test]
    async fn an_unauthorized_call_moves_back_to_login() {
        let server = TestServer::start(None).await;
        let store = Arc::new(MemoryStore::default());
        let mut core = core(&store);

        core.connect(&server.origin).await;
        assert_eq!(core.state(), State::Ready);

        core.require_login();
        assert_eq!(core.state(), State::Login);
    }
}
