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
use crate::model::{
    BOOK_COLORS, CookbookListItem, Recipe, RecipeFields, RecipePatch, RecipeSummary,
    cookbook_color, cookbook_description, cookbook_name, count_items,
};
use crate::recipes;
use crate::suggestions;

const INSTRUCTIONS: &str = "This connector is the user's personal recipe box (\"Crumb\").
- To save a recipe the user pasted or described, structure it yourself and call save_recipe. Keep every ingredient and step exactly as written.
- If the user shares only a link, call import_recipe_from_url.
- Use search_recipes to find recipes by name or ingredient, then get_recipe for the full text.
- When the user asks to tweak a saved recipe (scale it, substitute, fix steps), call update_recipe with only the changed fields.
- To tidy the library, search_recipes with `missing` finds recipes without an image, times, a category, a cookbook and so on; refresh_recipe_from_source fills blanks from the recipe's source page without touching what the user set.
- Cookbooks are the shelf: get_cookbook, update_cookbook (name, description, colour), add_to_cookbook and remove_from_cookbook organise it.
- delete_recipe and delete_cookbook first return a preview. Show it to the user and call again with confirm: true only after they agree.
- When the user asks what to cook, call suggest_recipes (or random_recipe for a surprise). When they say they cooked something, call mark_recipe_cooked.
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

/// A string-array argument, e.g. `missing` or `overwrite`; None if it isn't one.
fn string_list<'a>(a: &'a Map<String, Value>, key: &str) -> Option<Vec<&'a str>> {
    match a.get(key) {
        None => Some(Vec::new()),
        Some(Value::Array(items)) => items.iter().map(Value::as_str).collect(),
        Some(_) => None,
    }
}

/// An integer-array argument such as `recipeIds`; None unless it's a non-empty list of integers.
fn id_list(a: &Map<String, Value>, key: &str) -> Option<Vec<i64>> {
    a.get(key)
        .and_then(Value::as_array)
        .and_then(|l| l.iter().map(Value::as_i64).collect::<Option<Vec<_>>>())
        .filter(|l| !l.is_empty())
}

/// Finds a cookbook by id or case-insensitive name. `Ok(None)` is a name no cookbook has
/// yet; an unknown id or a malformed argument is an error for the caller to return.
fn resolve_book<'b>(
    books: &'b [CookbookListItem],
    arg: Option<&Value>,
) -> Result<Option<&'b CookbookListItem>, Value> {
    match arg {
        Some(Value::Number(n)) if n.as_i64().is_some() => {
            let wanted = n.as_i64().unwrap();
            books
                .iter()
                .find(|b| b.id == wanted)
                .map(Some)
                .ok_or_else(|| tool_error(format!("No cookbook with id {wanted}")))
        }
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(books
            .iter()
            .find(|b| b.name.to_lowercase() == s.trim().to_lowercase())),
        _ => Err(tool_error(
            "Input validation error: cookbook must be an id or a name",
        )),
    }
}

