//! The CLI driven the way an agent would, against the real router on a local port.

use crumb::{AppState, app, browser::Browser, config::Config, db};
use crumb_cli::{Cli, Outcome, run};
use crumb_client::{Client, TokenScope};
use serde_json::{Value, json};

struct Server {
    origin: String,
    token: String,
    _dist: tempfile::TempDir,
}

impl Server {
    async fn start() -> Self {
        let dist = tempfile::tempdir().unwrap();
        for (rel, title) in [("index.html", "home"), ("login/index.html", "login")] {
            let path = dist.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(
                path,
                format!(
                    "<!doctype html><title>{title}</title>{}",
                    crumb::web::MARKER
                ),
            )
            .unwrap();
        }
        let config = Config {
            web_dist: dist.path().to_path_buf(),
            app_password: Some("pw".into()),
            ..Config::default()
        };
        let state = AppState::new(db::open_in_memory().unwrap(), config, Browser::disabled());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            axum::serve(listener, app(state)).await.unwrap();
        });
        let person = Client::new(&origin).unwrap();
        person.login("pw").await.unwrap();
        let token = person
            .create_api_token("cli test", TokenScope::Write, None)
            .await
            .unwrap()
            .token;
        Self {
            origin,
            token,
            _dist: dist,
        }
    }

    fn crumb(&self, config: &tempfile::TempDir) -> Crumb {
        Crumb {
            server: self.origin.clone(),
            token: Some(self.token.clone()),
            config: config.path().to_path_buf(),
        }
    }
}

struct Crumb {
    server: String,
    token: Option<String>,
    config: std::path::PathBuf,
}

impl Crumb {
    async fn run(&self, args: &[&str]) -> Outcome {
        self.run_with(args, "").await
    }

    async fn run_with(&self, args: &[&str], stdin: &str) -> Outcome {
        let mut argv = vec![
            "crumb".to_string(),
            "--server".into(),
            self.server.clone(),
            "--config-dir".into(),
            self.config.display().to_string(),
        ];
        if let Some(token) = &self.token {
            argv.extend(["--token".into(), token.clone()]);
        }
        argv.extend(args.iter().map(|a| a.to_string()));
        let cli = <Cli as clap::Parser>::parse_from(argv);
        run(cli, &mut stdin.as_bytes()).await
    }
}

fn recipe(title: &str) -> Value {
    json!({
        "title": title,
        "description": "A test.",
        "recipeYield": "4 servings",
        "ingredients": [{"name": null, "items": ["2 cups flour", "1 onion"]}],
        "instructions": [{"name": null, "items": ["Mix.", "Bake."]}],
    })
}

async fn save(server: &Server, title: &str) -> i64 {
    let (r, _) = Client::with_token(&server.origin, &server.token)
        .unwrap()
        .create_recipe(&recipe(title))
        .await
        .unwrap();
    r.id
}

#[tokio::test]
async fn without_a_token_it_says_how_to_get_one() {
    let server = Server::start().await;
    let dir = tempfile::tempdir().unwrap();
    let mut c = server.crumb(&dir);
    c.token = None;
    let out = c.run(&["ls"]).await;
    assert_eq!(out.code, 3, "{}", out.stderr);
    assert!(out.stderr.contains("crumb login"));

    c.token = Some("crumb_pat_wrong".into());
    let out = c.run(&["ls"]).await;
    assert_eq!(out.code, 3);
    assert!(out.stdout.is_empty());
}

#[tokio::test]
async fn login_keeps_the_token_privately_and_logout_forgets_it() {
    let server = Server::start().await;
    let dir = tempfile::tempdir().unwrap();
    let mut c = server.crumb(&dir);

    // A wrong token is refused and nothing is saved
    c.token = Some("crumb_pat_wrong".into());
    assert_eq!(c.run(&["login"]).await.code, 3);
    assert!(!dir.path().join("credentials.toml").exists());

    c.token = Some(server.token.clone());
    let out = c.run(&["login"]).await;
    assert_eq!(out.code, 0, "{}", out.stderr);
    let file = dir.path().join("credentials.toml");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    // Now no flags are needed: the saved server and token do
    let argv = [
        "crumb",
        "--config-dir",
        dir.path().to_str().unwrap(),
        "whoami",
    ];
    let out = run(<Cli as clap::Parser>::parse_from(argv), &mut "".as_bytes()).await;
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains(&server.origin));

    let out = c.run(&["logout"]).await;
    assert_eq!(out.code, 0);
    let out = run(<Cli as clap::Parser>::parse_from(argv), &mut "".as_bytes()).await;
    assert_eq!(out.code, 2, "no server is a usage error");
}

