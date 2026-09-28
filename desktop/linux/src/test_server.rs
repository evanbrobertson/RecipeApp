//! A running Crumb server on a random local port, with a throwaway database, for tests.

use crumb::{AppState, app, browser::Browser, config::Config, db};

pub struct TestServer {
    pub origin: String,
    _dist: tempfile::TempDir,
}

impl TestServer {
    pub async fn start(password: Option<&str>) -> Self {
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
