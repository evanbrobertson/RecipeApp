//! Remote MCP endpoint for the Claude connector (Streamable HTTP, stateless, JSON responses).
//! Add `https://<your-app>/mcp` as a custom connector in Claude.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing};
use serde_json::{Map, Value, json};

use crate::AppState;
use crate::error::AppError;
use crate::markdown::recipe_to_markdown;
use crate::model::{Recipe, RecipeFields, RecipePatch, count_items};
use crate::recipes;

const INSTRUCTIONS: &str = "This connector is the user's personal recipe box (\"Crumb\").
- To save a recipe the user pasted or described, structure it yourself and call save_recipe. Keep every ingredient and step exactly as written.
- If the user shares only a link, call import_recipe_from_url.
- Use search_recipes to find recipes by name or ingredient, then get_recipe for the full text.
- When the user asks to tweak a saved recipe (scale it, substitute, fix steps), call update_recipe with only the changed fields.
- Always share the recipe link returned by the tools.";

const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

pub fn routes() -> Router<AppState> {
    Router::new().route("/mcp", routing::any(handle))
}

async fn handle(
    State(state): State<AppState>,
    method: axum::http::Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let origin = state.config.public_origin(&headers);

    if !crate::oauth::has_valid_access_token(&state, &headers) {
        let mut res = (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "unauthorized", "error_description": "Connect this app to Claude to get a token"})),
        )
            .into_response();
        if let Ok(v) = HeaderValue::from_str(&format!(
            "Bearer resource_metadata=\"{origin}/.well-known/oauth-protected-resource/mcp\""
        )) {
            res.headers_mut().insert(header::WWW_AUTHENTICATE, v);
        }
        return res;
    }

    // Stateless server: no standalone SSE stream or sessions to terminate
    if method != axum::http::Method::POST {
        let mut res = (
            StatusCode::METHOD_NOT_ALLOWED,
            Json(json!({"jsonrpc": "2.0", "error": {"code": -32000, "message": "Method not allowed"}, "id": null})),
        )
            .into_response();
        res.headers_mut()
            .insert(header::ALLOW, HeaderValue::from_static("POST"));
        return res;
    }

    let Ok(message) = serde_json::from_slice::<Value>(&body) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"jsonrpc": "2.0", "error": {"code": -32700, "message": "Parse error"}, "id": null})),
        )
            .into_response();
    };

    let ctx = Ctx {
        state: &state,
        origin: &origin,
    };
    let responses: Vec<Value> = match &message {
        Value::Array(batch) => {
            let mut out = Vec::new();
            for m in batch {
                if let Some(r) = ctx.dispatch(m).await {
                    out.push(r);
                }
            }
            out
        }
        single => ctx.dispatch(single).await.into_iter().collect(),
    };

    match (message.is_array(), responses.len()) {
        (_, 0) => StatusCode::ACCEPTED.into_response(),
        (false, _) => Json(responses.into_iter().next().unwrap()).into_response(),
        (true, _) => Json(Value::Array(responses)).into_response(),
    }
}

struct Ctx<'a> {
    state: &'a AppState,
    origin: &'a str,
}

fn text(value: impl Into<String>) -> Value {
    json!({"content": [{"type": "text", "text": value.into()}]})
}

fn tool_error(message: impl Into<String>) -> Value {
    json!({"content": [{"type": "text", "text": message.into()}], "isError": true})
}

