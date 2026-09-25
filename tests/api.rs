use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::{Layer, ServiceExt};
use tower_http::normalize_path::NormalizePathLayer;

use crumb::{AppState, app, browser::Browser, config::Config, db};

struct TestApp {
    state: AppState,
    _dist: tempfile::TempDir,
}

impl TestApp {
    fn new(password: Option<&str>) -> Self {
        Self::with_config(|c| c.app_password = password.map(String::from))
    }

    fn with_config(configure: impl FnOnce(&mut Config)) -> Self {
        let dist = tempfile::tempdir().unwrap();
        let marker = crumb::web::MARKER;
        for (rel, title) in [
            ("index.html", "home"),
            ("recipes/index.html", "recipes"),
            ("shell/recipe/index.html", "recipe"),
            ("shell/cookbook/index.html", "cookbook"),
            ("add/index.html", "add"),
            ("login/index.html", "login"),
            ("404.html", "missing"),
        ] {
            let path = dist.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(
                &path,
                format!("<!doctype html><title>{title}</title>{marker}"),
            )
            .unwrap();
        }
        std::fs::create_dir_all(dist.path().join("_astro")).unwrap();
        std::fs::write(dist.path().join("_astro/app.abc.js"), "console.log(1)").unwrap();

        let mut config = Config {
            web_dist: dist.path().to_path_buf(),
            ..Config::default()
        };
        configure(&mut config);
        let state = AppState::new(db::open_in_memory().unwrap(), config, Browser::disabled());
        Self { state, _dist: dist }
    }

    async fn send(&self, req: Request<Body>) -> (StatusCode, axum::http::HeaderMap, String) {
        let svc = NormalizePathLayer::trim_trailing_slash().layer(app(self.state.clone()));
        let res = svc.oneshot(req).await.unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            headers,
            String::from_utf8_lossy(&bytes).into_owned(),
        )
    }

    async fn json(&self, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(uri);
        let body = match body {
            Some(b) => {
                req = req.header(header::CONTENT_TYPE, "application/json");
                Body::from(b.to_string())
            }
            None => Body::empty(),
        };
        let (status, _, text) = self.send(req.body(body).unwrap()).await;
        (status, serde_json::from_str(&text).unwrap_or(Value::Null))
    }
}

