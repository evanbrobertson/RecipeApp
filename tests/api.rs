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
                 <script>boot()</script></head><body>{marker}<!--share:back--><!--share:photo--><!--share:intro-->\
                 <h2>Ingredients</h2><!--share:ingredients--><h2>Method</h2><!--share:method-->\
                 <!--share:source--></body></html>"
            ),
        )
        .unwrap();
        // The preview's, which is the share page's layout with its own island
        std::fs::create_dir_all(dist.path().join("shell/preview")).unwrap();
        std::fs::write(
            dist.path().join("shell/preview/index.html"),
            format!(
                "<!doctype html><html><head><title>Recipe · Crumb</title>\
                 <script>previewBoot()</script></head><body>{marker}<!--share:back--><!--share:photo-->\
                 <!--share:intro--><h2>Ingredients</h2><!--share:ingredients--><h2>Method</h2>\
                 <!--share:method--><!--share:source--></body></html>"
            ),
        )
        .unwrap();
        // And the shared cookbook's
        std::fs::create_dir_all(dist.path().join("shell/share-book")).unwrap();
        std::fs::write(
            dist.path().join("shell/share-book/index.html"),
            format!(
                "<!doctype html><html><head><title>Shared cookbook · Crumb</title>\
                 <script>bookBoot()</script></head><body>{marker}<!--share:book-->\
                 <!--share:recipes--></body></html>"
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
    // Filed under the fixed list
    assert_eq!(created["recipeCategory"], "Main");
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

    let (_, found) = t.json("GET", "/api/recipes?q=lemon%20main", None).await;
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
async fn recipe_video_is_saved_and_embedded() {
    let t = TestApp::new(None);
    let (status, created) = t
        .json(
            "POST",
            "/api/recipes",
            Some(json!({
                "title": "Beef and Broccoli",
                "video": "https://youtu.be/8eITNSfct3Q?si=YEDH1yr00LEEj6dN",
                "ingredients": [{"items": ["1 lb flank steak"]}],
            })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(
        created["video"],
        "https://youtu.be/8eITNSfct3Q?si=YEDH1yr00LEEj6dN"
    );
    assert_eq!(created["videoEmbed"]["provider"], "youtube");
    assert_eq!(
        created["videoEmbed"]["embedUrl"],
        "https://www.youtube-nocookie.com/embed/8eITNSfct3Q?rel=0"
    );
    let id = created["id"].as_i64().unwrap();

    // A site Crumb can't play in place is kept, and linked to rather than embedded
    let uri = format!("/api/recipes/{id}");
    let (_, r) = t
        .json(
            "PATCH",
            &uri,
            Some(json!({"video": "https://videos.test/beef"})),
        )
        .await;
    assert_eq!(r["video"], "https://videos.test/beef");
    assert!(r["videoEmbed"].is_null());

    let (status, _) = t
        .json("PATCH", &uri, Some(json!({"video": "javascript:alert(1)"})))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, r) = t.json("PATCH", &uri, Some(json!({"video": null}))).await;
    assert!(r["video"].is_null());
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
async fn staples_count_normalised_ingredients_but_not_salt() {
    let t = TestApp::new(None);
    let (_, body) = t.json("GET", "/api/staples", None).await;
    assert_eq!(body, json!({"recipes": 0, "staples": []}));

    for (title, ing) in [
        ("Tacos", "2 limes, juiced"),
        ("Curry", "1 lime"),
        ("Stir Fry", "3 garlic cloves, minced"),
    ] {
        add_recipe(&t, title, "Main", ing, "20m").await;
    }
    let (status, body) = t.json("GET", "/api/staples", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({"recipes": 3, "staples": [
            {"name": "limes", "recipes": 2},
        ]})
    );
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
    let (status, stats) = t
        .json("GET", &format!("/api/recipes/{id}/cooked"), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stats["count"], 1);
    let (status, _) = t.json("GET", "/api/recipes/999/cooked", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

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
async fn a_bot_check_is_reported_as_site_blocked() {
    let app = axum::Router::new()
        .route(
            "/denied",
            axum::routing::get(|| async { StatusCode::FORBIDDEN }),
        )
        .route(
            "/check",
            axum::routing::get(|| async {
                axum::response::Html("<html><head><title>Just a moment...</title></head></html>")
            }),
        )
        .route(
            "/gone",
            axum::routing::get(|| async { StatusCode::NOT_FOUND }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let site = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let t = TestApp::new(None);
    for path in ["/denied", "/check"] {
        let url = format!("{site}{path}");
        let (status, err) = t
            .json("POST", "/api/recipes/import", Some(json!({"url": url})))
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{path}: {err}");
        assert_eq!(err["code"], "site_blocked", "{path}: {err}");
        assert!(err["message"].as_str().unwrap().contains("extension"));

        // Claude is pointed at the extension, not shown the site's status
        let (msg, is_error) = mcp_call(&t, "import_recipe_from_url", json!({"url": url})).await;
        assert!(is_error, "{msg}");
        assert!(msg.contains("browser extension"), "{msg}");
    }

    // Any other failure keeps its own message and carries no code
    let (status, err) = t
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"url": format!("{site}/gone")})),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(err.get("code").is_none(), "{err}");
    assert!(err["message"].as_str().unwrap().contains("404"), "{err}");
    let (_, err) = t
        .json("POST", "/api/recipes/import", Some(json!({"url": "nope"})))
        .await;
    assert!(err.get("code").is_none(), "{err}");
}

#[tokio::test]
async fn imports_drop_a_dead_photo_link() {
    let png = png_bytes(40, 30);
    let page = |name: &str, image: &str| {
        format!(
            r#"<html><head><script type="application/ld+json">{{"@context":"https://schema.org","@type":"Recipe",
            "name":"{name}","image":"{image}","recipeIngredient":["1 egg"],
            "recipeInstructions":[{{"@type":"HowToStep","text":"Cook."}}]}}</script></head><body></body></html>"#
        )
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let site = format!("http://{}", listener.local_addr().unwrap());
    let pages: Vec<(String, String)> = vec![
        ("/fresh".into(), page("Fresh", &format!("{site}/photo.png"))),
        // A CDN link that went stale
        ("/stale".into(), page("Stale", &format!("{site}/gone.jpg"))),
        // A page where the photo should be
        (
            "/blocked".into(),
            page("Blocked", &format!("{site}/blocked.jpg")),
        ),
        // The photo server having a bad moment: kept
        ("/flaky".into(), page("Flaky", &format!("{site}/busy.jpg"))),
        (
            "/mcp-stale".into(),
            page("By Claude", &format!("{site}/gone.jpg")),
        ),
    ];
    let mut app = axum::Router::new()
        .route(
            "/photo.png",
            axum::routing::get(move || async move { ([(header::CONTENT_TYPE, "image/png")], png) }),
        )
        .route(
            "/gone.jpg",
            axum::routing::get(|| async { StatusCode::NOT_FOUND }),
        )
        .route(
            "/blocked.jpg",
            axum::routing::get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/html")],
                    "<html>no hotlinking</html>",
                )
            }),
        )
        .route(
            "/busy.jpg",
            axum::routing::get(|| async { StatusCode::SERVICE_UNAVAILABLE }),
        );
    for (path, html) in pages {
        app = app.route(
            &path,
            axum::routing::get(move || async move { axum::response::Html(html) }),
        );
    }
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let t = TestApp::new(None);
    for (path, kept) in [
        ("/fresh", true),
        ("/stale", false),
        ("/blocked", false),
        ("/flaky", true),
    ] {
        let url = format!("{site}{path}");
        let (status, res) = t
            .json("POST", "/api/recipes/import", Some(json!({"url": url})))
            .await;
        assert_eq!(status, StatusCode::OK, "{path}: {res}");
        assert_eq!(res.get("droppedPhoto").is_some(), !kept, "{path}: {res}");
        let (_, recipe) = t
            .json("GET", &format!("/api/recipes/{}", res["id"]), None)
            .await;
        assert_eq!(recipe["image"].is_string(), kept, "{path}: {recipe}");
    }

    // The same link again is the saved recipe, with nothing more to say
    let (_, res) = t
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"url": format!("{site}/stale")})),
        )
        .await;
    assert_eq!(res["isNew"], false);
    assert!(res.get("droppedPhoto").is_none(), "{res}");

    // Claude hears about it too
    let (msg, err) = mcp_call(
        &t,
        "import_recipe_from_url",
        json!({"url": format!("{site}/mcp-stale")}),
    )
    .await;
    assert!(!err, "{msg}");
    assert!(msg.contains("saved without a photo"), "{msg}");
}

