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
            ("suggestions/index.html", "suggestions"),
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
        // The share page's shell, with its markers and one inline script to hash
        std::fs::create_dir_all(dist.path().join("shell/share")).unwrap();
        std::fs::write(
            dist.path().join("shell/share/index.html"),
            format!(
                "<!doctype html><html><head><title>Shared recipe · Crumb</title>\
                 <script>boot()</script></head><body>{marker}<!--share:photo--><!--share:intro-->\
                 <h2>Ingredients</h2><!--share:ingredients--><h2>Method</h2><!--share:method-->\
                 <!--share:source--></body></html>"
            ),
        )
        .unwrap();
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
/// candidates plus one made-up id, and, when asked for an idea, always "Chicken Karaage"
/// (even when that's in the box); under /fail it always errors.
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
        let mut out = json!({"picks": [
            {"id": 424242, "blurb": "Not in your box"},
            {"id": ids[1], "blurb": "Bright and quick for a weeknight"},
            {"id": ids[0], "blurb": "You haven't tried this one yet"}
        ]});
        if payload.get("idea").is_some() {
            assert!(payload["idea"]["notTheseTitles"].is_array());
            let system = body["system"]
                .as_str()
                .or_else(|| body["messages"][0]["content"].as_str())
                .unwrap();
            assert!(system.contains("notTheseTitles"), "{system}");
            out["idea"] = json!({"title": "Chicken  Karaage", "why": "You love fried chicken, so try Karaage"});
        }
        out.to_string()
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
            c.idea_one_in = 0;
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
        assert!(s["idea"].is_null(), "not an idea day: {s}");
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

fn idea_app(base: &str) -> TestApp {
    use crumb::config::{LlmConfig, LlmProvider};
    TestApp::with_config(|c| {
        let mut llm = LlmConfig::new(LlmProvider::Anthropic, "test-key");
        llm.base_url = base.to_string();
        c.llm = Some(llm);
        // Every day is an idea day
        c.idea_one_in = 1;
    })
}

#[tokio::test]
async fn ai_idea_joins_try_next_on_idea_days() {
    let (base, _) = fake_ai().await;
    let t = idea_app(&base);
    for (title, ing) in [
        ("Chicken tikka masala", "chicken"),
        ("Beef stew", "beef"),
        ("Mapo tofu", "tofu"),
        ("Miso salmon", "salmon"),
        ("Pork carnitas", "pork"),
    ] {
        add_recipe(&t, title, "Main", ing, "30m").await;
    }
    t.json("GET", "/api/suggestions", None).await;
    let s = wait_for_ai(&t).await;
    assert_eq!(s["ai"], "ready", "{s}");
    // The idea takes the last card's place
    assert_eq!(s["items"].as_array().unwrap().len(), 3, "{s}");
    assert_eq!(s["idea"]["title"], "Chicken Karaage");
    assert_eq!(s["idea"]["why"], "You love fried chicken, so try Karaage");
    assert_eq!(
        s["idea"]["searchUrl"],
        "https://www.google.com/search?q=Chicken+Karaage+recipe"
    );
    let (_, _, html) = t.send(get("/")).await;
    assert!(html.contains("Chicken Karaage"));
    // Shuffles are the box only
    let (_, s) = t.json("GET", "/api/suggestions?seed=1", None).await;
    assert!(s["idea"].is_null());
    // The connector reads the cached idea
    let (out, _) = mcp_call(&t, "suggest_recipes", json!({})).await;
    assert!(
        out.contains("idea from Wee Chef: Chicken Karaage") && out.contains("google.com/search"),
        "{out}"
    );

    // The model suggested something already in the box: no idea card
    let t = idea_app(&base);
    for (title, ing) in [
        ("Chicken tikka masala", "chicken"),
        ("Beef stew", "beef"),
        ("Mapo tofu", "tofu"),
        ("Easy chicken karaage!", "chicken thighs"),
        ("Pork carnitas", "pork"),
    ] {
        add_recipe(&t, title, "Main", ing, "30m").await;
    }
    t.json("GET", "/api/suggestions", None).await;
    let s = wait_for_ai(&t).await;
    assert_eq!(s["ai"], "ready", "{s}");
    assert!(s["idea"].is_null(), "{s}");
    assert_eq!(s["items"].as_array().unwrap().len(), 4);

    // No key: never an idea
    let t = TestApp::with_config(|c| c.idea_one_in = 1);
    for title in ["Chicken", "Beef", "Tofu"] {
        add_recipe(&t, title, "Main", "salt", "30m").await;
    }
    let (_, s) = t.json("GET", "/api/suggestions", None).await;
    assert_eq!(s["ai"], "off");
    assert!(s["idea"].is_null());
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
        "private, max-age=31536000, immutable"
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

#[tokio::test]
async fn gzip_files_are_not_compressed_again() {
    let t = TestApp::new(None);
    let dist = &t.state.config.web_dist;
    std::fs::create_dir_all(dist.join("ocr")).unwrap();
    let model = vec![7u8; 4096];
    std::fs::write(dist.join("ocr/eng.traineddata.gz"), &model).unwrap();
    let req = Request::builder()
        .uri("/ocr/eng.traineddata.gz")
        .header(header::ACCEPT_ENCODING, "br, gzip")
        .body(Body::empty())
        .unwrap();
    let (status, headers, body) = t.send(req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "application/gzip");
    assert!(headers.get(header::CONTENT_ENCODING).is_none());
    assert_eq!(body.len(), model.len());
}

// ─── Recipes from photos ─────────────────────────────────────────────────────

type Seen = std::sync::Arc<std::sync::Mutex<Vec<Value>>>;

/// A fake vision API that keeps every request body and always reads the same recipe.
async fn fake_vision() -> (String, Seen) {
    let seen: Seen = Default::default();
    let recipe = json!({"isRecipe": true, "title": "Gran's Scones", "description": null,
        "author": "Gran", "prepTime": null, "cookTime": "12m", "totalTime": null,
        "recipeYield": "8 scones", "recipeCategory": null, "recipeCuisine": null,
        "ingredients": [{"name": null, "items": ["2 cups flour", "½ cup butter [?]"]}],
        "instructions": [{"name": null, "items": ["Rub in the butter.", "Bake at 425°F."]}],
        "notes": null})
    .to_string();
    let (s1, s2, r1, r2) = (seen.clone(), seen.clone(), recipe.clone(), recipe);
    let app = axum::Router::new()
        .route(
            "/v1/messages",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| async move {
                s1.lock().unwrap().push(body);
                axum::Json(json!({"stop_reason": "end_turn", "content": [{"type": "text", "text": r1}]}))
            }),
        )
        .route(
            "/v1/chat/completions",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| async move {
                s2.lock().unwrap().push(body);
                axum::Json(json!({"choices": [{"finish_reason": "stop", "message": {"role": "assistant", "content": r2, "refusal": null}}]}))
            }),
        )
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024 * 1024));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, seen)
}

fn vision_app(provider: crumb::config::LlmProvider, base: &str) -> TestApp {
    use crumb::config::{LlmConfig, LlmProvider};
    let base = base.to_string();
    TestApp::with_config(move |c| {
        let mut llm = LlmConfig::new(provider, "test-key");
        llm.base_url = match provider {
            LlmProvider::Anthropic => base,
            _ => format!("{base}/v1"),
        };
        c.llm = Some(llm);
    })
}

/// A JPEG stored `w`×`h` with EXIF orientation `o`.
fn exif_jpeg(w: u32, h: u32, o: u16) -> Vec<u8> {
    let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([x as u8, y as u8, 90]));
    let mut plain = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut plain, 80)
        .encode_image(&img)
        .unwrap();
    let mut tiff = b"II*\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
    tiff.extend_from_slice(&u32::from(o).to_le_bytes());
    tiff.extend_from_slice(&0u32.to_le_bytes());
    let mut app1 = b"Exif\0\0".to_vec();
    app1.extend_from_slice(&tiff);
    let mut out = plain[..2].to_vec();
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(&app1);
    out.extend_from_slice(&plain[2..]);
    out
}

