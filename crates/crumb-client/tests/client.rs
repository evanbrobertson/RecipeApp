//! End-to-end tests: the real server router, served on a local port, driven by the client.

use crumb::{AppState, app, browser::Browser, config::Config, db};
use crumb_client::{
    AiStatus, Client, Error, ImportInput, RecipeFormat, SESSION_COOKIE, Section, ShareKind,
};
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

/// Saves a recipe straight through the REST API, independent of the client under test.
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

#[tokio::test]
async fn creates_patches_and_deletes_a_recipe() {
    let server = Server::start(None).await;
    let client = Client::new(&server.origin).unwrap();

    let (recipe, is_new) = client
        .create_recipe(&json!({
            "title": "Soda Bread",
            "description": "Quick and plain",
            "ingredients": [{"items": ["500 g flour", "400 ml buttermilk"]}],
            "url": "https://food.test/soda-bread",
        }))
        .await
        .unwrap();
    assert!(is_new);
    // The same URL again returns the saved recipe rather than a copy
    let (again, is_new) = client
        .create_recipe(&json!({"title": "Other", "url": "https://food.test/soda-bread"}))
        .await
        .unwrap();
    assert!(!is_new);
    assert_eq!(again.id, recipe.id);

    let patched = client
        .patch_recipe(recipe.id, &json!({"title": "Irish Soda Bread"}))
        .await
        .unwrap();
    assert_eq!(patched.title, "Irish Soda Bread");
    assert_eq!(patched.description.as_deref(), Some("Quick and plain"));
    assert_eq!(patched.ingredients, recipe.ingredients);

    let cleared = client
        .patch_recipe(recipe.id, &json!({"description": null}))
        .await
        .unwrap();
    assert_eq!(cleared.description, None);

    client.delete_recipe(recipe.id).await.unwrap();
    assert!(matches!(
        client.recipe(recipe.id).await,
        Err(Error::Api { status: 404, .. })
    ));
    assert!(matches!(
        client.delete_recipe(recipe.id).await,
        Err(Error::Api { status: 404, .. })
    ));

    match client.create_recipe(&json!({"title": ""})).await {
        Err(Error::Api {
            status: 400,
            message,
        }) => assert!(!message.is_empty()),
        other => panic!("expected an Api 400, got {other:?}"),
    }
}

#[tokio::test]
async fn bulk_deletes_recipes() {
    let server = Server::start(None).await;
    let a = create(&server.origin, json!({"title": "A"})).await["id"]
        .as_i64()
        .unwrap();
    let b = create(&server.origin, json!({"title": "B"})).await["id"]
        .as_i64()
        .unwrap();
    let client = Client::new(&server.origin).unwrap();
    assert_eq!(client.bulk_delete(&[a, b, 999_999]).await.unwrap(), 2);
    assert!(client.recipes(None, None).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_cook_can_be_undone() {
    let server = Server::start(None).await;
    let id = create(&server.origin, json!({"title": "Pancakes"})).await["id"]
        .as_i64()
        .unwrap();
    let client = Client::new(&server.origin).unwrap();

    let cooked = client.cooked(id).await.unwrap();
    assert_eq!(cooked.stats.count, 1);
    assert!(cooked.stats.last_cooked_at.is_some());
    let event = cooked.event_id.expect("a new cook has an event to undo");

    // Cooking again straight away is the same cook: nothing new to undo
    let again = client.cooked(id).await.unwrap();
    assert_eq!(again.stats.count, 1);
    assert_eq!(again.event_id, None);

    let undone = client.undo_cooked(id, Some(event)).await.unwrap();
    assert_eq!(undone.count, 0);
    assert_eq!(undone.last_cooked_at, None);
}

#[tokio::test]
async fn files_recipes_in_cookbooks() {
    let server = Server::start(None).await;
    let recipe = create(&server.origin, json!({"title": "Scones"})).await["id"]
        .as_i64()
        .unwrap();
    let client = Client::new(&server.origin).unwrap();

    let book = client
        .create_cookbook("Baking", Some("Weekend things"), None)
        .await
        .unwrap();
    assert_eq!(book.name, "Baking");
    assert_eq!(book.description.as_deref(), Some("Weekend things"));

    assert_eq!(client.add_to_cookbook(book.id, &[recipe]).await.unwrap(), 1);
    assert_eq!(client.add_to_cookbook(book.id, &[recipe]).await.unwrap(), 0);
    let full = client.cookbook(book.id).await.unwrap();
    assert_eq!(full.recipes.len(), 1);
    assert_eq!(full.recipes[0].title, "Scones");
    assert_eq!(
        client.recipe_cookbooks(recipe).await.unwrap(),
        vec![book.id]
    );
    let listed = client.cookbooks().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].recipe_count, 1);

    client.remove_from_cookbook(book.id, recipe).await.unwrap();
    assert!(client.recipe_cookbooks(recipe).await.unwrap().is_empty());

    let color = crumb::model::BOOK_COLORS[1];
    let renamed = client
        .patch_cookbook(
            book.id,
            &json!({"name": "Bakes", "color": color, "description": null}),
        )
        .await
        .unwrap();
    assert_eq!(renamed.name, "Bakes");
    assert_eq!(renamed.color.as_deref(), Some(color));
    assert_eq!(renamed.description, None);

    client.delete_cookbook(book.id).await.unwrap();
    assert!(client.cookbooks().await.unwrap().is_empty());
    assert!(matches!(
        client.cookbook(book.id).await,
        Err(Error::Api { status: 404, .. })
    ));
    // The recipe stays in the box
    assert_eq!(client.recipe(recipe).await.unwrap().title, "Scones");
}

