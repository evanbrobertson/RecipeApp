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

        let config = Config {
            app_password: password.map(String::from),
            web_dist: dist.path().to_path_buf(),
            ..Config::default()
        };
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
    assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 14);

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