fn rpc_error(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

impl Ctx<'_> {
    fn link(&self, id: i64) -> String {
        format!("{}/recipes/{id}", self.origin)
    }

    fn saved(&self, r: &Recipe, is_new: bool) -> Value {
        text(format!(
            "{}: \"{}\" (id {})\n{}\n\n{} ingredients, {} steps.",
            if is_new { "Saved" } else { "Already saved" },
            r.title,
            r.id,
            self.link(r.id),
            count_items(&r.ingredients),
            count_items(&r.instructions),
        ))
    }

    /// Handles one JSON-RPC message; None for notifications.
    async fn dispatch(&self, msg: &Value) -> Option<Value> {
        let method = msg.get("method").and_then(Value::as_str);
        let Some(id) = msg.get("id").cloned() else {
            // Notifications (and stray responses) get no reply
            return None;
        };
        let Some(method) = method else {
            return Some(rpc_error(&id, -32600, "Invalid Request"));
        };
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        let result = match method {
            "initialize" => {
                let requested = params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let version = if PROTOCOL_VERSIONS.contains(&requested) {
                    requested
                } else {
                    PROTOCOL_VERSIONS[0]
                };
                json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {"name": "crumb", "title": "Crumb", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": INSTRUCTIONS,
                })
            }
            "ping" => json!({}),
            "tools/list" => json!({"tools": tool_definitions()}),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                let Some(result) = self.call_tool(name, &args).await else {
                    return Some(rpc_error(&id, -32602, &format!("Tool {name} not found")));
                };
                result
            }
            _ => return Some(rpc_error(&id, -32601, "Method not found")),
        };
        Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
    }

    async fn call_tool(&self, name: &str, args: &Value) -> Option<Value> {
        let empty = Map::new();
        let a = args.as_object().unwrap_or(&empty);
        let int = |k: &str| a.get(k).and_then(Value::as_i64);
        let invalid = |m: &str| tool_error(format!("Input validation error: {m}"));
        let db = &self.state.db;

        Some(match name {
            "search_recipes" => {
                let query = a
                    .get("query")
                    .and_then(Value::as_str)
                    .filter(|q| !q.is_empty());
                let limit = match a.get("limit") {
                    None => 20,
                    Some(v) => match v.as_i64().filter(|l| (1..=100).contains(l)) {
                        Some(l) => l,
                        None => return Some(invalid("limit must be an integer from 1 to 100")),
                    },
                };
                let rows = match recipes::list_recipes(&db.lock(), query, Some(limit), None) {
                    Ok(rows) => rows,
                    Err(err) => return Some(tool_error(err.message)),
                };
                if rows.is_empty() {
                    return Some(text(match query {
                        Some(q) => format!("No recipes match \"{q}\"."),
                        None => "No recipes saved yet.".into(),
                    }));
                }
                let lines: Vec<String> = rows
                    .iter()
                    .map(|r| {
                        let meta = [&r.total_time, &r.recipe_category, &r.recipe_cuisine]
                            .iter()
                            .filter_map(|v| v.as_deref().filter(|v| !v.is_empty()))
                            .collect::<Vec<_>>()
                            .join(" · ");
                        let meta = if meta.is_empty() {
                            String::new()
                        } else {
                            format!(" ({meta})")
                        };
                        format!("- [{}] {}{meta} — {}", r.id, r.title, self.link(r.id))
                    })
                    .collect();
                text(format!("{} recipe(s):\n{}", rows.len(), lines.join("\n")))
            }
            "get_recipe" => {
                let Some(id) = int("id") else {
                    return Some(invalid("id must be an integer"));
                };
                match recipes::get_recipe(&db.lock(), id) {
                    Ok(Some(r)) => text(format!(
                        "{}\n\nRecipe id: {} · {}",
                        recipe_to_markdown(&r),
                        r.id,
                        self.link(r.id)
                    )),
                    Ok(None) => tool_error(format!("No recipe with id {id}")),
                    Err(err) => tool_error(err.message),
                }
            }
            "save_recipe" => {
                let fields = match RecipeFields::from_json(args) {
                    Ok(f) => f,
                    Err(err) => return Some(invalid(&err.message)),
                };
                match recipes::create_recipe(&db.lock(), fields, "claude") {
                    Ok((r, is_new)) => self.saved(&r, is_new),
                    Err(err) => tool_error(err.message),
                }
            }
            "import_recipe_from_text" => {
                let Some(raw) = a
                    .get("text")
                    .and_then(Value::as_str)
                    .filter(|t| t.chars().count() >= 20)
                else {
                    return Some(invalid("text must be at least 20 characters"));
                };
                match recipes::import_from_text(self.state, raw, false).await {
                    Ok((r, is_new)) => self.saved(&r, is_new),
                    Err(err) => tool_error(err.message),
                }
            }
            "import_recipe_from_url" => {
                let Some(url) = a
                    .get("url")
                    .and_then(Value::as_str)
                    .filter(|u| crate::model::is_valid_url(u))
                else {
                    return Some(invalid("url must be a valid URL"));
                };
                match recipes::import_from_url(self.state, url).await {
                    Ok((r, is_new)) => self.saved(&r, is_new),
                    Err(err) => tool_error(err.message),
                }
            }
            "update_recipe" => {
                let Some(id) = int("id") else {
                    return Some(invalid("id must be an integer"));
                };
                let mut patch_input = a.clone();
                patch_input.remove("id");
                let patch = match RecipePatch::from_json(&Value::Object(patch_input)) {
                    Ok(p) => p,
                    Err(err) => return Some(invalid(&err.message)),
                };
                match recipes::update_recipe(&db.lock(), id, patch) {
                    Ok(r) => text(format!(
                        "Updated \"{}\" (id {})\n{}",
                        r.title,
                        r.id,
                        self.link(r.id)
                    )),
                    Err(err) => tool_error(err.message),
                }
            }
            "delete_recipe" => {
                let Some(id) = int("id") else {
                    return Some(invalid("id must be an integer"));
                };
                let conn = db.lock();
                match recipes::get_recipe(&conn, id) {
                    Ok(Some(r)) => match recipes::delete_recipes(&conn, &[id]) {
                        Ok(_) => text(format!("Deleted \"{}\".", r.title)),
                        Err(err) => tool_error(err.message),
                    },
                    Ok(None) => tool_error(format!("No recipe with id {id}")),
                    Err(err) => tool_error(err.message),
                }
            }
            "list_cookbooks" => match recipes::list_cookbooks(&db.lock()) {
                Ok(books) if books.is_empty() => text("No cookbooks yet."),
                Ok(books) => text(
                    books
                        .iter()
                        .map(|b| format!("- [{}] {} ({} recipes)", b.id, b.name, b.recipe_count))
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                Err(err) => tool_error(err.message),
            },
            "add_to_cookbook" => {
                let ids: Option<Vec<i64>> = a
                    .get("recipeIds")
                    .and_then(Value::as_array)
                    .and_then(|l| l.iter().map(Value::as_i64).collect());
                let Some(ids) = ids.filter(|l| !l.is_empty()) else {
                    return Some(invalid("recipeIds must be a non-empty array of integers"));
                };
                let result = (|| -> Result<Value, AppError> {
                    let conn = db.lock();
                    let books = recipes::list_cookbooks(&conn)?;
                    let (id, name) = match a.get("cookbook") {
                        Some(Value::Number(n)) if n.as_i64().is_some() => {
                            let wanted = n.as_i64().unwrap();
                            match books.iter().find(|b| b.id == wanted) {
                                Some(b) => (b.id, b.name.clone()),
                                None => {
                                    return Ok(tool_error(format!("No cookbook with id {wanted}")));
                                }
                            }
                        }
                        Some(Value::String(s)) if !s.is_empty() => {
                            match books
                                .iter()
                                .find(|b| b.name.to_lowercase() == s.trim().to_lowercase())
                            {
                                Some(b) => (b.id, b.name.clone()),
                                None => {
                                    let b = recipes::create_cookbook(&conn, s, None, None)?;
                                    (b.id, b.name)
                                }
                            }
                        }
                        _ => return Ok(invalid("cookbook must be an id or a name")),
                    };
                    let added = recipes::add_to_cookbook(&conn, id, &ids)?;
                    Ok(text(format!(
                        "Added {added} recipe(s) to \"{name}\".\n{}/cookbooks/{id}",
                        self.origin
                    )))
                })();
                result.unwrap_or_else(|err| tool_error(err.message))
            }
            _ => return None,
        })
    }
}