#[tokio::test]
async fn exports_recipes_books_and_the_whole_box() {
    let server = Server::start(None).await;
    let id = create(
        &server.origin,
        json!({"title": "Crème Brûlée", "ingredients": [{"items": ["4 egg yolks"]}]}),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let client = Client::new(&server.origin).unwrap();

    let json_file = client.export_recipe(id, RecipeFormat::Json).await.unwrap();
    assert_eq!(json_file.file_name, "crème-brûlée.json");
    assert!(json_file.content_type.starts_with("application/json"));
    let doc: serde_json::Value = serde_json::from_slice(&json_file.bytes).unwrap();
    assert!(doc.to_string().contains("4 egg yolks"));

    let md = client
        .export_recipe(id, RecipeFormat::Markdown)
        .await
        .unwrap();
    assert_eq!(md.file_name, "crème-brûlée.md");
    assert!(md.content_type.starts_with("text/markdown"));
    assert!(
        String::from_utf8(md.bytes)
            .unwrap()
            .contains("Crème Brûlée")
    );

    let book = client
        .create_cookbook("Puddings", None, None)
        .await
        .unwrap();
    client.add_to_cookbook(book.id, &[id]).await.unwrap();
    let book_file = client.export_cookbook(book.id).await.unwrap();
    assert_eq!(book_file.file_name, "puddings.json");
    assert!(!book_file.bytes.is_empty());

    let all = client.export_all().await.unwrap();
    assert!(all.file_name.starts_with("crumb-") && all.file_name.ends_with(".json"));
    assert!(
        String::from_utf8(all.bytes)
            .unwrap()
            .contains("Crème Brûlée")
    );
}

#[tokio::test]
async fn an_exported_recipe_imports_back_from_a_file() {
    let server = Server::start(None).await;
    let id = create(
        &server.origin,
        json!({"title": "Flapjacks", "ingredients": [{"items": ["250 g oats"]}]}),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let client = Client::new(&server.origin).unwrap();

    let file = client.export_recipe(id, RecipeFormat::Json).await.unwrap();
    client.delete_recipe(id).await.unwrap();

    let results = client
        .import_files(vec![(file.file_name.clone(), file.bytes)])
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].file, file.file_name);
    assert_eq!(results[0].error, None);
    assert_eq!(results[0].created.len(), 1);
    let back = client.recipe(results[0].created[0].id).await.unwrap();
    assert_eq!(back.title, "Flapjacks");
    assert_eq!(back.ingredients[0].items, vec!["250 g oats".to_string()]);

    assert!(matches!(
        client.import_files(Vec::new()).await,
        Err(Error::Api { status: 400, .. })
    ));
}