impl Ctx<'_> {
    fn link(&self, id: i64) -> String {
        format!("{}/recipes/{id}", self.origin)
    }

    fn book_link(&self, id: i64) -> String {
        format!("{}/cookbooks/{id}", self.origin)
    }

    fn recipe_line(&self, r: &RecipeSummary) -> String {
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
                let offset = match a.get("offset") {
                    None => 0,
                    Some(v) => match v.as_i64().filter(|o| *o >= 0) {
                        Some(o) => o,
                        None => return Some(invalid("offset must be a non-negative integer")),
                    },
                };
                let Some(missing) = string_list(a, "missing") else {
                    return Some(invalid("missing must be an array of field names"));
                };
                let rows = match recipes::search_recipes(
                    &db.lock(),
                    query,
                    &missing,
                    Some(limit),
                    Some(offset),
                ) {
                    Ok(rows) => rows,
                    Err(err) => return Some(tool_error(err.message)),
                };
                let filter = if missing.is_empty() {
                    String::new()
                } else {
                    format!(" missing {}", missing.join(" and "))
                };
                if rows.is_empty() {
                    return Some(text(match query {
                        Some(q) => format!("No recipes{filter} match \"{q}\"."),
                        None if offset > 0 => "No more recipes.".into(),
                        None if !missing.is_empty() => format!("No recipes{filter}."),
                        None => "No recipes saved yet.".into(),
                    }));
                }
                let lines: Vec<String> = rows.iter().map(|r| self.recipe_line(r)).collect();
                let more = if rows.len() as i64 == limit {
                    format!(
                        "\n\nThere may be more; call again with offset {}.",
                        offset + limit
                    )
                } else {
                    String::new()
                };
                text(format!(
                    "{} recipe(s){filter}:\n{}{more}",
                    rows.len(),
                    lines.join("\n")
                ))
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
                    Ok(f) => f.file_category(),
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
                    Ok(p) => p.file_category(),
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
                let ids = match (int("id"), a.get("ids")) {
                    (Some(id), None) => vec![id],
                    (None, Some(_)) => match id_list(a, "ids") {
                        Some(ids) => ids,
                        None => return Some(invalid("ids must be a non-empty array of integers")),
                    },
                    _ => return Some(invalid("pass either id or ids")),
                };
                let conn = db.lock();
                let mut found = Vec::new();
                for id in &ids {
                    match recipes::get_recipe(&conn, *id) {
                        Ok(Some(r)) => found.push(r),
                        Ok(None) => return Some(tool_error(format!("No recipe with id {id}"))),
                        Err(err) => return Some(tool_error(err.message)),
                    }
                }
                let titles: Vec<String> = found
                    .iter()
                    .map(|r| format!("- [{}] {}", r.id, r.title))
                    .collect();
                if a.get("confirm") != Some(&Value::Bool(true)) {
                    return Some(text(format!(
                        "Not deleted yet. This will permanently delete {} recipe(s):\n{}\n\n\
                         Show this list to the user. Only if they agree, call delete_recipe again \
                         with the same ids and confirm: true.",
                        found.len(),
                        titles.join("\n")
                    )));
                }
                match recipes::delete_recipes(&conn, &ids) {
                    Ok(n) => text(format!("Deleted {n} recipe(s):\n{}", titles.join("\n"))),
                    Err(err) => tool_error(err.message),
                }
            }
            "list_cookbooks" => match recipes::list_cookbooks(&db.lock()) {
                Ok(books) if books.is_empty() => text("No cookbooks yet."),
                Ok(books) => text(
                    books
                        .iter()
                        .map(|b| {
                            let color = b.color.as_deref().unwrap_or("no colour");
                            format!(
                                "- [{}] {} ({} recipes, {color})",
                                b.id, b.name, b.recipe_count
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                Err(err) => tool_error(err.message),
            },
            "get_cookbook" => {
                let conn = db.lock();
                let result = (|| -> Result<Value, AppError> {
                    let books = recipes::list_cookbooks(&conn)?;
                    let book = match resolve_book(&books, a.get("cookbook")) {
                        Ok(Some(b)) => b,
                        Ok(None) => return Ok(tool_error("No cookbook with that name")),
                        Err(e) => return Ok(e),
                    };
                    let full = recipes::get_cookbook(&conn, book.id)?;
                    let mut out = format!(
                        "Cookbook \"{}\" (id {}, {}) — {}\n",
                        full.name,
                        full.id,
                        full.color.as_deref().unwrap_or("no colour"),
                        self.book_link(full.id)
                    );
                    if let Some(d) = &full.description {
                        out.push_str(&format!("{d}\n"));
                    }
                    if full.recipes.is_empty() {
                        out.push_str("\nNo recipes in this cookbook yet.");
                    } else {
                        out.push_str(&format!("\n{} recipe(s):\n", full.recipes.len()));
                        let lines: Vec<String> =
                            full.recipes.iter().map(|r| self.recipe_line(r)).collect();
                        out.push_str(&lines.join("\n"));
                    }
                    Ok(text(out))
                })();
                result.unwrap_or_else(|err| tool_error(err.message))
            }
            "add_to_cookbook" => {
                let Some(ids) = id_list(a, "recipeIds") else {
                    return Some(invalid("recipeIds must be a non-empty array of integers"));
                };
                let result = (|| -> Result<Value, AppError> {
                    let conn = db.lock();
                    let books = recipes::list_cookbooks(&conn)?;
                    let (id, name) = match resolve_book(&books, a.get("cookbook")) {
                        Ok(Some(b)) => (b.id, b.name.clone()),
                        Ok(None) => {
                            let name = cookbook_name(&a["cookbook"], "Name is required")?;
                            let b = recipes::create_cookbook(&conn, &name, None, None)?;
                            (b.id, b.name)
                        }
                        Err(e) => return Ok(e),
                    };
                    let added = recipes::add_to_cookbook(&conn, id, &ids)?;
                    Ok(text(format!(
                        "Added {added} recipe(s) to \"{name}\".\n{}",
                        self.book_link(id)
                    )))
                })();
                result.unwrap_or_else(|err| tool_error(err.message))
            }
            "remove_from_cookbook" => {
                let Some(ids) = id_list(a, "recipeIds") else {
                    return Some(invalid("recipeIds must be a non-empty array of integers"));
                };
                let result = (|| -> Result<Value, AppError> {
                    let conn = db.lock();
                    let books = recipes::list_cookbooks(&conn)?;
                    let book = match resolve_book(&books, a.get("cookbook")) {
                        Ok(Some(b)) => b,
                        Ok(None) => return Ok(tool_error("No cookbook with that name")),
                        Err(e) => return Ok(e),
                    };
                    let mut removed = 0;
                    for id in &ids {
                        removed += recipes::remove_from_cookbook(&conn, book.id, *id)?;
                    }
                    Ok(text(format!(
                        "Removed {removed} recipe(s) from \"{}\". The recipes themselves are kept.\n{}",
                        book.name,
                        self.book_link(book.id)
                    )))
                })();
                result.unwrap_or_else(|err| tool_error(err.message))
            }
            "update_cookbook" => {
                let result = (|| -> Result<Value, AppError> {
                    let conn = db.lock();
                    let books = recipes::list_cookbooks(&conn)?;
                    let book = match resolve_book(&books, a.get("cookbook")) {
                        Ok(Some(b)) => b,
                        Ok(None) => return Ok(tool_error("No cookbook with that name")),
                        Err(e) => return Ok(e),
                    };
                    let validation = |e: AppError| invalid(&e.message);
                    let name = match a
                        .get("name")
                        .map(|n| cookbook_name(n, "Name can't be empty"))
                    {
                        None => None,
                        Some(Ok(n)) => Some(n),
                        Some(Err(e)) => return Ok(validation(e)),
                    };
                    if let Some(n) = &name
                        && let Some(other) = books
                            .iter()
                            .find(|b| b.id != book.id && b.name.to_lowercase() == n.to_lowercase())
                    {
                        return Ok(tool_error(format!(
                            "Another cookbook is already called \"{}\" (id {}). Move its recipes \
                             with add_to_cookbook instead, or pick a different name.",
                            other.name, other.id
                        )));
                    }
                    let description = match a.get("description").map(cookbook_description) {
                        None => None,
                        Some(Ok(d)) => Some(d),
                        Some(Err(e)) => return Ok(validation(e)),
                    };
                    let color = match a.get("color").map(cookbook_color) {
                        None => None,
                        Some(Ok(c)) => Some(c),
                        Some(Err(e)) => return Ok(validation(e)),
                    };
                    if name.is_none() && description.is_none() && color.is_none() {
                        return Ok(invalid("pass at least one of name, description or color"));
                    }
                    let b = recipes::update_cookbook(
                        &conn,
                        book.id,
                        recipes::CookbookPatch {
                            name,
                            description,
                            color,
                        },
                    )?;
                    Ok(text(format!(
                        "Updated cookbook \"{}\" (id {}, {}).\n{}",
                        b.name,
                        b.id,
                        b.color.as_deref().unwrap_or("no colour"),
                        self.book_link(b.id)
                    )))
                })();
                result.unwrap_or_else(|err| tool_error(err.message))
            }
            "delete_cookbook" => {
                let conn = db.lock();
                let books = match recipes::list_cookbooks(&conn) {
                    Ok(b) => b,
                    Err(err) => return Some(tool_error(err.message)),
                };
                let book = match resolve_book(&books, a.get("cookbook")) {
                    Ok(Some(b)) => b,
                    Ok(None) => return Some(tool_error("No cookbook with that name")),
                    Err(e) => return Some(e),
                };
                if a.get("confirm") != Some(&Value::Bool(true)) {
                    return Some(text(format!(
                        "Not deleted yet. This will delete the cookbook \"{}\" (id {}), which holds \
                         {} recipe(s). The recipes stay in the library.\n\nAsk the user to confirm, \
                         then call delete_cookbook again with confirm: true.",
                        book.name, book.id, book.recipe_count
                    )));
                }
                match recipes::delete_cookbook(&conn, book.id) {
                    Ok(()) => text(format!(
                        "Deleted the cookbook \"{}\". Its {} recipe(s) are still in the library.",
                        book.name, book.recipe_count
                    )),
                    Err(err) => tool_error(err.message),
                }
            }
            "suggest_recipes" | "random_recipe" => {
                let max_minutes = match a.get("maxMinutes") {
                    None | Some(Value::Null) => None,
                    Some(v) => match v.as_u64().filter(|m| (1..=1440).contains(m)) {
                        Some(m) => Some(m as u32),
                        None => {
                            return Some(invalid("maxMinutes must be an integer from 1 to 1440"));
                        }
                    },
                };
                let query = a
                    .get("query")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|q| !q.is_empty());
                let only = match query {
                    None => None,
                    Some(q) => {
                        match recipes::search_recipes(&db.lock(), Some(q), &[], Some(100_000), None)
                        {
                            Ok(rows) => Some(rows.iter().map(|r| r.id).collect()),
                            Err(err) => return Some(tool_error(err.message)),
                        }
                    }
                };
                let ctx = suggestions::context(self.state, &HeaderMap::new(), 0);
                let filters = match (query, max_minutes) {
                    (Some(q), Some(m)) => format!(" matching \"{q}\" in {m} minutes or less"),
                    (Some(q), None) => format!(" matching \"{q}\""),
                    (None, Some(m)) => format!(" in {m} minutes or less"),
                    (None, None) => String::new(),
                };
                if name == "random_recipe" {
                    let opts = suggestions::Options {
                        max_minutes,
                        only,
                        ..Default::default()
                    };
                    let picked = suggestions::random(self.state, &Default::default(), None, &opts)
                        .and_then(|id| {
                            recipes::summaries_by_ids(
                                &db.lock(),
                                &id.into_iter().collect::<Vec<_>>(),
                            )
                        });
                    return Some(match picked {
                        Ok(rows) if !rows.is_empty() => text(format!(
                            "How about this one?\n{}",
                            self.recipe_line(&rows[0])
                        )),
                        Ok(_) => text(format!("No recipes{filters} to pick from.")),
                        Err(err) => tool_error(err.message),
                    });
                }
                let limit = match a.get("limit") {
                    None => 5,
                    Some(v) => match v.as_u64().filter(|l| (1..=10).contains(l)) {
                        Some(l) => l as usize,
                        None => return Some(invalid("limit must be an integer from 1 to 10")),
                    },
                };
                let opts = suggestions::Options {
                    limit,
                    max_minutes,
                    only,
                    ..Default::default()
                };
                match suggestions::suggestions(self.state, ctx, &opts) {
                    Ok(s) if s.items.is_empty() => text(format!(
                        "Nothing{filters} to suggest right now (recipes cooked in the last {} days are left out).",
                        crate::suggest::COOLDOWN_DAYS
                    )),
                    Ok(s) => {
                        let lines: Vec<String> = s
                            .items
                            .iter()
                            .map(|i| {
                                let why = if i.reason.is_empty() {
                                    String::new()
                                } else {
                                    format!("\n  {}", i.reason)
                                };
                                format!("{}{why}", self.recipe_line(&i.recipe))
                            })
                            .collect();
                        let idea = s.idea.as_ref().map_or(String::new(), |i| {
                            format!(
                                "\n\nNot in the box yet, an idea from Wee Chef: {}\n  {}\n  Find a recipe: {}",
                                i.title, i.why, i.search_url
                            )
                        });
                        text(format!(
                            "Worth cooking next{filters}:\n{}{idea}",
                            lines.join("\n")
                        ))
                    }
                    Err(err) => tool_error(err.message),
                }
            }
            "mark_recipe_cooked" => {
                let Some(id) = int("id") else {
                    return Some(invalid("id must be an integer"));
                };
                let days_ago = match a.get("daysAgo") {
                    None => 0,
                    Some(v) => match v.as_i64().filter(|d| (0..=30).contains(d)) {
                        Some(d) => d,
                        None => return Some(invalid("daysAgo must be an integer from 0 to 30")),
                    },
                };
                let conn = db.lock();
                let at = crate::model::now_secs() - days_ago * crate::suggest::DAY;
                let result = recipes::log_event(&conn, id, recipes::EventKind::Cooked, at)
                    .and_then(|event| {
                        Ok((
                            event.is_some(),
                            recipes::cook_stats(&conn, id)?,
                            recipes::require_recipe(&conn, id)?,
                        ))
                    });
                match result {
                    Ok((new, stats, r)) => {
                        let when = match days_ago {
                            0 => "today".to_string(),
                            1 => "yesterday".to_string(),
                            d => format!("{d} days ago"),
                        };
                        let times = if stats.count == 1 {
                            "once".to_string()
                        } else {
                            format!("{} times", stats.count)
                        };
                        text(format!(
                            "{} \"{}\" as cooked {when}. Cooked {times} so far.\n{}",
                            if new { "Marked" } else { "Already marked" },
                            r.title,
                            self.link(r.id)
                        ))
                    }
                    Err(err) => tool_error(err.message),
                }
            }
            "refresh_recipe_from_source" => {
                let Some(id) = int("id") else {
                    return Some(invalid("id must be an integer"));
                };
                let Some(overwrite) = string_list(a, "overwrite") else {
                    return Some(invalid("overwrite must be an array of field names"));
                };
                match recipes::refresh_from_source(self.state, id, &overwrite).await {
                    Ok((r, changed)) if changed.is_empty() => text(format!(
                        "Nothing to fill in for \"{}\" (id {}); the source page had nothing new.\n{}",
                        r.title,
                        r.id,
                        self.link(r.id)
                    )),
                    Ok((r, changed)) => text(format!(
                        "Updated {} on \"{}\" (id {}) from its source page.\n{}",
                        changed.join(", "),
                        r.title,
                        r.id,
                        self.link(r.id)
                    )),
                    Err(err) => tool_error(err.message),
                }
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

/// Lists the allowed categories for Claude.
const CATEGORY_HINT: &str = "One of: Breakfast, Main, Side, Soup, Salad, Baking, Dessert, Snack, Sauce, Drink, Other. Other wording is filed under the closest one (\"Dinner\" or \"Lunch\" is Main, \"Cookies\" is Baking), or Other when nothing fits.";

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
        "recipeCategory": nullable(Some(CATEGORY_HINT), None),
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

fn cookbook_ref() -> Value {
    json!({"anyOf": [{"type": "integer"}, {"type": "string", "minLength": 1}], "description": "Cookbook id or name"})
}

fn props(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

pub fn tool_definitions() -> Vec<Value> {
    let mut update_props = Map::new();
    update_props.insert("id".into(), json!({"type": "integer"}));
    update_props.extend(recipe_properties());
    let missing: Vec<&str> = recipes::MISSING_FILTERS.iter().map(|(k, _)| *k).collect();

    vec![
        json!({
            "name": "search_recipes",
            "title": "Search recipes",
            "description": "Search saved recipes by title, ingredient, category or cuisine. Leave query empty to list the most recent recipes. Use missing to find recipes that need tidying, e.g. [\"image\"] or [\"cookbook\"] (recipes on no shelf yet).",
            "inputSchema": object(props(json!({
                "query": {"type": "string", "description": "Words to match, e.g. \"chicken lemon\""},
                "missing": {"type": "array", "items": {"type": "string", "enum": missing}, "description": "Only recipes where every listed field is empty; \"cookbook\" means not in any cookbook"},
                "limit": {"type": "integer", "minimum": 1, "maximum": 100, "default": 20},
                "offset": {"type": "integer", "minimum": 0, "default": 0}
            })), &[]),
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
            "description": "Let the app parse raw recipe text itself (e.g. a long copy-pasted web page): Wee Chef, its AI helper, when an AI key is set, otherwise a built-in parser. Prefer save_recipe when you can structure the recipe yourself.",
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
            "name": "refresh_recipe_from_source",
            "title": "Refresh recipe from its source page",
            "description": "Re-fetch a saved recipe's source URL and fill in fields that are empty (image, description, times, servings, category...). Never changes fields the user has set unless they're listed in overwrite. The title only changes if listed.",
            "inputSchema": object(props(json!({
                "id": {"type": "integer"},
                "overwrite": {"type": "array", "items": {"type": "string", "enum": recipes::REFRESHABLE}, "description": "Fields to replace from the source even if already set. Only include these when the user asked."}
            })), &["id"]),
            "annotations": {"openWorldHint": true}
        }),
        json!({
            "name": "suggest_recipes",
            "title": "Suggest what to cook",
            "description": "Recipes from the user's box worth cooking next, each with a short reason. Favours things they haven't made, variety, and what suits today; leaves out anything cooked in the last two weeks. Some days it also has one idea from Wee Chef for a dish that isn't in the box yet, with a search link.",
            "inputSchema": object(props(json!({
                "limit": {"type": "integer", "minimum": 1, "maximum": 10, "default": 5},
                "maxMinutes": {"type": "integer", "minimum": 1, "maximum": 1440, "description": "Only recipes with a known total time up to this"},
                "query": {"type": "string", "description": "Only recipes matching these words, as in search_recipes"}
            })), &[]),
            "annotations": {"readOnlyHint": true}
        }),
        json!({
            "name": "random_recipe",
            "title": "Random recipe",
            "description": "One recipe picked at random from the user's box (avoiding anything cooked in the last two weeks when possible).",
            "inputSchema": object(props(json!({
                "maxMinutes": {"type": "integer", "minimum": 1, "maximum": 1440},
                "query": {"type": "string"}
            })), &[]),
            "annotations": {"readOnlyHint": true}
        }),
        json!({
            "name": "mark_recipe_cooked",
            "title": "Mark recipe as cooked",
            "description": "Record that the user cooked a recipe (today, or daysAgo days ago), so suggestions can learn what they like and not repeat it.",
            "inputSchema": object(props(json!({
                "id": {"type": "integer"},
                "daysAgo": {"type": "integer", "minimum": 0, "maximum": 30, "default": 0}
            })), &["id"])
        }),
        json!({
            "name": "delete_recipe",
            "title": "Delete recipes",
            "description": "Permanently delete one or more recipes. Without confirm: true this only returns a preview; show it to the user and call again with confirm: true once they agree.",
            "inputSchema": object(props(json!({
                "id": {"type": "integer"},
                "ids": {"type": "array", "items": {"type": "integer"}, "minItems": 1, "description": "Several recipes at once (instead of id)"},
                "confirm": {"type": "boolean", "default": false, "description": "Set only after the user has approved the preview"}
            })), &[]),
            "annotations": {"destructiveHint": true}
        }),
        json!({
            "name": "list_cookbooks",
            "title": "List cookbooks",
            "description": "List the user's cookbooks (the shelf) with recipe counts and colours.",
            "inputSchema": object(Map::new(), &[]),
            "annotations": {"readOnlyHint": true}
        }),
        json!({
            "name": "get_cookbook",
            "title": "Get cookbook",
            "description": "Show a cookbook and the recipes in it.",
            "inputSchema": object(props(json!({"cookbook": cookbook_ref()})), &["cookbook"]),
            "annotations": {"readOnlyHint": true}
        }),
        json!({
            "name": "add_to_cookbook",
            "title": "Add recipes to cookbook",
            "description": "Add recipes to a cookbook by id or name. A cookbook with a new name is created.",
            "inputSchema": object(props(json!({
                "cookbook": cookbook_ref(),
                "recipeIds": {"type": "array", "items": {"type": "integer"}, "minItems": 1}
            })), &["cookbook", "recipeIds"])
        }),
        json!({
            "name": "remove_from_cookbook",
            "title": "Remove recipes from cookbook",
            "description": "Take recipes out of a cookbook. The recipes stay in the library and in any other cookbooks.",
            "inputSchema": object(props(json!({
                "cookbook": cookbook_ref(),
                "recipeIds": {"type": "array", "items": {"type": "integer"}, "minItems": 1}
            })), &["cookbook", "recipeIds"]),
            "annotations": {"idempotentHint": true}
        }),
        json!({
            "name": "update_cookbook",
            "title": "Update cookbook",
            "description": "Rename a cookbook, change its description, or change its cloth colour on the shelf.",
            "inputSchema": object(props(json!({
                "cookbook": cookbook_ref(),
                "name": {"type": "string", "minLength": 1, "maxLength": 100},
                "description": {"anyOf": [{"type": "string", "maxLength": 500}, {"type": "null"}]},
                "color": {"type": "string", "enum": BOOK_COLORS, "description": "Cloth colour on the shelf"}
            })), &["cookbook"]),
            "annotations": {"idempotentHint": true}
        }),
        json!({
            "name": "delete_cookbook",
            "title": "Delete cookbook",
            "description": "Delete a cookbook; its recipes stay in the library. Without confirm: true this only returns a preview; ask the user, then call again with confirm: true.",
            "inputSchema": object(props(json!({
                "cookbook": cookbook_ref(),
                "confirm": {"type": "boolean", "default": false, "description": "Set only after the user has approved"}
            })), &["cookbook"]),
            "annotations": {"destructiveHint": true}
        }),
    ]
}