#[tokio::test]
async fn broken_photo_links_are_flagged_without_wee_chef() {
    use axum::response::IntoResponse;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    // Down until `up` is set: then the same link works again
    let up = Arc::new(AtomicBool::new(false));
    let png = png_bytes(60, 40);
    let flag = up.clone();
    let origin = axum::Router::new()
        .route(
            "/gone.jpg",
            axum::routing::get(|| async { StatusCode::NOT_FOUND }),
        )
        .route(
            "/busy.jpg",
            axum::routing::get(|| async { StatusCode::SERVICE_UNAVAILABLE }),
        )
        .route(
            "/back.png",
            axum::routing::get(move || {
                let (png, up) = (png.clone(), flag.load(Ordering::SeqCst));
                async move {
                    if up {
                        ([(header::CONTENT_TYPE, "image/png")], png).into_response()
                    } else {
                        StatusCode::NOT_FOUND.into_response()
                    }
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let site = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, origin).await.unwrap() });
    let hint = |headers: &axum::http::HeaderMap| {
        headers
            .get_all("server-timing")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find(|v| v.starts_with("crumb-review"))
            .map(String::from)
    };
    let home = || {
        Request::get("/")
            .header("sec-fetch-dest", "document")
            .body(Body::empty())
            .unwrap()
    };

    // No Wee Chef checks set up: the nav keeps Suggestions hidden while nothing's flagged
    let t = TestApp::new(None);
    let stale = add_recipe(&t, "Stale", "Dinner", "eggs", "10 min").await;
    let busy = add_recipe(&t, "Busy", "Dinner", "eggs", "10 min").await;
    set_image(&t, stale, &format!("{site}/gone.jpg"));
    set_image(&t, busy, &format!("{site}/busy.jpg"));
    let (_, headers, _) = send_raw(&t, home()).await;
    assert_eq!(hint(&headers), None);

    // The resizer finds the dead link (once, however often it's asked); a site having a
    // bad moment isn't flagged
    for _ in 0..2 {
        let (status, _, _) = send_raw(&t, get(&format!("/img/{stale}/320"))).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    // It's most likely fine in a browser, so the page is sent there instead of a 404
    for _ in 0..2 {
        let (status, headers, _) = send_raw(&t, get(&format!("/img/{busy}/320"))).await;
        assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(
            headers[header::LOCATION],
            format!("{site}/busy.jpg").as_str()
        );
    }
    let (_, list) = t.json("GET", "/api/checks/review", None).await;
    assert_eq!(list["recipes"].as_array().unwrap().len(), 1, "{list}");
    assert_eq!(list["recipes"][0]["id"], stale);
    assert_eq!(list["recipes"][0]["fields"], json!({"image": 1}));
    let (_, headers, _) = send_raw(&t, home()).await;
    assert_eq!(hint(&headers).as_deref(), Some("crumb-review;desc=\"1\""));

    // The recipe page and editor get it, though Jev never checked the recipe
    let (_, checks) = t
        .json("GET", &format!("/api/recipes/{stale}/checks"), None)
        .await;
    assert_eq!(checks["status"], Value::Null);
    assert_eq!(checks["flags"].as_array().unwrap().len(), 1, "{checks}");
    let flag = &checks["flags"][0];
    assert_eq!(flag["field"], "image");
    assert_eq!(flag["kind"], "dead_photo");
    assert_eq!(flag["itemText"], format!("{site}/gone.jpg"));
    let (_, checks) = t
        .json("GET", &format!("/api/recipes/{busy}/checks"), None)
        .await;
    assert_eq!(checks, Value::Null);

    // "Keep as is" sticks for that link, even when the resizer tries it again later
    let (status, _) = t
        .json(
            "POST",
            &format!("/api/recipes/{stale}/flags/{}/dismiss", flag["id"]),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    crumb::checks::flag_dead_photo(&t.state.db.lock(), stale, &format!("{site}/gone.jpg")).unwrap();
    let (_, list) = t.json("GET", "/api/checks/review", None).await;
    assert_eq!(list["recipes"], json!([]));

    // A new link that's dead too is flagged again; saving another photo resolves it
    t.json(
        "PATCH",
        &format!("/api/recipes/{stale}"),
        Some(json!({"image": format!("{site}/back.png")})),
    )
    .await;
    send_raw(&t, get(&format!("/img/{stale}/320"))).await;
    let (_, list) = t.json("GET", "/api/checks/review", None).await;
    assert_eq!(list["recipes"].as_array().unwrap().len(), 1, "{list}");
    t.json(
        "PATCH",
        &format!("/api/recipes/{stale}"),
        Some(json!({"image": null})),
    )
    .await;
    let (_, list) = t.json("GET", "/api/checks/review", None).await;
    assert_eq!(list["recipes"], json!([]));

    // A link that comes back to life clears its own flag
    set_image(&t, busy, &format!("{site}/back.png"));
    crumb::checks::flag_dead_photo(&t.state.db.lock(), busy, &format!("{site}/back.png")).unwrap();
    up.store(true, Ordering::SeqCst);
    let (status, _, _) = send_raw(&t, get(&format!("/img/{busy}/320"))).await;
    assert_eq!(status, StatusCode::OK);
    let (_, list) = t.json("GET", "/api/checks/review", None).await;
    assert_eq!(list["recipes"], json!([]));
    let (_, headers, _) = send_raw(&t, home()).await;
    assert_eq!(hint(&headers), None);
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
                        } else if id == "category" {
                            // Only the recipes written to have one
                            let title = body["state"]["title"].as_str().unwrap_or_default();
                            if title.contains("Mystery") {
                                choice("other", "soup", 0.88)
                            } else {
                                choice("other", "other", 0.4)
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

#[tokio::test]
async fn suggestions_page_shows_check_all_even_with_nothing_waiting() {
    let hint = |headers: &axum::http::HeaderMap| {
        headers
            .get_all("server-timing")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find(|v| v.starts_with("crumb-review"))
            .map(String::from)
    };
    let page = |path: &str| {
        Request::get(path)
            .header("sec-fetch-dest", "document")
            .body(Body::empty())
            .unwrap()
    };

    // Checks set up, nothing waiting: the nav still gets a (zero) hint, so it keeps the
    // Suggestions item, and the page renders with its Check all progress
    let t = jev_app("http://127.0.0.1:9");
    let (status, headers, _) = send_raw(&t, page("/")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(hint(&headers).as_deref(), Some("crumb-review;desc=\"0\""));
    let (status, headers, html) = t.send(page("/suggestions")).await;
    assert_eq!(status, StatusCode::OK, "no redirect when empty");
    assert!(headers.get(header::LOCATION).is_none());
    assert!(html.contains(r#""recipes":[]"#), "{html}");
    assert!(html.contains(r#""checks":{"#), "{html}");
    assert!(html.contains(r#""enabled":true"#), "{html}");

    // Not set up: no hint (the item stays hidden) and no card data, but still a page
    let off = TestApp::new(None);
    let (_, headers, _) = send_raw(&off, page("/")).await;
    assert_eq!(hint(&headers), None);
    let (status, _, html) = off.send(page("/suggestions")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains(r#""recipes":[]"#), "{html}");
    assert!(!html.contains(r#""checks""#), "{html}");
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
    // Categories from before the fixed list, straight into A's database
    for (id, category) in [(&r["id"], "Cakes"), (&r2["id"], "Holiday")] {
        a.state
            .db
            .lock()
            .execute(
                "UPDATE recipes SET recipe_category = ?1 WHERE id = ?2",
                rusqlite::params![category, id.as_i64().unwrap()],
            )
            .unwrap();
    }
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
    // A's "Cakes" went out filed under the list
    assert_eq!(saved["recipeCategory"], "Baking");
    let (_, _, page) = a.send(get(&share_path(&links[0]))).await;
    assert!(page.contains(r#""recipeCategory":"Baking""#), "{page}");
    assert!(!page.contains("Cakes"));
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
    assert!(saved["recipeCategory"].is_null(), "{saved}");
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
async fn a_share_from_an_older_crumb_has_its_category_filed() {
    use axum::response::Html;
    use axum::routing::get as route;
    let mut old: Value = serde_json::from_str(&fake_export(
        "Old Pie",
        "https://food.test/old-pie",
        "https://img.test/p.jpg",
    ))
    .unwrap();
    old["recipes"][0]["recipeCategory"] = json!("Dinner, Entree");
    let old = old.to_string();
    let origin = serve(
        axum::Router::new()
            .route(
                "/s/old",
                route(|| async { Html(fake_share_page("/s/old/crumb.json")) }),
            )
            .route("/s/old/crumb.json", route(move || async move { old })),
    )
    .await;
    let b = TestApp::new(None);
    let (status, saved) = b
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"url": format!("{origin}/s/old")})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (_, saved) = b
        .json("GET", &format!("/api/recipes/{}", saved["id"]), None)
        .await;
    assert_eq!(saved["title"], "Old Pie");
    assert_eq!(saved["recipeCategory"], "Main");
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

#[tokio::test]
async fn categories_are_filed_under_the_fixed_list() {
    let site = recipe_site(vec![
        (
            "joes",
            json!({"@type": "Recipe", "name": "Sloppy Joes",
                "recipeCategory": ["Dinner", "Entree", "Sandwich"],
                "recipeIngredient": ["1 lb beef"], "recipeInstructions": ["Brown the beef."]}),
        ),
        (
            "fudge",
            json!({"@type": "Recipe", "name": "Fudge", "recipeCategory": "Holiday",
                "recipeIngredient": ["1 cup sugar"], "recipeInstructions": ["Boil."]}),
        ),
    ])
    .await;
    let t = TestApp::new(None);
    let (status, joes) = t
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"url": format!("{site}/joes")})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{joes}");
    let at = format!("/api/recipes/{}", joes["id"]);
    let (_, r) = t.json("GET", &at, None).await;
    assert_eq!(r["recipeCategory"], "Main");
    // Wording that names nothing on the list isn't kept (Wee Chef's check picks one)
    let (_, fudge) = t
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"url": format!("{site}/fudge")})),
        )
        .await;
    let (_, r) = t
        .json("GET", &format!("/api/recipes/{}", fudge["id"]), None)
        .await;
    assert_eq!(r["title"], "Fudge");
    assert_eq!(r["recipeCategory"], Value::Null);

    // Every save is filed too: synonyms map, anything else is Other
    for (set, saved) in [
        (json!("lunch"), json!("Main")),
        (json!("whatever"), json!("Other")),
        (json!("cookies"), json!("Baking")),
        (json!(" "), Value::Null),
        (Value::Null, Value::Null),
    ] {
        let (status, r) = t
            .json("PATCH", &at, Some(json!({"recipeCategory": set})))
            .await;
        assert_eq!(status, StatusCode::OK, "{r}");
        assert_eq!(r["recipeCategory"], saved, "{set}");
    }
    // The editor sends every field: an old category sent back as it is stays (a check
    // files it, under Undo), while a new one is filed
    t.state
        .db
        .lock()
        .execute(
            "UPDATE recipes SET recipe_category = 'Holiday' WHERE id = ?1",
            [joes["id"].as_i64().unwrap()],
        )
        .unwrap();
    let (status, r) = t
        .json(
            "PATCH",
            &at,
            Some(json!({"title": "Sloppy Joe", "recipeCategory": "Holiday"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{r}");
    assert_eq!(
        (&r["title"], &r["recipeCategory"]),
        (&json!("Sloppy Joe"), &json!("Holiday"))
    );
    let (msg, _) = mcp_call(
        &t,
        "update_recipe",
        json!({"id": joes["id"], "notes": "Toast the buns.", "recipeCategory": "Holiday"}),
    )
    .await;
    assert!(msg.contains("Updated"), "{msg}");
    let (_, r) = t.json("GET", &at, None).await;
    assert_eq!(r["recipeCategory"], "Holiday");
    let (msg, _) = mcp_call(
        &t,
        "update_recipe",
        json!({"id": joes["id"], "recipeCategory": "supper"}),
    )
    .await;
    assert!(msg.contains("Updated"), "{msg}");
    let (_, r) = t.json("GET", &at, None).await;
    assert_eq!(r["recipeCategory"], "Main");
    let (msg, _) = mcp_call(
        &t,
        "save_recipe",
        json!({"title": "Punch", "recipeCategory": "Beverages",
            "ingredients": [{"items": ["juice"]}], "instructions": [{"items": ["Stir."]}]}),
    )
    .await;
    assert!(msg.contains("Punch"), "{msg}");
    let (_, all) = t.json("GET", "/api/recipes?q=punch", None).await;
    assert_eq!(all[0]["recipeCategory"], "Drink");
}

#[tokio::test]
async fn check_all_files_old_categories_and_undo_puts_them_back() {
    let (jev, _) = fake_jev().await;
    let t = jev_app(&jev);
    let make = |title: &str, category: &str| crumb::model::RecipeFields {
        title: title.into(),
        recipe_category: Some(category.into()),
        ingredients: vec![crumb::model::Section {
            name: None,
            items: vec!["1 cup broth".into()],
        }],
        instructions: vec![crumb::model::Section {
            name: None,
            items: vec!["Simmer it.".into()],
        }],
        ..Default::default()
    };
    // Saved before the list, straight into the database
    let (old, _) = crumb::recipes::create_recipe(
        &t.state.db.lock(),
        make("Casserole", "One dish meal"),
        "url",
    )
    .unwrap();
    let (odd, _) =
        crumb::recipes::create_recipe(&t.state.db.lock(), make("Mystery Stew", "Holiday"), "url")
            .unwrap();
    let (_, all) = t.json("POST", "/api/checks", None).await;
    assert_eq!(all["queued"], 2);

    let c = wait_for_check(&t, old.id).await;
    assert_eq!(c["status"], "done", "{c}");
    let (_, r) = t
        .json("GET", &format!("/api/recipes/{}", old.id), None)
        .await;
    assert_eq!(r["recipeCategory"], "Main");
    assert_eq!(c["flags"][0]["kind"], "category");
    assert_eq!(c["flags"][0]["detail"]["was"], "One dish meal");
    assert_eq!(c["canUndo"], true);
    // Cleared, then filled in by Wee Chef
    wait_for_check(&t, odd.id).await;
    let (_, r) = t
        .json("GET", &format!("/api/recipes/{}", odd.id), None)
        .await;
    assert_eq!(r["recipeCategory"], "Soup");

    let (status, _) = t
        .json(
            "POST",
            &format!("/api/recipes/{}/checks/undo", old.id),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, r) = t
        .json("GET", &format!("/api/recipes/{}", old.id), None)
        .await;
    assert_eq!(r["recipeCategory"], "One dish meal");
    // Undone, so the next "Check all" leaves it as the cook has it
    let (_, all) = t.json("POST", "/api/checks", None).await;
    assert_eq!(all["queued"], 0);
}

// ─── Shared cookbooks ───────────────────────────────────────────────────────

fn book_recipe(title: &str, url: Option<&str>, notes: &str) -> Value {
    json!({
        "title": title,
        "url": url,
        "description": format!("All about {title}."),
        "totalTime": "40 mins",
        "recipeCategory": "Dinner",
        "ingredients": [{"name": null, "items": ["1 onion", "2 carrots"]}],
        "instructions": [{"name": null, "items": ["Chop.", "Cook."]}],
        "notes": notes,
    })
}

/// `/s/{token}` from a share link.
fn book_path(url: &str) -> String {
    let token = url.split("/s/").nth(1).unwrap();
    format!("/s/{token}")
}

async fn signed(t: &TestApp, cookie: &str, method: &str, uri: &str, body: Option<Value>) -> Value {
    let (status, v) = call(t, method, uri, body, Some(cookie)).await;
    assert!(status.is_success(), "{method} {uri}: {status} {v}");
    v
}

#[tokio::test]
async fn shared_cookbooks_are_live_public_pages() {
    let t = TestApp::new(Some("pw"));
    let cookie = sign_in(&t, "pw").await;
    let mut ids = std::collections::HashMap::new();
    for (title, url) in [
        ("Banana Bread", Some("https://food.test/banana")),
        ("Apple Pie", Some("https://food.test/apple")),
        ("Zucchini Soup", None),
        ("Carrot Cake", None),
    ] {
        let r = signed(
            &t,
            &cookie,
            "POST",
            "/api/recipes",
            Some(book_recipe(title, url, &format!("{title} note: go slow."))),
        )
        .await;
        ids.insert(title, r["id"].as_i64().unwrap());
    }
    let book = signed(
        &t,
        &cookie,
        "POST",
        "/api/cookbooks",
        Some(json!({"name": "Weeknight dinners", "color": "clay"})),
    )
    .await;
    let bid = book["id"].as_i64().unwrap();
    let secret = signed(
        &t,
        &cookie,
        "POST",
        "/api/cookbooks",
        Some(json!({"name": "Secret Shelf"})),
    )
    .await;
    signed(
        &t,
        &cookie,
        "POST",
        &format!("/api/cookbooks/{bid}/recipes"),
        Some(json!({"recipeIds": [ids["Banana Bread"], ids["Apple Pie"]]})),
    )
    .await;
    signed(
        &t,
        &cookie,
        "POST",
        &format!("/api/cookbooks/{}/recipes", secret["id"]),
        Some(json!({"recipeIds": ids.values().collect::<Vec<_>>()})),
    )
    .await;
    signed(
        &t,
        &cookie,
        "POST",
        &format!("/api/recipes/{}/cooked", ids["Apple Pie"]),
        None,
    )
    .await;

    // One link per book; making it needs the login
    let (status, _) = call(
        &t,
        "POST",
        &format!("/api/cookbooks/{bid}/share"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = call(&t, "POST", "/api/cookbooks/999/share", None, Some(&cookie)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let first = signed(
        &t,
        &cookie,
        "POST",
        &format!("/api/cookbooks/{bid}/share"),
        None,
    )
    .await;
    let again = signed(
        &t,
        &cookie,
        "POST",
        &format!("/api/cookbooks/{bid}/share"),
        None,
    )
    .await;
    assert_eq!(first["url"], again["url"]);
    assert_eq!(first["includeNotes"], true);
    let url = first["url"].as_str().unwrap().to_string();
    let path = book_path(&url);
    // The cookbook page knows its link
    let (_, _, html) = t
        .send(
            Request::builder()
                .uri(format!("/cookbooks/{bid}"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert!(html.contains(&format!(r#""url":"{url}""#)), "{html}");

    // Logged out: the book, its recipes in the book's order, nothing else
    let (status, headers, html) = t.send(get(&path)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["x-robots-tag"], "noindex, noimageindex");
    assert_eq!(headers[header::REFERRER_POLICY], "no-referrer");
    assert_eq!(headers[header::CACHE_CONTROL], "no-cache");
    let csp = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(
        csp.starts_with("default-src 'none'; script-src 'self' 'sha256-"),
        "{csp}"
    );
    assert!(csp.contains("frame-ancestors 'none'"));
    assert!(html.contains("<title>Weeknight dinners · Crumb</title>"));
    assert!(html.contains(r#"<meta property="og:title" content="Weeknight dinners">"#));
    assert!(html.contains(r#"data-color="clay""#));
    assert!(html.contains("2 recipes"));
    assert!(html.contains(&format!(
        r#"<link rel="alternate" type="application/vnd.crumb+json" href="{url}/crumb.json">"#
    )));
    // No photos in this book: no preview image
    assert!(!html.contains("og:image"));
    let apple = html.find("Apple Pie").expect("Apple Pie listed");
    let banana = html.find("Banana Bread").expect("Banana Bread listed");
    assert!(apple < banana, "listed by title");
    assert!(html.contains(&format!(r#"href="{path}/{}""#, ids["Apple Pie"])));
    for hidden in [
        "Zucchini Soup",
        "Carrot Cake",
        "Secret Shelf",
        "note: go slow",
    ] {
        assert!(!html.contains(hidden), "{hidden} leaked");
    }

    // A recipe in the book: its page, with the way back
    let apple_path = format!("{path}/{}", ids["Apple Pie"]);
    let (status, headers, html) = t.send(get(&apple_path)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["x-robots-tag"], "noindex, noimageindex");
    assert!(
        headers[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap()
            .contains("script-src 'self' 'sha256-")
    );
    assert!(html.contains("<title>Apple Pie · Crumb</title>"));
    assert!(html.contains(&format!(r#"href="{path}">"#)));
    assert!(html.contains("Weeknight dinners"));
    assert!(html.contains("Apple Pie note: go slow."));
    assert!(html.contains(&format!(r#"href="{url}/{}/crumb.json""#, ids["Apple Pie"])));
    assert!(!html.contains("Secret Shelf"));
    let (status, _, json) = t.send(get(&format!("{apple_path}/crumb.json"))).await;
    assert_eq!(status, StatusCode::OK);
    let doc: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(doc["recipes"][0]["title"], "Apple Pie");
    assert_eq!(doc["recipes"][0]["notes"], "Apple Pie note: go slow.");
    assert!(!json.contains("cookedAt") && !json.contains("Secret Shelf"));

    // Not in the book, or not a recipe: the same 404 as an unknown link
    for missing in [
        format!("{path}/{}", ids["Zucchini Soup"]),
        format!("{path}/{}/crumb.json", ids["Zucchini Soup"]),
        format!("{path}/99999"),
        format!("{path}/abc"),
        format!("{path}/0{}", ids["Apple Pie"]),
        format!("{path}/og.jpg"),
        format!("{apple_path}/nope"),
    ] {
        let (status, _, _) = t.send(get(&missing)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");
    }

    // Live: a recipe added later shows up, one taken out is gone at once
    signed(
        &t,
        &cookie,
        "POST",
        &format!("/api/cookbooks/{bid}/recipes"),
        Some(json!({"recipeIds": [ids["Carrot Cake"]]})),
    )
    .await;
    let (_, _, html) = t.send(get(&path)).await;
    assert!(html.contains("Carrot Cake") && html.contains("3 recipes"));
    let (status, _, _) = t.send(get(&format!("{path}/{}", ids["Carrot Cake"]))).await;
    assert_eq!(status, StatusCode::OK);
    signed(
        &t,
        &cookie,
        "DELETE",
        &format!("/api/cookbooks/{bid}/recipes/{}", ids["Apple Pie"]),
        None,
    )
    .await;
    for gone in [apple_path.clone(), format!("{apple_path}/crumb.json")] {
        let (status, _, _) = t.send(get(&gone)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{gone}");
    }
    let (_, _, html) = t.send(get(&path)).await;
    assert!(!html.contains("Apple Pie"));

    // The whole book as a Crumb file: each recipe in this book only, under its own link
    let (status, headers, json) = t.send(get(&format!("{path}/crumb.json"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    let doc: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(doc["format"], "crumb");
    assert_eq!(doc["kind"], "cookbook");
    assert_eq!(doc["cookbooks"][0]["name"], "Weeknight dinners");
    assert_eq!(doc["cookbooks"][0]["color"], "clay");
    let list = doc["recipes"].as_array().unwrap();
    assert_eq!(
        list.iter().map(|r| r["title"].clone()).collect::<Vec<_>>(),
        vec![json!("Banana Bread"), json!("Carrot Cake")]
    );
    for r in list {
        assert_eq!(r["cookbooks"], json!(["Weeknight dinners"]));
        assert!(
            r["shareUrl"]
                .as_str()
                .unwrap()
                .starts_with(&format!("{url}/"))
        );
        assert!(r["notes"].as_str().unwrap().contains("note: go slow"));
    }
    assert_eq!(list[0]["url"], "https://food.test/banana");
    assert!(!json.contains("cookedAt") && !json.contains("Secret Shelf"));

    // Notes off: gone from the recipe pages and both kinds of file
    let off = signed(
        &t,
        &cookie,
        "PATCH",
        &format!("/api/cookbooks/{bid}/share"),
        Some(json!({"includeNotes": false})),
    )
    .await;
    assert_eq!(
        (off["url"].clone(), off["includeNotes"].clone()),
        (json!(url), json!(false))
    );
    let banana_path = format!("{path}/{}", ids["Banana Bread"]);
    for p in [
        banana_path.clone(),
        format!("{banana_path}/crumb.json"),
        format!("{path}/crumb.json"),
    ] {
        let (status, _, body) = t.send(get(&p)).await;
        assert_eq!(status, StatusCode::OK, "{p}");
        assert!(!body.contains("note: go slow"), "{p}");
    }

    // The More page's list: both kinds, newest first, with no token in any URL it asks for
    let recipe_share = signed(
        &t,
        &cookie,
        "POST",
        &format!("/api/recipes/{}/share", ids["Zucchini Soup"]),
        None,
    )
    .await;
    let (status, _) = call(&t, "GET", "/api/shares", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let shares = signed(&t, &cookie, "GET", "/api/shares", None).await;
    let shares = shares.as_array().unwrap();
    assert_eq!(shares.len(), 2, "{shares:?}");
    let kinds: Vec<_> = shares
        .iter()
        .map(|s| (s["kind"].as_str().unwrap(), s["title"].as_str().unwrap()))
        .collect();
    assert!(kinds.contains(&("cookbook", "Weeknight dinners")));
    assert!(kinds.contains(&("recipe", "Zucchini Soup")));
    let book_row = shares.iter().find(|s| s["kind"] == "cookbook").unwrap();
    assert_eq!(book_row["id"], bid);
    assert_eq!(book_row["url"], url.as_str());
    assert!(book_row["createdAt"].is_string());
    assert!(book_row["lastOpenedAt"].is_string(), "the book was opened");
    let recipe_row = shares.iter().find(|s| s["kind"] == "recipe").unwrap();
    assert_eq!(recipe_row["url"], recipe_share["url"]);
    assert!(recipe_row["lastOpenedAt"].is_null());

    // The owner's download of the book
    let (status, _) = call(
        &t,
        "GET",
        &format!("/api/cookbooks/{bid}/export"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, headers, json) = t
        .send(
            Request::builder()
                .uri(format!("/api/cookbooks/{bid}/export"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers[header::CONTENT_DISPOSITION],
        "attachment; filename=\"weeknight-dinners.json\""
    );
    let doc: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(doc["recipes"].as_array().unwrap().len(), 2);
    assert!(
        doc["recipes"][0]["notes"].is_string(),
        "the owner's own notes"
    );
    assert!(!json.contains("Secret Shelf") && !json.contains("shareUrl"));
    let (status, _) = call(&t, "GET", "/api/cookbooks/999/export", None, Some(&cookie)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Stopping, and deleting the book, end the link
    signed(
        &t,
        &cookie,
        "DELETE",
        &format!("/api/cookbooks/{bid}/share"),
        None,
    )
    .await;
    let (status, _, _) = t.send(get(&path)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let fresh = signed(
        &t,
        &cookie,
        "POST",
        &format!("/api/cookbooks/{bid}/share"),
        None,
    )
    .await;
    assert_ne!(fresh["url"], json!(url));
    signed(
        &t,
        &cookie,
        "DELETE",
        &format!("/api/cookbooks/{bid}"),
        None,
    )
    .await;
    let (status, _, _) = t
        .send(get(&book_path(fresh["url"].as_str().unwrap())))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let shares = signed(&t, &cookie, "GET", "/api/shares", None).await;
    assert_eq!(shares.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn shared_cookbook_pages_escape_the_book_name() {
    let t = TestApp::new(None);
    let (_, r) = t
        .json(
            "POST",
            "/api/recipes",
            Some(book_recipe("<b>Bold</b> Stew", None, "")),
        )
        .await;
    let (_, book) = t
        .json(
            "POST",
            "/api/cookbooks",
            Some(json!({"name": "<img src=x onerror=alert(1)> & \"friends\""})),
        )
        .await;
    t.json(
        "POST",
        &format!("/api/cookbooks/{}/recipes", book["id"]),
        Some(json!({"recipeIds": [r["id"]]})),
    )
    .await;
    let (_, s) = t
        .json(
            "POST",
            &format!("/api/cookbooks/{}/share", book["id"]),
            None,
        )
        .await;
    let path = book_path(s["url"].as_str().unwrap());
    for p in [path.clone(), format!("{path}/{}", r["id"])] {
        let (status, _, html) = t.send(get(&p)).await;
        assert_eq!(status, StatusCode::OK);
        assert!(!html.contains("<img src=x"), "{p}: {html}");
        assert!(!html.contains("<b>Bold"), "{p}");
        assert!(html.contains("&lt;img src=x onerror=alert(1)&gt; &amp; &quot;friends&quot;"));
    }
}

/// Serves a test app on a real local port; returns its origin.
async fn serve_app(t: &TestApp) -> String {
    let svc = NormalizePathLayer::trim_trailing_slash().layer(app(t.state.clone()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(
            listener,
            axum::ServiceExt::<Request<Body>>::into_make_service(svc),
        )
        .await
        .unwrap()
    });
    origin
}

async fn import_file_json(t: &TestApp, body: &str) -> Value {
    let boundary = "XBOUNDARY";
    let multipart = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"book.json\"\r\n\
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
    serde_json::from_str::<Value>(&text).unwrap()[0].clone()
}

#[tokio::test]
async fn saving_a_shared_cookbook_recreates_it_in_another_crumb() {
    // Crumb A shares a book of two recipes (one with an original link), on a real port
    let a = TestApp::new(None);
    let (_, soup) = a
        .json(
            "POST",
            "/api/recipes",
            Some(book_recipe(
                "Leek Soup",
                Some("https://food.test/leek"),
                "Salt late.",
            )),
        )
        .await;
    let (_, tart) = a
        .json(
            "POST",
            "/api/recipes",
            Some(book_recipe("Onion Tart", None, "")),
        )
        .await;
    let (_, book) = a
        .json(
            "POST",
            "/api/cookbooks",
            Some(json!({"name": "Weeknight dinners", "color": "sage"})),
        )
        .await;
    a.json(
        "POST",
        &format!("/api/cookbooks/{}/recipes", book["id"]),
        Some(json!({"recipeIds": [soup["id"], tart["id"]]})),
    )
    .await;
    let (_, s) = a
        .json(
            "POST",
            &format!("/api/cookbooks/{}/share", book["id"]),
            None,
        )
        .await;
    let origin = serve_app(&a).await;
    let link = format!("{origin}{}", book_path(s["url"].as_str().unwrap()));

    // Crumb B saves the link: every recipe, into a cookbook of that name
    let b = TestApp::new(None);
    let (status, saved) = b
        .json("POST", "/api/recipes/import", Some(json!({"url": link})))
        .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["cookbook"]["name"], "Weeknight dinners");
    assert_eq!(saved["cookbook"]["added"], 2);
    assert_eq!(saved["cookbook"]["duplicates"], 0);
    assert_eq!(saved["isNew"], true);
    let book_id = saved["cookbook"]["id"].clone();
    let (_, got) = b
        .json("GET", &format!("/api/cookbooks/{book_id}"), None)
        .await;
    assert_eq!(got["color"], "sage");
    assert_eq!(got["recipes"].as_array().unwrap().len(), 2);
    let leek = got["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["title"] == "Leek Soup")
        .unwrap();
    let (_, leek) = b
        .json("GET", &format!("/api/recipes/{}", leek["id"]), None)
        .await;
    assert_eq!(leek["url"], format!("{link}/{}", soup["id"]));
    assert_eq!(leek["originalUrl"], "https://food.test/leek");
    assert_eq!(leek["notes"], "Salt late.");

    // Again: nothing new, same book
    let (_, again) = b
        .json("POST", "/api/recipes/import", Some(json!({"url": link})))
        .await;
    assert_eq!(again["cookbook"]["added"], 0, "{again}");
    assert_eq!(again["cookbook"]["duplicates"], 2);
    assert_eq!(again["cookbook"]["id"], book_id);
    // One recipe's own link from inside the book is the same recipe
    let (_, one) = b
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"url": format!("{link}/{}", tart["id"])})),
        )
        .await;
    assert_eq!(one["isNew"], false, "{one}");
    let (_, books) = b.json("GET", "/api/cookbooks", None).await;
    assert_eq!(books.as_array().unwrap().len(), 1);
    let (_, list) = b.json("GET", "/api/recipes", None).await;
    assert_eq!(list.as_array().unwrap().len(), 2);

    // The share's file, imported twice in Crumb C: the book once, no duplicates
    let (_, _, file) = a
        .send(get(&format!(
            "{}/crumb.json",
            book_path(s["url"].as_str().unwrap())
        )))
        .await;
    let c = TestApp::new(None);
    let first = import_file_json(&c, &file).await;
    assert_eq!(first["created"].as_array().unwrap().len(), 2, "{first}");
    let second = import_file_json(&c, &file).await;
    assert_eq!(second["created"].as_array().unwrap().len(), 0);
    assert_eq!(second["duplicates"], 2);
    let (_, books) = c.json("GET", "/api/cookbooks", None).await;
    assert_eq!(books.as_array().unwrap().len(), 1);
    assert_eq!(books[0]["name"], "Weeknight dinners");
    assert_eq!(books[0]["recipeCount"], 2);

    // The owner's download reads back into a cookbook too
    let (_, _, owner) = a
        .send(get(&format!("/api/cookbooks/{}/export", book["id"])))
        .await;
    let d = TestApp::new(None);
    let got = import_file_json(&d, &owner).await;
    assert_eq!(got["created"].as_array().unwrap().len(), 2);
    let (_, books) = d.json("GET", "/api/cookbooks", None).await;
    assert_eq!(books[0]["name"], "Weeknight dinners");
    assert_eq!(books[0]["recipeCount"], 2);
}

#[tokio::test]
async fn a_shared_cookbook_export_cant_file_recipes_elsewhere() {
    use axum::response::Html;
    use axum::routing::get as route;
    let export = json!({"format": "crumb", "version": 1, "kind": "cookbook",
    "cookbooks": [{"name": "Stolen", "color": "nope"}],
    "recipes": [
        {"title": "Claimer", "url": "https://popular.test/x", "shareUrl": "https://popular.test/x",
         "ingredients": [{"items": ["1 egg"]}], "instructions": [{"items": ["Cook."]}]},
        {"title": "Other Share", "shareUrl": "http://127.0.0.1/s/otherTokenAAAAAAAAAA/5",
         "ingredients": [{"items": ["1 egg"]}], "instructions": [{"items": ["Cook."]}]},
        {"title": "Fine", "shareUrl": "http://elsewhere.test/s/bookTokenAAAAAAAAAAA/7",
         "cookedAt": ["2024-01-01T00:00:00Z"],
         "ingredients": [{"items": ["1 egg"]}], "instructions": [{"items": ["Cook."]}]}
    ]})
    .to_string();
    let page = r#"<html><head><title>Stolen</title>
        <link rel="alternate" type="application/vnd.crumb+json" href="/s/bookTokenAAAAAAAAAAA/crumb.json">
        </head><body></body></html>"#;
    let origin = serve(
        axum::Router::new()
            .route(
                "/s/bookTokenAAAAAAAAAAA",
                route(move || async move { Html(page) }),
            )
            .route(
                "/s/bookTokenAAAAAAAAAAA/crumb.json",
                route(move || async move { export }),
            ),
    )
    .await;
    let b = TestApp::new(None);
    let link = format!("{origin}/s/bookTokenAAAAAAAAAAA");
    let (status, saved) = b
        .json("POST", "/api/recipes/import", Some(json!({"url": link})))
        .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["cookbook"]["added"], 1, "{saved}");
    let (_, list) = b.json("GET", "/api/recipes", None).await;
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 1);
    let (_, fine) = b
        .json("GET", &format!("/api/recipes/{}", list[0]["id"]), None)
        .await;
    assert_eq!(fine["title"], "Fine");
    // Filed under the link as pasted, whatever host the export named
    assert_eq!(fine["url"], format!("{link}/7"));
    let (_, stats) = b
        .json("GET", &format!("/api/recipes/{}", list[0]["id"]), None)
        .await;
    assert!(stats.get("cookedAt").is_none());
    let (_, books) = b.json("GET", "/api/cookbooks", None).await;
    assert_eq!(books[0]["name"], "Stolen");
    assert!(
        crumb::model::BOOK_COLORS.contains(&books[0]["color"].as_str().unwrap()),
        "an unknown colour isn't kept"
    );
    // The claimed URL is still free: nothing was saved under it
    let (_, found) = b
        .json(
            "POST",
            "/api/recipes",
            Some(json!({"title": "Real", "url": "https://popular.test/x",
                "ingredients": [{"items": ["a"]}], "instructions": [{"items": ["b"]}]})),
        )
        .await;
    assert_eq!(found["isNew"], true, "{found}");
}

#[tokio::test]
async fn shared_cookbook_photos_come_from_the_books_recipes() {
    use base64::Engine;
    let cache = tempfile::tempdir().unwrap();
    let dir = cache.path().join("img-cache");
    let t = TestApp::with_config(|c| c.image_cache = Some(dir.clone()));
    let image = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png_bytes(1600, 1000))
    );
    let mut ids = Vec::new();
    for title in ["A Plain", "B Photo", "C Outside"] {
        let (_, r) = t
            .json("POST", "/api/recipes", Some(book_recipe(title, None, "")))
            .await;
        ids.push(r["id"].as_i64().unwrap());
    }
    set_image(&t, ids[1], &image);
    set_image(&t, ids[2], &image);
    let key = crumb::images::image_key(&image);
    let (_, book) = t
        .json("POST", "/api/cookbooks", Some(json!({"name": "Photos"})))
        .await;
    t.json(
        "POST",
        &format!("/api/cookbooks/{}/recipes", book["id"]),
        Some(json!({"recipeIds": [ids[0], ids[1]]})),
    )
    .await;
    let (_, s) = t
        .json(
            "POST",
            &format!("/api/cookbooks/{}/share", book["id"]),
            None,
        )
        .await;
    let url = s["url"].as_str().unwrap();
    let path = book_path(url);
    let (_, _, html) = t.send(get(&path)).await;
    // The cover is the first recipe with a photo; cards use the book's own photo URLs
    assert!(html.contains(&format!(
        r#"<meta property="og:image" content="{url}/og.jpg?v={key}">"#
    )));
    assert!(html.contains(&format!(r#"src="{path}/{}/img/320?v={key}""#, ids[1])));
    assert!(html.contains("photo-empty"), "a card without a photo");
    let (status, headers, _) = send_raw(&t, get(&format!("{path}/og.jpg?v={key}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/jpeg");
    let (status, headers, _) =
        send_raw(&t, get(&format!("{path}/{}/img/320?v={key}", ids[1]))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/webp");
    // Not in the book: no photo either
    let (status, _, _) = send_raw(&t, get(&format!("{path}/{}/img/320?v={key}", ids[2]))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = send_raw(&t, get(&format!("{path}/{}/img/321", ids[1]))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// The token in a share link.
fn token_of(url: &str) -> String {
    url.split("/s/")
        .nth(1)
        .unwrap()
        .split('/')
        .next()
        .unwrap()
        .to_string()
}

/// Crumb C shares a book; Crumb B saves it.
async fn saved_from_a_shared_book(
    b: &TestApp,
    name: &str,
    recipes: &[Value],
) -> (TestApp, String, Value) {
    let c = TestApp::new(None);
    let (_, book) = c
        .json("POST", "/api/cookbooks", Some(json!({"name": name})))
        .await;
    for r in recipes {
        let (_, r) = c.json("POST", "/api/recipes", Some(r.clone())).await;
        c.json(
            "POST",
            &format!("/api/cookbooks/{}/recipes", book["id"]),
            Some(json!({"recipeIds": [r["id"]]})),
        )
        .await;
    }
    let (_, s) = c
        .json(
            "POST",
            &format!("/api/cookbooks/{}/share", book["id"]),
            None,
        )
        .await;
    let origin = serve_app(&c).await;
    let link = format!("{origin}{}", book_path(s["url"].as_str().unwrap()));
    let (status, saved) = b
        .json("POST", "/api/recipes/import", Some(json!({"url": link})))
        .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    (c, link, saved)
}

#[tokio::test]
async fn sharing_a_recipe_saved_from_a_share_never_passes_on_that_link() {
    let b = TestApp::new(None);
    let (_c, link, saved) =
        saved_from_a_shared_book(&b, "Stews", &[book_recipe("Plain Stew", None, "")]).await;
    let token_c = token_of(&link);
    assert_eq!(saved["cookbook"]["added"], 1, "{saved}");
    let stew_id = saved["id"].clone();
    let (_, stew) = b
        .json("GET", &format!("/api/recipes/{stew_id}"), None)
        .await;
    // Kept under C's link (so saving it again dedupes), with no source of its own
    assert!(stew["url"].as_str().unwrap().contains(&token_c));
    assert!(stew["originalUrl"].is_null());

    // B shares the recipe on its own, and in a book of B's
    let (_, rs) = b
        .json("POST", &format!("/api/recipes/{stew_id}/share"), None)
        .await;
    let recipe_path = book_path(rs["url"].as_str().unwrap());
    let (_, mine) = b
        .json("POST", "/api/cookbooks", Some(json!({"name": "Mine"})))
        .await;
    b.json(
        "POST",
        &format!("/api/cookbooks/{}/recipes", mine["id"]),
        Some(json!({"recipeIds": [stew_id]})),
    )
    .await;
    let (_, bs) = b
        .json(
            "POST",
            &format!("/api/cookbooks/{}/share", mine["id"]),
            None,
        )
        .await;
    let book = book_path(bs["url"].as_str().unwrap());
    let mut seen = Vec::new();
    for path in [
        recipe_path.clone(),
        format!("{recipe_path}/crumb.json"),
        book.clone(),
        format!("{book}/crumb.json"),
        format!("{book}/{stew_id}"),
        format!("{book}/{stew_id}/crumb.json"),
        // The owner's files, which may be handed on
        format!("/api/cookbooks/{}/export", mine["id"]),
        format!("/api/recipes/{stew_id}/export"),
        format!("/api/recipes/{stew_id}/export?format=md"),
    ] {
        let (status, _, body) = b.send(get(&path)).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(!body.contains(&token_c), "{path} holds C's token: {body}");
        seen.push(body);
    }
    // No "Original recipe" link, no url in the JSON-LD or the export
    assert!(!seen[0].contains("Original recipe"));
    assert!(!seen[0].contains("isBasedOn"));
    assert!(seen[0].contains("og:url"));
    let doc: Value = serde_json::from_str(&seen[1]).unwrap();
    assert!(doc["recipes"][0]["url"].is_null(), "{doc}");
    let doc: Value = serde_json::from_str(&seen[3]).unwrap();
    assert!(doc["recipes"][0]["url"].is_null(), "{doc}");
    assert!(
        doc["recipes"][0]["shareUrl"]
            .as_str()
            .unwrap()
            .ends_with(&format!("{book}/{stew_id}"))
    );

    // A full backup still restores losslessly (the link is how the recipe dedupes)
    let (_, _, backup) = b.send(get("/api/export")).await;
    assert!(backup.contains(&token_c));
}

#[tokio::test]
async fn a_share_link_is_never_taken_as_where_a_recipe_came_from() {
    use axum::response::Html;
    use axum::routing::get as route;
    let export = json!({"format": "crumb", "version": 1, "kind": "cookbook",
    "cookbooks": [{"name": "Relayed"}],
    "recipes": [
        {"title": "Relayed Pie", "url": "https://third.test/s/someoneElsesTokenXYZ/4",
         "shareUrl": "http://elsewhere.test/s/bookTokenBBBBBBBBBBB/7",
         "ingredients": [{"items": ["1 egg"]}], "instructions": [{"items": ["Cook."]}]},
        {"title": "Real Pie", "url": "https://food.test/pie",
         "shareUrl": "http://elsewhere.test/s/bookTokenBBBBBBBBBBB/8",
         "ingredients": [{"items": ["1 egg"]}], "instructions": [{"items": ["Cook."]}]}
    ]})
    .to_string();
    let page = r#"<html><head><title>Relayed</title>
        <link rel="alternate" type="application/vnd.crumb+json" href="/s/bookTokenBBBBBBBBBBB/crumb.json">
        </head><body></body></html>"#;
    let origin = serve(
        axum::Router::new()
            .route(
                "/s/bookTokenBBBBBBBBBBB",
                route(move || async move { Html(page) }),
            )
            .route(
                "/s/bookTokenBBBBBBBBBBB/crumb.json",
                route(move || async move { export }),
            ),
    )
    .await;
    let b = TestApp::new(None);
    let (_, saved) = b
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"url": format!("{origin}/s/bookTokenBBBBBBBBBBB")})),
        )
        .await;
    assert_eq!(saved["cookbook"]["added"], 2, "{saved}");
    let (_, list) = b.json("GET", "/api/recipes", None).await;
    for r in list.as_array().unwrap() {
        let (_, full) = b
            .json("GET", &format!("/api/recipes/{}", r["id"]), None)
            .await;
        match full["title"].as_str().unwrap() {
            "Relayed Pie" => assert!(full["originalUrl"].is_null(), "{full}"),
            _ => assert_eq!(full["originalUrl"], "https://food.test/pie"),
        }
    }
    // Nor from a file's originalUrl
    let file = json!({"format": "crumb", "version": 1, "recipes": [{
        "title": "Filed Pie", "url": "https://mine.test/filed",
        "originalUrl": "https://third.test/s/someoneElsesTokenXYZ",
        "ingredients": [{"items": ["1 egg"]}], "instructions": [{"items": ["Cook."]}]}]})
    .to_string();
    let got = import_file_json(&b, &file).await;
    let (_, filed) = b
        .json(
            "GET",
            &format!("/api/recipes/{}", got["created"][0]["id"]),
            None,
        )
        .await;
    assert!(filed["originalUrl"].is_null(), "{filed}");
}

#[tokio::test]
async fn a_saved_book_only_joins_a_cookbook_it_made_or_an_empty_one() {
    // B already has a cookbook of that name with a recipe in it
    let b = TestApp::new(None);
    let (_, own) = b
        .json(
            "POST",
            "/api/recipes",
            Some(book_recipe("Own Stew", Some("https://food.test/own"), "")),
        )
        .await;
    let (_, mine) = b
        .json("POST", "/api/cookbooks", Some(json!({"name": "Weeknight"})))
        .await;
    b.json(
        "POST",
        &format!("/api/cookbooks/{}/recipes", mine["id"]),
        Some(json!({"recipeIds": [own["id"]]})),
    )
    .await;
    let (_c, link, saved) =
        saved_from_a_shared_book(&b, "weeknight", &[book_recipe("Their Stew", None, "")]).await;
    assert_eq!(saved["cookbook"]["name"], "weeknight (2)", "{saved}");
    assert_ne!(saved["cookbook"]["id"], mine["id"]);
    let (_, got) = b
        .json("GET", &format!("/api/cookbooks/{}", mine["id"]), None)
        .await;
    assert_eq!(got["recipes"].as_array().unwrap().len(), 1);
    // Saving the same link again goes back into the book it made
    let (_, again) = b
        .json("POST", "/api/recipes/import", Some(json!({"url": link})))
        .await;
    assert_eq!(again["cookbook"]["id"], saved["cookbook"]["id"], "{again}");
    assert_eq!(again["cookbook"]["duplicates"], 1);
    // A second book of that name, from another link: "(3)"
    let (_c2, _, third) =
        saved_from_a_shared_book(&b, "Weeknight", &[book_recipe("Other Stew", None, "")]).await;
    assert_eq!(third["cookbook"]["name"], "Weeknight (3)", "{third}");

    // An empty cookbook of that name is used
    let d = TestApp::new(None);
    let (_, empty) = d
        .json("POST", "/api/cookbooks", Some(json!({"name": "Soups"})))
        .await;
    let (_c3, _, saved) =
        saved_from_a_shared_book(&d, "Soups", &[book_recipe("Leek Soup", None, "")]).await;
    assert_eq!(saved["cookbook"]["id"], empty["id"], "{saved}");
    let (_, books) = d.json("GET", "/api/cookbooks", None).await;
    assert_eq!(books.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn a_recipe_that_cant_be_saved_is_rolled_back_and_counted() {
    // Anything filing a recipe titled "Bad" into a cookbook fails
    let fail_bad = "CREATE TRIGGER fail_bad BEFORE INSERT ON cookbook_recipes
        WHEN (SELECT title FROM recipes WHERE id = NEW.recipe_id) = 'Bad'
        BEGIN SELECT RAISE(ABORT, 'nope'); END;";
    let t = TestApp::new(None);
    t.state.db.lock().execute_batch(fail_bad).unwrap();
    let recipe = |title: &str| {
        json!({"title": title, "url": format!("https://food.test/{title}"), "cookbooks": ["Box"],
            "ingredients": [{"items": ["1 egg"]}], "instructions": [{"items": ["Cook."]}]})
    };
    let file = json!({"format": "crumb", "version": 1,
        "recipes": [recipe("Good"), recipe("Bad"), recipe("Fine")]})
    .to_string();
    let got = import_file_json(&t, &file).await;
    assert_eq!(got["created"].as_array().unwrap().len(), 2, "{got}");
    assert_eq!(got["skipped"], 1);
    let (_, list) = t.json("GET", "/api/recipes", None).await;
    let titles: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["title"].as_str().unwrap())
        .collect();
    assert!(!titles.contains(&"Bad"), "half-saved: {titles:?}");
    assert_eq!(titles.len(), 2);

    // Saving a shared book: the same, and the result says so
    let b = TestApp::new(None);
    b.state.db.lock().execute_batch(fail_bad).unwrap();
    let (_c, _, saved) = saved_from_a_shared_book(
        &b,
        "Mixed",
        &[book_recipe("Good", None, ""), book_recipe("Bad", None, "")],
    )
    .await;
    assert_eq!(saved["cookbook"]["added"], 1, "{saved}");
    assert_eq!(saved["cookbook"]["skipped"], 1);
    let (_, list) = b.json("GET", "/api/recipes", None).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn a_big_shared_book_says_it_shows_the_first_500() {
    let t = TestApp::new(None);
    let (_, book) = t
        .json("POST", "/api/cookbooks", Some(json!({"name": "Huge"})))
        .await;
    let book_id = book["id"].as_i64().unwrap();
    {
        let conn = t.state.db.lock();
        for i in 0..501 {
            let (r, _) = crumb::recipes::create_recipe(
                &conn,
                crumb::model::RecipeFields {
                    title: format!("Dish {i:03}"),
                    ..older_recipe()
                },
                "manual",
            )
            .unwrap();
            crumb::recipes::add_to_cookbook(&conn, book_id, &[r.id]).unwrap();
        }
    }
    // The owner's own download has every recipe
    let (_, _, owner) = t
        .send(get(&format!("/api/cookbooks/{book_id}/export")))
        .await;
    let owner: Value = serde_json::from_str(&owner).unwrap();
    assert_eq!(owner["recipes"].as_array().unwrap().len(), 501);
    // The share shows 500, and says so
    let (_, s) = t
        .json("POST", &format!("/api/cookbooks/{book_id}/share"), None)
        .await;
    let path = book_path(s["url"].as_str().unwrap());
    let (_, _, html) = t.send(get(&path)).await;
    assert!(html.contains("501 recipes"));
    assert!(html.contains("Showing the first 500 recipes"));
    assert_eq!(html.matches("class=\"share-card\"").count(), 500);
    let (_, _, doc) = t.send(get(&format!("{path}/crumb.json"))).await;
    let doc: Value = serde_json::from_str(&doc).unwrap();
    assert_eq!(doc["recipes"].as_array().unwrap().len(), 500);
    assert_eq!(doc["cookbooks"][0]["recipeCount"], 501);
    assert_eq!(doc["note"], "Showing the first 500 recipes");
}

#[tokio::test]
async fn photos_kept_in_a_recipe_survive_shares_backups_and_json_files() {
    use base64::Engine;
    let photo = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png_bytes(64, 40))
    );
    let a = TestApp::new(None);
    let (_, r) = a
        .json(
            "POST",
            "/api/recipes",
            Some(book_recipe("Photo Pie", None, "")),
        )
        .await;
    let id = r["id"].as_i64().unwrap();
    set_image(&a, id, &photo);
    let image_of = |t: &TestApp, id: &Value| -> String {
        t.state
            .db
            .lock()
            .query_row(
                "SELECT image FROM recipes WHERE id = ?1",
                [id.as_i64().unwrap()],
                |r| r.get(0),
            )
            .unwrap()
    };

    // A backup and the recipe's own .json read back with the photo, as it was
    for uri in [
        "/api/export",
        &format!("/api/recipes/{id}/export?format=json"),
    ] {
        let (_, _, file) = a.send(get(uri)).await;
        let c = TestApp::new(None);
        let got = import_file_json(&c, &file).await;
        assert_eq!(got["created"].as_array().unwrap().len(), 1, "{uri}: {got}");
        assert_eq!(image_of(&c, &got["created"][0]["id"]), photo, "{uri}");
    }

    // A share's export links to the share's copy of the photo instead of carrying it
    let (_, s) = a
        .json("POST", &format!("/api/recipes/{id}/share"), None)
        .await;
    let (_, _, export) = a
        .send(get(&format!(
            "{}/crumb.json",
            share_path(s["url"].as_str().unwrap())
        )))
        .await;
    let export: Value = serde_json::from_str(&export).unwrap();
    let linked = export["recipes"][0]["image"].as_str().unwrap();
    assert!(
        linked.contains("/img/1200?v=") && !linked.contains("data:"),
        "{linked}"
    );

    // Another Crumb saving the share keeps the photo itself (the share can be stopped)
    let origin = serve_app(&a).await;
    let link = format!("{origin}{}", share_path(s["url"].as_str().unwrap()));
    let b = TestApp::new(None);
    let (status, saved) = b
        .json("POST", "/api/recipes/import", Some(json!({"url": link})))
        .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let kept = image_of(&b, &saved["id"]);
    assert!(kept.starts_with("data:image/webp;base64,"), "{kept}");
    let (status, _, _) = send_raw(
        &b,
        get(&format!("/img/{}/320", saved["id"].as_i64().unwrap())),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // So does saving a shared cookbook holding it
    let (_, book) = a
        .json("POST", "/api/cookbooks", Some(json!({"name": "Pies"})))
        .await;
    a.json(
        "POST",
        &format!("/api/cookbooks/{}/recipes", book["id"]),
        Some(json!({"recipeIds": [id]})),
    )
    .await;
    let (_, bs) = a
        .json(
            "POST",
            &format!("/api/cookbooks/{}/share", book["id"]),
            None,
        )
        .await;
    let book_link = format!("{origin}{}", book_path(bs["url"].as_str().unwrap()));
    let e = TestApp::new(None);
    let (status, saved) = e
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"url": book_link})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (_, got) = e
        .json(
            "GET",
            &format!("/api/cookbooks/{}", saved["cookbook"]["id"]),
            None,
        )
        .await;
    let kept = image_of(&e, &got["recipes"][0]["id"]);
    assert!(kept.starts_with("data:image/webp;base64,"), "{kept}");

    // Sharing it on from there links to that box's own share, never to the first one
    let (_, s2) = b
        .json(
            "POST",
            &format!("/api/recipes/{}/share", saved_id(&b)),
            None,
        )
        .await;
    let (_, _, export) = b
        .send(get(&format!(
            "{}/crumb.json",
            share_path(s2["url"].as_str().unwrap())
        )))
        .await;
    let export: Value = serde_json::from_str(&export).unwrap();
    let linked = export["recipes"][0]["image"].as_str().unwrap();
    assert!(!linked.contains(&token_of(&link)), "{linked}");
    assert!(
        linked.contains(&token_of(s2["url"].as_str().unwrap())),
        "{linked}"
    );
}

/// The id of the only recipe in a box.
fn saved_id(t: &TestApp) -> i64 {
    t.state
        .db
        .lock()
        .query_row("SELECT id FROM recipes", [], |r| r.get(0))
        .unwrap()
}

#[tokio::test]
async fn a_share_photo_that_cant_be_had_is_dropped_not_kept_as_a_link() {
    use axum::response::Html;
    use axum::routing::get as route;
    let origin_slot = std::sync::Arc::new(std::sync::OnceLock::<String>::new());
    let o = origin_slot.clone();
    let token = "abcdefghijklmnopqrstuv";
    let routes = axum::Router::new()
        .route(
            &format!("/s/{token}"),
            route(|| async { Html(fake_share_page("/s/abcdefghijklmnopqrstuv/crumb.json")) }),
        )
        .route(
            &format!("/s/{token}/crumb.json"),
            route(move || {
                let o = o.clone();
                async move {
                    let origin = o.get().unwrap();
                    fake_export(
                        "Gone Pie",
                        "https://food.test/pie",
                        &format!("{origin}/s/{token}/img/1200?v=1"),
                    )
                }
            }),
        );
    let origin = serve(routes).await;
    origin_slot.set(origin.clone()).unwrap();
    let b = TestApp::new(None);
    let (status, saved) = b
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"url": format!("{origin}/s/{token}")})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (_, got) = b
        .json("GET", &format!("/api/recipes/{}", saved["id"]), None)
        .await;
    assert_eq!(got["image"], Value::Null, "{got}");
}

/// Requests as household `id` would make them once signed in: the router's state scoped
/// to that household, as the login check does.
async fn as_household(t: &TestApp, id: i64, req: Request<Body>) -> (StatusCode, Value) {
    let scoped = crumb::Scoped(t.state.for_household(id).unwrap());
    let svc = NormalizePathLayer::trim_trailing_slash()
        .layer(app(t.state.clone()).layer(axum::Extension(scoped)));
    let res = svc.oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn json_req(method: &str, uri: &str, body: Option<Value>) -> Request<Body> {
    let mut req = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(b) => {
            req = req.header(header::CONTENT_TYPE, "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    req.body(body).unwrap()
}

#[tokio::test]
async fn households_each_have_their_own_box() {
    let t = TestApp::new(None);
    let (_, home) = t
        .json(
            "POST",
            "/api/recipes",
            Some(book_recipe("Home Soup", None, "")),
        )
        .await;
    let (status, theirs) = as_household(
        &t,
        2,
        json_req(
            "POST",
            "/api/recipes",
            Some(book_recipe("Their Pie", None, "")),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{theirs}");
    // Ids are per box, so both are recipe 1: each household sees only its own
    assert_eq!(home["id"], theirs["id"]);
    let (_, list) = t.json("GET", "/api/recipes", None).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["title"], "Home Soup");
    let (_, list) = as_household(&t, 2, json_req("GET", "/api/recipes", None)).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["title"], "Their Pie");
    let (_, one) = as_household(&t, 2, json_req("GET", "/api/recipes/1", None)).await;
    assert_eq!(one["title"], "Their Pie");
    // Cookbooks, the backup and deletes stay in their box too
    as_household(
        &t,
        2,
        json_req("POST", "/api/cookbooks", Some(json!({"name": "Theirs"}))),
    )
    .await;
    let (_, books) = t.json("GET", "/api/cookbooks", None).await;
    assert_eq!(books.as_array().unwrap().len(), 0);
    let (_, backup) = as_household(&t, 2, json_req("GET", "/api/export", None)).await;
    assert_eq!(backup["recipes"].as_array().unwrap().len(), 1);
    assert_eq!(backup["recipes"][0]["title"], "Their Pie");
    let (status, _) = as_household(&t, 2, json_req("DELETE", "/api/recipes/1", None)).await;
    assert!(status.is_success());
    let (_, got) = t.json("GET", "/api/recipes/1", None).await;
    assert_eq!(
        got["title"], "Home Soup",
        "the home box's recipe 1 is untouched"
    );
}

fn accounts_app(app_password: Option<&str>, open_signup: bool) -> TestApp {
    TestApp::with_config(|c| {
        c.auth_mode = crumb::config::AuthMode::Accounts;
        c.open_signup = open_signup;
        c.app_password = app_password.map(String::from);
    })
}

/// A JSON POST that may sign in: (status, the session cookie it set, body).
async fn auth_post(t: &TestApp, uri: &str, body: Value) -> (StatusCode, Option<String>, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let (status, headers, text) = t.send(req).await;
    let cookie = headers
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(|c| c.split(';').next().unwrap().to_string())
        .filter(|c| !c.ends_with('='));
    (
        status,
        cookie,
        serde_json::from_str(&text).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn accounts_set_up_the_existing_box_and_sign_in() {
    let t = accounts_app(Some("old-shared-pw"), false);
    // A recipe from before accounts, in the one box
    t.state
        .db
        .lock()
        .execute(
            "INSERT INTO recipes (title, ingredients, instructions, created_at, updated_at)
             VALUES ('Old Faithful', '[]', '[]', 1, 1)",
            [],
        )
        .unwrap();
    let (status, _) = t.json("GET", "/api/recipes", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, headers, _) = t.send(get("/recipes")).await;
    assert_eq!(status, StatusCode::FOUND);
    assert!(
        headers[header::LOCATION]
            .to_str()
            .unwrap()
            .starts_with("/setup?next=")
    );
    let (_, s) = t.json("GET", "/api/auth/status", None).await;
    assert_eq!(s["mode"], "accounts");
    assert_eq!(s["setupNeeded"], true);
    assert_eq!(s["setupNeedsAppPassword"], true);

    // The old app password is needed to claim the box
    let owner =
        json!({"email": "ann@example.com", "name": "Ann Cook", "password": "correct horse"});
    let (status, cookie, _) = auth_post(&t, "/api/auth/setup", owner.clone()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(cookie.is_none());
    let mut with_pw = owner.clone();
    with_pw["appPassword"] = json!("old-shared-pw");
    let (status, cookie, body) = auth_post(&t, "/api/auth/setup", with_pw.clone()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let cookie = cookie.unwrap();
    // Only once
    let (status, _, _) = auth_post(&t, "/api/auth/setup", with_pw).await;
    assert_eq!(status, StatusCode::CONFLICT);

    // The owner's box is the one that was here
    let (_, list) = call(&t, "GET", "/api/recipes", None, Some(&cookie)).await;
    assert_eq!(list[0]["title"], "Old Faithful");
    let (_, s) = call(&t, "GET", "/api/auth/status", None, Some(&cookie)).await;
    assert_eq!(s["user"]["name"], "Ann Cook");
    assert_eq!(s["household"]["id"], 1);
    assert_eq!(s["household"]["role"], "owner");

    // Signing in: email and password, not the old shared password
    let (status, _, _) =
        auth_post(&t, "/api/auth/login", json!({"password": "old-shared-pw"})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, cookie2, _) = auth_post(
        &t,
        "/api/auth/login",
        json!({"email": "ANN@example.com", "password": "wrong horse"}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(cookie2.is_none());
    let (status, cookie2, _) = auth_post(
        &t,
        "/api/auth/login",
        json!({"email": "ANN@example.com", "password": "correct horse"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let cookie2 = cookie2.unwrap();

    // Two sessions; signing the others out ends the first
    let (_, sessions) = call(&t, "GET", "/api/auth/sessions", None, Some(&cookie2)).await;
    assert_eq!(sessions.as_array().unwrap().len(), 2);
    let (_, done) = call(
        &t,
        "POST",
        "/api/auth/sessions/revoke-others",
        None,
        Some(&cookie2),
    )
    .await;
    assert_eq!(done["ended"], 1);
    let (status, _) = call(&t, "GET", "/api/recipes", None, Some(&cookie)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // Signing out ends the session itself, not just the cookie
    call(&t, "POST", "/api/auth/logout", None, Some(&cookie2)).await;
    let (status, _) = call(&t, "GET", "/api/recipes", None, Some(&cookie2)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // A made-up or old-style cookie gets nowhere
    let (status, _) = call(&t, "GET", "/api/recipes", None, Some("crumb_session=nope")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn accounts_sign_up_gets_a_box_of_its_own() {
    let closed = accounts_app(None, false);
    let (_, owner, _) = auth_post(
        &closed,
        "/api/auth/setup",
        json!({"email": "ann@example.com", "name": "Ann", "password": "correct horse"}),
    )
    .await;
    assert!(owner.is_some());
    let (status, _, _) = auth_post(
        &closed,
        "/api/auth/signup",
        json!({"email": "bob@example.com", "name": "Bob", "password": "another pass"}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "sign-up is off by default");

    let t = accounts_app(None, true);
    let (_, ann, _) = auth_post(
        &t,
        "/api/auth/setup",
        json!({"email": "ann@example.com", "name": "Ann", "password": "correct horse"}),
    )
    .await;
    let ann = ann.unwrap();
    call(
        &t,
        "POST",
        "/api/recipes",
        Some(book_recipe("Ann's Soup", None, "")),
        Some(&ann),
    )
    .await;
    let (status, bob, body) = auth_post(
        &t,
        "/api/auth/signup",
        json!({"email": "bob@example.com", "name": "Bob", "password": "another pass"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let bob = bob.unwrap();
    let (_, list) = call(&t, "GET", "/api/recipes", None, Some(&bob)).await;
    assert_eq!(list.as_array().unwrap().len(), 0, "Bob's box starts empty");
    let (_, made) = call(
        &t,
        "POST",
        "/api/recipes",
        Some(book_recipe("Bob's Pie", None, "")),
        Some(&bob),
    )
    .await;
    // Same id in two boxes, each seeing its own
    let (_, mine) = call(
        &t,
        "GET",
        &format!("/api/recipes/{}", made["id"]),
        None,
        Some(&ann),
    )
    .await;
    assert_eq!(mine["title"], "Ann's Soup");
    let (_, theirs) = call(
        &t,
        "GET",
        &format!("/api/recipes/{}", made["id"]),
        None,
        Some(&bob),
    )
    .await;
    assert_eq!(theirs["title"], "Bob's Pie");
    let (_, s) = call(&t, "GET", "/api/auth/status", None, Some(&bob)).await;
    assert_eq!(s["household"]["name"], "Bob's kitchen");
    // Bob can't end Ann's sessions
    let (_, annsessions) = call(&t, "GET", "/api/auth/sessions", None, Some(&ann)).await;
    let id = annsessions[0]["id"].as_i64().unwrap();
    let (status, _) = call(
        &t,
        "DELETE",
        &format!("/api/auth/sessions/{id}"),
        None,
        Some(&bob),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&t, "GET", "/api/recipes", None, Some(&ann)).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn accounts_connect_claude_to_the_signed_in_household() {
    let t = accounts_app(None, true);
    let (_, ann, _) = auth_post(
        &t,
        "/api/auth/setup",
        json!({"email": "ann@example.com", "name": "Ann", "password": "correct horse"}),
    )
    .await;
    let ann = ann.unwrap();
    call(
        &t,
        "POST",
        "/api/recipes",
        Some(book_recipe("Ann's Soup", None, "")),
        Some(&ann),
    )
    .await;
    let (_, bob, _) = auth_post(
        &t,
        "/api/auth/signup",
        json!({"email": "bob@example.com", "name": "Bob", "password": "another pass"}),
    )
    .await;
    let bob = bob.unwrap();
    call(
        &t,
        "POST",
        "/api/recipes",
        Some(book_recipe("Bob's Pie", None, "")),
        Some(&bob),
    )
    .await;

    let (_, client) = t
        .json(
            "POST",
            "/oauth/register",
            Some(json!({"client_name": "Claude", "redirect_uris": ["https://claude.ai/api/mcp/auth_callback"]})),
        )
        .await;
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
    let authorize = |cookie: Option<&str>, form: Option<String>| {
        let mut req = Request::builder().uri(match &form {
            Some(_) => "/oauth/authorize".to_string(),
            None => format!("/oauth/authorize?{q}"),
        });
        if let Some(c) = cookie {
            req = req.header(header::COOKIE, c);
        }
        match form {
            Some(f) => req
                .method("POST")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(f))
                .unwrap(),
            None => req.method("GET").body(Body::empty()).unwrap(),
        }
    };
    // Signed out: no password box, just a way to sign in; allowing does nothing
    let (_, _, html) = t.send(authorize(None, None)).await;
    assert!(
        html.contains("Sign in to Crumb") && !html.contains("App password"),
        "{html}"
    );
    let (status, _, _) = t
        .send(authorize(
            None,
            Some(format!("{q}&action=allow&password=correct+horse")),
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "no code without a session");
    // Bob approves it for his household
    let (_, _, html) = t.send(authorize(Some(&bob), None)).await;
    assert!(
        html.contains("Bob&#39;s kitchen") || html.contains("Bob's kitchen"),
        "{html}"
    );
    let (status, headers, _) = t
        .send(authorize(Some(&bob), Some(format!("{q}&action=allow"))))
        .await;
    assert_eq!(status, StatusCode::FOUND);
    let location = url::Url::parse(headers[header::LOCATION].to_str().unwrap()).unwrap();
    let code = location
        .query_pairs()
        .find(|(k, _)| k == "code")
        .unwrap()
        .1
        .into_owned();
    let form = serde_urlencoded::to_string([
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
        .body(Body::from(form))
        .unwrap();
    let (_, _, text) = t.send(req).await;
    let tokens: Value = serde_json::from_str(&text).unwrap();
    let access = tokens["access_token"].as_str().unwrap().to_string();

    let mcp = |token: Option<String>| {
        let mut req = Request::builder()
            .method("POST")
            .uri("/mcp")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json, text/event-stream");
        if let Some(tok) = token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {tok}"));
        }
        req.body(Body::from(
            json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": "search_recipes", "arguments": {}}})
            .to_string(),
        ))
        .unwrap()
    };
    let (status, _, _) = t.send(mcp(None)).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "accounts always need a token"
    );
    let (status, _, text) = t.send(mcp(Some(access))).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert!(
        text.contains("Bob's Pie") && !text.contains("Ann's Soup"),
        "{text}"
    );
}

#[tokio::test]
async fn accounts_share_links_open_the_right_box_and_old_connectors_keep_working() {
    let t = accounts_app(None, true);
    // A connector token issued while the box had one password
    let old_token = "old-connector-access-token-000000000000";
    t.state
        .db
        .lock()
        .execute(
            "INSERT INTO oauth_tokens (hash, kind, client_id, expires_at, created_at)
             VALUES (?1, 'access', 'c1', ?2, 1)",
            rusqlite::params![
                crumb::auth::sha256_hex(old_token),
                crumb::model::now_secs() + 3600
            ],
        )
        .unwrap();
    let (_, ann, _) = auth_post(
        &t,
        "/api/auth/setup",
        json!({"email": "ann@example.com", "name": "Ann", "password": "correct horse"}),
    )
    .await;
    let ann = ann.unwrap();
    call(
        &t,
        "POST",
        "/api/recipes",
        Some(book_recipe("Ann's Soup", None, "")),
        Some(&ann),
    )
    .await;
    let req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {old_token}"))
        .body(Body::from(
            json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": "search_recipes", "arguments": {}}})
            .to_string(),
        ))
        .unwrap();
    let (status, _, text) = t.send(req).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert!(text.contains("Ann's Soup"), "the owner's box: {text}");

    // Bob shares a recipe from his own box; the public link opens it, signed out
    let (_, bob, _) = auth_post(
        &t,
        "/api/auth/signup",
        json!({"email": "bob@example.com", "name": "Bob", "password": "another pass"}),
    )
    .await;
    let bob = bob.unwrap();
    let (_, pie) = call(
        &t,
        "POST",
        "/api/recipes",
        Some(book_recipe("Bob's Pie", None, "")),
        Some(&bob),
    )
    .await;
    let (status, share) = call(
        &t,
        "POST",
        &format!("/api/recipes/{}/share", pie["id"]),
        None,
        Some(&bob),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{share}");
    let path = share_path(share["url"].as_str().unwrap());
    let (status, _, html) = t.send(get(&path)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("Bob&#39;s Pie") || html.contains("Bob's Pie"));
    assert!(!html.contains("Ann's Soup"));
    let (_, _, export) = t.send(get(&format!("{path}/crumb.json"))).await;
    assert!(export.contains("Bob's Pie"), "{export}");
    // Stopped: gone
    call(
        &t,
        "DELETE",
        &format!("/api/recipes/{}/share", pie["id"]),
        None,
        Some(&bob),
    )
    .await;
    let (status, _, _) = t.send(get(&path)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// An owner's invite link, as `/api/auth/invites` hands it out: (its token, its id).
async fn invite(t: &TestApp, owner: &str) -> (String, i64) {
    let made = signed(t, owner, "POST", "/api/auth/invites", None).await;
    let url = made["url"].as_str().unwrap();
    let (_, token) = url.split_once("/invite#").expect("token in the fragment");
    (token.to_string(), made["id"].as_i64().unwrap())
}

#[tokio::test]
async fn accounts_invite_people_into_a_household() {
    let t = accounts_app(None, false);
    let (_, ann, _) = auth_post(
        &t,
        "/api/auth/setup",
        json!({"email": "ann@example.com", "name": "Ann", "password": "correct horse"}),
    )
    .await;
    let ann = ann.unwrap();
    signed(
        &t,
        &ann,
        "POST",
        "/api/recipes",
        Some(book_recipe("Ann's Soup", None, "")),
    )
    .await;

    // Only signed-in owners make links; anyone can look one up
    let (status, _) = t.json("POST", "/api/auth/invites", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (token, _) = invite(&t, &ann).await;
    let (status, preview) = t
        .json(
            "POST",
            "/api/auth/invite/preview",
            Some(json!({"token": token})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(preview["householdName"], "Ann's kitchen");
    assert_eq!(preview["invitedBy"], "Ann");
    let (status, _) = t
        .json(
            "POST",
            "/api/auth/invite/preview",
            Some(json!({"token": "made-up"})),
        )
        .await;
    assert_eq!(status, StatusCode::GONE);

    // Sign-up is closed, but an invite makes an account in Ann's household
    let (status, bob, body) = auth_post(
        &t,
        "/api/auth/invite/accept",
        json!({"token": token, "name": "Bob", "email": "bob@example.com", "password": "another pass"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let bob = bob.unwrap();
    let list = signed(&t, &bob, "GET", "/api/recipes", None).await;
    assert_eq!(list[0]["title"], "Ann's Soup", "Bob sees the shared box");
    let h = signed(&t, &bob, "GET", "/api/auth/household", None).await;
    assert_eq!(h["role"], "member");
    assert_eq!(h["members"].as_array().unwrap().len(), 2);
    assert_eq!(h["members"][0]["name"], "Ann", "owner first");
    assert_eq!(h["invites"], json!([]), "members don't see invites");
    // One use only
    let (status, _, _) = auth_post(
        &t,
        "/api/auth/invite/accept",
        json!({"token": token, "name": "Eve", "email": "eve@example.com", "password": "sneaky pass"}),
    )
    .await;
    assert_eq!(status, StatusCode::GONE);
    // Members can't invite or remove
    let (status, _) = call(&t, "POST", "/api/auth/invites", None, Some(&bob)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Someone with a box of their own joins with a second link and can switch between them
    let open = accounts_app(None, true);
    let (_, ann2, _) = auth_post(
        &open,
        "/api/auth/setup",
        json!({"email": "ann@example.com", "name": "Ann", "password": "correct horse"}),
    )
    .await;
    let ann2 = ann2.unwrap();
    let (_, cat, _) = auth_post(
        &open,
        "/api/auth/signup",
        json!({"email": "cat@example.com", "name": "Cat", "password": "meow meow"}),
    )
    .await;
    let cat = cat.unwrap();
    let cats_own = signed(&open, &cat, "GET", "/api/auth/household", None).await["id"].clone();
    let (token2, id2) = invite(&open, &ann2).await;
    let (token3, _) = invite(&open, &ann2).await;
    let pending = signed(&open, &ann2, "GET", "/api/auth/household", None).await;
    assert_eq!(pending["invites"].as_array().unwrap().len(), 2);
    signed(
        &open,
        &ann2,
        "DELETE",
        &format!("/api/auth/invites/{id2}"),
        None,
    )
    .await;
    let (status, _) = open
        .json(
            "POST",
            "/api/auth/invite/preview",
            Some(json!({"token": token2})),
        )
        .await;
    assert_eq!(status, StatusCode::GONE, "cancelled");
    signed(
        &open,
        &cat,
        "POST",
        "/api/auth/invite/accept",
        Some(json!({"token": token3})),
    )
    .await;
    let h = signed(&open, &cat, "GET", "/api/auth/household", None).await;
    assert_eq!(h["id"], 1, "this session moved into Ann's household");
    assert_eq!(h["households"].as_array().unwrap().len(), 2);
    signed(
        &open,
        &cat,
        "POST",
        "/api/auth/household/switch",
        Some(json!({"id": cats_own})),
    )
    .await;
    let s = signed(&open, &cat, "GET", "/api/auth/status", None).await;
    assert_eq!(s["household"]["name"], "Cat's kitchen");
    let (status, _) = call(
        &open,
        "POST",
        "/api/auth/household/switch",
        Some(json!({"id": 999})),
        Some(&cat),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Signing in on the invite page joins too
    let (token4, _) = invite(&t, &ann).await;
    let (status, _, _) = auth_post(
        &t,
        "/api/auth/invite/accept",
        json!({"token": token4, "email": "bob@example.com", "password": "wrong pass"}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Removing Bob: his session moves to a new, empty box of his own
    let h = signed(&t, &ann, "GET", "/api/auth/household", None).await;
    let bob_id = h["members"][1]["userId"].as_i64().unwrap();
    let ann_id = h["members"][0]["userId"].as_i64().unwrap();
    let (status, _) = call(
        &t,
        "DELETE",
        &format!("/api/auth/members/{ann_id}"),
        None,
        Some(&ann),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "not yourself");
    let (status, _) = call(&t, "POST", "/api/auth/household/leave", None, Some(&ann)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "owners stay");
    signed(
        &t,
        &ann,
        "DELETE",
        &format!("/api/auth/members/{bob_id}"),
        None,
    )
    .await;
    let list = signed(&t, &bob, "GET", "/api/recipes", None).await;
    assert_eq!(list, json!([]));
    let s = signed(&t, &bob, "GET", "/api/auth/status", None).await;
    assert_eq!(s["household"]["name"], "Bob's kitchen");
    assert_eq!(s["household"]["role"], "owner");

    // He can come back with the unused link, then leave on his own
    signed(
        &t,
        &bob,
        "POST",
        "/api/auth/invite/accept",
        Some(json!({"token": token4})),
    )
    .await;
    let list = signed(&t, &bob, "GET", "/api/recipes", None).await;
    assert_eq!(list[0]["title"], "Ann's Soup");
    let left = signed(&t, &bob, "POST", "/api/auth/household/leave", None).await;
    assert_ne!(left["household"], 1);
    let list = signed(&t, &bob, "GET", "/api/recipes", None).await;
    assert_eq!(list, json!([]));

    // Owners rename their household
    let renamed = signed(
        &t,
        &ann,
        "PATCH",
        "/api/auth/household",
        Some(json!({"name": "  The   Cooks "})),
    )
    .await;
    assert_eq!(renamed["name"], "The Cooks");
}

/// Approves Claude's connector as whoever `cookie` signs in, and returns its access token.
async fn connect_claude(t: &TestApp, cookie: &str) -> String {
    use base64::Engine;
    use sha2::Digest;
    let (_, client) = t
        .json(
            "POST",
            "/oauth/register",
            Some(json!({"client_name": "Claude", "redirect_uris": ["https://claude.ai/api/mcp/auth_callback"]})),
        )
        .await;
    let client_id = client["client_id"].as_str().unwrap().to_string();
    let verifier = "a-very-long-code-verifier-string-with-enough-entropy-1234567890";
    let challenge =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier));
    let redirect = "https://claude.ai/api/mcp/auth_callback";
    let form = serde_urlencoded::to_string([
        ("client_id", client_id.as_str()),
        ("redirect_uri", redirect),
        ("state", "xyz"),
        ("code_challenge", challenge.as_str()),
        ("code_challenge_method", "S256"),
        ("response_type", "code"),
        ("action", "allow"),
    ])
    .unwrap();
    let req = Request::builder()
        .method("POST")
        .uri("/oauth/authorize")
        .header(header::COOKIE, cookie)
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
    let form = serde_urlencoded::to_string([
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
        .body(Body::from(form))
        .unwrap();
    let (_, _, text) = t.send(req).await;
    let tokens: Value = serde_json::from_str(&text).unwrap();
    tokens["access_token"].as_str().unwrap().to_string()
}

async fn mcp_search(t: &TestApp, token: &str) -> (StatusCode, String) {
    let req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(
            json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": "search_recipes", "arguments": {}}})
            .to_string(),
        ))
        .unwrap();
    let (status, _, text) = t.send(req).await;
    (status, text)
}

/// What the fake auth service was sent: (path, X-Real-IP, X-Forwarded-For).
type Seen3 = std::sync::Arc<std::sync::Mutex<Vec<(String, Option<String>, Option<String>)>>>;

/// A stand-in for the Better Auth service (`auth/`): two people, each signed in by a
/// cookie naming them, and Bob's membership of Ann's kitchen switchable.
#[derive(Clone, Default)]
struct FakeAuth {
    bob_in_anns: std::sync::Arc<std::sync::atomic::AtomicBool>,
    seen: Seen3,
}

async fn fake_auth() -> (String, FakeAuth) {
    use axum::extract::{Query, State};
    use axum::routing::{any, get};
    let fake = FakeAuth::default();
    async fn session(
        State(f): State<FakeAuth>,
        Query(q): Query<std::collections::HashMap<String, String>>,
        headers: axum::http::HeaderMap,
    ) -> axum::Json<Value> {
        // Cat is in no household: one is only made when asked to
        let create = q.get("create").map(String::as_str) != Some("0");
        assert_eq!(headers["x-crumb-internal"], "shh");
        let cookie = headers
            .get(header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let who = cookie
            .split(';')
            .filter_map(|p| p.trim().split_once('='))
            .find(|(k, _)| k.ends_with("crumb.session_token"))
            .map(|(_, v)| v.to_string());
        let bob_there = f.bob_in_anns.load(std::sync::atomic::Ordering::SeqCst);
        let session = match who.as_deref() {
            Some("ann") => json!({
                "user": {"id": "u-ann", "email": "Ann@Example.com", "name": "Ann", "emailVerified": true},
                "household": {"id": "org-ann", "name": "Ann's kitchen", "role": "owner"},
            }),
            Some("bob") if bob_there => json!({
                "user": {"id": "u-bob", "email": "bob@example.com", "name": "Bob", "emailVerified": true},
                "household": {"id": "org-ann", "name": "Ann's kitchen", "role": "member"},
            }),
            Some("bob") => json!({
                "user": {"id": "u-bob", "email": "bob@example.com", "name": "Bob", "emailVerified": true},
                "household": {"id": "org-bob", "name": "Bob's kitchen", "role": "owner"},
            }),
            Some("cat") => json!({
                "user": {"id": "u-cat", "email": "cat@example.com", "name": "Cat", "emailVerified": true},
                "household": if create {
                    json!({"id": "org-cat", "name": "Cat's kitchen", "role": "owner"})
                } else {
                    Value::Null
                },
            }),
            _ => Value::Null,
        };
        axum::Json(json!({ "session": session }))
    }
    async fn member(
        State(f): State<FakeAuth>,
        Query(q): Query<std::collections::HashMap<String, String>>,
    ) -> axum::Json<Value> {
        let member = match (q["user"].as_str(), q["household"].as_str()) {
            ("u-ann", "org-ann") | ("u-bob", "org-bob") => true,
            ("u-bob", "org-ann") => f.bob_in_anns.load(std::sync::atomic::Ordering::SeqCst),
            _ => false,
        };
        axum::Json(json!({ "member": member }))
    }
    async fn better_auth(
        State(f): State<FakeAuth>,
        req: axum::http::Request<axum::body::Body>,
    ) -> axum::response::Response {
        let h = |n: &str| {
            req.headers()
                .get(n)
                .and_then(|v| v.to_str().ok())
                .map(String::from)
        };
        let path = req.uri().path().to_string();
        f.seen
            .lock()
            .unwrap()
            .push((path.clone(), h("x-real-ip"), h("x-forwarded-for")));
        if path == "/api/auth/organization/remove-member" {
            f.bob_in_anns
                .store(false, std::sync::atomic::Ordering::SeqCst);
        }
        axum::response::Response::builder()
            .status(200)
            .header(
                header::SET_COOKIE,
                "crumb.session_token=ann; Path=/; HttpOnly",
            )
            .header(header::SET_COOKIE, "crumb.session_data=x; Path=/; HttpOnly")
            .header(header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(r#"{"ok":true}"#))
            .unwrap()
    }
    async fn providers() -> axum::Json<Value> {
        axum::Json(json!({"providers": ["google", "apple"]}))
    }
    /// Bob's data and deletion (his own kitchen goes with him); anyone else is refused.
    async fn account(
        axum::extract::Path(action): axum::extract::Path<String>,
        headers: axum::http::HeaderMap,
        body: axum::body::Bytes,
    ) -> axum::response::Response {
        use axum::response::IntoResponse;
        let bob = headers
            .get(header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|c| c.contains("crumb.session_token=bob"));
        if !bob {
            return (
                StatusCode::UNAUTHORIZED,
                axum::Json(json!({"message": "Not signed in"})),
            )
                .into_response();
        }
        let body: Value = serde_json::from_slice(&body).unwrap_or_default();
        match action.as_str() {
            "export" => axum::Json(json!({
                "account": {"name": "Bob", "email": "bob@example.com"},
                "households": [
                    {"externalId": "org-bob", "name": "Bob's kitchen", "role": "owner"},
                    {"externalId": "org-new", "name": "Never opened", "role": "member"},
                ],
            }))
            .into_response(),
            _ if body["password"] != "pw" => (
                StatusCode::UNAUTHORIZED,
                axum::Json(json!({"message": "password: Incorrect password"})),
            )
                .into_response(),
            _ => axum::Json(json!({"ok": true, "deletedHouseholds": ["org-bob"]})).into_response(),
        }
    }
    let routes = axum::Router::new()
        .route("/internal/session", get(session))
        .route("/internal/member", get(member))
        .route("/internal/providers", get(providers))
        .route("/internal/{action}", axum::routing::post(account))
        .route("/api/auth/{*rest}", any(better_auth))
        .with_state(fake.clone());
    (serve(routes).await, fake)
}

#[tokio::test]
async fn hosted_asks_better_auth_who_is_signed_in() {
    let (url, fake) = fake_auth().await;
    let t = TestApp::with_config(|c| {
        c.auth_mode = crumb::config::AuthMode::Hosted;
        c.auth_service_url = Some(url);
        c.auth_internal_secret = Some("shh".into());
        c.hosted_home_owner = Some("ann@example.com".into());
    });
    // A recipe from before, in the database Crumb already had
    t.state
        .db
        .lock()
        .execute(
            "INSERT INTO recipes (title, ingredients, instructions, created_at, updated_at)
             VALUES ('Old Faithful', '[]', '[]', 1, 1)",
            [],
        )
        .unwrap();
    let (status, _) = t.json("GET", "/api/recipes", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, headers, _) = t.send(get("/recipes")).await;
    assert_eq!(status, StatusCode::FOUND);
    assert!(
        headers[header::LOCATION]
            .to_str()
            .unwrap()
            .starts_with("/login?next=")
    );
    // Crumb's own account routes step aside for Better Auth's
    let (status, _) = t
        .json("POST", "/api/auth/setup", Some(json!({"email": "x@y.z"})))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Signing in goes through to the service, cookies and all
    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/sign-in/email")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-forwarded-for", "6.6.6.6")
        .body(Body::from(r#"{"email":"ann@example.com","password":"pw"}"#))
        .unwrap();
    let (status, headers, text) = t.send(req).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert_eq!(headers.get_all(header::SET_COOKIE).iter().count(), 2);
    let seen = fake.seen.lock().unwrap().clone();
    assert_eq!(seen[0].0, "/api/auth/sign-in/email");
    assert_eq!(
        seen[0].2, None,
        "the client's forwarded-for isn't passed on"
    );

    // Ann's household is the one that was here (HOSTED_HOME_OWNER)
    let ann = "tz=UTC; crumb.session_token=ann";
    let list = signed(&t, ann, "GET", "/api/recipes", None).await;
    assert_eq!(list[0]["title"], "Old Faithful");
    let s = signed(&t, ann, "GET", "/api/auth/status", None).await;
    assert_eq!(s["mode"], "hosted");
    assert_eq!(s["household"]["id"], 1);
    assert_eq!(s["household"]["name"], "Ann's kitchen");

    // Bob's own household is a new, empty box
    let bob = "__Secure-crumb.session_token=bob";
    let list = signed(&t, bob, "GET", "/api/recipes", None).await;
    assert_eq!(list, json!([]));
    signed(
        &t,
        bob,
        "POST",
        "/api/recipes",
        Some(book_recipe("Bob's Pie", None, "")),
    )
    .await;
    let s = signed(&t, bob, "GET", "/api/auth/status", None).await;
    let bobs = s["household"]["id"].as_i64().unwrap();
    assert!(bobs > 1);
    let (status, _) = call(
        &t,
        "GET",
        "/api/recipes",
        None,
        Some("crumb.session_token=eve"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Bob joins Ann's kitchen (the service says so from his next lookup)
    fake.bob_in_anns
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let (status, _, _) = t
        .send(
            Request::builder()
                .method("POST")
                .uri("/api/auth/organization/accept-invitation")
                .header(header::COOKIE, bob)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let list = signed(&t, bob, "GET", "/api/recipes", None).await;
    assert_eq!(
        list[0]["title"], "Old Faithful",
        "a proxied change isn't cached over"
    );

    // A connector Bob approves there stops working once he's removed
    let token = connect_claude(&t, bob).await;
    let (status, text) = mcp_search(&t, &token).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert!(text.contains("Old Faithful"), "{text}");
    let (status, _, _) = t
        .send(
            Request::builder()
                .method("POST")
                .uri("/api/auth/organization/remove-member")
                .header(header::COOKIE, ann)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = mcp_search(&t, &token).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let list = signed(&t, bob, "GET", "/api/recipes", None).await;
    assert_eq!(list[0]["title"], "Bob's Pie", "back in his own box");
    let s = signed(&t, bob, "GET", "/api/auth/status", None).await;
    assert_eq!(s["household"]["id"], bobs, "the same box as before");
}

/// A stand-in for Google's token endpoint: the code is the ID token's claims (base64url
/// JSON), so a test says who signs in. Answers 400 unless the secret and PKCE verifier came.
async fn fake_google() -> String {
    use axum::routing::post;
    async fn token(body: axum::body::Bytes) -> axum::response::Response {
        use axum::response::IntoResponse;
        let form: std::collections::HashMap<String, String> =
            serde_urlencoded::from_bytes(&body).unwrap();
        if form.get("client_secret").map(String::as_str) != Some("g-secret")
            || form.get("code_verifier").is_none_or(|v| v.len() < 43)
            || form.get("grant_type").map(String::as_str) != Some("authorization_code")
        {
            return (StatusCode::BAD_REQUEST, "{\"error\":\"invalid_grant\"}").into_response();
        }
        axum::Json(json!({"id_token": format!("e30.{}.sig", form["code"])})).into_response()
    }
    serve(axum::Router::new().route("/token", post(token))).await
}

fn google_app(fake: &str, open_signup: bool) -> TestApp {
    let fake = fake.to_string();
    TestApp::with_config(move |c| {
        c.auth_mode = crumb::config::AuthMode::Accounts;
        c.open_signup = open_signup;
        c.social = vec![crumb::social::Provider {
            auth_url: format!("{fake}/auth"),
            token_url: format!("{fake}/token"),
            ..crumb::social::Provider::google("g-client", "g-secret")
        }];
    })
}

/// Signs in with (fake) Google as `sub` / `email`: starts with `start` (and `cookie`, when
/// signed in), comes back, and answers (where it sent the browser, the new session cookie).
async fn with_google(
    t: &TestApp,
    start: Value,
    cookie: Option<&str>,
    sub: &str,
    email: &str,
) -> (String, Option<String>) {
    use base64::Engine;
    let mut req = Request::builder()
        .method("POST")
        .uri("/api/auth/social/google/start")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, c);
    }
    let (status, headers, text) = t
        .send(req.body(Body::from(start.to_string())).unwrap())
        .await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let social = headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let body: Value = serde_json::from_str(&text).unwrap();
    let url = url::Url::parse(body["url"].as_str().unwrap()).unwrap();
    let param = |k: &str| {
        url.query_pairs()
            .find(|(q, _)| q == k)
            .map(|(_, v)| v.into_owned())
            .unwrap()
    };
    assert_eq!(param("client_id"), "g-client");
    assert_eq!(param("code_challenge_method"), "S256");
    assert!(param("redirect_uri").ends_with("/api/auth/social/google/callback"));
    let claims = json!({
        "iss": "https://accounts.google.com", "aud": "g-client",
        "exp": crumb::model::now_secs() + 600, "nonce": param("nonce"),
        "sub": sub, "email": email, "email_verified": true, "name": "Carl Cook",
    });
    let code = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
    let q = serde_urlencoded::to_string([("code", code), ("state", param("state"))]).unwrap();
    let cookies = match cookie {
        Some(c) => format!("{social}; {c}"),
        None => social,
    };
    let req = Request::builder()
        .uri(format!("/api/auth/social/google/callback?{q}"))
        .header(header::COOKIE, cookies)
        .body(Body::empty())
        .unwrap();
    let (status, headers, _) = t.send(req).await;
    assert_eq!(status, StatusCode::FOUND);
    let session = headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(|c| c.split(';').next().unwrap().to_string())
        .find(|c| c.starts_with("crumb_session=") && !c.ends_with('='));
    (
        headers[header::LOCATION].to_str().unwrap().to_string(),
        session,
    )
}

#[tokio::test]
async fn accounts_sign_in_with_google() {
    let fake = fake_google().await;
    let t = google_app(&fake, false);
    let (_, ann, _) = auth_post(
        &t,
        "/api/auth/setup",
        json!({"email": "ann@example.com", "name": "Ann", "password": "correct horse"}),
    )
    .await;
    let ann = ann.unwrap();
    let (_, s) = t.json("GET", "/api/auth/status", None).await;
    assert_eq!(s["providers"], json!(["google"]));

    // Sign-up is closed: a stranger's Google account doesn't get in
    let (to, session) = with_google(&t, json!({}), None, "g-bob", "bob@example.com").await;
    assert_eq!((to.as_str(), session), ("/login?error=signup_closed", None));
    // Nor does a Google account with Ann's address that she hasn't linked
    let (to, session) = with_google(&t, json!({}), None, "g-eve", "ann@example.com").await;
    assert_eq!((to.as_str(), session), ("/login?error=email_taken", None));

    // Ann links hers (any address), then signs in with it
    let (to, _) = with_google(
        &t,
        json!({"intent": "link"}),
        Some(&ann),
        "g-ann",
        "ann.cook@gmail.test",
    )
    .await;
    assert_eq!(to, "/more/account?linked=google");
    let methods = signed(&t, &ann, "GET", "/api/auth/identities", None).await;
    assert_eq!(methods["password"], true);
    assert_eq!(methods["linked"][0]["provider"], "google");
    assert_eq!(methods["linked"][0]["email"], "ann.cook@gmail.test");
    assert_eq!(methods["available"], json!(["google"]));
    let (to, session) = with_google(
        &t,
        json!({"next": "/recipes"}),
        None,
        "g-ann",
        "ann.cook@gmail.test",
    )
    .await;
    assert_eq!(to, "/recipes");
    let s = signed(&t, &session.unwrap(), "GET", "/api/auth/status", None).await;
    assert_eq!(s["user"]["email"], "ann@example.com");
    // Nobody else can link the same Google account
    let (_, cy, _) = auth_post(
        &t,
        "/api/auth/invite/accept",
        json!({"token": invite(&t, &ann).await.0, "name": "Cy", "email": "cy@example.com", "password": "long enough"}),
    )
    .await;
    let (to, _) = with_google(
        &t,
        json!({"intent": "link"}),
        cy.as_deref(),
        "g-ann",
        "x@y.z",
    )
    .await;
    assert_eq!(to, "/more/account?error=already_linked");

    // An invite lets a new Google account in, straight into Ann's kitchen
    let (token, _) = invite(&t, &ann).await;
    let (to, carl) = with_google(
        &t,
        json!({"intent": "invite", "invite": token}),
        None,
        "g-carl",
        "carl@example.com",
    )
    .await;
    assert_eq!(to, "/");
    let carl = carl.unwrap();
    let s = signed(&t, &carl, "GET", "/api/auth/status", None).await;
    assert_eq!(s["household"]["id"], 1);
    assert_eq!(s["user"]["name"], "Carl Cook");
    // Used up
    let (status, _) = t
        .json(
            "POST",
            "/api/auth/social/google/start",
            Some(json!({"intent": "invite", "invite": token})),
        )
        .await;
    assert_eq!(status, StatusCode::GONE);
    // Carl has no password, so Google is his only way in and can't be unlinked
    let methods = signed(&t, &carl, "GET", "/api/auth/identities", None).await;
    assert_eq!(methods["password"], false);
    let (status, _) = call(
        &t,
        "DELETE",
        "/api/auth/identities/google",
        None,
        Some(&carl),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    // Nor does his empty password let anyone in
    let (status, _, _) = auth_post(
        &t,
        "/api/auth/login",
        json!({"email": "carl@example.com", "password": ""}),
    )
    .await;
    assert_ne!(status, StatusCode::OK);
    // Ann has a password, so she can
    signed(&t, &ann, "DELETE", "/api/auth/identities/google", None).await;

    // A state this browser didn't start is refused, and so is a used one
    let req = Request::builder()
        .uri("/api/auth/social/google/callback?code=x&state=made-up")
        .header(header::COOKIE, "crumb_social=other")
        .body(Body::empty())
        .unwrap();
    let (_, headers, _) = t.send(req).await;
    assert_eq!(headers[header::LOCATION], "/login?error=expired");
    // Apple's form post becomes the same GET
    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/social/google/callback")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("code=c1&state=s1&extra=no"))
        .unwrap();
    let (status, headers, _) = t.send(req).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(
        headers[header::LOCATION],
        "/api/auth/social/google/callback?code=c1&state=s1"
    );
    // Not set up: nothing to start
    let (status, _) = t
        .json("POST", "/api/auth/social/apple/start", Some(json!({})))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn accounts_connected_apps_export_and_deletion() {
    let t = accounts_app(None, true);
    let (_, ann, _) = auth_post(
        &t,
        "/api/auth/setup",
        json!({"email": "ann@example.com", "name": "Ann", "password": "correct horse"}),
    )
    .await;
    let ann = ann.unwrap();
    signed(
        &t,
        &ann,
        "POST",
        "/api/recipes",
        Some(book_recipe("Ann's Soup", None, "")),
    )
    .await;
    let (_, bob, _) = auth_post(
        &t,
        "/api/auth/signup",
        json!({"email": "bob@example.com", "name": "Bob", "password": "another pass"}),
    )
    .await;
    let bob = bob.unwrap();
    signed(
        &t,
        &bob,
        "POST",
        "/api/recipes",
        Some(book_recipe("Bob's Pie", None, "")),
    )
    .await;
    let bobs = signed(&t, &bob, "GET", "/api/auth/status", None).await["household"]["id"]
        .as_i64()
        .unwrap();
    // Bob joins Ann's kitchen too (and his session moves there)
    let (token, _) = invite(&t, &ann).await;
    signed(
        &t,
        &bob,
        "POST",
        "/api/auth/invite/accept",
        Some(json!({"token": token})),
    )
    .await;

    // Connected apps: each person sees their own
    let claude = connect_claude(&t, &ann).await;
    let apps = signed(&t, &ann, "GET", "/api/connections", None).await;
    assert_eq!(apps.as_array().unwrap().len(), 1);
    assert_eq!(apps[0]["name"], "Claude");
    assert_eq!(
        apps[0]["household"],
        json!({"id": 1, "name": "Ann's kitchen"})
    );
    assert!(apps[0]["connectedAt"].as_i64().unwrap() > 0);
    assert_eq!(
        signed(&t, &bob, "GET", "/api/connections", None).await,
        json!([])
    );
    let id = apps[0]["id"].as_str().unwrap().to_string();
    let (status, _) = call(
        &t,
        "DELETE",
        &format!("/api/connections/{id}"),
        None,
        Some(&bob),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "not Bob's to disconnect");
    assert_eq!(mcp_search(&t, &claude).await.0, StatusCode::OK);
    signed(&t, &ann, "DELETE", &format!("/api/connections/{id}"), None).await;
    assert_eq!(mcp_search(&t, &claude).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        signed(&t, &ann, "GET", "/api/connections", None).await,
        json!([])
    );
    let (status, _) = t.json("GET", "/api/connections", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Bob's data: his account and both his households, recipes and all
    let (status, headers, text) = t
        .send(
            Request::builder()
                .uri("/api/account/export")
                .header(header::COOKIE, &bob)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .contains("crumb-account-")
    );
    let data: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(data["account"]["email"], "bob@example.com");
    assert_eq!(data["signInMethods"]["password"], true);
    let households = data["households"].as_array().unwrap();
    assert_eq!(households.len(), 2);
    let named = |name: &str| households.iter().find(|h| h["name"] == name).unwrap();
    assert_eq!(
        named("Bob's kitchen")["recipes"]["recipes"][0]["title"],
        "Bob's Pie"
    );
    assert_eq!(named("Ann's kitchen")["role"], "member");
    assert_eq!(
        named("Ann's kitchen")["recipes"]["recipes"][0]["title"],
        "Ann's Soup"
    );
    assert!(!data["devices"].as_array().unwrap().is_empty());

    // Deleting needs the password
    let (status, _) = call(
        &t,
        "POST",
        "/api/account/delete",
        Some(json!({"password": "wrong"})),
        Some(&ann),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // Ann goes: Bob keeps her kitchen, as its owner, recipes and all
    let claude = connect_claude(&t, &ann).await;
    signed(
        &t,
        &ann,
        "POST",
        "/api/account/delete",
        Some(json!({"password": "correct horse"})),
    )
    .await;
    let (status, _) = call(&t, "GET", "/api/recipes", None, Some(&ann)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(mcp_search(&t, &claude).await.0, StatusCode::UNAUTHORIZED);
    let (status, _, _) = auth_post(
        &t,
        "/api/auth/login",
        json!({"email": "ann@example.com", "password": "correct horse"}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let h = signed(&t, &bob, "GET", "/api/auth/household", None).await;
    assert_eq!(
        (h["id"].as_i64(), h["role"].as_str()),
        (Some(1), Some("owner"))
    );
    let list = signed(&t, &bob, "GET", "/api/recipes", None).await;
    assert_eq!(list[0]["title"], "Ann's Soup");

    // Bob goes too: both households were his alone, so they go with him
    signed(
        &t,
        &bob,
        "POST",
        "/api/account/delete",
        Some(json!({"password": "another pass"})),
    )
    .await;
    let home: i64 = t
        .state
        .db
        .lock()
        .query_row("SELECT count(*) FROM recipes", [], |r| r.get(0))
        .unwrap();
    assert_eq!(home, 0, "the home box is emptied");
    let accounts = t.state.accounts.as_ref().unwrap();
    assert_eq!(accounts.household_name(bobs).unwrap(), None);
    assert!(accounts.needs_setup().unwrap(), "nobody is left");
    let fresh = t.state.households.get(bobs).unwrap();
    let left: i64 = fresh
        .db
        .lock()
        .query_row("SELECT count(*) FROM recipes", [], |r| r.get(0))
        .unwrap();
    assert_eq!(left, 0, "Bob's own box is gone");
}

#[tokio::test]
async fn accounts_change_their_email_with_the_password() {
    let t = accounts_app(None, true);
    let (_, ann, _) = auth_post(
        &t,
        "/api/auth/setup",
        json!({"email": "ann@exmaple.com", "name": "Ann", "password": "correct horse"}),
    )
    .await;
    let ann = ann.unwrap();
    let (_, phone, _) = auth_post(
        &t,
        "/api/auth/login",
        json!({"email": "ann@exmaple.com", "password": "correct horse"}),
    )
    .await;
    let phone = phone.unwrap();
    auth_post(
        &t,
        "/api/auth/signup",
        json!({"email": "bob@example.com", "name": "Bob", "password": "another pass"}),
    )
    .await;

    // Signed in isn't enough: it takes the password
    let change = |email: &str, password: &str| json!({"email": email, "password": password});
    let (status, _) = t
        .json(
            "POST",
            "/api/account/email",
            Some(change("ann@example.com", "correct horse")),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    for (body, want) in [
        (change("ann@example.com", "wrong horse"), 401),
        (json!({"email": "ann@example.com"}), 401),
        (change("bob@example.com", "correct horse"), 409),
        (change("ann@exmaple.com", "correct horse"), 400),
        (change("nope", "correct horse"), 400),
    ] {
        let (status, _) = call(&t, "POST", "/api/account/email", Some(body), Some(&ann)).await;
        assert_eq!(status.as_u16(), want);
    }
    let done = signed(
        &t,
        &ann,
        "POST",
        "/api/account/email",
        Some(change("ann@example.com", "correct horse")),
    )
    .await;
    assert_eq!(
        done,
        json!({"ok": true, "pending": false, "email": "ann@example.com"})
    );

    // Signed in with the new address only, and signed out everywhere else
    let s = signed(&t, &ann, "GET", "/api/auth/status", None).await;
    assert_eq!(s["user"]["email"], "ann@example.com");
    let (status, _) = call(&t, "GET", "/api/recipes", None, Some(&phone)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = auth_post(
        &t,
        "/api/auth/login",
        json!({"email": "ann@exmaple.com", "password": "correct horse"}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = auth_post(
        &t,
        "/api/auth/login",
        json!({"email": "ann@example.com", "password": "correct horse"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Links to confirm an address are only the hosted edition's
    let (status, _) = t
        .json(
            "POST",
            "/api/account/email/confirm",
            Some(json!({"token": "x"})),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn hosted_accounts_export_and_delete_through_better_auth() {
    let (url, _) = fake_auth().await;
    let t = TestApp::with_config(|c| {
        c.auth_mode = crumb::config::AuthMode::Hosted;
        c.auth_service_url = Some(url);
        c.auth_internal_secret = Some("shh".into());
    });
    let bob = "crumb.session_token=bob";
    let s = signed(&t, bob, "GET", "/api/auth/status", None).await;
    assert_eq!(s["providers"], json!(["google", "apple"]));
    let bobs = s["household"]["id"].as_i64().unwrap();
    signed(
        &t,
        bob,
        "POST",
        "/api/recipes",
        Some(book_recipe("Bob's Pie", None, "")),
    )
    .await;

    // The service's account, with Bob's recipes added (a household never opened has none)
    let data = signed(&t, bob, "GET", "/api/account/export", None).await;
    assert_eq!(data["account"]["email"], "bob@example.com");
    assert_eq!(data["households"][0]["id"], bobs);
    assert_eq!(
        data["households"][0]["recipes"]["recipes"][0]["title"],
        "Bob's Pie"
    );
    assert_eq!(data["households"][1]["id"], Value::Null);
    assert_eq!(data["households"][1].get("recipes"), None);

    // The service's refusal comes back as it was
    let (status, body) = call(
        &t,
        "POST",
        "/api/account/delete",
        Some(json!({"password": "no"})),
        Some(bob),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["message"], "password: Incorrect password");
    signed(
        &t,
        bob,
        "POST",
        "/api/account/delete",
        Some(json!({"password": "pw"})),
    )
    .await;
    let accounts = t.state.accounts.as_ref().unwrap();
    assert_eq!(accounts.household_name(bobs).unwrap(), None);
    assert_eq!(accounts.find_hosted_household("org-bob").unwrap(), None);
}

#[tokio::test]
async fn a_video_link_is_queued_and_polled() {
    use crumb::video_jobs::{Limits, VideoJobs, WAIT_LIMIT};
    let mut t = TestApp::new(None);
    // The video pipeline stood in for: each job waits a moment, then saves a recipe
    t.state.video_jobs = std::sync::Arc::new(VideoJobs::with_runner(
        Limits {
            workers: 1,
            queue_max: 1,
            wait_limit: WAIT_LIMIT,
        },
        |state, url| {
            Box::pin(async move {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                let fields = crumb::model::RecipeFields {
                    title: "Garlic noodles".into(),
                    url: Some(url),
                    ingredients: vec![crumb::model::Section::unnamed(vec!["noodles".into()])],
                    ..Default::default()
                };
                crumb::recipes::create_recipe(&state.db.lock(), fields, "video")
            })
        },
    ));
    let video = |n: u32| json!({"url": format!("https://www.tiktok.com/@chef/video/{n}")});

    let (status, first) = t.json("POST", "/api/recipes/import", Some(video(1))).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let job = first["jobId"].as_str().unwrap().to_string();
    assert_eq!(first["status"], "queued");
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    let (_, running) = t
        .json("GET", &format!("/api/import/jobs/{job}"), None)
        .await;
    assert_eq!(running["status"], "running");

    // One more may wait; the next is refused at once, in the API's error shape
    let (status, second) = t.json("POST", "/api/recipes/import", Some(video(2))).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(second["position"], 1);
    let (status, full) = t.json("POST", "/api/recipes/import", Some(video(3))).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(full["statusCode"], 429);
    assert!(full["message"].as_str().unwrap().contains("Wee Chef"));

    // Other links never wait behind the videos
    let (status, text) = t
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"text": "Toast\n\nIngredients\n1 slice bread\n\nMethod\n1. Toast it."})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(text["isNew"], true);

    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    let (_, done) = t
        .json("GET", &format!("/api/import/jobs/{job}"), None)
        .await;
    assert_eq!(done["status"], "done");
    assert_eq!(done["recipe"]["title"], "Garlic noodles");
    assert_eq!(done["recipe"]["isNew"], true);

    // The saved video is found again without queueing
    let (status, again) = t.json("POST", "/api/recipes/import", Some(video(1))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["isNew"], false);

    let (status, _) = t.json("GET", "/api/import/jobs/nope", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_video_read_in_the_browser_is_saved_without_asking_the_site() {
    // No Wee Chef and no video tools: the description the extension read is the recipe
    let t = TestApp::new(None);
    let url = "https://www.youtube.com/watch?v=Xy_djhH3WE4&t=122s";
    let body = json!({
        "url": url,
        "video": {
            "title": "Weeknight pad thai",
            "description": "My go-to pad thai.\n\nIngredients\n200 g rice noodles\n2 tbsp fish sauce\n1 tbsp tamarind paste\n2 eggs\n\nMethod\n1. Soak the noodles.\n2. Fry everything together.",
            "author": "Noodle Co",
            "thumbnail": "http://127.0.0.1:1/not-youtube.jpg",
            "duration": 600,
            "transcript": "so today we're making pad thai"
        }
    });
    let (status, job) = t.json("POST", "/api/recipes/import", Some(body)).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{job}");
    let id = job["jobId"].as_str().unwrap().to_string();
    let mut done = Value::Null;
    for _ in 0..100 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let (_, now) = t.json("GET", &format!("/api/import/jobs/{id}"), None).await;
        if now["status"] == "done" || now["status"] == "failed" {
            done = now;
            break;
        }
    }
    assert_eq!(done["status"], "done", "{done}");
    let recipe_id = done["recipe"]["id"].as_i64().unwrap();
    let (_, recipe) = t
        .json("GET", &format!("/api/recipes/{recipe_id}"), None)
        .await;
    assert_eq!(recipe["author"], "Noodle Co");
    assert_eq!(recipe["url"], url);
    assert_eq!(
        recipe["ingredients"][0]["items"].as_array().unwrap().len(),
        4
    );
    assert_eq!(
        recipe["image"],
        Value::Null,
        "only YouTube's own images are fetched"
    );

    let (status, _) = t
        .json(
            "POST",
            "/api/recipes/import",
            Some(json!({"url": "https://www.youtube.com/watch?v=abc", "video": {"duration": "long"}})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// The JSON in a page's `#page-data`.
fn page_data(html: &str) -> Value {
    let start = html.find(r#"id="page-data">"#).unwrap() + r#"id="page-data">"#.len();
    let end = start + html[start..].find("</script>").unwrap();
    serde_json::from_str(&html[start..end]).unwrap()
}

fn preview_req(uri: &str, site: Option<&str>) -> Request<Body> {
    let mut req = Request::builder().uri(uri);
    if let Some(site) = site {
        req = req.header("sec-fetch-site", site);
    }
    req.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn previews_read_a_recipe_before_it_is_saved() {
    // A recipe site that counts its visits
    let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let site = format!("http://{}", listener.local_addr().unwrap());
    let counter = hits.clone();
    let origin = axum::Router::new().route(
        "/soup",
        axum::routing::get(move || {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async {
                axum::response::Html(format!(
                    r#"<html><head><script type="application/ld+json">{}</script></head></html>"#,
                    json!({"@type": "Recipe", "name": "Leek Soup",
                        "image": "https://images.example/leek.jpg",
                        "recipeIngredient": ["2 leeks", "1 potato"],
                        "recipeInstructions": ["Sweat the leeks.", "Simmer & blend."]})
                ))
            }
        }),
    );
    tokio::spawn(async move { axum::serve(listener, origin).await.unwrap() });
    let t = TestApp::new(None);
    let url = format!("{site}/soup");
    let q: String = url::form_urlencoded::byte_serialize(url.as_bytes()).collect();

    // From the extension's new tab: "reading…" at once, which carries on by itself
    let (status, headers, html) = t
        .send(preview_req(&format!("/preview?url={q}"), Some("none")))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page_data(&html)["preview"]["state"], "loading");
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0);

    // From another site: only the question, never a scrape, even with go=1
    for uri in [
        format!("/preview?url={q}"),
        format!("/preview?url={q}&go=1"),
    ] {
        let (_, _, html) = t.send(preview_req(&uri, Some("cross-site"))).await;
        assert_eq!(page_data(&html)["preview"]["state"], "ask", "{uri}");
    }
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0);

    // Carrying on from Crumb's own page: the recipe, in the share layout, not saved
    let (status, headers, html) = t
        .send(preview_req(
            &format!("/preview?url={q}&go=1"),
            Some("same-origin"),
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{html}");
    let data = page_data(&html);
    assert_eq!(data["preview"]["state"], "ready");
    assert_eq!(data["preview"]["title"], "Leek Soup");
    assert_eq!(data["preview"]["url"], url);
    assert!(html.contains("<title>Leek Soup · Crumb</title>"));
    assert!(html.contains("2 leeks") && html.contains("Simmer &amp; blend."));
    assert!(html.contains(r#"src="https://images.example/leek.jpg""#));
    assert!(
        !html.contains("og:title"),
        "a preview has no link-preview tags"
    );
    let csp = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(csp.contains("img-src 'self' data: https: http:"), "{csp}");
    assert!(
        csp.contains("'sha256-"),
        "the template's script runs by its hash"
    );
    let (_, list) = t.json("GET", "/api/recipes", None).await;
    assert_eq!(list.as_array().map_or(0, Vec::len), 0, "{list}");
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);

    // Adding it saves what the preview showed, without fetching the page again
    let (status, res) = t
        .json("POST", "/api/recipes/import", Some(json!({"url": url})))
        .await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["isNew"], true);
    assert_eq!(res["title"], "Leek Soup");
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);

    // Once it's in the box, a preview opens the recipe itself
    let id = res["id"].as_i64().unwrap();
    let (status, headers, _) = t
        .send(preview_req(
            &format!("/preview?url={q}"),
            Some("cross-site"),
        ))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(headers[header::LOCATION], format!("/recipes/{id}"));

    // A page without a recipe says why, on the same page
    let missing: String =
        url::form_urlencoded::byte_serialize(format!("{site}/nothing").as_bytes()).collect();
    let (status, _, html) = t
        .send(preview_req(&format!("/preview?url={missing}&go=1"), None))
        .await;
    assert!(status.is_client_error(), "{status}");
    let data = page_data(&html);
    assert_eq!(data["preview"]["state"], "failed");
    assert!(data["preview"]["message"].as_str().unwrap().contains("404"));

    // No link: the Add page
    let (status, headers, _) = t.send(get("/preview?url=nope")).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(headers[header::LOCATION], "/add");
}

#[tokio::test]
async fn previews_are_behind_the_login() {
    let t = TestApp::new(Some("pw"));
    let (status, headers, _) = t
        .send(get("/preview?url=https%3A%2F%2Fexample.com%2Fsoup"))
        .await;
    assert!(status.is_redirection(), "{status}");
    let to = headers[header::LOCATION].to_str().unwrap();
    assert!(to.starts_with("/login?next=%2Fpreview%3Furl%3D"), "{to}");
}

#[tokio::test]
async fn import_refuses_archive_bombs_and_oversized_uploads() {
    use std::io::Write;
    let t = TestApp::new(None);
    let upload = |name: &str, data: Vec<u8>| {
        let boundary = "XBOMB";
        let mut body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"{name}\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n"
        )
        .into_bytes();
        body.extend(data);
        body.extend(format!("\r\n--{boundary}--\r\n").into_bytes());
        Request::builder()
            .method("POST")
            .uri("/api/import/files")
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap()
    };
    // 30 MB of zeros, a few KB gzipped: over the per-file cap once unpacked
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    e.write_all(&vec![0u8; 30 * 1024 * 1024]).unwrap();
    let bomb = e.finish().unwrap();
    assert!(bomb.len() < 1_000_000);
    let (status, _, text) = t.send(upload("bomb.txt.gz", bomb)).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let results: Value = serde_json::from_str(&text).unwrap();
    assert!(results[0]["error"].as_str().is_some(), "{text}");

    // A file over 25 MB is refused while it streams in, and says so
    let (status, _, text) = t
        .send(upload("big.txt", vec![b'a'; 26 * 1024 * 1024]))
        .await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let results: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(results[0]["error"], "File is over 25 MB");

    // More than the whole request may carry is a 4xx before anything is imported
    let (status, _, _) = t
        .send(upload("huge.txt", vec![b'a'; 60 * 1024 * 1024]))
        .await;
    assert!(status.is_client_error(), "{status}");
}

fn login_request(password: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "password": password }).to_string()))
        .unwrap()
}

#[tokio::test]
async fn password_guesses_back_off_even_in_parallel() {
    let t = TestApp::new(Some("secret"));
    // A burst of parallel wrong guesses: only the free tries get an answer, the rest wait
    let replies = futures_util::future::join_all(
        (0..30).map(|i| t.send(login_request(&format!("guess{i}")))),
    )
    .await;
    let wrong = replies
        .iter()
        .filter(|(s, _, _)| *s == StatusCode::UNAUTHORIZED)
        .count();
    let limited = replies
        .iter()
        .filter(|(s, _, _)| *s == StatusCode::TOO_MANY_REQUESTS)
        .count();
    assert_eq!(wrong, 5, "only the free tries are checked");
    assert_eq!(limited, 25);
    // Backing off means even the right password waits its turn
    let (status, _, text) = t.send(login_request("secret")).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{text}");
    assert!(text.contains("Try again in"), "{text}");
}

#[tokio::test]
async fn a_good_password_clears_the_count() {
    let t = TestApp::new(Some("secret"));
    for _ in 0..4 {
        let (status, _, _) = t.send(login_request("nope")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(t.send(login_request("secret")).await.0, StatusCode::OK);
    for _ in 0..4 {
        assert_eq!(
            t.send(login_request("nope")).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
}

fn consent_query(client_id: &str, redirect: &str) -> String {
    serde_urlencoded::to_string([
        ("client_id", client_id),
        ("redirect_uri", redirect),
        ("state", "s"),
        (
            "code_challenge",
            "abcabcabcabcabcabcabcabcabcabcabcabcabcabcabc",
        ),
        ("code_challenge_method", "S256"),
        ("response_type", "code"),
    ])
    .unwrap()
}

#[tokio::test]
async fn oauth_password_guesses_share_the_login_limit() {
    let t = TestApp::new(Some("secret"));
    let (_, client) = t
        .json(
            "POST",
            "/oauth/register",
            Some(json!({"client_name": "Claude", "redirect_uris": ["https://claude.ai/cb"]})),
        )
        .await;
    let q = consent_query(
        client["client_id"].as_str().unwrap(),
        "https://claude.ai/cb",
    );
    let post = |password: &str| {
        Request::builder()
            .method("POST")
            .uri("/oauth/authorize")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!("{q}&action=allow&password={password}")))
            .unwrap()
    };
    for _ in 0..5 {
        let (status, _, html) = t.send(post("nope")).await;
        assert_eq!(status, StatusCode::OK);
        assert!(html.contains("Incorrect password"));
    }
    // The login form's own attempts count too
    assert_eq!(
        t.send(login_request("nope")).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );
    let (_, _, html) = t.send(post("secret")).await;
    assert!(html.contains("Try again in"), "{html}");
}

#[tokio::test]
async fn session_cookies_depend_on_the_servers_secret() {
    let a = TestApp::with_config(|c| {
        c.app_password = Some("secret".into());
        c.session_secret = vec![1; 32];
    });
    let b = TestApp::with_config(|c| {
        c.app_password = Some("secret".into());
        c.session_secret = vec![2; 32];
    });
    let (status, headers, _) = a.send(login_request("secret")).await;
    assert_eq!(status, StatusCode::OK);
    let pair = headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let with_cookie = |uri: &str| {
        Request::builder()
            .uri(uri)
            .header(header::COOKIE, pair.clone())
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(a.send(with_cookie("/api/recipes")).await.0, StatusCode::OK);
    assert_eq!(
        b.send(with_cookie("/api/recipes")).await.0,
        StatusCode::UNAUTHORIZED,
        "same password, other secret: not a valid session"
    );
}

#[tokio::test]
async fn oauth_registration_is_limited_and_pruned() {
    let t = TestApp::new(Some("secret"));
    let register = |name: &str| {
        t.json(
            "POST",
            "/oauth/register",
            Some(json!({"client_name": name, "redirect_uris": ["https://a.example/cb"]})),
        )
    };
    // Old clients nobody connected are dropped when new ones register; used ones stay
    {
        let conn = t.state.db.lock();
        let now = crumb::model::now_secs();
        for (id, age) in [("stale", 30 * 86400), ("fresh", 60), ("used", 30 * 86400)] {
            conn.execute(
                "INSERT INTO oauth_clients (id, name, redirect_uris, created_at) VALUES (?1, 'x', '[]', ?2)",
                rusqlite::params![id, now - age],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO oauth_tokens (hash, kind, client_id, expires_at, created_at) VALUES ('h', 'access', 'used', ?1, ?2)",
            rusqlite::params![now + 1000, now],
        )
        .unwrap();
    }
    assert_eq!(register("one").await.0, StatusCode::CREATED);
    let ids: Vec<String> = {
        let conn = t.state.db.lock();
        let mut stmt = conn
            .prepare("SELECT id FROM oauth_clients ORDER BY id")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert!(ids.contains(&"fresh".to_string()) && ids.contains(&"used".to_string()));
    assert!(!ids.contains(&"stale".to_string()), "{ids:?}");

    // Ten an hour from one address
    for _ in 0..9 {
        assert_eq!(register("more").await.0, StatusCode::CREATED);
    }
    assert_eq!(register("too many").await.0, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn the_consent_screen_does_not_let_a_name_pose_as_claude() {
    let t = TestApp::new(Some("secret"));
    let screen = async |name: &str, redirect: &str| {
        let (_, client) = t
            .json(
                "POST",
                "/oauth/register",
                Some(json!({"client_name": name, "redirect_uris": [redirect]})),
            )
            .await;
        let q = consent_query(client["client_id"].as_str().unwrap(), redirect);
        t.send(get(&format!("/oauth/authorize?{q}"))).await.2
    };
    let html = screen("Claude", "https://attacker.example/cb").await;
    assert!(
        html.contains("Connect an app at <code>attacker.example</code>?"),
        "{html}"
    );
    assert!(html.contains("calling itself"));
    assert!(!html.contains("Connect Claude?"));
    let html = screen("Claude", "https://claude.ai/api/mcp/auth_callback").await;
    assert!(html.contains("Connect Claude?"));
}

#[tokio::test]
async fn state_changes_from_other_sites_are_refused() {
    let t = TestApp::new(None);
    let post = |extra: &[(&str, &str)]| {
        let mut req = Request::builder()
            .method("POST")
            .uri("/api/recipes/bulk-delete")
            .header(header::CONTENT_TYPE, "text/plain")
            .header(header::HOST, "crumb.test");
        for (k, v) in extra {
            req = req.header(*k, *v);
        }
        req.body(Body::from(r#"{"ids":[1]}"#)).unwrap()
    };
    for site in ["cross-site", "same-site"] {
        let (status, _, body) = t.send(post(&[("sec-fetch-site", site)])).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{site}: {body}");
    }
    // No Fetch Metadata: the Origin has to be this site's
    let (status, _, _) = t.send(post(&[("origin", "https://evil.example")])).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = t.send(post(&[("origin", "null")])).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // This site's own pages, browsers that only send Origin, and non-browser clients pass
    for extra in [
        vec![("sec-fetch-site", "same-origin")],
        vec![("origin", "http://crumb.test")],
        vec![],
    ] {
        let (status, _, body) = t.send(post(&extra)).await;
        assert_ne!(status, StatusCode::FORBIDDEN, "{extra:?}: {body}");
    }
    // Reads are never refused, and the protocol endpoints take other origins' calls
    let (status, _, _) = t
        .send(
            Request::builder()
                .uri("/api/recipes")
                .header("sec-fetch-site", "cross-site")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = t
        .send(
            Request::builder()
                .method("POST")
                .uri("/oauth/register")
                .header(header::CONTENT_TYPE, "application/json")
                .header("sec-fetch-site", "cross-site")
                .body(Body::from(
                    r#"{"redirect_uris":["https://claude.ai/cb"],"client_name":"c"}"#,
                ))
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn mcp_checks_a_browsers_origin() {
    let t = TestApp::with_config(|c| c.site_url = Some("https://crumb.example".into()));
    let call = |origin: Option<&str>| {
        let mut req = Request::builder()
            .method("POST")
            .uri("/mcp")
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(o) = origin {
            req = req.header(header::ORIGIN, o);
        }
        req.body(Body::from(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
        ))
        .unwrap()
    };
    let (status, _, _) = t.send(call(Some("https://evil.example"))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    for ok in [
        None,
        Some("https://crumb.example"),
        Some("https://claude.ai"),
    ] {
        let (status, _, body) = t.send(call(ok)).await;
        assert_ne!(status, StatusCode::FORBIDDEN, "{ok:?}: {body}");
    }
}

#[tokio::test]
async fn shell_templates_stay_hidden_however_the_path_is_spelled() {
    let t = TestApp::new(None);
    for path in [
        "/shell/preview/index.html",
        "/%73hell/preview/index.html",
        "/%53HELL/preview/index.html",
        "/shell%2Fpreview/index.html",
    ] {
        let (status, _, body) = t.send(get(path)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(!body.contains("previewBoot"), "{path}");
    }
}

#[tokio::test]
async fn app_pages_carry_a_csp_that_allows_only_their_own_scripts() {
    let t = TestApp::new(None);
    let (status, headers, _) = t.send(get("/add")).await;
    assert_eq!(status, StatusCode::OK);
    let csp = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(csp.contains("script-src 'self'"), "{csp}");
    assert!(!csp.contains("script-src 'self' 'unsafe-inline'"));
    assert!(csp.contains("object-src 'none'") && csp.contains("frame-ancestors 'none'"));
    assert!(!headers.contains_key(header::STRICT_TRANSPORT_SECURITY));
}

#[tokio::test]
async fn hsts_is_sent_over_https_only() {
    let t = TestApp::with_config(|c| c.trust_proxy_headers = true);
    let (_, headers, _) = t
        .send(
            Request::builder()
                .uri("/api/health")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert!(
        headers[header::STRICT_TRANSPORT_SECURITY]
            .to_str()
            .unwrap()
            .starts_with("max-age=")
    );
    let (_, headers, _) = t.send(get("/api/health")).await;
    assert!(!headers.contains_key(header::STRICT_TRANSPORT_SECURITY));
}

#[test]
fn the_public_origin_ignores_forwarded_headers_unless_a_proxy_is_trusted() {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("host", "real.example".parse().unwrap());
    headers.insert("x-forwarded-host", "evil.example".parse().unwrap());
    headers.insert("x-forwarded-proto", "https".parse().unwrap());
    let mut config = Config::default();
    assert_eq!(config.public_origin(&headers), "http://real.example");
    config.trust_proxy_headers = true;
    assert_eq!(config.public_origin(&headers), "https://evil.example");
    config.site_url = Some("https://fixed.example/".into());
    assert_eq!(config.public_origin(&headers), "https://fixed.example");
}

#[test]
fn accounts_modes_refuse_to_start_without_a_fixed_address() {
    use crumb::config::AuthMode;
    let mut config = Config::default();
    assert!(config.check().is_ok(), "password mode only warns");
    config.auth_mode = AuthMode::Accounts;
    assert!(config.check().unwrap_err().contains("SITE_URL"));
    config.auth_mode = AuthMode::Hosted;
    assert!(config.check().is_err());
    config.site_url = Some("https://recipes.example".into());
    assert!(config.check().is_ok());
    config.site_url = None;
    config.railway_domain = Some("app.up.railway.app".into());
    assert!(config.check().is_ok());
}