#[tokio::test]
async fn add_ls_show_cooked_and_export() {
    let server = Server::start().await;
    let dir = tempfile::tempdir().unwrap();
    let c = server.crumb(&dir);

    let text = "Lemon Pasta\n\nIngredients\n200 g spaghetti\n1 lemon\n\nInstructions\nBoil the pasta.\nAdd lemon.\n";
    let out = c.run_with(&["add", "-"], text).await;
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.starts_with("Saved as #"), "{}", out.stdout);
    let out = c.run_with(&["--json", "add", "-n", "-"], text).await;
    assert_eq!(out.code, 0);

    let id = save(&server, "Onion Soup").await;
    let other = save(&server, "Onion Tart").await;

    // ls: tab-separated, ids only, JSON
    let out = c.run(&["ls", "soup"]).await;
    assert_eq!(out.stdout.lines().count(), 1);
    assert!(out.stdout.starts_with(&format!("{id}\tOnion Soup")));
    let out = c.run(&["-q", "ls", "onion"]).await;
    let ids: Vec<i64> = out.stdout.lines().map(|l| l.parse().unwrap()).collect();
    assert!(ids.contains(&id) && ids.contains(&other));
    let out = c.run(&["--json", "ls", "onion"]).await;
    assert_eq!(
        serde_json::from_str::<Value>(&out.stdout)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );

    // show: by id, by a title that matches one, and ambiguity and misses
    let by_id = c.run(&["show", &id.to_string()]).await;
    assert_eq!(by_id.code, 0, "{}", by_id.stderr);
    assert!(by_id.stdout.contains("Onion Soup") && by_id.stdout.contains("2 cups flour"));
    let by_title = c.run(&["show", "soup"]).await;
    assert_eq!(by_title.stdout, by_id.stdout);
    let ambiguous = c.run(&["show", "onion"]).await;
    assert_eq!(ambiguous.code, 2);
    assert!(ambiguous.stderr.contains("Onion Tart"));
    assert_eq!(c.run(&["show", "nothing like this"]).await.code, 5);
    assert_eq!(c.run(&["show", "999999"]).await.code, 5);

    // The same text every client prints
    let recipe = Client::with_token(&server.origin, &server.token)
        .unwrap()
        .recipe(id)
        .await
        .unwrap();
    assert_eq!(
        by_id.stdout.trim_end(),
        crumb_core::format::recipe_to_text(
            &recipe,
            Some(&format!("{}/recipes/{id}", server.origin))
        )
        .trim_end()
    );

    // Scaling goes through crumb-core
    let doubled = c.run(&["show", &id.to_string(), "--scale", "2"]).await;
    assert!(
        doubled.stdout.contains("4 cups flour"),
        "{}",
        doubled.stdout
    );
    assert_eq!(
        c.run(&["show", &id.to_string(), "--scale", "0"]).await.code,
        2
    );

    // Cooking and exporting
    let out = c.run(&["cooked", &id.to_string()]).await;
    assert!(out.stdout.starts_with("1 cook logged"), "{}", out.stdout);
    let out = c.run(&["cooked", &id.to_string(), "--undo"]).await;
    assert_eq!(out.code, 0, "{}", out.stderr);
    let out = c.run(&["export", &id.to_string()]).await;
    assert!(out.stdout.contains("Onion Soup"));
    let file = dir.path().join("soup.json");
    let out = c
        .run(&[
            "export",
            &id.to_string(),
            "--format",
            "json",
            "-o",
            file.to_str().unwrap(),
        ])
        .await;
    assert_eq!(out.code, 0);
    serde_json::from_slice::<Value>(&std::fs::read(file).unwrap()).unwrap();

    // Random and Try next answer; completions print
    assert_eq!(c.run(&["random"]).await.code, 0);
    assert_eq!(c.run(&["--json", "next"]).await.code, 0);
    assert!(
        c.run(&["completions", "bash"])
            .await
            .stdout
            .contains("crumb")
    );
}

#[tokio::test]
async fn errors_are_json_on_stderr_when_asked() {
    let server = Server::start().await;
    let dir = tempfile::tempdir().unwrap();
    let c = server.crumb(&dir);
    let out = c.run(&["--json", "show", "424242"]).await;
    assert_eq!(out.code, 5);
    assert!(out.stdout.is_empty());
    let err: Value = serde_json::from_str(&out.stderr).unwrap();
    assert_eq!(err["error"]["statusCode"], 404);

    // A server nobody is on is a network failure
    let mut down = server.crumb(&dir);
    down.server = "http://127.0.0.1:1".into();
    assert_eq!(down.run(&["ls"]).await.code, 6);
}