fn nullable(description: Option<&str>, format: Option<&str>) -> Value {
    let mut inner = json!({"type": "string"});
    if let Some(f) = format {
        inner["format"] = json!(f);
    }
    let mut v = json!({"anyOf": [inner, {"type": "null"}]});
    if let Some(d) = description {
        v["description"] = json!(d);
    }
    v
}

fn sections(description: &str) -> Value {
    json!({
        "type": "array",
        "description": description,
        "items": {
            "type": "object",
            "properties": {
                "name": {"anyOf": [{"type": "string"}, {"type": "null"}], "default": null},
                "items": {"type": "array", "items": {"type": "string"}}
            },
            "required": ["items"]
        }
    })
}

fn recipe_properties() -> Map<String, Value> {
    let v = json!({
        "title": {"type": "string", "minLength": 1, "description": "Recipe name"},
        "description": nullable(Some("One or two sentence summary"), None),
        "ingredients": sections("Ingredient groups; each item is one ingredient line with quantity"),
        "instructions": sections("Step groups; each item is one step"),
        "prepTime": nullable(Some("e.g. \"15m\""), None),
        "cookTime": nullable(Some("e.g. \"1h 10m\""), None),
        "totalTime": nullable(None, None),
        "recipeYield": nullable(Some("e.g. \"4 servings\""), None),
        "recipeCategory": nullable(Some("e.g. \"Dessert\""), None),
        "recipeCuisine": nullable(Some("e.g. \"Italian\""), None),
        "author": nullable(None, None),
        "notes": nullable(Some("Tips, substitutions, storage notes"), None),
        "url": nullable(Some("Original source URL, if known"), Some("uri")),
        "image": nullable(Some("Image URL, if known"), Some("uri")),
    });
    v.as_object().unwrap().clone()
}