/// POSTs `photos` (and a `text` note) to the photo import as multipart.
async fn post_photos(t: &TestApp, photos: &[Vec<u8>], text: Option<&str>) -> (StatusCode, Value) {
    let boundary = "XPHOTOBOUNDARY";
    let mut body = Vec::new();
    for (i, photo) in photos.iter().enumerate() {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"photo\"; filename=\"p{i}.jpg\"\r\nContent-Type: image/jpeg\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(photo);
        body.extend_from_slice(b"\r\n");
    }
    if let Some(text) = text {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"text\"\r\n\r\n{text}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    let req = Request::builder()
        .method("POST")
        .uri("/api/recipes/import/photos")
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    let (status, _, text) = t.send(req).await;
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

fn decoded_size(b64: &str) -> (u32, u32) {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .unwrap();
    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg).unwrap();
    (img.width(), img.height())
}

#[tokio::test]
async fn photo_import_needs_wee_chef() {
    let t = TestApp::new(None);
    let (status, body) = post_photos(&t, &[exif_jpeg(64, 48, 1)], None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["message"].as_str().unwrap().contains("Wee Chef"));
    let (_, info) = t.json("GET", "/api/connector", None).await;
    assert_eq!(info["vision"], false);
}

#[tokio::test]
async fn photo_import_reads_the_pages_with_the_vision_model() {
    use crumb::config::LlmProvider;
    let (base, seen) = fake_vision().await;
    let t = vision_app(LlmProvider::Anthropic, &base);
    let (_, info) = t.json("GET", "/api/connector", None).await;
    assert_eq!(info["vision"], true);

    // Page 1 is a sideways phone photo (stored landscape, EXIF says rotate 90°)
    let pages = [exif_jpeg(2400, 1800, 6), png_bytes(300, 200)];
    let (status, res) = post_photos(&t, &pages, Some("The scones, not the jam")).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["isNew"], true);
    assert_eq!(res["title"], "Gran's Scones");
    let (_, recipe) = t
        .json("GET", &format!("/api/recipes/{}", res["id"]), None)
        .await;
    assert_eq!(recipe["source"], "photo");
    assert_eq!(recipe["ingredients"][0]["items"][1], "½ cup butter [?]");
    assert_eq!(recipe["cookTime"], "12m");

    let bodies = seen.lock().unwrap().clone();
    assert_eq!(bodies.len(), 1);
    let body = &bodies[0];
    assert_eq!(body["model"], "claude-sonnet-5");
    assert_eq!(body["max_tokens"], 4000);
    assert!(body["system"].as_str().unwrap().contains("[?]"));
    let content = body["messages"][0]["content"].as_array().unwrap();
    assert_eq!(content.len(), 5);
    assert_eq!(content[0], json!({"type": "text", "text": "Page 1:"}));
    assert_eq!(content[1]["type"], "image");
    assert_eq!(content[1]["source"]["media_type"], "image/jpeg");
    let (w, h) = decoded_size(content[1]["source"]["data"].as_str().unwrap());
    assert!(h > w && h <= 1568, "{w}x{h}");
    assert_eq!(content[2]["text"], "Page 2:");
    assert_eq!(
        decoded_size(content[3]["source"]["data"].as_str().unwrap()),
        (300, 200)
    );
    let text = content[4]["text"].as_str().unwrap();
    assert!(text.contains("these 2 photos") && text.contains("The scones, not the jam"));
}

#[tokio::test]
async fn photo_import_on_openai_style_apis() {
    use crumb::config::LlmProvider;
    let (base, seen) = fake_vision().await;
    let t = vision_app(LlmProvider::OpenAi, &base);
    let (status, _) = post_photos(&t, &[exif_jpeg(640, 480, 1)], None).await;
    assert_eq!(status, StatusCode::OK);
    let t = vision_app(LlmProvider::DeepSeek, &base);
    let (status, res) = post_photos(&t, &[exif_jpeg(640, 480, 1)], None).await;
    assert_eq!(status, StatusCode::OK, "{res}");

    let bodies = seen.lock().unwrap().clone();
    let (openai, deepseek) = (&bodies[0], &bodies[1]);
    let block = &openai["messages"][1]["content"][1];
    assert_eq!(block["type"], "image_url");
    assert_eq!(block["image_url"]["detail"], "high");
    assert!(
        block["image_url"]["url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/jpeg;base64,/9j/")
    );
    assert_eq!(openai["response_format"]["type"], "json_schema");
    // DeepSeek reads photos with its Flash model, whatever the main model is
    assert_eq!(deepseek["model"], "deepseek-flash");
    assert_eq!(deepseek["messages"][1]["content"][1]["type"], "image_url");
    assert_eq!(deepseek["response_format"]["type"], "json_object");
}

#[tokio::test]
async fn photo_import_refuses_what_it_cant_read() {
    use crumb::config::LlmProvider;
    let (base, seen) = fake_vision().await;
    let t = vision_app(LlmProvider::Anthropic, &base);

    let (status, body) = post_photos(&t, &[], None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let seven: Vec<Vec<u8>> = (0..7).map(|_| exif_jpeg(32, 32, 1)).collect();
    let (status, body) = post_photos(&t, &seven, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["message"].as_str().unwrap().contains("6 photos"));

    let heic = b"\0\0\0\x18ftypheic\0\0\0\0mif1heic....".to_vec();
    let (status, body) = post_photos(&t, &[exif_jpeg(32, 32, 1), heic], None).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert!(body["message"].as_str().unwrap().starts_with("Photo 2: "));

    // Over 40 megapixels, going by the header
    let mut huge = exif_jpeg(16, 16, 1);
    let sof = huge.windows(2).position(|w| w == [0xFF, 0xC0]).unwrap();
    huge[sof + 5..sof + 9].copy_from_slice(&[0x27, 0x10, 0x27, 0x10]);
    let (status, _) = post_photos(&t, &[huge], None).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);

    // Over the 40 MB upload limit
    let big = vec![0xFFu8; 41 * 1024 * 1024];
    let (status, body) = post_photos(&t, &[big], None).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{body}");
    assert!(body["message"].as_str().unwrap().contains("40 MB"));

    assert!(seen.lock().unwrap().is_empty());
}

// ─── Wee Chef's import checks ───────────────────────────────────────────────

