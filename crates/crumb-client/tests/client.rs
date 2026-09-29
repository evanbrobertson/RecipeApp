//! End-to-end tests: the real server router, served on a local port, driven by the client.

use crumb::{AppState, app, browser::Browser, config::Config, db};
use crumb_client::{
    AiStatus, Client, CookStats, Cooked, Credentials, Device, Error, ImportInput, Imported,
    ImportedCookbook, Mode, Preview, Provider, Recipe, RecipeFormat, SESSION_COOKIE, Section,
    ShareKind, Status, StatusHousehold,
};
use serde_json::{Value, json};

/// A running Crumb server on a random local port, with a throwaway database.
struct Server {
    origin: String,
    _dist: tempfile::TempDir,
}

impl Server {
    async fn start(password: Option<&str>) -> Self {
        Self::configured(|config| config.app_password = password.map(String::from)).await
    }

    /// `AUTH_MODE=accounts`, with sign-up open or not.
    async fn accounts(open_signup: bool) -> Self {
        Self::configured(|config| {
            config.auth_mode = crumb::config::AuthMode::Accounts;
            config.open_signup = open_signup;
        })
        .await
    }

    async fn configured(configure: impl FnOnce(&mut Config)) -> Self {
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

        let mut config = Config {
            web_dist: dist.path().to_path_buf(),
            ..Config::default()
        };
        configure(&mut config);
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
        Err(Error::Api {
            status, message, ..
        }) => {
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
            code: None,
            site: None,
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

    // Both are in the trash, newest first, and can be put back
    let trash = client.trash().await.unwrap();
    assert_eq!(trash.len(), 2);
    assert!(trash.iter().any(|t| t.title == "A"));
    let (back, is_new) = client.restore_recipe(a).await.unwrap();
    assert_eq!((back.id, back.title.as_str(), is_new), (a, "A", true));
    assert!(matches!(
        client.restore_recipe(a).await,
        Err(Error::Api { status: 404, .. })
    ));
    client.purge_trashed(b).await.unwrap();
    assert_eq!(client.empty_trash().await.unwrap(), 0);
    assert_eq!(client.recipes(None, None).await.unwrap().len(), 1);
}

#[tokio::test]
async fn popular_is_off_with_one_household_and_can_be_opted_out_of() {
    let server = Server::start(None).await;
    let client = Client::new(&server.origin).unwrap();
    let popular = client.popular().await.unwrap();
    assert!(!popular.enabled);
    assert!(popular.items.is_empty());

    assert!(!client.popular_setting().await.unwrap().opted_out);
    client.set_popular_opt_out(true).await.unwrap();
    assert!(client.popular_setting().await.unwrap().opted_out);
    assert!(client.popular().await.unwrap().opted_out);
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
    assert_eq!(client.cook_stats(id).await.unwrap().count, 1);

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

#[test]
fn responses_serialize_with_the_apis_names() {
    let recipe: Recipe = serde_json::from_value(json!({
        "id": 1, "title": "Pie", "source": "manual", "ingredients": [], "instructions": [],
        "createdAt": "2026-01-01T00:00:00.000Z", "updatedAt": "2026-01-01T00:00:00.000Z",
    }))
    .unwrap();
    let imported = Imported {
        recipe,
        is_new: true,
        from_video: false,
        dropped_photo: true,
        cookbook: Some(ImportedCookbook {
            id: 3,
            name: "Weeknights".into(),
            added: 2,
            duplicates: 0,
            skipped: 1,
        }),
    };
    let out = serde_json::to_value(&imported).unwrap();
    let mut keys: Vec<&str> = out
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        ["cookbook", "droppedPhoto", "fromVideo", "isNew", "recipe"]
    );
    assert_eq!(out["recipe"]["title"], "Pie");
    assert_eq!(out["cookbook"]["added"], 2);

    let cooked = Cooked {
        stats: CookStats {
            count: 2,
            last_cooked_at: Some("2026-01-01T00:00:00.000Z".into()),
        },
        event_id: Some(9),
    };
    assert_eq!(
        serde_json::to_value(&cooked).unwrap(),
        json!({"count": 2, "lastCookedAt": "2026-01-01T00:00:00.000Z", "eventId": 9})
    );

    let status = Status {
        mode: Mode::Hosted,
        signed_in: true,
        household: Some(StatusHousehold {
            id: 4,
            name: "Kitchen".into(),
            role: "owner".into(),
        }),
        ..Status::default()
    };
    let out = serde_json::to_value(&status).unwrap();
    assert_eq!(out["mode"], "hosted");
    assert_eq!(out["signedIn"], true);
    assert_eq!(out["setupNeedsAppPassword"], false);
    assert_eq!(out["household"]["role"], "owner");

    let device = Device {
        id: "1".into(),
        user_agent: None,
        last_seen_at: 5,
        current: true,
    };
    assert_eq!(
        serde_json::to_value(&device).unwrap(),
        json!({"id": "1", "userAgent": null, "lastSeenAt": 5, "current": true})
    );
    let back: Device = serde_json::from_value(serde_json::to_value(&device).unwrap()).unwrap();
    assert_eq!(back, device);
}

#[tokio::test]
async fn password_mode_is_the_status_fallback() {
    let server = Server::start(Some("secret")).await;
    let client = Client::new(&server.origin).unwrap();
    let status = client.auth_status().await;
    assert_eq!(status.mode, Mode::Password);
    assert!(status.password_required && !status.signed_in);

    let down = Client::new("http://127.0.0.1:1").unwrap();
    assert_eq!(down.auth_status().await.mode, Mode::Password);
}

#[tokio::test]
async fn accounts_set_up_sign_in_and_out() {
    let server = Server::accounts(false).await;
    let client = Client::new(&server.origin).unwrap();
    let status = client.auth_status().await;
    assert_eq!(status.mode, Mode::Accounts);
    assert!(status.setup_needed && !status.signed_in);

    let accounts = client.accounts(status.mode);
    accounts
        .set_up("Ann Cook", "ann@example.com", "correct horse", None)
        .await
        .unwrap();
    let status = client.auth_status().await;
    assert!(status.signed_in && !status.setup_needed);
    assert_eq!(status.user.unwrap().name, "Ann Cook");
    assert_eq!(status.household.unwrap().role, "owner");
    assert!(client.recipes(None, None).await.is_ok());

    // Once, and sign-up is closed
    assert!(matches!(
        accounts
            .set_up("Ann", "b@example.com", "correct horse", None)
            .await,
        Err(Error::Api { status: 409, .. })
    ));
    assert!(matches!(
        accounts
            .sign_up("Bob", "bob@example.com", "correct horse")
            .await,
        Err(Error::Api { status: 403, .. })
    ));
    assert!(!accounts.can_reset_password());
    assert!(matches!(
        accounts.request_reset("ann@example.com").await,
        Err(Error::Api { status: 404, .. })
    ));

    // The session is the plain cookie, and restores
    let saved = client.session_cookie().unwrap();
    assert!(!saved.contains("crumb_session"));
    let restored = Client::with_session(&server.origin, &saved).unwrap();
    assert!(restored.recipes(None, None).await.is_ok());
    assert_eq!(restored.session_cookie().as_deref(), Some(saved.as_str()));

    accounts.sign_out().await.unwrap();
    assert!(client.session_cookie().is_none());
    assert!(matches!(
        client.recipes(None, None).await,
        Err(Error::Unauthorized)
    ));

    match accounts.sign_in("ann@example.com", "wrong").await {
        Err(Error::Api {
            status, message, ..
        }) => {
            assert_eq!(status, 401);
            assert_eq!(message, "Incorrect email or password");
        }
        other => panic!("expected an Api 401, got {other:?}"),
    }
    accounts
        .sign_in("ann@example.com", "correct horse")
        .await
        .unwrap();
    assert!(client.recipes(None, None).await.is_ok());
}

#[tokio::test]
async fn accounts_household_invites_and_devices() {
    let server = Server::accounts(false).await;
    let ann = Client::new(&server.origin).unwrap();
    let accounts = ann.accounts(Mode::Accounts);
    accounts
        .set_up("Ann Cook", "ann@example.com", "correct horse", None)
        .await
        .unwrap();

    let home = accounts.household().await.unwrap();
    assert_eq!(home.role, "owner");
    assert_eq!(home.members.len(), 1);
    assert!(home.members[0].you && home.members[0].email == "ann@example.com");
    assert_eq!(home.households.len(), 1);
    accounts.rename("Ann's Kitchen").await.unwrap();
    assert_eq!(accounts.household().await.unwrap().name, "Ann's Kitchen");

    // A link made and taken back
    let link = accounts.invite(None).await.unwrap();
    assert!(link.url.contains("/invite#"), "{}", link.url);
    let pending = accounts.household().await.unwrap().invites;
    assert_eq!(pending.len(), 1);
    accounts.cancel_invite(&pending[0].id).await.unwrap();
    assert!(accounts.household().await.unwrap().invites.is_empty());

    // A link used by someone new
    let link = accounts.invite(None).await.unwrap();
    let token = link.url.split_once('#').unwrap().1;
    let bob = Client::new(&server.origin).unwrap();
    let theirs = bob.accounts(Mode::Accounts);
    let preview = theirs.preview_invite(token).await.unwrap().unwrap();
    assert_eq!(preview.household_name.as_deref(), Some("Ann's Kitchen"));
    assert!(theirs.preview_invite("nope").await.unwrap().is_none());
    let creds = Credentials {
        name: Some("Bob".into()),
        email: "bob@example.com".into(),
        password: "correct horse".into(),
    };
    assert!(
        !theirs
            .accept_invite(token, Some(&creds))
            .await
            .unwrap()
            .verify
    );
    assert!(bob.auth_status().await.signed_in);
    let home = accounts.household().await.unwrap();
    assert_eq!(home.members.len(), 2);
    let bobs = home.members.iter().find(|m| !m.you).unwrap();
    assert_eq!(bobs.name, "Bob");
    assert!(home.invites.is_empty(), "the link was used");

    // The owner takes a member out
    accounts.remove_member(&bobs.id).await.unwrap();
    assert_eq!(accounts.household().await.unwrap().members.len(), 1);
    let bobs_own = theirs.household().await.unwrap();
    assert_ne!(bobs_own.id, home.id, "Bob is on his own again");

    // Devices: this one is current; a second sign-in shows and can be ended
    let phone = Client::new(&server.origin).unwrap();
    phone
        .accounts(Mode::Accounts)
        .sign_in("ann@example.com", "correct horse")
        .await
        .unwrap();
    let devices = accounts.devices().await.unwrap();
    assert_eq!(devices.len(), 2);
    assert_eq!(devices.iter().filter(|d| d.current).count(), 1);
    accounts.sign_out_others().await.unwrap();
    assert_eq!(accounts.devices().await.unwrap().len(), 1);
    assert!(matches!(
        phone.recipes(None, None).await,
        Err(Error::Unauthorized)
    ));

    // The account's own pages
    let methods = accounts.sign_in_methods().await.unwrap();
    assert!(methods.password && methods.linked.is_empty());
    assert!(ann.connected_apps().await.unwrap().is_empty());
    let export = ann.export_account().await.unwrap();
    assert!(
        export.file_name.starts_with("crumb-account-"),
        "{}",
        export.file_name
    );
    let data: Value = serde_json::from_slice(&export.bytes).unwrap();
    assert_eq!(data["households"].as_array().unwrap().len(), 1);

    assert!(matches!(
        ann.delete_account(Some("wrong"), None).await,
        Err(Error::Api { status: 401, .. })
    ));
    ann.delete_account(Some("correct horse"), None)
        .await
        .unwrap();
    assert!(ann.session_cookie().is_none());
}

/// What the Better Auth stub was asked.
#[derive(Clone, Debug)]
struct Seen {
    method: String,
    path: String,
    origin: Option<String>,
    cookie: Option<String>,
    body: Value,
}

type Log = std::sync::Arc<std::sync::Mutex<Vec<Seen>>>;

/// Answers the few Better Auth routes the client calls, with the shapes `auth/src` sends.
async fn better_auth_stub() -> (String, Log) {
    use axum::body::Body;
    use axum::extract::{Request, State};
    use axum::http::{HeaderMap, StatusCode, header};
    use axum::response::{IntoResponse, Response};

    async fn answer(State(log): State<Log>, req: Request) -> Response {
        let (parts, body) = req.into_parts();
        let bytes = axum::body::to_bytes(Body::new(body), 1 << 20)
            .await
            .unwrap();
        let header_of = |name| {
            parts
                .headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(String::from)
        };
        let query = parts.uri.query().unwrap_or("").to_string();
        let seen = Seen {
            method: parts.method.to_string(),
            path: parts.uri.path().to_string(),
            origin: header_of(header::ORIGIN),
            cookie: header_of(header::COOKIE),
            body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        };
        let route = format!("{} {}", seen.method, seen.path);
        log.lock().unwrap().push(seen);
        let ok = |v: Value| (StatusCode::OK, axum::Json(v)).into_response();
        match route.as_str() {
            "POST /api/auth/sign-in/email" => {
                let mut headers = HeaderMap::new();
                headers.insert(
                    header::SET_COOKIE,
                    "crumb.session_token=tok.sig%2B; Path=/; HttpOnly"
                        .parse()
                        .unwrap(),
                );
                (
                    headers,
                    axum::Json(json!({"redirect": false, "token": "tok", "user": {}})),
                )
                    .into_response()
            }
            "POST /api/auth/sign-up/email" => ok(json!({"token": null, "user": {}})),
            "POST /api/auth/sign-out" => ok(json!({"success": true})),
            "GET /api/auth/list-sessions" => ok(json!([
                {"id": "s2", "token": "t2", "userAgent": null, "updatedAt": "2026-02-01T00:00:00.000Z"},
                {"id": "s1", "token": "t1", "userAgent": "Firefox/1 (X11; Linux)", "updatedAt": "2026-01-01T00:00:00.000Z"},
            ])),
            "GET /api/auth/get-session" => {
                ok(json!({"session": {"id": "s1"}, "user": {"id": "u1"}}))
            }
            "GET /api/auth/organization/get-full-organization" => ok(json!({
                "id": "o1", "name": "Kitchen",
                "members": [
                    {"id": "m2", "userId": "u2", "role": "member", "user": {"name": "Bob", "email": "bob@example.com"}},
                    {"id": "m1", "userId": "u1", "role": "owner", "user": {"name": "Ann", "email": "ann@example.com"}},
                ],
                "invitations": [
                    {"id": "i1", "email": "c@example.com", "status": "pending", "expiresAt": "2999-01-01T00:00:00.000Z"},
                    {"id": "i2", "email": "d@example.com", "status": "accepted", "expiresAt": "2999-01-01T00:00:00.000Z"},
                    {"id": "i3", "email": "e@example.com", "status": "pending", "expiresAt": "2000-01-01T00:00:00.000Z"},
                ],
            })),
            "GET /api/auth/organization/list" => ok(json!([
                {"id": "o1", "name": "Kitchen"}, {"id": "o2", "name": "Ann's"},
            ])),
            "POST /api/auth/organization/invite-member" => ok(json!({"id": "inv1"})),
            "GET /api/auth/organization/get-invitation" if query == "id=inv1" => ok(json!({
                "organizationName": "Kitchen", "inviterEmail": "ann@example.com", "email": "c@example.com",
            })),
            "GET /api/auth/organization/get-invitation" => (
                StatusCode::NOT_FOUND,
                axum::Json(json!({"message": "Invitation not found"})),
            )
                .into_response(),
            "POST /api/auth/organization/accept-invitation" => {
                ok(json!({"member": {"organizationId": "o2"}}))
            }
            "GET /api/auth/list-accounts" => ok(json!([
                {"id": "a1", "providerId": "credential", "createdAt": "2026-01-01T00:00:00.000Z"},
                {"id": "a2", "providerId": "google", "createdAt": "2026-03-01T00:00:00.000Z"},
            ])),
            "POST /api/auth/organization/update"
            | "POST /api/auth/organization/cancel-invitation"
            | "POST /api/auth/organization/remove-member"
            | "POST /api/auth/organization/leave"
            | "POST /api/auth/organization/set-active"
            | "POST /api/auth/revoke-session"
            | "POST /api/auth/revoke-other-sessions"
            | "POST /api/auth/unlink-account"
            | "POST /api/auth/request-password-reset"
            | "POST /api/auth/reset-password" => ok(json!({"status": true})),
            _ => (
                StatusCode::NOT_FOUND,
                axum::Json(json!({"message": "Not found"})),
            )
                .into_response(),
        }
    }

    let log = Log::default();
    let app = axum::Router::new().fallback(answer).with_state(log.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (origin, log)
}

/// The requests made to `route` (`"POST /api/auth/sign-out"`), oldest first.
fn seen(log: &Log, route: &str) -> Vec<Seen> {
    log.lock()
        .unwrap()
        .iter()
        .filter(|s| format!("{} {}", s.method, s.path) == route)
        .cloned()
        .collect()
}

#[tokio::test]
async fn hosted_sign_in_keeps_better_auths_cookie() {
    let (origin, log) = better_auth_stub().await;
    let client = Client::new(&origin).unwrap();
    let accounts = client.accounts(Mode::Hosted);
    accounts
        .sign_in("ann@example.com", "correct horse")
        .await
        .unwrap();

    let sent = &seen(&log, "POST /api/auth/sign-in/email")[0];
    assert_eq!(
        sent.body,
        json!({"email": "ann@example.com", "password": "correct horse"})
    );
    assert_eq!(
        sent.origin.as_deref(),
        Some(origin.as_str()),
        "Better Auth wants an origin"
    );

    // Not `crumb_session`: the cookie is kept with its name
    let saved = client.session_cookie().unwrap();
    assert_eq!(saved, "crumb.session_token=tok.sig%2B");
    let restored = Client::with_session(&origin, &saved).unwrap();
    assert_eq!(restored.session_cookie().as_deref(), Some(saved.as_str()));
    restored.accounts(Mode::Hosted).devices().await.unwrap();
    let sent = &seen(&log, "GET /api/auth/list-sessions")[0];
    assert_eq!(
        sent.cookie.as_deref(),
        Some("crumb.session_token=tok.sig%2B")
    );

    // A value saved before hosted mode existed is still a `crumb_session`
    let old = Client::with_session(&origin, "abc.def").unwrap();
    assert_eq!(old.session_cookie().as_deref(), Some("abc.def"));

    accounts.sign_out().await.unwrap();
    assert_eq!(seen(&log, "POST /api/auth/sign-out")[0].body, json!({}));
}

#[tokio::test]
async fn hosted_devices_and_household_map_to_the_shared_shapes() {
    let (origin, log) = better_auth_stub().await;
    let client = Client::new(&origin).unwrap();
    let accounts = client.accounts(Mode::Hosted);

    let devices = accounts.devices().await.unwrap();
    let ids: Vec<&str> = devices.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, ["t1", "t2"], "the current device first, by its token");
    assert!(devices[0].current && !devices[1].current);
    assert_eq!(devices[0].last_seen_at, 1_767_225_600);
    assert_eq!(devices[1].user_agent, None);
    accounts.sign_out_device("t2").await.unwrap();
    assert_eq!(
        seen(&log, "POST /api/auth/revoke-session")[0].body,
        json!({"token": "t2"})
    );
    accounts.sign_out_others().await.unwrap();
    assert_eq!(seen(&log, "POST /api/auth/revoke-other-sessions").len(), 1);

    let home = accounts.household().await.unwrap();
    assert_eq!(
        (home.id.as_str(), home.name.as_str(), home.role.as_str()),
        ("o1", "Kitchen", "owner")
    );
    let members: Vec<(&str, bool)> = home
        .members
        .iter()
        .map(|m| (m.id.as_str(), m.you))
        .collect();
    assert_eq!(members, [("m1", true), ("m2", false)], "the owner first");
    assert_eq!(
        home.invites.len(),
        1,
        "only the pending one that hasn't expired"
    );
    assert_eq!(home.invites[0].id, "i1");
    assert_eq!(home.invites[0].email.as_deref(), Some("c@example.com"));
    assert_eq!(home.households.len(), 2);

    accounts.rename("Home").await.unwrap();
    assert_eq!(
        seen(&log, "POST /api/auth/organization/update")[0].body,
        json!({"data": {"name": "Home"}})
    );
    accounts.cancel_invite("i1").await.unwrap();
    assert_eq!(
        seen(&log, "POST /api/auth/organization/cancel-invitation")[0].body,
        json!({"invitationId": "i1"})
    );
    accounts.remove_member("m2").await.unwrap();
    assert_eq!(
        seen(&log, "POST /api/auth/organization/remove-member")[0].body,
        json!({"memberIdOrEmail": "m2"})
    );
    accounts.leave("o1").await.unwrap();
    assert_eq!(
        seen(&log, "POST /api/auth/organization/leave")[0].body,
        json!({"organizationId": "o1"})
    );
    accounts.switch_to("o2").await.unwrap();
    assert_eq!(
        seen(&log, "POST /api/auth/organization/set-active")[0].body,
        json!({"organizationId": "o2"})
    );
}

#[tokio::test]
async fn hosted_invites_sign_up_and_reset() {
    let (origin, log) = better_auth_stub().await;
    let client = Client::new(&origin).unwrap();
    let accounts = client.accounts(Mode::Hosted);

    let link = accounts.invite(Some("c@example.com")).await.unwrap();
    assert_eq!(link.url, format!("{origin}/invite#inv1"));
    assert_eq!(
        seen(&log, "POST /api/auth/organization/invite-member")[0].body,
        json!({"email": "c@example.com", "role": "member"})
    );
    let preview = accounts.preview_invite("inv1").await.unwrap().unwrap();
    assert_eq!(preview.household_name.as_deref(), Some("Kitchen"));
    assert_eq!(preview.invited_by.as_deref(), Some("ann@example.com"));
    assert_eq!(preview.email.as_deref(), Some("c@example.com"));
    assert!(accounts.preview_invite("gone").await.unwrap().is_none());

    // Without a token back, the account waits on its email
    let made = accounts
        .sign_up("Cy", "c@example.com", "correct horse")
        .await
        .unwrap();
    assert!(made.verify);
    let sent = &seen(&log, "POST /api/auth/sign-up/email")[0];
    assert_eq!(sent.body["callbackURL"], "/");
    assert_eq!(sent.body["name"], "Cy");

    // Joining signed out with an existing account: sign in, accept, then work there
    let creds = Credentials {
        name: None,
        email: "c@example.com".into(),
        password: "correct horse".into(),
    };
    assert!(
        !accounts
            .accept_invite("inv1", Some(&creds))
            .await
            .unwrap()
            .verify
    );
    assert_eq!(seen(&log, "POST /api/auth/sign-in/email").len(), 1);
    assert_eq!(
        seen(&log, "POST /api/auth/organization/accept-invitation")[0].body,
        json!({"invitationId": "inv1"})
    );
    assert_eq!(
        seen(&log, "POST /api/auth/organization/set-active")[0].body,
        json!({"organizationId": "o2"})
    );
    // A new account has to verify first, and never reaches the accept
    let fresh = Credentials {
        name: Some("Cy".into()),
        ..creds
    };
    assert!(
        accounts
            .accept_invite("inv1", Some(&fresh))
            .await
            .unwrap()
            .verify
    );
    assert_eq!(
        seen(&log, "POST /api/auth/sign-up/email")[1].body["callbackURL"],
        "/invite#inv1"
    );
    assert_eq!(
        seen(&log, "POST /api/auth/organization/accept-invitation").len(),
        1
    );

    assert!(accounts.can_reset_password());
    accounts.request_reset("c@example.com").await.unwrap();
    assert_eq!(
        seen(&log, "POST /api/auth/request-password-reset")[0].body,
        json!({"email": "c@example.com", "redirectTo": "/reset-password"})
    );
    accounts
        .reset_password("tok", "new password")
        .await
        .unwrap();
    assert_eq!(
        seen(&log, "POST /api/auth/reset-password")[0].body,
        json!({"token": "tok", "newPassword": "new password"})
    );
}

#[tokio::test]
async fn hosted_sign_in_methods_and_unlink() {
    let (origin, log) = better_auth_stub().await;
    let client = Client::new(&origin).unwrap();
    let accounts = client.accounts(Mode::Hosted);

    let methods = accounts.sign_in_methods().await.unwrap();
    assert!(methods.password);
    assert_eq!(methods.linked.len(), 1);
    assert_eq!(methods.linked[0].provider, Provider::Google);
    assert_eq!(methods.linked[0].created_at, 1_772_323_200);

    accounts.unlink(Provider::Google).await.unwrap();
    assert_eq!(
        seen(&log, "POST /api/auth/unlink-account")[0].body,
        json!({"accountId": "a2"})
    );
    // Nothing linked for Apple: nothing to do
    accounts.unlink(Provider::Apple).await.unwrap();
    assert_eq!(seen(&log, "POST /api/auth/unlink-account").len(), 1);
}

#[tokio::test]
async fn a_listed_site_error_carries_its_name() {
    let server = Server::start(None).await;
    let client = Client::new(&server.origin).unwrap();

    let url = "https://www.allrecipes.com/recipe/1/apple-pie/";
    let err = client
        .import(ImportInput::Url(url.into()))
        .await
        .unwrap_err();
    assert_eq!(err.code(), Some("site_terms"));
    assert_eq!(err.site(), Some("Allrecipes"));
    assert!(matches!(err, Error::Api { status: 422, .. }));
}

#[tokio::test]
async fn previews_a_link_before_it_is_saved() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let site = format!("http://{}", listener.local_addr().unwrap());
    let recipe_site = axum::Router::new().route(
        "/soup",
        axum::routing::get(|| async {
            axum::response::Html(format!(
                r#"<html><head><script type="application/ld+json">{}</script></head></html>"#,
                json!({"@type": "Recipe", "name": "Leek Soup",
                    "recipeIngredient": ["2 leeks"], "recipeInstructions": ["Simmer."]})
            ))
        }),
    );
    tokio::spawn(async move { axum::serve(listener, recipe_site).await.unwrap() });
    // The test's recipe site is on this machine
    let server = Server::configured(|config| config.scrape_allow_private = true).await;
    let client = Client::new(&server.origin).unwrap();

    let url = format!("{site}/soup");
    let Preview::Ready { recipe } = client.preview(&url).await.unwrap() else {
        panic!("expected a recipe");
    };
    assert_eq!(recipe.title, "Leek Soup");
    assert_eq!(recipe.url.as_deref(), Some(url.as_str()));

    let imported = client.import(ImportInput::Url(url.clone())).await.unwrap();
    match client.preview(&url).await.unwrap() {
        Preview::Saved { id, title } => {
            assert_eq!((id, title.as_str()), (imported.recipe.id, "Leek Soup"));
        }
        other => panic!("expected saved, got {other:?}"),
    }

    let video = "https://www.youtube.com/watch?v=Xy_djhH3WE4";
    assert!(matches!(
        client.preview(video).await.unwrap(),
        Preview::Import
    ));
    let err = client.preview("nope").await.unwrap_err();
    assert!(matches!(err, Error::Api { status: 400, .. }), "{err:?}");
}