#[tokio::test]
async fn set_rm_trash_books_share_and_api() {
    let server = Server::start().await;
    let dir = tempfile::tempdir().unwrap();
    let c = server.crumb(&dir);
    let a = save(&server, "Plain Rice").await;
    let b = save(&server, "Plain Beans").await;

    // set: aliases, clearing, and unknown fields
    let out = c
        .run(&[
            "set",
            &a.to_string(),
            "title=Garlic Rice",
            "servings=6",
            "notes=Rinse it.",
        ])
        .await;
    assert_eq!(out.code, 0, "{}", out.stderr);
    let shown = c.run(&["--json", "show", &a.to_string()]).await.stdout;
    let shown: Value = serde_json::from_str(&shown).unwrap();
    assert_eq!(shown["title"], "Garlic Rice");
    assert_eq!(shown["recipeYield"], "6");
    assert_eq!(shown["notes"], "Rinse it.");
    c.run(&["set", &a.to_string(), "notes="]).await;
    let shown = c.run(&["--json", "show", &a.to_string()]).await.stdout;
    assert!(serde_json::from_str::<Value>(&shown).unwrap()["notes"].is_null());
    assert_eq!(c.run(&["set", &a.to_string(), "colour=red"]).await.code, 2);
    assert_eq!(c.run(&["set", &a.to_string(), "nonsense"]).await.code, 2);
    assert_eq!(c.run(&["set", "999999", "title=x"]).await.code, 5);

    // books
    let out = c.run(&["-q", "books", "new", "Weeknights"]).await;
    assert_eq!(out.code, 0, "{}", out.stderr);
    let book = out.stdout.trim().to_string();
    let out = c
        .run(&["books", "add", &book, &a.to_string(), &b.to_string()])
        .await;
    assert!(
        out.stdout.contains("Added 2"),
        "{}{}",
        out.stdout,
        out.stderr
    );
    assert!(c.run(&["books"]).await.stdout.contains("Weeknights\t2"));
    let out = c.run(&["-q", "books", "show", &book]).await;
    assert_eq!(out.stdout.lines().count(), 2);
    c.run(&["books", "remove", &book, &b.to_string()]).await;
    assert_eq!(
        c.run(&["-q", "books", "show", &book])
            .await
            .stdout
            .lines()
            .count(),
        1
    );
    c.run(&["books", "rename", &book, "Dinners"]).await;
    assert!(c.run(&["books"]).await.stdout.contains("Dinners"));
    assert_eq!(c.run(&["books", "rm", &book]).await.code, 2);
    assert_eq!(c.run(&["books", "rm", &book, "-y"]).await.code, 0);

    // share
    let out = c.run(&["share", &a.to_string()]).await;
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.trim().starts_with("http") && out.stdout.trim().lines().count() == 1);
    assert_eq!(c.run(&["share", &a.to_string(), "--stop"]).await.code, 0);

    // rm: one is plain, several need -y; the Trash lists and restores, and can't be emptied
    let out = c.run(&["rm", &a.to_string(), &b.to_string()]).await;
    assert_eq!(out.code, 2);
    assert!(c.run(&["-q", "ls"]).await.stdout.contains(&a.to_string()));
    let out = c.run(&["rm", &a.to_string(), &b.to_string(), "-y"]).await;
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("crumb trash restore"));
    assert!(c.run(&["-q", "ls"]).await.stdout.is_empty());
    assert_eq!(c.run(&["-q", "trash"]).await.stdout.lines().count(), 2);
    let out = c.run(&["trash", "restore", &a.to_string()]).await;
    assert!(out.stdout.starts_with("Restored"), "{}", out.stdout);
    assert_eq!(c.run(&["-q", "ls"]).await.stdout.trim(), a.to_string());

    // api: any route, but not the ones tokens can't use, and nothing outside /api
    let out = c.run(&["api", "GET", "/api/health"]).await;
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(c.run(&["api", "GET", "/api/tokens"]).await.code, 3);
    assert_eq!(c.run(&["api", "DELETE", "/api/trash"]).await.code, 3);
    assert_eq!(c.run(&["api", "GET", "/recipes"]).await.code, 2);
    assert_eq!(
        c.run(&["api", "POST", "/api/recipes", "--data", "{oops"])
            .await
            .code,
        2
    );
}