/// A fake TypeSafe API: labels lines by simple rules (a "Filling"-style line or one
/// ending in ":" is a heading, "Nutrition Facts" is junk, "1 cup sugar 2 eggs" might be
/// merged, a lowercase step is a fragment, "Keeps..." is a tip). Under /fail it's a 422.
async fn fake_jev() -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let calls = std::sync::Arc::new(AtomicUsize::new(0));
    let choice = |ok: &str, label: &str, p: f64| {
        let mut probs = serde_json::Map::new();
        probs.insert(ok.into(), json!(if label == ok { p } else { 1.0 - p }));
        probs.insert(label.into(), json!(p));
        json!({"type": "choice", "choice": label, "confidence": p, "probabilities": probs})
    };
    let (c1, c2) = (calls.clone(), calls.clone());
    let app = axum::Router::new()
        .route(
            "/v1/systemone",
            axum::routing::post(
                move |headers: axum::http::HeaderMap, axum::Json(body): axum::Json<Value>| async move {
                    c1.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(headers["authorization"], "Bearer test-key");
                    assert_eq!(body["model"], "jev-1.13.0");
                    assert!(body["state"]["ingredients"].is_array());
                    assert!(body["state"].get("image").is_none());
                    let mut answers = serde_json::Map::new();
                    for (id, q) in body["questions"].as_object().unwrap() {
                        let a = if id.starts_with("ing_") {
                            let line = q["instructions"]["line"].as_str().unwrap();
                            if line == "Filling" || line.ends_with(':') {
                                choice("ingredient", "heading", 1.0)
                            } else if line == "Nutrition Facts" {
                                choice("ingredient", "junk", 0.99)
                            } else if line == "1 cup sugar 2 eggs" {
                                choice("ingredient", "merged", 0.85)
                            } else {
                                choice("ingredient", "ingredient", 1.0)
                            }
                        } else if id.starts_with("step_") {
                            let step = q["instructions"]["step"].as_str().unwrap();
                            if step.ends_with(':') {
                                choice("step", "heading", 0.98)
                            } else if step.starts_with(char::is_lowercase) {
                                choice("step", "fragment", 0.95)
                            } else if step.starts_with("Keeps") {
                                choice("step", "not_instruction", 0.97)
                            } else {
                                choice("step", "step", 1.0)
                            }
                        } else {
                            json!({"type": "noul", "noul": 0.2})
                        };
                        answers.insert(id.clone(), a);
                    }
                    axum::Json(json!({"model": "jev-1.13.0", "answers": answers,
                        "usage": {"input_tokens": 1234, "output_tokens": 99}}))
                },
            ),
        )
        .route(
            "/fail/v1/systemone",
            axum::routing::post(move || async move {
                c2.fetch_add(1, Ordering::SeqCst);
                (StatusCode::UNPROCESSABLE_ENTITY, "bad request")
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, calls)
}

/// A recipe site serving `/{name}` pages with this JSON-LD.
async fn recipe_site(pages: Vec<(&'static str, Value)>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mut app = axum::Router::new();
    for (name, ld) in pages {
        let html = format!(
            r#"<html><head><script type="application/ld+json">{ld}</script></head><body></body></html>"#
        );
        app = app.route(
            &format!("/{name}"),
            axum::routing::get(move || async move { axum::response::Html(html) }),
        );
    }
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    base
}

fn jev_app(base: &str) -> TestApp {
    let base = base.to_string();
    TestApp::with_config(move |c| {
        let mut ts = crumb::config::TypesafeConfig::new("test-key");
        ts.base_url = base;
        c.typesafe = Some(ts);
    })
}

async fn wait_for_check(t: &TestApp, id: i64) -> Value {
    for _ in 0..200 {
        let (_, c) = t
            .json("GET", &format!("/api/recipes/{id}/checks"), None)
            .await;
        if c["status"] != "pending" {
            return c;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("the check never finished");
}

fn big_mac() -> Value {
    json!({"@context": "https://schema.org", "@type": "Recipe", "name": "Big Mac Sauce",
    "prepTime": "PT10M", "cookTime": "PT20M",
    "recipeIngredient": ["▢ 1 cup mayo", "Nutrition Facts", "Filling", "▢ 1 lb beef", "1 cup sugar 2 eggs"],
    "recipeInstructions": [
        {"@type": "HowToStep", "text": "Sauce:"},
        {"@type": "HowToStep", "text": "Whisk the mayo and"},
        {"@type": "HowToStep", "text": "relish together."},
        {"@type": "HowToStep", "text": "Keeps for a week in the fridge."},
        {"@type": "HowToStep", "text": "Don&amp;#039;t skip the pickles."}
    ]})
}

#[tokio::test]
async fn wee_chef_checks_tidy_imports_and_undo() {
    let (jev, calls) = fake_jev().await;
    let site = recipe_site(vec![
        ("big-mac", big_mac()),
        (
            "toast",
            json!({"@type": "Recipe",
        "name": "Toast", "recipeIngredient": ["1 slice bread", "1 cup sugar 2 eggs"],
        "recipeInstructions": ["Toast the bread."]}),
        ),
    ])
    .await;
    let t = jev_app(&jev);
    let (_, info) = t.json("GET", "/api/connector", None).await;
    assert_eq!(info["weeChefChecks"], true);

    let (status, res) = t
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"url": format!("{site}/big-mac")})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{res}");
    let id = res["id"].as_i64().unwrap();

    let checks = wait_for_check(&t, id).await;
    assert_eq!(checks["status"], "done", "{checks}");
    assert_eq!(checks["canUndo"], true);
    let flags = checks["flags"].as_array().unwrap();
    let of = |state: &str| {
        flags
            .iter()
            .filter(|f| f["state"] == state)
            .map(|f| (f["itemText"].as_str().unwrap(), f["kind"].as_str().unwrap()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        of("fixed"),
        [
            ("Nutrition Facts", "junk"),
            ("Filling", "heading"),
            ("Sauce:", "heading"),
            ("relish together.", "fragment"),
            ("Keeps for a week in the fridge.", "not_instruction"),
        ]
    );
    assert_eq!(of("review"), [("1 cup sugar 2 eggs", "merged")]);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);

    // Tidied at import (no AI): glyphs, entities, total time. Then Wee Chef's fixes.
    let (_, r) = t.json("GET", &format!("/api/recipes/{id}"), None).await;
    assert_eq!(r["totalTime"], "30m");
    assert_eq!(
        r["ingredients"],
        json!([{"name": null, "items": ["1 cup mayo"]},
               {"name": "Filling", "items": ["1 lb beef", "1 cup sugar 2 eggs"]}])
    );
    assert_eq!(
        r["instructions"],
        json!([{"name": "Sauce", "items": ["Whisk the mayo and relish together.", "Don't skip the pickles."]}])
    );
    assert_eq!(r["notes"], "Keeps for a week in the fridge.");

    // The recipe page carries the check, so there's no extra request
    let (_, _, html) = t.send(get(&format!("/recipes/{id}"))).await;
    assert!(
        html.contains(r#""checks":{"status":"done","canUndo":true"#),
        "{html}"
    );

    // Keep as is: gone now, and remembered
    let flag = flags.iter().find(|f| f["state"] == "review").unwrap()["id"]
        .as_i64()
        .unwrap();
    let (status, after) = t
        .json(
            "POST",
            &format!("/api/recipes/{id}/flags/{flag}/dismiss"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        after["flags"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["state"] != "review")
    );
    let (status, err) = t
        .json(
            "POST",
            &format!("/api/recipes/{id}/flags/{flag}/dismiss"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(err["statusCode"], 404);

    // Undo puts back the imported version (still tidied, no AI fixes)
    let (status, undone) = t
        .json("POST", &format!("/api/recipes/{id}/checks/undo"), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{undone}");
    assert_eq!(undone["checks"]["canUndo"], false);
    assert_eq!(undone["checks"]["flags"], json!([]));
    assert_eq!(
        undone["recipe"]["ingredients"][0]["items"],
        json!([
            "1 cup mayo",
            "Nutrition Facts",
            "Filling",
            "1 lb beef",
            "1 cup sugar 2 eggs"
        ])
    );
    assert_eq!(undone["recipe"]["instructions"][0]["items"][0], "Sauce:");
    assert_eq!(undone["recipe"]["notes"], Value::Null);
    let (status, _) = t
        .json("POST", &format!("/api/recipes/{id}/checks/undo"), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // A second import: fixed, then edited, so Undo would lose the edit
    let (_, res) = t
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"url": format!("{site}/toast")})),
        )
        .await;
    let toast = res["id"].as_i64().unwrap();
    let checks = wait_for_check(&t, toast).await;
    assert_eq!(checks["flags"][0]["kind"], "merged");
    assert_eq!(checks["canUndo"], false); // nothing was fixed
    // Editing the flagged line away resolves the flag
    t.json(
        "PATCH",
        &format!("/api/recipes/{toast}"),
        Some(json!({"ingredients": [{"name": null, "items": ["1 slice bread", "1 cup sugar", "2 eggs"]}]})),
    )
    .await;
    let (_, checks) = t
        .json("GET", &format!("/api/recipes/{toast}/checks"), None)
        .await;
    assert_eq!(checks["flags"], json!([]));

    // Same URL again: the duplicate isn't checked twice
    t.json(
        "POST",
        "/api/recipes/import",
        Some(json!({"url": format!("{site}/toast")})),
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);

    // Recipes the cook writes aren't checked; "Check all" only re-checks the toast, which
    // was edited after its check
    let (_, mine) = t
        .json(
            "POST",
            "/api/recipes",
            Some(json!({"title": "Mine", "ingredients": [{"items": ["Filling"]}]})),
        )
        .await;
    let (_, c) = t
        .json("GET", &format!("/api/recipes/{}/checks", mine["id"]), None)
        .await;
    assert_eq!(c, Value::Null);
    let (status, all) = t.json("POST", "/api/checks", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(all["queued"], 1);
    assert_eq!(all["eligible"], 2);
    assert_eq!(all["checked"], 1);
    wait_for_check(&t, toast).await;
    let (_, all) = t.json("POST", "/api/checks", None).await;
    assert_eq!(all["queued"], 0);
    assert_eq!(all["checked"], 2);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
}

fn older_recipe() -> crumb::model::RecipeFields {
    crumb::model::RecipeFields {
        title: "Older".into(),
        ingredients: vec![crumb::model::Section {
            name: None,
            items: vec!["Filling".into(), "▢ 1 egg".into(), "Nutrition Facts".into()],
        }],
        instructions: vec![crumb::model::Section {
            name: None,
            items: vec!["Cook it.".into()],
        }],
        ..Default::default()
    }
}

#[tokio::test]
async fn check_all_only_suggests_on_recipes_already_in_the_box() {
    let (jev, _) = fake_jev().await;
    let t = jev_app(&jev);
    let (older, _) =
        crumb::recipes::create_recipe(&t.state.db.lock(), older_recipe(), "url").unwrap();
    let (_, all) = t.json("POST", "/api/checks", None).await;
    assert_eq!(all["queued"], 1);
    let c = wait_for_check(&t, older.id).await;
    assert_eq!(c["status"], "done", "{c}");
    let (_, r) = t
        .json("GET", &format!("/api/recipes/{}", older.id), None)
        .await;
    // Only the checkbox glyph went; the heading and the junk line are suggestions
    assert_eq!(
        r["ingredients"],
        json!([{"name": null, "items": ["Filling", "1 egg", "Nutrition Facts"]}])
    );
    let kinds: Vec<(&str, &str)> = c["flags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["kind"].as_str().unwrap(), f["state"].as_str().unwrap()))
        .collect();
    assert_eq!(
        kinds,
        [("tidy", "fixed"), ("heading", "review"), ("junk", "review")]
    );
    assert_eq!(c["canUndo"], true);
}

#[tokio::test]
async fn wee_chef_checks_existing_recipes_and_survive_failures() {
    let (jev, calls) = fake_jev().await;
    // A backup restore isn't tidied or checked on the way in
    let t = jev_app(&format!("{jev}/fail"));
    let backup = json!({"format": "crumb", "version": 1, "recipes": [
        {"title": "Restored", "ingredients": [{"name": null, "items": ["▢ Filling", "1 egg"]}],
         "instructions": [{"name": null, "items": ["Cook it."]}]}
    ]});
    let boundary = "XBOUNDARY";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"backup.json\"\r\nContent-Type: application/json\r\n\r\n{backup}\r\n--{boundary}--\r\n"
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
    let (status, _, text) = t.send(req).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let (_, r) = t.json("GET", "/api/recipes/1", None).await;
    assert_eq!(r["ingredients"][0]["items"][0], "▢ Filling");
    let (_, c) = t.json("GET", "/api/recipes/1/checks", None).await;
    assert_eq!(c["status"], "skipped");
    assert_eq!(c["flags"], json!([]));
    // ..."Check all" picks it up later, as a restore
    let (_, status) = t.json("GET", "/api/checks", None).await;
    assert_eq!(
        (&status["eligible"], &status["due"], &status["restored"]),
        (&json!(1), &json!(1), &json!(1))
    );

    // A recipe from before the checks (the legacy upgrade marked them all 'url')
    let (older, _) =
        crumb::recipes::create_recipe(&t.state.db.lock(), older_recipe(), "url").unwrap();

    // "Check all" picks both up; a failing API marks them failed and leaves them alone
    let (_, all) = t.json("POST", "/api/checks", None).await;
    assert_eq!(all["queued"], 2);
    let c = wait_for_check(&t, older.id).await;
    assert_eq!(c["status"], "failed");
    assert_eq!(wait_for_check(&t, 1).await["status"], "failed");
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2); // a 422 isn't retried
    let (_, r2) = t
        .json("GET", &format!("/api/recipes/{}", older.id), None)
        .await;
    assert_eq!(r2["ingredients"][0]["items"][1], "▢ 1 egg");
    let (_, r) = t.json("GET", "/api/recipes/1", None).await;
    assert_eq!(r["ingredients"][0]["items"][0], "▢ Filling");
    let (_, status) = t.json("GET", "/api/checks", None).await;
    assert_eq!(status["failed"], 2);
    // Failed checks are retried by the next "Check all"
    let (_, all) = t.json("POST", "/api/checks", None).await;
    assert_eq!(all["queued"], 2);
    wait_for_check(&t, older.id).await;
    wait_for_check(&t, 1).await;

    // Without a key the feature is off: nothing queued, no UI
    let off = TestApp::new(None);
    let (_, info) = off.json("GET", "/api/connector", None).await;
    assert_eq!(info["weeChefChecks"], false);
    let (status, err) = off.json("POST", "/api/checks", None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        err["statusMessage"],
        "Wee Chef checks aren't set up on this server"
    );
    let text = "Toast\nIngredients\n▢ 1 slice bread\nInstructions\n1. Toast the bread.";
    let (_, res) = off
        .json("POST", "/api/recipes/import", Some(json!({"text": text})))
        .await;
    let (_, r) = off
        .json("GET", &format!("/api/recipes/{}", res["id"]), None)
        .await;
    // The clean-up still runs
    assert_eq!(r["ingredients"][0]["items"][0], "1 slice bread");
    let (_, c) = off
        .json("GET", &format!("/api/recipes/{}/checks", res["id"]), None)
        .await;
    assert_eq!(c, Value::Null);
}

#[tokio::test]
async fn suggestions_page_and_nav_hint_follow_open_flags() {
    let (jev, _) = fake_jev().await;
    let t = jev_app(&jev);
    let (older, _) =
        crumb::recipes::create_recipe(&t.state.db.lock(), older_recipe(), "url").unwrap();
    t.json("POST", "/api/checks", None).await;
    let c = wait_for_check(&t, older.id).await;
    let hint = |headers: &axum::http::HeaderMap| {
        headers
            .get_all("server-timing")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find(|v| v.starts_with("crumb-review"))
            .map(String::from)
    };
    let page = || {
        Request::get("/")
            .header("sec-fetch-dest", "document")
            .body(Body::empty())
            .unwrap()
    };

    let (_, list) = t.json("GET", "/api/checks/review", None).await;
    assert_eq!(list["recipes"][0]["id"], older.id);
    assert_eq!(list["recipes"][0]["count"], 2);
    assert_eq!(list["recipes"][0]["fields"], json!({"ingredients": 2}));

    let (status, headers, _) = send_raw(&t, page()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(hint(&headers).as_deref(), Some("crumb-review;desc=\"1\""));
    let tag = headers[header::ETAG].to_str().unwrap().to_string();
    assert!(tag.ends_with("-r1\""), "{tag}");

    let (status, _, html) = t.send(get("/suggestions")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains(&format!("\"id\":{}", older.id)),
        "page data inlined"
    );

    // Settle every suggestion: the hint drops to 0 and the old ETag no longer matches
    for f in c["flags"].as_array().unwrap() {
        if f["state"] == "review" {
            t.json(
                "POST",
                &format!("/api/recipes/{}/flags/{}/dismiss", older.id, f["id"]),
                None,
            )
            .await;
        }
    }
    let revalidate = Request::get("/")
        .header("sec-fetch-dest", "document")
        .header(header::IF_NONE_MATCH, &tag)
        .body(Body::empty())
        .unwrap();
    let (status, headers, _) = send_raw(&t, revalidate).await;
    assert_eq!(status, StatusCode::OK, "a changed count is never a 304");
    assert_eq!(hint(&headers).as_deref(), Some("crumb-review;desc=\"0\""));
    let (_, list) = t.json("GET", "/api/checks/review", None).await;
    assert_eq!(list["recipes"], json!([]));
}

/// Uploads one JSON file to the Import page's endpoint; returns its summary.
async fn upload_json(t: &TestApp, name: &str, file: &str) -> Value {
    let boundary = "XBOUNDARY";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"{name}\"\r\nContent-Type: application/json\r\n\r\n{file}\r\n--{boundary}--\r\n"
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
    let (status, _, text) = t.send(req).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    serde_json::from_str(&text).unwrap()
}

fn flag_kinds(c: &Value) -> Vec<(String, String)> {
    c["flags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["kind"].as_str().unwrap().to_string(),
                f["state"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[tokio::test]
async fn check_all_rechecks_restores_and_recipes_edited_since() {
    let (jev, calls) = fake_jev().await;
    let t = jev_app(&jev);
    let backup = json!({"format": "crumb", "version": 1, "recipes": [
        {"title": "Restored", "ingredients": [{"name": null, "items": ["Filling", "▢ 1 egg", "Nutrition Facts"]}],
         "instructions": [{"name": null, "items": ["Cook it."]}]}
    ]});
    upload_json(&t, "backup.json", &backup.to_string()).await;
    let (_, status) = t.json("GET", "/api/checks", None).await;
    assert_eq!(status["due"], 1);
    assert_eq!(status["restored"], 1);
    assert_eq!(status["checked"], 0);

    // A restore is checked by "Check all", suggestions only (plus the glyph clean-up)
    let (_, all) = t.json("POST", "/api/checks", None).await;
    assert_eq!(all["queued"], 1);
    let c = wait_for_check(&t, 1).await;
    assert_eq!(c["status"], "done", "{c}");
    let (_, r) = t.json("GET", "/api/recipes/1", None).await;
    assert_eq!(
        r["ingredients"],
        json!([{"name": null, "items": ["Filling", "1 egg", "Nutrition Facts"]}])
    );
    let expect = |v: &[(&str, &str)]| -> Vec<(String, String)> {
        v.iter()
            .map(|(k, s)| (k.to_string(), s.to_string()))
            .collect()
    };
    assert_eq!(
        flag_kinds(&c),
        expect(&[("tidy", "fixed"), ("heading", "review"), ("junk", "review")])
    );
    let (_, status) = t.json("GET", "/api/checks", None).await;
    assert_eq!(
        (status["due"].as_i64(), status["checked"].as_i64()),
        (Some(0), Some(1))
    );
    let (_, all) = t.json("POST", "/api/checks", None).await;
    assert_eq!(all["queued"], 0);

    // "Keep as is" on the junk line, then an edit (in the same second as the check)
    let junk = c["flags"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["kind"] == "junk")
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    t.json(
        "POST",
        &format!("/api/recipes/1/flags/{junk}/dismiss"),
        None,
    )
    .await;
    let (status, _) = t
        .json("PATCH", "/api/recipes/1", Some(json!({"notes": "Mine"})))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, status) = t.json("GET", "/api/checks", None).await;
    assert_eq!(status["due"], 1);
    assert_eq!(status["edited"], 1);
    assert_eq!(status["checked"], 0);

    // Checked again, still suggest-only; the kept line isn't raised again
    let (_, all) = t.json("POST", "/api/checks", None).await;
    assert_eq!(all["queued"], 1);
    let c = wait_for_check(&t, 1).await;
    assert_eq!(c["status"], "done");
    assert_eq!(flag_kinds(&c), expect(&[("heading", "review")]));
    let (_, r) = t.json("GET", "/api/recipes/1", None).await;
    assert_eq!(r["ingredients"][0]["items"][0], "Filling");
    assert_eq!(r["notes"], "Mine");
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    let (_, all) = t.json("POST", "/api/checks", None).await;
    assert_eq!(all["queued"], 0);
}

#[tokio::test]
async fn one_recipe_can_be_checked_again_on_request() {
    let off = TestApp::new(None);
    let id = add_recipe(&off, "Chili", "Main", "1 lb beef", "1h").await;
    let (status, _) = off
        .json("POST", &format!("/api/recipes/{id}/checks"), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (_, _, html) = off.send(get(&format!("/recipes/{id}"))).await;
    assert!(html.contains(r#""weeChefChecks":false"#));

    let (jev, calls) = fake_jev().await;
    let t = jev_app(&jev);
    let (status, _) = t.json("POST", "/api/recipes/999/checks", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Saved before Wee Chef (no check row at all, like an old restore): already in the
    // box, so only suggestions, plus the deterministic clean-up of the checkbox glyph
    let (old, _) =
        crumb::recipes::create_recipe(&t.state.db.lock(), older_recipe(), "import").unwrap();
    let (status, c) = t
        .json("POST", &format!("/api/recipes/{}/checks", old.id), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{c}");
    let c = wait_for_check(&t, old.id).await;
    assert_eq!(c["status"], "done");
    let (_, r) = t
        .json("GET", &format!("/api/recipes/{}", old.id), None)
        .await;
    assert_eq!(
        r["ingredients"],
        json!([{"name": null, "items": ["Filling", "1 egg", "Nutrition Facts"]}])
    );
    assert!(
        c["flags"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["state"] == "review" || f["kind"] == "tidy"),
        "{c}"
    );

    // A fresh import whose check never ran (only tidied on the way in): fixed as its
    // import check would have been
    let (fresh, _) =
        crumb::recipes::create_recipe(&t.state.db.lock(), older_recipe(), "url").unwrap();
    t.state
        .db
        .lock()
        .execute(
            "INSERT INTO recipe_checks (recipe_id, status, queued_at) VALUES (?1, 'tidied', 0)",
            [fresh.id],
        )
        .unwrap();
    let (status, c) = t
        .json("POST", &format!("/api/recipes/{}/checks", fresh.id), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{c}");
    assert_eq!(c["status"], "pending");
    let c = wait_for_check(&t, fresh.id).await;
    assert_eq!(c["status"], "done");
    let (_, r) = t
        .json("GET", &format!("/api/recipes/{}", fresh.id), None)
        .await;
    assert_eq!(
        r["ingredients"],
        json!([{"name": "Filling", "items": ["1 egg"]}])
    );

    // Asked again: re-queued even though it's done, and only suggests now
    let (_, c) = t
        .json("POST", &format!("/api/recipes/{}/checks", fresh.id), None)
        .await;
    assert_eq!(c["status"], "pending");
    let c = wait_for_check(&t, fresh.id).await;
    assert_eq!(c["status"], "done");
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
    let (_, again) = t
        .json("GET", &format!("/api/recipes/{}", fresh.id), None)
        .await;
    assert_eq!(again["ingredients"], r["ingredients"]);

    // A recipe the cook wrote (never checked by "Check all"): suggestions only
    let (status, mine) = t
        .json(
            "POST",
            "/api/recipes",
            Some(
                json!({"title": "Mine", "ingredients": [{"items": ["Filling", "Nutrition Facts"]}],
                "instructions": [{"items": ["Cook it."]}]}),
            ),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let mine = mine["id"].as_i64().unwrap();
    let (_, all) = t.json("GET", "/api/checks", None).await;
    assert_eq!(all["due"], 0);
    t.json("POST", &format!("/api/recipes/{mine}/checks"), None)
        .await;
    let c = wait_for_check(&t, mine).await;
    assert_eq!(c["status"], "done");
    let (_, r) = t.json("GET", &format!("/api/recipes/{mine}"), None).await;
    assert_eq!(
        r["ingredients"],
        json!([{"name": null, "items": ["Filling", "Nutrition Facts"]}])
    );
    assert!(
        c["flags"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["state"] == "review")
    );
    let (_, _, html) = t.send(get(&format!("/recipes/{mine}"))).await;
    assert!(html.contains(r#""weeChefChecks":true"#));
}

#[tokio::test]
async fn one_recipe_exports_as_json_and_markdown() {
    let t = TestApp::new(None);
    let id = add_recipe(&t, "Lemon Tart!", "Dessert", "3 lemons", "1h").await;
    let (_, book) = t
        .json("POST", "/api/cookbooks", Some(json!({"name": "Sweet"})))
        .await;
    t.json(
        "POST",
        &format!("/api/cookbooks/{}/recipes", book["id"]),
        Some(json!({"recipeIds": [id]})),
    )
    .await;
    t.json("POST", "/api/cookbooks", Some(json!({"name": "Other"})))
        .await;
    t.json("POST", &format!("/api/recipes/{id}/cooked"), None)
        .await;
    add_recipe(&t, "Not this one", "Main", "1 egg", "5m").await;

    let (status, headers, body) = t
        .send(get(&format!("/api/recipes/{id}/export?format=json")))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers[header::CONTENT_TYPE],
        "application/json; charset=utf-8"
    );
    assert_eq!(
        headers[header::CONTENT_DISPOSITION],
        "attachment; filename=\"lemon-tart.json\""
    );
    let file: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(file["format"], "crumb");
    assert_eq!(file["recipes"].as_array().unwrap().len(), 1);
    assert_eq!(file["recipes"][0]["title"], "Lemon Tart!");
    assert_eq!(file["recipes"][0]["cookbooks"], json!(["Sweet"]));
    assert_eq!(
        file["cookbooks"],
        json!([{"name": "Sweet", "description": null}])
    );
    assert_eq!(file["recipes"][0]["cookedAt"].as_array().unwrap().len(), 1);
    // JSON is the default
    let (_, _, default) = t.send(get(&format!("/api/recipes/{id}/export"))).await;
    assert_eq!(default, body);

    let (status, headers, md) = t
        .send(get(&format!("/api/recipes/{id}/export?format=md")))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers[header::CONTENT_TYPE],
        "text/markdown; charset=utf-8"
    );
    assert_eq!(
        headers[header::CONTENT_DISPOSITION],
        "attachment; filename=\"lemon-tart.md\""
    );
    assert!(md.starts_with("# Lemon Tart!\n"), "{md}");
    assert!(md.contains("- 3 lemons"));

    let (status, _) = t
        .json("GET", &format!("/api/recipes/{id}/export?format=pdf"), None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = t.json("GET", "/api/recipes/999/export", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Letters outside ASCII get a UTF-8 filename* too
    let (status, r) = t
        .json(
            "POST",
            "/api/recipes",
            Some(
                json!({"title": "Crème brûlée", "ingredients": [{"items": ["cream"]}],
                "instructions": [{"items": ["Bake."]}]}),
            ),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, headers, _) = t
        .send(get(&format!("/api/recipes/{}/export?format=md", r["id"])))
        .await;
    assert_eq!(
        headers[header::CONTENT_DISPOSITION],
        "attachment; filename=\"cr-me-br-l-e.md\"; filename*=UTF-8''cr%C3%A8me-br%C3%BBl%C3%A9e.md"
    );

    // The JSON goes back in through the Import page, into a fresh box
    let fresh = TestApp::new(None);
    let summary = upload_json(&fresh, "lemon-tart.json", &body).await;
    assert_eq!(
        summary[0]["created"].as_array().unwrap().len(),
        1,
        "{summary}"
    );
    let (_, list) = fresh.json("GET", "/api/recipes", None).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    let rid = list[0]["id"].as_i64().unwrap();
    let (_, r) = fresh
        .json("GET", &format!("/api/recipes/{rid}"), None)
        .await;
    assert_eq!(r["title"], "Lemon Tart!");
    assert_eq!(r["ingredients"][0]["items"], json!(["3 lemons", "salt"]));
    let (_, books) = fresh.json("GET", "/api/cookbooks", None).await;
    assert_eq!(books[0]["name"], "Sweet");
    assert_eq!(books.as_array().unwrap().len(), 1);
    let (_, _, html) = fresh.send(get(&format!("/recipes/{rid}"))).await;
    assert!(html.contains(r#""cookStats":{"count":1"#));
}

// ─── Share links ────────────────────────────────────────────────────────────

/// Signs in to an app with a password; the cookie pair to send.
async fn sign_in(t: &TestApp, password: &str) -> String {
    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"password": password}).to_string()))
        .unwrap();
    let (status, headers, _) = t.send(req).await;
    assert_eq!(status, StatusCode::OK);
    let cookie = headers[header::SET_COOKIE].to_str().unwrap();
    cookie.split(';').next().unwrap().to_string()
}

/// A JSON request with an optional session cookie.
async fn call(
    t: &TestApp,
    method: &str,
    uri: &str,
    body: Option<Value>,
    cookie: Option<&str>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, c);
    }
    let body = match body {
        Some(b) => {
            req = req.header(header::CONTENT_TYPE, "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    let (status, _, text) = t.send(req.body(body).unwrap()).await;
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

fn shared_recipe(title: &str) -> Value {
    json!({
        "title": title,
        "url": "https://food.test/lemon-cake",
        "description": "Bright and tender.",
        "image": "https://img.test/cake.jpg",
        "author": "A Baker",
        "prepTime": "20m",
        "cookTime": "45 mins",
        "totalTime": "1h 5m",
        "recipeYield": "8 slices",
        "recipeCategory": "Dessert",
        "recipeCuisine": "British",
        "ingredients": [
            {"name": "Cake", "items": ["200 g flour", "2 eggs"]},
            {"name": "Glaze", "items": ["100 g icing sugar", "1 lemon"]}
        ],
        "instructions": [
            {"name": "Bake", "items": ["Mix the cake.", "Bake for 45 minutes."]},
            {"name": "Finish", "items": ["Glaze it."]}
        ],
        "nutrition": {"calories": "320 kcal"},
        "notes": "Grandma's secret: extra zest."
    })
}

/// The JSON-LD block on a page, parsed.
fn json_ld(html: &str) -> Value {
    let start = html
        .find(r#"<script type="application/ld+json">"#)
        .expect("JSON-LD")
        + r#"<script type="application/ld+json">"#.len();
    let end = start + html[start..].find("</script>").unwrap();
    serde_json::from_str(&html[start..end]).expect("JSON-LD parses")
}

fn share_path(url: &str) -> String {
    format!("/s/{}", url.rsplit('/').next().unwrap())
}

#[tokio::test]
async fn share_links_are_one_per_recipe_until_stopped() {
    let t = TestApp::new(None);
    let (_, r) = t
        .json("POST", "/api/recipes", Some(shared_recipe("Lemon Cake")))
        .await;
    let id = r["id"].as_i64().unwrap();

    let (status, first) = t
        .json("POST", &format!("/api/recipes/{id}/share"), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["includeNotes"], true);
    let url = first["url"].as_str().unwrap();
    assert!(url.starts_with("http://localhost:3000/s/"), "{url}");
    assert_eq!(first["token"].as_str().unwrap().len(), 22);
    let (_, again) = t
        .json("POST", &format!("/api/recipes/{id}/share"), None)
        .await;
    assert_eq!(again["url"], first["url"]);
    let (status, _) = t.json("POST", "/api/recipes/999/share", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // In the recipe page's data, so the sheet needs no fetch
    let (_, _, page) = t.send(get(&format!("/recipes/{id}"))).await;
    assert!(page.contains(&format!(
        "\"share\":{{\"token\":\"{}\"",
        first["token"].as_str().unwrap()
    )));

    // SITE_URL wins for the absolute link
    let site = TestApp::with_config(|c| c.site_url = Some("https://crumb.example/".into()));
    let (_, r) = site
        .json("POST", "/api/recipes", Some(shared_recipe("Cake")))
        .await;
    let (_, s) = site
        .json("POST", &format!("/api/recipes/{}/share", r["id"]), None)
        .await;
    assert!(
        s["url"]
            .as_str()
            .unwrap()
            .starts_with("https://crumb.example/s/")
    );

    // Stopping deletes it; sharing again makes a new token
    let path = share_path(url);
    let (status, _, _) = t.send(get(&path)).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = t
        .json("DELETE", &format!("/api/recipes/{id}/share"), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, headers, body) = t.send(get(&path)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body, "Not found");
    assert_eq!(headers["x-robots-tag"], "noindex, noimageindex");
    for sub in ["crumb.json", "og.jpg", "img/768"] {
        let (status, _, body) = t.send(get(&format!("{path}/{sub}"))).await;
        assert_eq!(
            (status, body.as_str()),
            (StatusCode::NOT_FOUND, "Not found")
        );
    }
    let (_, fresh) = t
        .json("POST", &format!("/api/recipes/{id}/share"), None)
        .await;
    assert_ne!(fresh["url"], first["url"]);

    // Deleting the recipe takes its share with it
    let (_, _) = t.json("DELETE", &format!("/api/recipes/{id}"), None).await;
    let (status, _, _) = t
        .send(get(&share_path(fresh["url"].as_str().unwrap())))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let left: i64 = t
        .state
        .db
        .lock()
        .query_row("SELECT count(*) FROM shares", [], |r| r.get(0))
        .unwrap();
    assert_eq!(left, 0);
}

#[tokio::test]
async fn share_pages_are_public_read_only_pages() {
    let t = TestApp::new(Some("pw"));
    let cookie = sign_in(&t, "pw").await;
    let (_, r) = call(
        &t,
        "POST",
        "/api/recipes",
        Some(shared_recipe("Lemon Cake")),
        Some(&cookie),
    )
    .await;
    let id = r["id"].as_i64().unwrap();
    // Nothing about the box: log a cook, put it in a cookbook
    call(
        &t,
        "POST",
        &format!("/api/recipes/{id}/cooked"),
        None,
        Some(&cookie),
    )
    .await;
    let (_, book) = call(
        &t,
        "POST",
        "/api/cookbooks",
        Some(json!({"name": "Private Shelf"})),
        Some(&cookie),
    )
    .await;
    call(
        &t,
        "POST",
        &format!("/api/cookbooks/{}/recipes", book["id"]),
        Some(json!({"recipeIds": [id]})),
        Some(&cookie),
    )
    .await;

    // Creating a share needs the login
    let (status, _) = call(&t, "POST", &format!("/api/recipes/{id}/share"), None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (_, share) = call(
        &t,
        "POST",
        &format!("/api/recipes/{id}/share"),
        None,
        Some(&cookie),
    )
    .await;
    let url = share["url"].as_str().unwrap().to_string();
    let path = share_path(&url);

    // No cookie: the page
    let (status, headers, html) = t.send(get(&path)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["x-robots-tag"], "noindex, noimageindex");
    assert_eq!(headers[header::REFERRER_POLICY], "no-referrer");
    assert_eq!(headers[header::X_FRAME_OPTIONS], "DENY");
    assert_eq!(headers[header::CACHE_CONTROL], "no-cache");
    assert!(headers.get("speculation-rules").is_none());
    let csp = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(
        csp.starts_with("default-src 'none'; script-src 'self' 'sha256-"),
        "{csp}"
    );
    for part in [
        "frame-ancestors 'none'",
        "base-uri 'none'",
        "img-src 'self' data:",
    ] {
        assert!(csp.contains(part), "{csp}");
    }
    assert!(html.contains("<title>Lemon Cake · Crumb</title>"));
    assert!(html.contains(r#"<meta property="og:title" content="Lemon Cake">"#));
    assert!(html.contains(r#"<meta property="og:type" content="article">"#));
    assert!(html.contains(&format!(r#"<meta property="og:url" content="{url}">"#)));
    assert!(html.contains(r#"<meta name="description" content="Bright and tender.">"#));
    // The photo is a third-party URL here, so a preview card is offered
    assert!(html.contains(r#"<meta name="twitter:card" content="summary_large_image">"#));
    assert!(html.contains(&format!(
        r#"<meta property="og:image" content="{url}/og.jpg?v="#
    )));
    assert!(html.contains(&format!(
        r#"<link rel="alternate" type="application/vnd.crumb+json" href="{url}/crumb.json">"#
    )));
    let ld = json_ld(&html);
    assert_eq!(ld["@type"], "Recipe");
    assert_eq!(ld["name"], "Lemon Cake");
    assert_eq!(ld["prepTime"], "PT20M");
    assert_eq!(ld["cookTime"], "PT45M");
    assert_eq!(ld["totalTime"], "PT1H5M");
    assert_eq!(ld["recipeYield"], "8 slices");
    assert_eq!(ld["url"], "https://food.test/lemon-cake");
    assert_eq!(ld["isBasedOn"], "https://food.test/lemon-cake");
    assert_eq!(ld["recipeIngredient"].as_array().unwrap().len(), 4);
    assert_eq!(ld["recipeInstructions"][0]["@type"], "HowToSection");
    assert_eq!(
        ld["recipeInstructions"][1]["itemListElement"][0]["text"],
        "Glaze it."
    );
    assert_eq!(ld["author"]["name"], "A Baker");
    assert!(
        ld["image"][0]
            .as_str()
            .unwrap()
            .starts_with(&format!("{url}/og.jpg"))
    );
    // Server-rendered lists with their headings, notes by default, the original link
    assert!(html.contains(r#"<h3 class="kicker">Glaze</h3>"#));
    assert!(html.contains(r#"<li data-scale-raw="100 g icing sugar">100 g icing sugar</li>"#));
    assert!(html.contains("Bake for 45 minutes."));
    assert!(html.contains("Grandma&#39;s secret: extra zest."));
    assert!(html.contains(r#"rel="noopener noreferrer nofollow""#));
    assert!(html.contains(">food.test<"));
    // Never the box's private parts
    for private in [
        "Private Shelf",
        "cookStats",
        "cookedAt",
        "checks",
        "createdAt",
        "\"id\"",
    ] {
        assert!(!html.contains(private), "{private} leaked");
    }
    // Another scraper keeps title, sections, steps and times
    let parsed = crumb::scraper::parse_recipe_html(&html, &url).unwrap();
    assert_eq!(parsed.title, "Lemon Cake");
    assert_eq!(parsed.ingredients.len(), 2);
    assert_eq!(parsed.ingredients[1].name.as_deref(), Some("Glaze"));
    assert_eq!(
        parsed.ingredients[1].items,
        vec!["100 g icing sugar", "1 lemon"]
    );
    assert_eq!(parsed.instructions.len(), 2);
    assert_eq!(parsed.instructions[0].name.as_deref(), Some("Bake"));
    assert_eq!(parsed.prep_time.as_deref(), Some("20m"));
    assert_eq!(parsed.total_time.as_deref(), Some("1h 5m"));

    // Notes off: gone from the page and the export
    let token = share["token"].as_str().unwrap();
    let (status, _) = call(
        &t,
        "PATCH",
        &format!("/api/recipes/{id}/share"),
        Some(json!({"includeNotes": false})),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, updated) = call(
        &t,
        "PATCH",
        &format!("/api/recipes/{id}/share"),
        Some(json!({"includeNotes": false})),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["includeNotes"], false);
    let (_, _, html) = t.send(get(&path)).await;
    assert!(!html.contains("extra zest"));
    let (status, headers, export) = t.send(get(&format!("{path}/crumb.json"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    let doc: Value = serde_json::from_str(&export).unwrap();
    assert_eq!(doc["format"], "crumb");
    assert!(doc["recipes"][0]["notes"].is_null());
    let (status, _) = call(
        &t,
        "PATCH",
        "/api/recipes/999/share",
        Some(json!({"includeNotes": true})),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // The old address, with the token in it, is gone
    let (status, _) = call(
        &t,
        "PATCH",
        &format!("/api/shares/{token}"),
        Some(json!({"includeNotes": true})),
        Some(&cookie),
    )
    .await;
    assert!(status == StatusCode::NOT_FOUND || status == StatusCode::METHOD_NOT_ALLOWED);
    call(
        &t,
        "PATCH",
        &format!("/api/recipes/{id}/share"),
        Some(json!({"includeNotes": true})),
        Some(&cookie),
    )
    .await;

    // The export: no cook log, no cookbooks, notes back on
    let (_, _, export) = t.send(get(&format!("{path}/crumb.json"))).await;
    assert!(
        !export.contains("cookedAt") && !export.contains("cookbooks"),
        "{export}"
    );
    assert!(!export.contains("Private Shelf"));
    let doc: Value = serde_json::from_str(&export).unwrap();
    assert_eq!(doc["recipes"][0]["notes"], "Grandma's secret: extra zest.");
    assert_eq!(doc["recipes"][0]["ingredients"][1]["name"], "Glaze");

    // ...and it round-trips through the importer, twice without a duplicate
    let other = TestApp::new(None);
    for expect_new in [1, 0] {
        let form = format!(
            "--X\r\nContent-Disposition: form-data; name=\"file\"; filename=\"cake.json\"\r\n\
             Content-Type: application/json\r\n\r\n{export}\r\n--X--\r\n"
        );
        let req = Request::builder()
            .method("POST")
            .uri("/api/import/files")
            .header(header::CONTENT_TYPE, "multipart/form-data; boundary=X")
            .body(Body::from(form))
            .unwrap();
        let (status, _, body) = other.send(req).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let summary: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            summary[0]["created"].as_array().unwrap().len(),
            expect_new,
            "{summary}"
        );
    }
    let (_, list) = other.json("GET", "/api/recipes", None).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    let (_, books) = other.json("GET", "/api/cookbooks", None).await;
    assert_eq!(books, json!([]));
    let (_, copy) = other
        .json("GET", &format!("/api/recipes/{}", list[0]["id"]), None)
        .await;
    assert_eq!(copy["notes"], "Grandma's secret: extra zest.");
    assert_eq!(
        copy["ingredients"][1]["items"],
        json!(["100 g icing sugar", "1 lemon"])
    );

    // Unknown tokens: the same plain 404
    for bad in [
        "/s/AAAAAAAAAAAAAAAAAAAAAA",
        "/s/short",
        "/s/AAAAAAAAAAAAAAAAAAAAAA/crumb.json",
    ] {
        let (status, _, body) = t.send(get(bad)).await;
        assert_eq!(
            (status, body.as_str()),
            (StatusCode::NOT_FOUND, "Not found"),
            "{bad}"
        );
    }
    // Only /s/ itself is public: not /s, /sx or a climb out of it
    for private in ["/s", "/sx", "/sxyz/abc", "/s/../api/recipes"] {
        let (status, _, _) = t.send(get(private)).await;
        assert_ne!(status, StatusCode::OK, "{private}");
    }
    let (status, _, _) = t.send(get("/sx")).await;
    assert_eq!(status, StatusCode::FOUND);
}

#[tokio::test]
async fn share_photos_are_public_while_img_stays_private() {
    use base64::Engine;
    let cache = tempfile::tempdir().unwrap();
    let dir = cache.path().join("img-cache");
    let t = TestApp::with_config(|c| {
        c.app_password = Some("pw".into());
        c.image_cache = Some(dir.clone());
    });
    let cookie = sign_in(&t, "pw").await;
    let (_, r) = call(
        &t,
        "POST",
        "/api/recipes",
        Some(shared_recipe("Cake")),
        Some(&cookie),
    )
    .await;
    let id = r["id"].as_i64().unwrap();
    let image = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png_bytes(1600, 1000))
    );
    set_image(&t, id, &image);
    let key = crumb::images::image_key(&image);
    let (_, share) = call(
        &t,
        "POST",
        &format!("/api/recipes/{id}/share"),
        None,
        Some(&cookie),
    )
    .await;
    let path = share_path(share["url"].as_str().unwrap());

    let (status, headers, body) = send_raw(&t, get(&format!("{path}/img/768?v={key}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/webp");
    assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=86400");
    assert_eq!(webp_size(&body), (768, 480));
    // Only the widths the resizer makes
    let (status, _, _) = send_raw(&t, get(&format!("{path}/img/700?v={key}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // The link-preview card: a 1200x630 JPEG crop, cached on disk
    let (status, headers, body) = send_raw(&t, get(&format!("{path}/og.jpg?v={key}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/jpeg");
    assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=86400");
    let og = image::load_from_memory_with_format(&body, image::ImageFormat::Jpeg).unwrap();
    assert_eq!((og.width(), og.height()), (1200, 630));
    assert!(dir.join(format!("{id}-{key}-og.jpg")).exists());

    // The same photo at /img still needs the login
    let (status, headers, _) = send_raw(&t, get(&format!("/img/{id}/768?v={key}"))).await;
    assert_eq!(status, StatusCode::FOUND);
    assert!(
        headers[header::LOCATION]
            .to_str()
            .unwrap()
            .starts_with("/login")
    );

    // No photo: no preview tags, and og.jpg 404s
    clear_image(&t, id);
    let (_, _, html) = t.send(get(&path)).await;
    assert!(!html.contains("og:image"));
    assert!(html.contains(r#"<meta name="twitter:card" content="summary">"#));
    let (status, _, _) = send_raw(&t, get(&format!("{path}/og.jpg"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn share_pages_escape_everything() {
    let t = TestApp::new(None);
    let evil = "</script><img src=x onerror=alert(1)>";
    let mut recipe = shared_recipe(evil);
    recipe["description"] = json!(format!("\"><script>alert(2)</script>{evil}"));
    recipe["ingredients"][0]["name"] = json!(evil);
    recipe["ingredients"][0]["items"][0] = json!(evil);
    recipe["instructions"][0]["items"][0] = json!(evil);
    recipe["notes"] = json!(evil);
    recipe["author"] = json!(evil);
    recipe["url"] = json!("https://food.test/\"><img src=x>");
    let (status, r) = t.json("POST", "/api/recipes", Some(recipe)).await;
    assert_eq!(status, StatusCode::CREATED, "{r}");
    let (_, share) = t
        .json("POST", &format!("/api/recipes/{}/share", r["id"]), None)
        .await;
    let (status, _, html) = t
        .send(get(&share_path(share["url"].as_str().unwrap())))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!html.contains("<img src=x"), "{html}");
    assert!(!html.contains("<script>alert"));
    assert_eq!(
        html.matches("</script>").count(),
        3,
        "only the template's, page data's and JSON-LD's own"
    );
    assert!(
        html.contains("<title>&lt;/script&gt;&lt;img src=x onerror=alert(1)&gt; · Crumb</title>")
    );
    assert!(html.contains(
        r#"<meta property="og:title" content="&lt;/script&gt;&lt;img src=x onerror=alert(1)&gt;">"#
    ));
    assert_eq!(json_ld(&html)["name"], evil);
    assert_eq!(json_ld(&html)["recipeIngredient"][0], evil);
}

#[tokio::test]
async fn share_misses_are_rate_limited() {
    let t = TestApp::new(None);
    for _ in 0..crumb::share::MISS_LIMIT {
        let (status, _, _) = t.send(get("/s/AAAAAAAAAAAAAAAAAAAAAA")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    let (status, headers, _) = t.send(get("/s/BBBBBBBBBBBBBBBBBBBBBB")).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(headers[header::RETRY_AFTER], "60");
    // The rest of the app doesn't care
    let (status, _) = t.json("GET", "/api/recipes", None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn saving_another_crumbs_share_imports_its_export() {
    // Crumb A, on a real port, shares two recipes: one with an original link, one without
    let a = TestApp::new(None);
    let (_, r) = a
        .json("POST", "/api/recipes", Some(shared_recipe("Lemon Cake")))
        .await;
    let mut own = shared_recipe("House Bread");
    own["url"] = Value::Null;
    let (_, r2) = a.json("POST", "/api/recipes", Some(own)).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let svc = NormalizePathLayer::trim_trailing_slash().layer(app(a.state.clone()));
    tokio::spawn(async move {
        axum::serve(
            listener,
            axum::ServiceExt::<Request<Body>>::into_make_service(svc),
        )
        .await
        .unwrap()
    });
    let mut links = Vec::new();
    for id in [&r["id"], &r2["id"]] {
        let (_, s) = a
            .json("POST", &format!("/api/recipes/{id}/share"), None)
            .await;
        links.push(format!(
            "{origin}{}",
            share_path(s["url"].as_str().unwrap())
        ));
    }

    // Crumb B saves them from the links
    let b = TestApp::new(None);
    let (status, first) = b
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"url": links[0]})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["isNew"], true);
    let (_, saved) = b
        .json("GET", &format!("/api/recipes/{}", first["id"]), None)
        .await;
    // Lossless: sections, notes and the original link. Kept under the share link (the
    // dedupe key), with the export's link as where it came from
    assert_eq!(saved["url"], links[0].as_str());
    assert_eq!(saved["originalUrl"], "https://food.test/lemon-cake");
    assert_eq!(saved["notes"], "Grandma's secret: extra zest.");
    assert_eq!(saved["ingredients"][0]["name"], "Cake");
    assert_eq!(saved["instructions"][1]["name"], "Finish");
    assert_eq!(saved["cookTime"], "45 mins");
    // Saved as it was, like a restore
    let (_, checks) = b
        .json("GET", &format!("/api/recipes/{}/checks", first["id"]), None)
        .await;
    assert_eq!(checks["status"], "skipped");
    // Again: the recipe already in the box
    let (_, again) = b
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"url": links[0]})),
        )
        .await;
    assert_eq!(
        (again["id"].clone(), again["isNew"].clone()),
        (first["id"].clone(), json!(false))
    );
    // The export's link claims nothing: saving that URL itself is a recipe of its own
    let (status, genuine) = b
        .json(
            "POST",
            "/api/recipes",
            Some(json!({
                "title": "Lemon Cake (the site's)",
                "url": "https://food.test/lemon-cake",
                "ingredients": [{"items": ["flour"]}],
                "instructions": [{"items": ["Bake."]}]
            })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{genuine}");
    assert_ne!(genuine["id"], first["id"]);
    // Shared on from B: the original link travels, not A's share link
    let (_, s) = b
        .json("POST", &format!("/api/recipes/{}/share", first["id"]), None)
        .await;
    let (_, _, export) = b
        .send(get(&format!(
            "{}/crumb.json",
            share_path(s["url"].as_str().unwrap())
        )))
        .await;
    let doc: Value = serde_json::from_str(&export).unwrap();
    assert_eq!(doc["recipes"][0]["url"], "https://food.test/lemon-cake");
    assert!(!export.contains(links[0].as_str()));
    // Backups keep both
    let (_, _, backup) = b.send(get("/api/export")).await;
    let backup: Value = serde_json::from_str(&backup).unwrap();
    let kept = backup["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["url"] == links[0].as_str())
        .unwrap();
    assert_eq!(kept["originalUrl"], "https://food.test/lemon-cake");

    // No original link: kept under the share link, so it dedupes on that
    let (_, bread) = b
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"url": links[1]})),
        )
        .await;
    assert_eq!(bread["isNew"], true, "{bread}");
    let (_, saved) = b
        .json("GET", &format!("/api/recipes/{}", bread["id"]), None)
        .await;
    assert_eq!(saved["url"], links[1].as_str());
    assert!(saved["originalUrl"].is_null());
    let (_, again) = b
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"url": links[1]})),
        )
        .await;
    assert_eq!(again["isNew"], false);
    let (_, list) = b.json("GET", "/api/recipes", None).await;
    assert_eq!(list.as_array().unwrap().len(), 3);
}

/// Serves `routes` on a real local port; returns its origin (`http://127.0.0.1:port`).
async fn serve(routes: axum::Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, routes).await.unwrap() });
    origin
}

/// A page that says it's a Crumb share, with a recipe in its JSON-LD to fall back on.
fn fake_share_page(export_href: &str) -> String {
    format!(
        r#"<html><head><title>Page Pie</title>
        <link rel="alternate" type="application/vnd.crumb+json" href="{export_href}">
        <script type="application/ld+json">{{"@context":"https://schema.org","@type":"Recipe",
        "name":"Page Pie","recipeIngredient":["1 pie"],"recipeInstructions":["Eat it."]}}</script>
        </head><body></body></html>"#
    )
}

fn fake_export(title: &str, url: &str, image: &str) -> String {
    json!({"format": "crumb", "version": 1, "recipes": [{
        "title": title, "url": url, "image": image,
        "ingredients": [{"name": null, "items": ["1 egg"]}],
        "instructions": [{"name": null, "items": ["Cook it."]}]
    }]})
    .to_string()
}

#[tokio::test]
async fn a_made_up_share_export_cant_plant_a_script_link_or_claim_a_url() {
    use axum::response::Html;
    use axum::routing::get as route;
    let evil = fake_export(
        "Evil Pie",
        "javascript://evil.test/%0Aalert(document.cookie)",
        "javascript:alert(1)",
    );
    let own_host = fake_export(
        "Self Pie",
        "http://127.0.0.1/popular-recipe",
        "https://img.test/p.jpg",
    );
    let origin = serve(
        axum::Router::new()
            .route(
                "/s/evil",
                route(|| async { Html(fake_share_page("/s/evil/crumb.json")) }),
            )
            .route("/s/evil/crumb.json", route(move || async move { evil }))
            .route(
                "/s/own",
                route(|| async { Html(fake_share_page("/s/own/crumb.json")) }),
            )
            .route("/s/own/crumb.json", route(move || async move { own_host })),
    )
    .await;
    let b = TestApp::new(None);

    // A javascript: "source" is dropped; the recipe is kept under the share link
    let link = format!("{origin}/s/evil");
    let (status, saved) = b
        .json("POST", "/api/recipes/import", Some(json!({"url": link})))
        .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (_, saved) = b
        .json("GET", &format!("/api/recipes/{}", saved["id"]), None)
        .await;
    assert_eq!(saved["title"], "Evil Pie", "{saved}");
    assert_eq!(saved["url"], link.as_str(), "{saved}");
    assert!(saved["originalUrl"].is_null());
    assert!(saved["image"].is_null());
    assert!(!saved.to_string().contains("javascript"));

    // A "source" on the share page's own host isn't taken as the original either
    let (_, saved) = b
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"url": format!("{origin}/s/own")})),
        )
        .await;
    let (_, saved) = b
        .json("GET", &format!("/api/recipes/{}", saved["id"]), None)
        .await;
    assert_eq!(saved["title"], "Self Pie");
    assert!(saved["originalUrl"].is_null());
}

#[tokio::test]
async fn crumb_exports_arent_fetched_from_another_origin() {
    use axum::response::{Html, Redirect};
    use axum::routing::get as route;
    // Another port on the same host is another origin
    let elsewhere = serve(axum::Router::new().route(
        "/x.json",
        route(|| async {
            fake_export("Hijacked", "https://food.test/h", "https://img.test/h.jpg")
        }),
    ))
    .await;
    let target = format!("{elsewhere}/x.json");
    let origin = serve(
        axum::Router::new()
            .route(
                "/s/hop",
                route(|| async { Html(fake_share_page("/s/hop/crumb.json")) }),
            )
            .route(
                "/s/hop/crumb.json",
                route(move || async move { Redirect::temporary(&target) }),
            ),
    )
    .await;
    let b = TestApp::new(None);
    let (status, saved) = b
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"url": format!("{origin}/s/hop")})),
        )
        .await;
    // The redirect isn't followed; the page itself is scraped instead
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["title"], "Page Pie");
}

#[tokio::test]
async fn a_stored_script_link_is_never_rendered() {
    let t = TestApp::new(None);
    let (_, r) = t
        .json("POST", "/api/recipes", Some(shared_recipe("Old Row")))
        .await;
    // An older row, from before links were checked
    t.state
        .db
        .lock()
        .execute(
            "UPDATE recipes SET url = 'javascript://x.test/%0Aalert(1)', original_url = 'data:text/html,hi' WHERE id = ?1",
            [r["id"].as_i64().unwrap()],
        )
        .unwrap();
    let (_, s) = t
        .json("POST", &format!("/api/recipes/{}/share", r["id"]), None)
        .await;
    let (status, _, html) = t.send(get(&share_path(s["url"].as_str().unwrap()))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !html.contains("javascript:") && !html.contains("data:text"),
        "{html}"
    );
    assert!(!html.contains("Original recipe"));
    // And it can't be written that way through the API
    let (status, _) = t
        .json(
            "PATCH",
            &format!("/api/recipes/{}", r["id"]),
            Some(json!({"url": "javascript:alert(1)"})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[test]
fn crumb_exports_are_only_taken_from_the_same_host() {
    use crumb::scraper::crumb_alternate;
    let page = |href: &str| {
        format!(
            r#"<html><head><link rel="alternate" type="application/vnd.crumb+json" href="{href}"></head></html>"#
        )
    };
    assert_eq!(
        crumb_alternate(&page("/s/abc/crumb.json"), "https://a.test/s/abc").as_deref(),
        Some("https://a.test/s/abc/crumb.json")
    );
    assert_eq!(
        crumb_alternate(&page("https://evil.test/x.json"), "https://a.test/s/abc"),
        None
    );
    assert_eq!(
        crumb_alternate(&page("file:///etc/passwd"), "https://a.test/s/abc"),
        None
    );
    // Same host, another scheme or port: another origin
    assert_eq!(
        crumb_alternate(
            &page("http://a.test/s/abc/crumb.json"),
            "https://a.test/s/abc"
        ),
        None
    );
    assert_eq!(
        crumb_alternate(
            &page("https://a.test:8443/s/abc/crumb.json"),
            "https://a.test/s/abc"
        ),
        None
    );
    assert_eq!(
        crumb_alternate(
            &page("https://a.test:443/s/abc/crumb.json"),
            "https://a.test/s/abc"
        )
        .as_deref(),
        Some("https://a.test/s/abc/crumb.json")
    );
    assert_eq!(
        crumb_alternate("<html></html>", "https://a.test/s/abc"),
        None
    );
}