#[tokio::test]
async fn shares_a_recipe_link() {
    let server = Server::start(None).await;
    let id = create(&server.origin, json!({"title": "Shortbread"})).await["id"]
        .as_i64()
        .unwrap();
    let client = Client::new(&server.origin).unwrap();

    let share = client.create_share(ShareKind::Recipe, id).await.unwrap();
    assert!(
        share.url.contains(&format!("/s/{}", share.token)),
        "{}",
        share.url
    );
    assert!(share.include_notes, "a new link shows the cook's notes");
    assert_eq!(
        client.create_share(ShareKind::Recipe, id).await.unwrap(),
        share
    );

    let with_notes = client
        .update_share(ShareKind::Recipe, id, false)
        .await
        .unwrap();
    assert!(!with_notes.include_notes);
    assert_eq!(with_notes.token, share.token);

    let links = client.shares().await.unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].kind, ShareKind::Recipe);
    assert_eq!(links[0].id, id);
    assert_eq!(links[0].title, "Shortbread");

    client.stop_share(ShareKind::Recipe, id).await.unwrap();
    assert!(client.shares().await.unwrap().is_empty());

    let book = client
        .create_cookbook("Biscuits", None, None)
        .await
        .unwrap();
    let book_share = client
        .create_share(ShareKind::Cookbook, book.id)
        .await
        .unwrap();
    assert_ne!(book_share.token, share.token);
    assert_eq!(client.shares().await.unwrap()[0].kind, ShareKind::Cookbook);
}

#[tokio::test]
async fn suggests_and_picks_at_random() {
    let server = Server::start(None).await;
    let client = Client::new(&server.origin).unwrap();
    assert!(matches!(
        client.random_recipe(None, &[]).await,
        Err(Error::Api { status: 404, .. })
    ));

    // Try next only suggests dishes: two ingredients and a step at least
    let dish = |title: &str| {
        json!({
            "title": title,
            "ingredients": [{"items": ["1 onion", "500 g beef"]}],
            "instructions": [{"items": ["Cook it slowly."]}],
        })
    };
    let a = create(&server.origin, dish("Stew")).await["id"]
        .as_i64()
        .unwrap();
    let b = create(&server.origin, dish("Curry")).await["id"]
        .as_i64()
        .unwrap();

    let picked = client.random_recipe(Some(a), &[]).await.unwrap();
    assert_eq!(picked.id, b, "the recipe on screen isn't picked again");

    let suggestions = client.suggestions(4, 0, &[]).await.unwrap();
    assert_eq!(suggestions.ai, AiStatus::Off, "no AI key in tests");
    assert!(!suggestions.items.is_empty());
    assert!(suggestions.items.iter().all(|item| !item.reason.is_empty()));
    let excluded = client.suggestions(4, 0, &[a]).await.unwrap();
    assert!(excluded.items.iter().all(|item| item.recipe.id != a));
}

#[tokio::test]
async fn wee_chef_checks_report_that_they_are_off() {
    let server = Server::start(None).await;
    let id = create(&server.origin, json!({"title": "Soup"})).await["id"]
        .as_i64()
        .unwrap();
    let client = Client::new(&server.origin).unwrap();

    let connector = client.connector().await.unwrap();
    assert!(!connector.wee_chef);
    assert!(!connector.wee_chef_checks);
    assert!(connector.mcp_url.ends_with("/mcp"));

    let status = client.checks_status().await.unwrap();
    assert!(!status.enabled);
    assert_eq!(status.pending, 0);
    assert_eq!(status.queued, None);
    assert!(!crumb_core::checks::checks_status_text(status.counts(), None).is_empty());

    assert!(client.recipe_checks(id).await.unwrap().is_none());
    assert!(client.checks_review().await.unwrap().is_empty());
    assert!(matches!(
        client.check_recipe(id).await,
        Err(Error::Api { status: 409, .. })
    ));
    assert!(matches!(
        client.check_all().await,
        Err(Error::Api { status: 409, .. })
    ));
    assert!(matches!(
        client.undo_checks(id).await,
        Err(Error::Api { status: 409, .. })
    ));
    assert!(matches!(
        client.dismiss_flag(id, 1).await,
        Err(Error::Api { status: 404, .. })
    ));
}

#[tokio::test]
async fn every_area_needs_a_session() {
    let server = Server::start(Some("secret")).await;
    let client = Client::new(&server.origin).unwrap();
    let results = [
        client.create_recipe(&json!({"title": "x"})).await.err(),
        client.suggestions(4, 0, &[]).await.err(),
        client.cookbook(1).await.err(),
        client.export_all().await.err(),
        client.shares().await.err(),
        client.checks_status().await.err(),
        client
            .import_files(vec![("a.json".into(), b"{}".to_vec())])
            .await
            .err(),
        client.connector().await.err(),
    ];
    for result in results {
        assert!(matches!(result, Some(Error::Unauthorized)), "{result:?}");
    }
    client.login("secret").await.unwrap();
    assert!(client.connector().await.unwrap().auth_enabled);
}