fn object(properties: Map<String, Value>, required: &[&str]) -> Value {
    json!({"type": "object", "properties": properties, "required": required})
}

pub fn tool_definitions() -> Vec<Value> {
    let mut update_props = Map::new();
    update_props.insert("id".into(), json!({"type": "integer"}));
    update_props.extend(recipe_properties());

    vec![
        json!({
            "name": "search_recipes",
            "title": "Search recipes",
            "description": "Search saved recipes by title, ingredient, category or cuisine. Leave query empty to list the most recent recipes.",
            "inputSchema": object(json!({
                "query": {"type": "string", "description": "Words to match, e.g. \"chicken lemon\""},
                "limit": {"type": "integer", "minimum": 1, "maximum": 100, "default": 20}
            }).as_object().unwrap().clone(), &[]),
            "annotations": {"readOnlyHint": true}
        }),
        json!({
            "name": "get_recipe",
            "title": "Get recipe",
            "description": "Get the full recipe (ingredients, steps, notes) by id.",
            "inputSchema": object(json!({"id": {"type": "integer", "description": "Recipe id from search_recipes"}}).as_object().unwrap().clone(), &["id"]),
            "annotations": {"readOnlyHint": true}
        }),
        json!({
            "name": "save_recipe",
            "title": "Save recipe",
            "description": "Save a recipe that you have structured into ingredients and steps (for example from text the user pasted or dictated). Preserve quantities, temperatures and times exactly.",
            "inputSchema": object(recipe_properties(), &["title", "ingredients", "instructions"])
        }),
        json!({
            "name": "import_recipe_from_text",
            "title": "Import recipe from raw text",
            "description": "Let the app parse raw recipe text itself (e.g. a long copy-pasted web page). Prefer save_recipe when you can structure the recipe yourself.",
            "inputSchema": object(json!({"text": {"type": "string", "minLength": 20, "description": "The raw recipe text"}}).as_object().unwrap().clone(), &["text"])
        }),
        json!({
            "name": "import_recipe_from_url",
            "title": "Import recipe from URL",
            "description": "Fetch a recipe web page and save just the recipe. Returns the existing recipe if that URL was already saved.",
            "inputSchema": object(json!({"url": {"type": "string", "format": "uri", "description": "Recipe page URL"}}).as_object().unwrap().clone(), &["url"]),
            "annotations": {"openWorldHint": true}
        }),
        json!({
            "name": "update_recipe",
            "title": "Update recipe",
            "description": "Change fields on a saved recipe. Only send the fields that change; ingredients/instructions replace the whole list, so send the complete updated list.",
            "inputSchema": object(update_props, &["id"]),
            "annotations": {"idempotentHint": true}
        }),
        json!({
            "name": "delete_recipe",
            "title": "Delete recipe",
            "description": "Permanently delete a saved recipe. Confirm with the user first.",
            "inputSchema": object(json!({"id": {"type": "integer"}}).as_object().unwrap().clone(), &["id"]),
            "annotations": {"destructiveHint": true}
        }),
        json!({
            "name": "list_cookbooks",
            "title": "List cookbooks",
            "description": "List the user's cookbooks (collections of recipes).",
            "inputSchema": object(Map::new(), &[]),
            "annotations": {"readOnlyHint": true}
        }),
        json!({
            "name": "add_to_cookbook",
            "title": "Add recipes to cookbook",
            "description": "Add recipes to a cookbook by id or name. A cookbook with a new name is created.",
            "inputSchema": object(json!({
                "cookbook": {"anyOf": [{"type": "integer"}, {"type": "string", "minLength": 1}], "description": "Cookbook id or name"},
                "recipeIds": {"type": "array", "items": {"type": "integer"}, "minItems": 1}
            }).as_object().unwrap().clone(), &["cookbook", "recipeIds"])
        }),
    ]
}