fn get(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

#[tokio::test]
async fn recipes_crud_search_and_cookbooks() {
    let t = TestApp::new(None);

    let (status, created) = t
        .json(
            "POST",
            "/api/recipes",
            Some(json!({
                "title": "  Lemon Chicken ",
                "url": "https://food.test/lemon",
                "prepTime": "10m",
                "recipeCategory": "Dinner",
                "ingredients": [{"name": null, "items": ["2 lemons", " ", "1 chicken"]}],
                "instructions": [{"items": ["Roast it."]}],
                "nutrition": {"calories": "400"}
            })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["title"], "Lemon Chicken");
    assert_eq!(created["isNew"], true);
    assert_eq!(created["source"], "manual");
    assert_eq!(
        created["ingredients"][0]["items"],
        json!(["2 lemons", "1 chicken"])
    );
    assert!(created["createdAt"].as_str().unwrap().ends_with(".000Z"));
    let id = created["id"].as_i64().unwrap();

    // Same URL is deduplicated
    let (status, dup) = t
        .json(
            "POST",
            "/api/recipes",
            Some(json!({"title": "Other", "url": "https://food.test/lemon"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dup["id"], id);
    assert_eq!(dup["isNew"], false);

    t.json(
        "POST",
        "/api/recipes",
        Some(json!({"title": "Tomato Soup", "ingredients": [{"items": ["tomatoes"]}]})),
    )
    .await;

    let (_, all) = t.json("GET", "/api/recipes", None).await;
    assert_eq!(all.as_array().unwrap().len(), 2);
    assert_eq!(all[0]["title"], "Tomato Soup");
    assert!(all[0].get("ingredients").is_none());

    let (_, found) = t.json("GET", "/api/recipes?q=lemon%20dinner", None).await;
    assert_eq!(found.as_array().unwrap().len(), 1);
    let (_, found) = t.json("GET", "/api/recipes?q=tomatoes", None).await;
    assert_eq!(found[0]["title"], "Tomato Soup");
    let (status, _) = t.json("GET", "/api/recipes?limit=0", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, limited) = t.json("GET", "/api/recipes?limit=1", None).await;
    assert_eq!(limited.as_array().unwrap().len(), 1);

    let (status, patched) = t
        .json(
            "PATCH",
            &format!("/api/recipes/{id}"),
            Some(json!({"notes": "Great", "prepTime": null})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(patched["notes"], "Great");
    assert_eq!(patched["prepTime"], Value::Null);
    assert_eq!(patched["title"], "Lemon Chicken");

    let (status, err) = t.json("GET", "/api/recipes/9999", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        err,
        json!({"statusCode": 404, "statusMessage": "Recipe not found", "message": "Recipe not found"})
    );
    let (status, _) = t.json("GET", "/api/recipes/abc", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Cookbooks
    let (status, book) = t
        .json(
            "POST",
            "/api/cookbooks",
            Some(json!({"name": "Weeknights", "color": "sage"})),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(book["color"], "sage");
    let book_id = book["id"].as_i64().unwrap();
    let (_, added) = t
        .json(
            "POST",
            &format!("/api/cookbooks/{book_id}/recipes"),
            Some(json!({"recipeIds": [id, id, 424242]})),
        )
        .await;
    assert_eq!(added, json!({"added": 1}));
    let (_, books) = t.json("GET", "/api/cookbooks", None).await;
    assert_eq!(books[0]["recipeCount"], 1);
    let (_, ids) = t
        .json("GET", &format!("/api/recipes/{id}/cookbooks"), None)
        .await;
    assert_eq!(ids, json!([book_id]));
    let (_, full) = t
        .json("GET", &format!("/api/cookbooks/{book_id}"), None)
        .await;
    assert_eq!(full["recipes"][0]["id"], id);
    let (_, renamed) = t
        .json(
            "PATCH",
            &format!("/api/cookbooks/{book_id}"),
            Some(json!({"name": "Fast", "description": ""})),
        )
        .await;
    assert_eq!(renamed["name"], "Fast");
    assert_eq!(renamed["description"], Value::Null);
    let (status, _) = t
        .json(
            "PATCH",
            &format!("/api/cookbooks/{book_id}"),
            Some(json!({"color": "neon"})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // Names from the old palette map onto the new one
    let (status, recoloured) = t
        .json(
            "PATCH",
            &format!("/api/cookbooks/{book_id}"),
            Some(json!({"color": "tomato"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(recoloured["color"], "clay");
    let (_, legacy) = t
        .json(
            "POST",
            "/api/cookbooks",
            Some(json!({"name": "Old blue", "color": "ocean"})),
        )
        .await;
    assert_eq!(legacy["color"], "tile");
    let (_, random) = t
        .json("POST", "/api/cookbooks", Some(json!({"name": "Any"})))
        .await;
    assert!(
        crumb::model::BOOK_COLORS.contains(&random["color"].as_str().unwrap()),
        "{random}"
    );
    for name in ["Old blue", "Any"] {
        let (_, books) = t.json("GET", "/api/cookbooks", None).await;
        let id = books
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["name"] == name)
            .unwrap()["id"]
            .as_i64()
            .unwrap();
        t.json("DELETE", &format!("/api/cookbooks/{id}"), None)
            .await;
    }

    // Export → import round trip
    let (status, headers, body) = t.send(get("/api/export")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .contains("crumb-")
    );
    let backup: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(backup["format"], "crumb");
    assert_eq!(backup["recipes"][0]["cookbooks"], json!(["Fast"]));

    let (_, deleted) = t
        .json(
            "POST",
            "/api/recipes/bulk-delete",
            Some(json!({"ids": [id]})),
        )
        .await;
    assert_eq!(deleted, json!({"deleted": 1}));
    let (_, books) = t.json("GET", "/api/cookbooks", None).await;
    assert_eq!(books[0]["recipeCount"], 0);

    let boundary = "XBOUNDARY";
    let multipart = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"backup.json\"\r\n\
         Content-Type: application/json\r\n\r\n{body}\r\n--{boundary}--\r\n"
    );
    let req = Request::builder()
        .method("POST")
        .uri("/api/import/files")
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(multipart))
        .unwrap();
    let (status, _, text) = t.send(req).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let results: Value = serde_json::from_str(&text).unwrap();
    // Recipes without a source URL can't be deduplicated, so both come back
    assert_eq!(results[0]["created"].as_array().unwrap().len(), 2);
    assert_eq!(results[0]["duplicates"], 0);
    let (_, books) = t.json("GET", "/api/cookbooks", None).await;
    assert_eq!(
        books.as_array().unwrap().len(),
        1,
        "import reuses the existing cookbook by name"
    );
    assert_eq!(books[0]["recipeCount"], 1);

    let (status, _) = t
        .json("DELETE", &format!("/api/cookbooks/{book_id}"), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = t
        .json("DELETE", &format!("/api/cookbooks/{book_id}"), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn text_import_and_validation_messages() {
    let t = TestApp::new(None);
    let (status, err) = t
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"text": "short"})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(err["message"], "Paste a bit more of the recipe");
    let (_, err) = t
        .json("POST", "/api/recipes/import", Some(json!({"url": "nope"})))
        .await;
    assert_eq!(err["message"], "Please enter a valid URL");

    let text = "Toast\nIngredients\n1 slice bread\nInstructions\n1. Toast the bread.";
    let (status, res) = t
        .json("POST", "/api/recipes/import", Some(json!({"text": text})))
        .await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["title"], "Toast");
    assert_eq!(res["isNew"], true);

    let (status, _) = t
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"text": "just some words without any recipe"})),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn pages_inline_their_data_and_static_files_are_cached() {
    let t = TestApp::new(None);
    t.json(
        "POST",
        "/api/recipes",
        Some(json!({"title": "Pie </script>", "ingredients": [{"items": ["apples"]}]})),
    )
    .await;

    let (status, headers, html) = t.send(get("/")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CACHE_CONTROL], "no-cache");
    assert_eq!(headers["speculation-rules"], "\"/speculation-rules.json\"");
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert!(html.contains(r#"<script type="application/json" id="page-data">{"recipes":[{"#));
    assert!(html.contains("Pie \\u003c/script>"));

    let (status, _, html) = t.send(get("/recipes/1")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("<title>recipe</title>") && html.contains("\"inCookbooks\":[]"));
    let (status, _, html) = t.send(get("/recipes/99")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(html.contains("missing"));

    let (status, _, html) = t.send(get("/recipes/?q=pie")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("\"q\":\"pie\""));

    let (status, _, html) = t.send(get("/add/")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("<title>add</title>"));

    let (status, headers, _) = t.send(get("/_astro/app.abc.js")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );

    for path in [
        "/shell/recipe",
        "/shell/recipe/index.html",
        "/nope",
        "/nope.js",
    ] {
        let (status, _, _) = t.send(get(path)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn auth_redirects_and_sessions() {
    let t = TestApp::new(Some("secret"));

    let (status, headers, _) = t.send(get("/recipes?q=a%20b")).await;
    assert_eq!(status, StatusCode::FOUND);
    assert_eq!(
        headers[header::LOCATION],
        "/login?next=%2Frecipes%3Fq%3Da%2520b"
    );
    let (status, err) = t.json("GET", "/api/recipes", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(err["message"], "Not signed in");
    let (status, _) = t.json("GET", "/api/health", None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = t.send(get("/login")).await;
    assert_eq!(status, StatusCode::OK);

    let (status, err) = t
        .json(
            "POST",
            "/api/auth/login",
            Some(json!({"password": "wrong"})),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(err["message"], "Incorrect password");

    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-forwarded-proto", "https")
        .body(Body::from(r#"{"password":"secret"}"#))
        .unwrap();
    let (status, headers, _) = t.send(req).await;
    assert_eq!(status, StatusCode::OK);
    let cookie = headers[header::SET_COOKIE].to_str().unwrap();
    assert!(
        cookie.starts_with("crumb_session=")
            && cookie.contains("HttpOnly")
            && cookie.contains("Secure")
    );
    let pair = cookie.split(';').next().unwrap();

    let req = Request::builder()
        .uri("/api/recipes")
        .header(header::COOKIE, pair)
        .body(Body::empty())
        .unwrap();
    let (status, _, _) = t.send(req).await;
    assert_eq!(status, StatusCode::OK);
    let req = Request::builder()
        .uri("/login")
        .header(header::COOKIE, pair)
        .body(Body::empty())
        .unwrap();
    let (status, headers, _) = t.send(req).await;
    assert_eq!(status, StatusCode::FOUND);
    assert_eq!(headers[header::LOCATION], "/");

    // MCP needs a bearer token when auth is on
    let req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#))
        .unwrap();
    let (status, headers, _) = t.send(req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        headers[header::WWW_AUTHENTICATE]
            .to_str()
            .unwrap()
            .contains("oauth-protected-resource/mcp")
    );
}

#[tokio::test]
async fn oauth_flow_then_mcp_tools() {
    let t = TestApp::new(Some("secret"));

    let (status, client) = t
        .json(
            "POST",
            "/oauth/register",
            Some(json!({"client_name": "Claude", "redirect_uris": ["https://claude.ai/api/mcp/auth_callback"]})),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let client_id = client["client_id"].as_str().unwrap().to_string();

    let verifier = "a-very-long-code-verifier-string-with-enough-entropy-1234567890";
    use base64::Engine;
    use sha2::Digest;
    let challenge =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier));
    let redirect = "https://claude.ai/api/mcp/auth_callback";

    let q = serde_urlencoded::to_string([
        ("client_id", client_id.as_str()),
        ("redirect_uri", redirect),
        ("state", "xyz"),
        ("code_challenge", challenge.as_str()),
        ("code_challenge_method", "S256"),
        ("response_type", "code"),
    ])
    .unwrap();
    let (status, _, html) = t.send(get(&format!("/oauth/authorize?{q}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("Connect Claude?") && html.contains("App password"));

    let form = format!("{q}&action=allow&password=secret");
    let req = Request::builder()
        .method("POST")
        .uri("/oauth/authorize")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap();
    let (status, headers, _) = t.send(req).await;
    assert_eq!(status, StatusCode::FOUND);
    let location = url::Url::parse(headers[header::LOCATION].to_str().unwrap()).unwrap();
    let code = location
        .query_pairs()
        .find(|(k, _)| k == "code")
        .unwrap()
        .1
        .into_owned();
    assert_eq!(
        location
            .query_pairs()
            .find(|(k, _)| k == "state")
            .unwrap()
            .1,
        "xyz"
    );

    let token_form = serde_urlencoded::to_string([
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("code_verifier", verifier),
        ("client_id", client_id.as_str()),
        ("redirect_uri", redirect),
    ])
    .unwrap();
    let req = Request::builder()
        .method("POST")
        .uri("/oauth/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(token_form.clone()))
        .unwrap();
    let (status, _, text) = t.send(req).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let tokens: Value = serde_json::from_str(&text).unwrap();
    let access = tokens["access_token"].as_str().unwrap().to_string();

    // Codes are single use
    let req = Request::builder()
        .method("POST")
        .uri("/oauth/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(token_form))
        .unwrap();
    let (status, _, text) = t.send(req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(text.contains("invalid_grant"));

    let rpc = |body: Value| {
        Request::builder()
            .method("POST")
            .uri("/mcp")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {access}"))
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let (status, _, text) = t
        .send(rpc(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}})))
        .await;
    assert_eq!(status, StatusCode::OK);
    let init: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(init["result"]["serverInfo"]["name"], "crumb");

    let (status, _, _) = t
        .send(rpc(
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        ))
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);

    let (_, _, text) = t
        .send(rpc(
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        ))
        .await;
    let list: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 17);

    let (_, _, text) = t
        .send(rpc(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "save_recipe",
            "arguments": {"title": "Salad", "ingredients": [{"name": null, "items": ["lettuce"]}],
                          "instructions": [{"name": null, "items": ["Toss."]}]}}})))
        .await;
    let saved: Value = serde_json::from_str(&text).unwrap();
    let msg = saved["result"]["content"][0]["text"].as_str().unwrap();
    assert!(msg.starts_with("Saved: \"Salad\" (id 1)"), "{msg}");
    assert!(msg.ends_with("1 ingredients, 1 steps."));

    let (_, _, text) = t
        .send(rpc(json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": {"name": "add_to_cookbook", "arguments": {"cookbook": "Greens", "recipeIds": [1]}}})))
        .await;
    assert!(
        text.contains("Added 1 recipe(s) to \\\"Greens\\\""),
        "{text}"
    );

    let (_, _, text) = t
        .send(rpc(
            json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call",
            "params": {"name": "get_recipe", "arguments": {"id": 1}}}),
        ))
        .await;
    assert!(text.contains("# Salad") && text.contains("1. Toss."));

    let (_, _, text) = t
        .send(rpc(
            json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call",
            "params": {"name": "get_recipe", "arguments": {"id": 99}}}),
        ))
        .await;
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["result"]["isError"], true);

    let (_, _, text) = t
        .send(rpc(json!({"jsonrpc": "2.0", "id": 7, "method": "nope"})))
        .await;
    assert!(text.contains("-32601"));
}

#[tokio::test]
async fn mcp_organising_tools() {
    let t = TestApp::new(None);

    // A recipe page for refresh_recipe_from_source to scrape
    let page = r#"<html><head><script type="application/ld+json">{"@context":"https://schema.org","@type":"Recipe",
        "name":"Tomato Soup","image":"https://img.test/soup.jpg","totalTime":"PT40M","recipeYield":"4",
        "recipeCategory":"Soup","recipeIngredient":["2 lb tomatoes","1 onion"],
        "recipeInstructions":[{"@type":"HowToStep","text":"Roast."},{"@type":"HowToStep","text":"Blend."}]}</script>
        </head><body></body></html>"#;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let source = format!("http://{}/soup", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/soup",
            axum::routing::get(move || async move { axum::response::Html(page) }),
        );
        axum::serve(listener, app).await.unwrap();
    });

    let mut seq = 0;
    let mut call = async |name: &str, arguments: Value| -> (String, bool) {
        seq += 1;
        let req = Request::builder()
            .method("POST")
            .uri("/mcp")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({"jsonrpc": "2.0", "id": seq, "method": "tools/call",
                    "params": {"name": name, "arguments": arguments}})
                .to_string(),
            ))
            .unwrap();
        let (_, _, body) = t.send(req).await;
        let v: Value = serde_json::from_str(&body).unwrap();
        let text = v["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default();
        (text.to_string(), v["result"]["isError"] == true)
    };

    let (msg, _) = call(
        "save_recipe",
        json!({"title": "My Soup", "url": source, "recipeCategory": "Mine",
            "ingredients": [{"items": ["tomatoes"]}], "instructions": [{"items": ["Cook."]}]}),
    )
    .await;
    assert!(msg.contains("(id 1)"), "{msg}");
    call(
        "save_recipe",
        json!({"title": "Toast", "image": "https://img.test/toast.jpg",
            "ingredients": [{"items": ["bread"]}], "instructions": [{"items": ["Toast."]}]}),
    )
    .await;

    // Missing-field filters
    let (msg, _) = call("search_recipes", json!({"missing": ["image"]})).await;
    assert!(
        msg.starts_with("1 recipe(s) missing image") && msg.contains("My Soup"),
        "{msg}"
    );
    let (msg, _) = call("search_recipes", json!({"missing": ["cookbook"]})).await;
    assert!(msg.starts_with("2 recipe(s)"), "{msg}");
    let (msg, err) = call("search_recipes", json!({"missing": ["colour"]})).await;
    assert!(err && msg.contains("unknown field"), "{msg}");

    // Refresh fills only the blanks
    let (msg, _) = call("refresh_recipe_from_source", json!({"id": 1})).await;
    assert!(
        msg.starts_with("Updated image, totalTime, recipeYield on \"My Soup\""),
        "{msg}"
    );
    let (msg, _) = call("get_recipe", json!({"id": 1})).await;
    assert!(
        msg.contains("# My Soup") && msg.contains("tomatoes") && !msg.contains("Blend"),
        "{msg}"
    );
    let (msg, _) = call("search_recipes", json!({"missing": ["image"]})).await;
    assert_eq!(msg, "No recipes missing image.");
    let (msg, _) = call("refresh_recipe_from_source", json!({"id": 1})).await;
    assert!(msg.starts_with("Nothing to fill in"), "{msg}");
    let (msg, _) = call(
        "refresh_recipe_from_source",
        json!({"id": 1, "overwrite": ["instructions", "recipeCategory"]}),
    )
    .await;
    assert!(
        msg.starts_with("Updated recipeCategory, instructions"),
        "{msg}"
    );
    let (msg, err) = call("refresh_recipe_from_source", json!({"id": 2})).await;
    assert!(err && msg.contains("no source URL"), "{msg}");

    // Cookbooks
    call(
        "add_to_cookbook",
        json!({"cookbook": "Soups", "recipeIds": [1, 2]}),
    )
    .await;
    call(
        "add_to_cookbook",
        json!({"cookbook": "Other", "recipeIds": [2]}),
    )
    .await;
    let (msg, _) = call("get_cookbook", json!({"cookbook": "soups"})).await;
    assert!(
        msg.contains("2 recipe(s):") && msg.contains("Toast"),
        "{msg}"
    );
    let (msg, _) = call(
        "remove_from_cookbook",
        json!({"cookbook": "Soups", "recipeIds": [2]}),
    )
    .await;
    assert!(
        msg.starts_with("Removed 1 recipe(s) from \"Soups\""),
        "{msg}"
    );
    let (msg, _) = call("search_recipes", json!({"missing": ["cookbook"]})).await;
    assert_eq!(msg, "No recipes missing cookbook.");

    let (msg, _) = call(
        "update_cookbook",
        json!({"cookbook": "Soups", "name": "Soup & Stew", "color": "sage", "description": "Warm things"}),
    )
    .await;
    assert!(
        msg.starts_with("Updated cookbook \"Soup & Stew\" (id 1, sage)"),
        "{msg}"
    );
    let (msg, err) = call("update_cookbook", json!({"cookbook": 1, "color": "neon"})).await;
    assert!(err && msg.contains("expected one of"), "{msg}");
    let (msg, err) = call("update_cookbook", json!({"cookbook": 1, "color": "plum"})).await;
    assert!(!err && msg.contains("(id 1, forest)"), "{msg}");
    let (msg, err) = call("update_cookbook", json!({"cookbook": 1, "name": "other"})).await;
    assert!(err && msg.contains("already called \"Other\""), "{msg}");
    let (msg, err) = call("get_cookbook", json!({"cookbook": 99})).await;
    assert!(err && msg.contains("No cookbook with id 99"), "{msg}");

    // Deletes need a confirmed second call
    let (msg, err) = call("delete_cookbook", json!({"cookbook": "Other"})).await;
    assert!(!err && msg.starts_with("Not deleted yet"), "{msg}");
    let (msg, _) = call("list_cookbooks", json!({})).await;
    assert!(msg.contains("Other"), "{msg}");
    let (msg, _) = call(
        "delete_cookbook",
        json!({"cookbook": "Other", "confirm": true}),
    )
    .await;
    assert!(msg.starts_with("Deleted the cookbook \"Other\""), "{msg}");
    let (msg, _) = call("list_cookbooks", json!({})).await;
    assert!(
        !msg.contains("Other") && msg.contains("Soup & Stew"),
        "{msg}"
    );

    let (msg, _) = call("delete_recipe", json!({"ids": [1, 2]})).await;
    assert!(
        msg.starts_with("Not deleted yet") && msg.contains("[2] Toast"),
        "{msg}"
    );
    let (msg, _) = call("search_recipes", json!({})).await;
    assert!(msg.starts_with("2 recipe(s)"), "{msg}");
    let (msg, err) = call("delete_recipe", json!({"ids": [1, 99], "confirm": true})).await;
    assert!(err && msg.contains("No recipe with id 99"), "{msg}");
    let (msg, _) = call("delete_recipe", json!({"id": 2, "confirm": true})).await;
    assert!(msg.starts_with("Deleted 1 recipe(s)"), "{msg}");
    let (msg, _) = call("search_recipes", json!({})).await;
    assert!(msg.starts_with("1 recipe(s)"), "{msg}");
}

/// Saves a small recipe and returns its id.
async fn add_recipe(t: &TestApp, title: &str, category: &str, ingredient: &str, time: &str) -> i64 {
    let (status, r) = t
        .json(
            "POST",
            "/api/recipes",
            Some(json!({
                "title": title,
                "image": "https://img.test/x.jpg",
                "totalTime": time,
                "recipeCategory": category,
                "ingredients": [{"items": [ingredient, "salt"]}],
                "instructions": [{"items": ["Cook it.", "Eat it."]}]
            })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{r}");
    r["id"].as_i64().unwrap()
}

#[tokio::test]
async fn cook_log_and_views() {
    let t = TestApp::new(None);
    let id = add_recipe(&t, "Chili", "Main", "1 lb beef", "1h").await;

    let (status, stats) = t
        .json("POST", &format!("/api/recipes/{id}/cooked"), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stats["count"], 1);
    assert!(stats["lastCookedAt"].as_str().unwrap().ends_with("Z"));
    // Finishing again straight away counts once
    let (_, stats) = t
        .json("POST", &format!("/api/recipes/{id}/cooked"), None)
        .await;
    assert_eq!(stats["count"], 1);

    let (_, _, html) = t.send(get(&format!("/recipes/{id}"))).await;
    assert!(html.contains(r#""cookStats":{"count":1"#), "{html}");

    let (_, stats) = t
        .json("DELETE", &format!("/api/recipes/{id}/cooked"), None)
        .await;
    assert_eq!(stats["count"], 0);
    assert_eq!(stats["lastCookedAt"], Value::Null);

    let (status, _) = t
        .json("POST", &format!("/api/recipes/{id}/viewed"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, err) = t.json("POST", "/api/recipes/999/cooked", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(err["statusCode"], 404);

    // The cook log goes into backups and comes back on restore, without doubling
    t.json("POST", &format!("/api/recipes/{id}/cooked"), None)
        .await;
    let (_, _, backup) = t.send(get("/api/export")).await;
    let backup: Value = serde_json::from_str(&backup).unwrap();
    assert_eq!(
        backup["recipes"][0]["cookedAt"].as_array().unwrap().len(),
        1
    );
    let fresh = TestApp::new(None);
    for _ in 0..2 {
        let boundary = "XBOUNDARY";
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"crumb.json\"\r\nContent-Type: application/json\r\n\r\n{backup}\r\n--{boundary}--\r\n"
        );
        let req = Request::builder()
            .method("POST")
            .uri("/api/import/files")
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();
        let (status, _, _) = fresh.send(req).await;
        assert_eq!(status, StatusCode::OK);
    }
    let (_, restored) = fresh.json("GET", "/api/recipes", None).await;
    let rid = restored[0]["id"].as_i64().unwrap();
    let (_, _, html) = fresh.send(get(&format!("/recipes/{rid}"))).await;
    assert!(html.contains(r#""cookStats":{"count":1"#));

    // Deleting the recipe clears its events
    t.json("DELETE", &format!("/api/recipes/{id}"), None).await;
    let left: i64 = t
        .state
        .db
        .lock()
        .query_row("SELECT count(*) FROM recipe_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(left, 0);
}

#[tokio::test]
async fn suggestions_and_random() {
    let t = TestApp::new(None);
    let (status, err) = t.json("GET", "/api/recipes/random", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(err["statusCode"], 404);
    let (status, headers, _) = t.send(get("/random")).await;
    assert_eq!(status, StatusCode::FOUND);
    assert_eq!(headers[header::LOCATION], "/");

    let mut ids = Vec::new();
    for (title, cat, ing) in [
        ("Lemon Chicken", "Main", "1 chicken"),
        ("Beef Stew", "Main", "2 lb beef"),
        ("Salmon Bowl", "Main", "salmon"),
        ("Tofu Stir Fry", "Main", "tofu"),
        ("Pork Tacos", "Main", "pork shoulder"),
        ("Shrimp Pasta", "Main", "shrimp"),
    ] {
        ids.push(add_recipe(&t, title, cat, ing, "30m").await);
    }

    let (_, _, html) = t.send(get("/")).await;
    assert!(
        html.contains(r#""suggestions":{"#) && html.contains(r#""items":[{"#),
        "{html}"
    );

    let (status, s) = t.json("GET", "/api/suggestions?limit=4", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(s["ai"], "off");
    let items = s["items"].as_array().unwrap();
    assert_eq!(items.len(), 4);
    assert!(
        items
            .iter()
            .all(|i| !i["reason"].as_str().unwrap().is_empty())
    );
    assert!(items[0]["recipe"]["title"].is_string());

    // Cooked recipes and excluded ones are left out; a new seed shuffles
    t.json("POST", &format!("/api/recipes/{}/cooked", ids[0]), None)
        .await;
    let first: Vec<i64> = {
        let (_, s) = t.json("GET", "/api/suggestions?limit=4", None).await;
        s["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["recipe"]["id"].as_i64().unwrap())
            .collect()
    };
    assert!(!first.contains(&ids[0]));
    let exclude = first
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let (_, s) = t
        .json(
            "GET",
            &format!("/api/suggestions?limit=4&exclude={exclude},junk"),
            None,
        )
        .await;
    let next: Vec<i64> = s["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["recipe"]["id"].as_i64().unwrap())
        .collect();
    assert!(!next.is_empty() && next.iter().all(|id| !first.contains(id) && *id != ids[0]));
    let (status, _) = t.json("GET", "/api/suggestions?limit=99", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Random: a no-store redirect, and it honours exclude
    let (status, headers, _) = t.send(get("/random")).await;
    assert_eq!(status, StatusCode::FOUND);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    let loc = headers[header::LOCATION].to_str().unwrap();
    assert!(
        loc.starts_with("/recipes/") && loc.ends_with("?from=random"),
        "{loc}"
    );
    let all_but_last = ids[..5]
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let (_, headers, _) = t
        .send(get(&format!("/random?exclude={all_but_last}")))
        .await;
    assert_eq!(
        headers[header::LOCATION],
        format!("/recipes/{}?from=random", ids[5])
    );
    let (_, r) = t
        .json(
            "GET",
            &format!("/api/recipes/random?exclude={all_but_last}"),
            None,
        )
        .await;
    assert_eq!(r["id"], ids[5]);
}

async fn mcp_call(t: &TestApp, name: &str, arguments: Value) -> (String, bool) {
    let req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": name, "arguments": arguments}})
            .to_string(),
        ))
        .unwrap();
    let (_, _, body) = t.send(req).await;
    let v: Value = serde_json::from_str(&body).unwrap();
    let text = v["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    (text.to_string(), v["result"]["isError"] == true)
}

#[tokio::test]
async fn mcp_suggest_tools() {
    let t = TestApp::new(None);
    let chili = add_recipe(&t, "Chili", "Main", "1 lb beef", "1h").await;
    add_recipe(&t, "Salad", "Salad", "lettuce", "10m").await;
    add_recipe(&t, "Soup", "Soup", "onions", "45m").await;

    let req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}).to_string(),
        ))
        .unwrap();
    let (_, _, body) = t.send(req).await;
    for tool in ["suggest_recipes", "random_recipe", "mark_recipe_cooked"] {
        assert!(body.contains(tool), "{tool}");
    }

    let (out, err) = mcp_call(&t, "suggest_recipes", json!({})).await;
    assert!(
        !err && out.contains("Chili") && out.contains("/recipes/"),
        "{out}"
    );
    let (out, _) = mcp_call(&t, "suggest_recipes", json!({"maxMinutes": 20})).await;
    assert!(out.contains("Salad") && !out.contains("Chili"), "{out}");
    let (out, _) = mcp_call(&t, "random_recipe", json!({"query": "onions"})).await;
    assert!(out.contains("Soup"), "{out}");

    let (out, err) = mcp_call(&t, "mark_recipe_cooked", json!({"id": chili, "daysAgo": 1})).await;
    assert!(
        !err && out.contains("Marked \"Chili\" as cooked yesterday"),
        "{out}"
    );
    let (out, _) = mcp_call(&t, "mark_recipe_cooked", json!({"id": chili, "daysAgo": 1})).await;
    assert!(out.starts_with("Already marked"), "{out}");
    let (out, _) = mcp_call(&t, "suggest_recipes", json!({})).await;
    assert!(!out.contains("Chili"), "{out}");
    let (_, err) = mcp_call(
        &t,
        "mark_recipe_cooked",
        json!({"id": chili, "daysAgo": 90}),
    )
    .await;
    assert!(err);
    let (_, err) = mcp_call(&t, "mark_recipe_cooked", json!({"id": 999})).await;
    assert!(err);
}

/// A fake AI API: answers Anthropic and OpenAI-style requests by picking the first two
/// candidates plus one made-up id; under /fail it always errors.
async fn fake_ai() -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let calls = std::sync::Arc::new(AtomicUsize::new(0));
    let reply = |body: &Value| -> String {
        let user = body["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["role"] == "user")
            .unwrap()["content"]
            .as_str()
            .unwrap()
            .to_string();
        let payload: Value = serde_json::from_str(&user).unwrap();
        let ids: Vec<i64> = payload["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].as_i64().unwrap())
            .collect();
        json!({"picks": [
            {"id": 424242, "blurb": "Not in your box"},
            {"id": ids[1], "blurb": "Bright and quick for a weeknight"},
            {"id": ids[0], "blurb": "You haven't tried this one yet"}
        ]})
        .to_string()
    };
    let (c1, c2, c3) = (calls.clone(), calls.clone(), calls.clone());
    let app = axum::Router::new()
        .route(
            "/v1/messages",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| async move {
                c1.fetch_add(1, Ordering::SeqCst);
                axum::Json(json!({"stop_reason": "end_turn", "content": [{"type": "text", "text": reply(&body)}]}))
            }),
        )
        .route(
            "/v1/chat/completions",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| async move {
                c2.fetch_add(1, Ordering::SeqCst);
                assert_eq!(body["response_format"]["type"], "json_schema");
                axum::Json(json!({"choices": [{"finish_reason": "stop", "message": {"role": "assistant", "content": reply(&body), "refusal": null}}]}))
            }),
        )
        .route(
            "/fail/v1/messages",
            axum::routing::post(move || async move {
                c3.fetch_add(1, Ordering::SeqCst);
                (StatusCode::INTERNAL_SERVER_ERROR, "boom")
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, calls)
}

async fn wait_for_ai(t: &TestApp) -> Value {
    for _ in 0..50 {
        let (_, s) = t.json("GET", "/api/suggestions", None).await;
        if s["ai"] != "pending" {
            return s;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("AI suggestions never finished");
}

#[tokio::test]
async fn ai_suggestions_cache_and_fallback() {
    use crumb::config::{LlmConfig, LlmProvider};
    use std::sync::atomic::Ordering;
    let (base, calls) = fake_ai().await;

    for (provider, url) in [
        (LlmProvider::Anthropic, base.clone()),
        (LlmProvider::OpenAi, format!("{base}/v1")),
    ] {
        let before = calls.load(Ordering::SeqCst);
        let t = TestApp::with_config(|c| {
            let mut llm = LlmConfig::new(provider, "test-key");
            llm.base_url = url.clone();
            c.llm = Some(llm);
        });
        for (title, ing) in [
            ("Chicken", "chicken"),
            ("Beef", "beef"),
            ("Tofu", "tofu"),
            ("Salmon", "salmon"),
            ("Pork", "pork"),
        ] {
            add_recipe(&t, title, "Main", ing, "30m").await;
        }
        let (_, s) = t.json("GET", "/api/suggestions", None).await;
        assert_eq!(s["ai"], "pending");
        let s = wait_for_ai(&t).await;
        assert_eq!(s["ai"], "ready", "{provider:?}: {s}");
        let items = s["items"].as_array().unwrap();
        assert_eq!(items.len(), 4);
        assert_eq!(items[0]["ai"], true);
        assert_eq!(items[0]["reasonKind"], "ai");
        assert_eq!(items[0]["reason"], "Bright and quick for a weeknight");
        assert_eq!(items[1]["reason"], "You haven't tried this one yet");
        assert_eq!(items[2]["ai"], false);
        assert!(items.iter().all(|i| i["recipe"]["id"] != 424242));
        // Cached: no second call, and shuffles never call
        t.json("GET", "/api/suggestions", None).await;
        t.json("GET", "/api/suggestions?seed=1", None).await;
        assert_eq!(calls.load(Ordering::SeqCst), before + 1, "{provider:?}");
        // The home page serves the cached blurbs too
        let (_, _, html) = t.send(get("/")).await;
        assert!(html.contains("Bright and quick for a weeknight"));
    }

    // A failing API: the algorithm's list stands, and there's no retry storm
    let before = calls.load(Ordering::SeqCst);
    let t = TestApp::with_config(|c| {
        let mut llm = LlmConfig::new(LlmProvider::Anthropic, "test-key");
        llm.base_url = format!("{base}/fail");
        c.llm = Some(llm);
    });
    for (title, ing) in [("Chicken", "chicken"), ("Beef", "beef"), ("Tofu", "tofu")] {
        add_recipe(&t, title, "Main", ing, "30m").await;
    }
    t.json("GET", "/api/suggestions", None).await;
    let s = wait_for_ai(&t).await;
    assert_eq!(s["ai"], "off");
    assert_eq!(s["items"].as_array().unwrap().len(), 3);
    assert!(
        s["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["ai"] == false)
    );
    t.json("GET", "/api/suggestions", None).await;
    // One request plus its single retry, then the failure is remembered
    assert_eq!(calls.load(Ordering::SeqCst), before + 2);
}

#[tokio::test]
async fn undo_only_removes_its_own_cook() {
    let t = TestApp::new(None);
    let id = add_recipe(&t, "Chili", "Main", "1 lb beef", "1h").await;
    let (_, first) = t
        .json("POST", &format!("/api/recipes/{id}/cooked"), None)
        .await;
    let event = first["eventId"].as_i64().unwrap();
    // A second tap soon after is a repeat: nothing to undo
    let (_, again) = t
        .json("POST", &format!("/api/recipes/{id}/cooked"), None)
        .await;
    assert_eq!(again["eventId"], Value::Null);
    assert_eq!(again["count"], 1);
    let (_, stats) = t
        .json(
            "DELETE",
            &format!("/api/recipes/{id}/cooked?event={}", event + 100),
            None,
        )
        .await;
    assert_eq!(stats["count"], 1);
    let (_, stats) = t
        .json(
            "DELETE",
            &format!("/api/recipes/{id}/cooked?event={event}"),
            None,
        )
        .await;
    assert_eq!(stats["count"], 0);
}

fn png_bytes(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([x as u8, y as u8, 120]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn clear_image(t: &TestApp, id: i64) {
    t.state
        .db
        .lock()
        .execute("UPDATE recipes SET image = NULL WHERE id = ?1", [id])
        .unwrap();
}

fn set_image(t: &TestApp, id: i64, image: &str) {
    t.state
        .db
        .lock()
        .execute(
            "UPDATE recipes SET image = ?1 WHERE id = ?2",
            rusqlite::params![image, id],
        )
        .unwrap();
}

async fn send_raw(t: &TestApp, req: Request<Body>) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let svc = NormalizePathLayer::trim_trailing_slash().layer(app(t.state.clone()));
    let res = svc.oneshot(req).await.unwrap();
    let (parts, body) = res.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes().to_vec();
    (parts.status, parts.headers, bytes)
}

fn webp_size(bytes: &[u8]) -> (u32, u32) {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::WebP).unwrap();
    (img.width(), img.height())
}

#[tokio::test]
async fn sized_images_from_data_uris() {
    use base64::Engine;
    let cache = tempfile::tempdir().unwrap();
    let dir = cache.path().join("img-cache");
    let t = TestApp::with_config(|c| c.image_cache = Some(dir.clone()));
    let id = add_recipe(&t, "Photo", "Dinner", "eggs", "10 min").await;
    let bare = add_recipe(&t, "No photo", "Dinner", "eggs", "10 min").await;
    clear_image(&t, bare);

    // No image, unknown recipe, junk ids: 404
    for uri in [
        format!("/img/{bare}/320"),
        "/img/9999/320".to_string(),
        "/img/abc/320".to_string(),
        format!("/img/{id}/wide"),
    ] {
        let (status, _, _) = send_raw(&t, get(&uri)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }

    let b64 = base64::engine::general_purpose::STANDARD.encode(png_bytes(1600, 800));
    let image = format!("data:image/png;base64,{b64}");
    set_image(&t, id, &image);
    let key = crumb::images::image_key(&image);

    let (status, headers, body) = send_raw(&t, get(&format!("/img/{id}/700?v={key}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/webp");
    assert_eq!(
        headers[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    // 700 snaps to 768
    assert_eq!(webp_size(&body), (768, 384));
    assert!(dir.join(format!("{id}-{key}-768.webp")).exists());
    let etag = headers[header::ETAG].to_str().unwrap().to_string();
    assert_eq!(etag, format!("\"{key}-768\""));

    // Served from the cache, still revalidatable
    let req = Request::builder()
        .uri(format!("/img/{id}/768?v={key}"))
        .header(header::IF_NONE_MATCH, &etag)
        .body(Body::empty())
        .unwrap();
    let (status, headers, body) = send_raw(&t, req).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    assert!(body.is_empty());
    assert_eq!(headers[header::ETAG], etag.as_str());

    // Stale or missing key: the current image, but not cached forever
    for uri in [
        format!("/img/{id}/1200?v=deadbeef"),
        format!("/img/{id}/1200"),
    ] {
        let (status, headers, body) = send_raw(&t, get(&uri)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CACHE_CONTROL], "no-cache");
        assert_eq!(webp_size(&body), (1200, 600));
    }

    // Never upscaled
    let small = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png_bytes(200, 100))
    );
    set_image(&t, id, &small);
    let (status, _, body) = send_raw(&t, get(&format!("/img/{id}/1200"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(webp_size(&body), (200, 100));

    // Broken image: 404
    set_image(&t, id, "data:image/png;base64,bm90IGFuIGltYWdl");
    let (status, _, _) = send_raw(&t, get(&format!("/img/{id}/320"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sized_images_are_behind_the_login() {
    let t = TestApp::new(Some("pw"));
    let (status, headers, _) = send_raw(&t, get("/img/1/320")).await;
    assert_eq!(status, StatusCode::FOUND);
    assert!(
        headers[header::LOCATION]
            .to_str()
            .unwrap()
            .starts_with("/login")
    );
}

#[tokio::test]
async fn sized_images_fetch_remote_photos_and_remember_failures() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let hits = Arc::new(AtomicUsize::new(0));
    let png = png_bytes(900, 600);
    let counter = hits.clone();
    let origin = axum::Router::new()
        .route(
            "/photo.png",
            axum::routing::get(move || {
                let png = png.clone();
                async move { ([(header::CONTENT_TYPE, "image/png")], png) }
            }),
        )
        .route(
            "/broken.jpg",
            axum::routing::get(move || {
                counter.fetch_add(1, Ordering::SeqCst);
                async {
                    (
                        [(header::CONTENT_TYPE, "text/html")],
                        "<html>blocked</html>",
                    )
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, origin).await.unwrap() });

    let t = TestApp::new(None);
    let id = add_recipe(&t, "Remote", "Dinner", "eggs", "10 min").await;
    set_image(&t, id, &format!("http://{addr}/photo.png"));
    let (status, headers, body) = send_raw(&t, get(&format!("/img/{id}/320"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/webp");
    assert_eq!(webp_size(&body), (320, 213));

    set_image(&t, id, &format!("http://{addr}/broken.jpg"));
    for _ in 0..3 {
        let (status, _, _) = send_raw(&t, get(&format!("/img/{id}/320"))).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    let (status, _, _) = send_raw(&t, get(&format!("/img/{id}/768"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn recipe_pages_preload_the_hero() {
    let t = TestApp::new(None);
    let id = add_recipe(&t, "Hero", "Dinner", "eggs", "10 min").await;
    clear_image(&t, id);
    let (_, headers, _) = t.send(get(&format!("/recipes/{id}"))).await;
    assert!(headers.get(header::LINK).is_none());

    let image = "https://photos.test/hero.jpg";
    set_image(&t, id, image);
    let key = crumb::images::image_key(image);
    let (status, headers, _) = t.send(get(&format!("/recipes/{id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers[header::LINK],
        format!(
            "</img/{id}/768?v={key}>; rel=preload; as=image; fetchpriority=high; \
             imagesrcset=\"/img/{id}/768?v={key} 768w, /img/{id}/1200?v={key} 1200w\"; \
             imagesizes=\"(min-width: 1024px) 640px, 100vw\""
        )
        .as_str()
    );
    // Only the detail page
    let (_, headers, _) = t.send(get(&format!("/recipes/{id}/cook"))).await;
    assert!(headers.get(header::LINK).is_none());
    let (status, headers, _) = t.send(get("/recipes/424242")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(headers.get(header::LINK).is_none());
}

#[tokio::test]
async fn pages_revalidate_with_etags() {
    let t = TestApp::new(None);
    add_recipe(&t, "Soup", "Dinner", "leeks", "10 min").await;

    let (status, headers, body) = t.send(get("/")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CACHE_CONTROL], "no-cache");
    let etag = headers[header::ETAG].to_str().unwrap().to_string();
    assert!(etag.starts_with('"') && etag.ends_with('"'), "{etag}");

    let conditional = |uri: &str, tag: &str, encoding: Option<&str>| {
        let mut req = Request::builder()
            .uri(uri)
            .header(header::IF_NONE_MATCH, tag);
        if let Some(e) = encoding {
            req = req.header(header::ACCEPT_ENCODING, e);
        }
        req.body(Body::empty()).unwrap()
    };
    let (status, headers, empty) = t.send(conditional("/", &etag, None)).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    assert!(empty.is_empty());
    assert_eq!(headers[header::ETAG], etag.as_str());
    assert_eq!(headers[header::CACHE_CONTROL], "no-cache");

    // New data, new tag
    add_recipe(&t, "Stew", "Dinner", "beef", "10 min").await;
    let (status, headers, again) = t.send(conditional("/", &etag, None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(again, body);
    assert_ne!(headers[header::ETAG], etag.as_str());

    // Each content encoding carries its own strong tag
    let (status, headers, _) = t.send(conditional("/", "\"nope\"", Some("br"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_ENCODING], "br");
    let br_tag = headers[header::ETAG].to_str().unwrap().to_string();
    assert!(br_tag.ends_with("-br\""), "{br_tag}");
    let (status, _, _) = t.send(conditional("/", &br_tag, Some("br"))).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);

    // Static pages without data get one too
    let (status, headers, _) = t.send(get("/add")).await;
    assert_eq!(status, StatusCode::OK);
    let tag = headers[header::ETAG].to_str().unwrap().to_string();
    let (status, _, _) = t.send(conditional("/add", &tag, None)).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
}

#[tokio::test]
async fn served_html_files_keep_their_validators() {
    let t = TestApp::new(None);
    let dist = &t.state.config.web_dist;
    std::fs::write(
        dist.join("offline.html"),
        "<!doctype html><title>offline</title>",
    )
    .unwrap();
    std::fs::write(dist.join("offline.html.br"), b"not really brotli").unwrap();

    let br = Request::builder()
        .uri("/offline.html")
        .header(header::ACCEPT_ENCODING, "br")
        .body(Body::empty())
        .unwrap();
    let (status, headers, _) = t.send(br).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_ENCODING], "br");
    let modified = headers[header::LAST_MODIFIED].to_str().unwrap().to_string();

    let again = Request::builder()
        .uri("/offline.html")
        .header(header::ACCEPT_ENCODING, "br")
        .header(header::IF_MODIFIED_SINCE, &modified)
        .body(Body::empty())
        .unwrap();
    let (status, _, _) = t.send(again).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
}
