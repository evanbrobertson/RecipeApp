//! End-to-end tests: the real server router, served on a local port, driven by the client.

use crumb::{AppState, app, browser::Browser, config::Config, db};
use crumb_client::{Client, Error, ImportInput, SESSION_COOKIE, Section};
use serde_json::{Value, json};

/// A running Crumb server on a random local port, with a throwaway database.
struct Server {
    origin: String,
    _dist: tempfile::TempDir,
}

impl Server {
    async fn start(password: Option<&str>) -> Self {
        let dist = tempfile::tempdir().unwrap();
        // The router also serves pages and static files; give it the minimal build
        // `tests/api.rs` uses, so nothing 404s into a panic.
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

/// Saves a recipe straight through the REST API (the client has no create method).
async fn create(origin: &str, body: Value) -> Value {
    reqwest::Client::new()
        .post(format!("{origin}/api/recipes"))
        .json(&body)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn health_succeeds_without_a_password() {
    let server = Server::start(None).await;
    let client = Client::new(&server.origin).unwrap();
    client.health().await.unwrap();
}

#[tokio::test]
async fn auth_is_required_until_login() {
    let server = Server::start(Some("secret")).await;
    let client = Client::new(&server.origin).unwrap();

    assert!(matches!(
        client.recipes(None, None).await,
        Err(Error::Unauthorized)
    ));

    match client.login("wrong").await {
        Err(Error::Api { status, message }) => {
            assert_eq!(status, 401);
            assert_eq!(message, "Incorrect password");
        }
        other => panic!("expected an Api 401, got {other:?}"),
    }

    client.login("secret").await.unwrap();
    assert!(client.recipes(None, None).await.is_ok());
    assert!(client.session_cookie().is_some());
}

#[tokio::test]
async fn a_saved_session_can_be_restored() {
    let server = Server::start(Some("secret")).await;
    let client = Client::new(&server.origin).unwrap();
    client.login("secret").await.unwrap();
    let cookie = client.session_cookie().unwrap();

    let restored = Client::with_session(&server.origin, &cookie).unwrap();
    assert!(restored.recipes(None, None).await.is_ok());
    assert_eq!(restored.session_cookie().as_deref(), Some(cookie.as_str()));

    let garbage = Client::with_session(&server.origin, "not-a-real-session").unwrap();
    assert!(matches!(
        garbage.recipes(None, None).await,
        Err(Error::Unauthorized)
    ));
}

#[tokio::test]
async fn lists_searches_and_limits_recipes() {
    let server = Server::start(None).await;
    create(&server.origin, json!({"title": "Lemon Chicken"})).await;
    create(&server.origin, json!({"title": "Tomato Soup"})).await;

    let client = Client::new(&server.origin).unwrap();
    let all = client.recipes(None, None).await.unwrap();
    assert_eq!(all.len(), 2);
    let titles: Vec<&str> = all.iter().map(|recipe| recipe.title.as_str()).collect();
    assert!(titles.contains(&"Lemon Chicken"), "{titles:?}");
    assert!(titles.contains(&"Tomato Soup"), "{titles:?}");

    let found = client.recipes(Some("lemon"), None).await.unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].title, "Lemon Chicken");

    assert_eq!(client.recipes(None, Some(1)).await.unwrap().len(), 1);
}

#[tokio::test]
async fn fetches_a_recipe_and_maps_a_missing_one_to_404() {
    let server = Server::start(None).await;
    let created = create(
        &server.origin,
        json!({
            "title": "Lemon Chicken",
            "ingredients": [{"name": "Sauce", "items": ["2 lemons", "1 chicken"]}],
            "instructions": [{"items": ["Roast it."]}],
        }),
    )
    .await;
    let id = created["id"].as_i64().unwrap();

    let client = Client::new(&server.origin).unwrap();
    let recipe = client.recipe(id).await.unwrap();
    assert_eq!(recipe.title, "Lemon Chicken");
    assert_eq!(
        recipe.ingredients,
        vec![Section {
            name: Some("Sauce".into()),
            items: vec!["2 lemons".into(), "1 chicken".into()],
        }]
    );
    assert_eq!(
        recipe.instructions,
        vec![Section {
            name: None,
            items: vec!["Roast it.".into()],
        }]
    );

    match client.recipe(999_999).await {
        Err(Error::Api { status, .. }) => assert_eq!(status, 404),
        other => panic!("expected an Api 404, got {other:?}"),
    }
}

#[tokio::test]
async fn imports_pasted_text() {
    let server = Server::start(None).await;
    let client = Client::new(&server.origin).unwrap();

    let text = "Toast\nIngredients\n1 slice bread\nInstructions\n1. Toast the bread.";
    let imported = client.import(ImportInput::Text(text.into())).await.unwrap();

    assert!(imported.is_new);
    assert_eq!(imported.recipe.title, "Toast");
    assert_eq!(
        imported.recipe.ingredients,
        vec![Section {
            name: None,
            items: vec!["1 slice bread".into()],
        }]
    );
    assert_eq!(
        imported.recipe.instructions,
        vec![Section {
            name: None,
            items: vec!["Toast the bread.".into()],
        }]
    );
}

#[tokio::test]
async fn logs_views_and_cooks_and_404s_on_missing() {
    let server = Server::start(None).await;
    let created = create(&server.origin, json!({"title": "Pancakes"})).await;
    let id = created["id"].as_i64().unwrap();

    let client = Client::new(&server.origin).unwrap();
    client.viewed(id).await.unwrap();
    client.cooked(id).await.unwrap();

    assert!(matches!(
        client.viewed(999_999).await,
        Err(Error::Api { status: 404, .. })
    ));
    assert!(matches!(
        client.cooked(999_999).await,
        Err(Error::Api { status: 404, .. })
    ));
}

#[tokio::test]
async fn lists_cookbooks() {
    let server = Server::start(None).await;
    let client = Client::new(&server.origin).unwrap();
    assert!(client.cookbooks().await.unwrap().is_empty());
}

#[test]
fn rejects_bad_base_urls_and_normalizes_good_ones() {
    assert!(matches!(Client::new("ftp://x"), Err(Error::InvalidUrl)));
    assert!(matches!(Client::new("not a url"), Err(Error::InvalidUrl)));
    assert_eq!(
        Client::new("https://crumb.example/").unwrap().base_url(),
        "https://crumb.example"
    );
}

#[test]
fn the_session_cookie_name_matches_the_server() {
    assert_eq!(SESSION_COOKIE, crumb::auth::COOKIE);
}

#[test]
fn image_url_uses_the_webs_fnv1a() {
    let client = Client::new("http://127.0.0.1:3000").unwrap();
    // Vectors computed with the web's `imageKey()` (web/src/lib/img.ts) under bun:
    //   imageKey("https://food.test/cake.jpg") === "2278d4f4"
    //   imageKey("https://food.test/lemon.png") === "db670889"
    //   imageKey("")                            === "811c9dc5"
    assert_eq!(
        client.image_url(7, 768, "https://food.test/cake.jpg"),
        "http://127.0.0.1:3000/img/7/768?v=2278d4f4"
    );
    assert_eq!(
        client.image_url(12, 320, "https://food.test/lemon.png"),
        "http://127.0.0.1:3000/img/12/320?v=db670889"
    );
    assert_eq!(
        client.image_url(1, 160, ""),
        "http://127.0.0.1:3000/img/1/160?v=811c9dc5"
    );
}
