//! The JSON API under /api, same routes and shapes as the Nuxt server had.

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing};
use serde_json::{Value, json};
use std::collections::HashMap;

use crate::AppState;
use crate::auth;
use crate::checks;
use crate::error::{AppError, AppResult};
use crate::model::{
    RecipeFields, RecipePatch, cookbook_color, cookbook_description, cookbook_name, now_secs,
};
use crate::recipes::{self, CookbookPatch, EventKind, ImportSummary};
use crate::suggestions;

const MAX_FILE_BYTES: usize = 50 * 1024 * 1024;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/health", routing::get(health))
        .route("/api/auth/login", routing::post(login))
        .route("/api/auth/logout", routing::post(logout))
        .route("/api/connector", routing::get(connector))
        .route("/api/export", routing::get(export))
        .route(
            "/api/import/files",
            routing::post(import_files).layer(DefaultBodyLimit::max(10 * MAX_FILE_BYTES)),
        )
        .route(
            "/api/recipes",
            routing::get(list_recipes).post(create_recipe),
        )
        .route("/api/recipes/import", routing::post(import_recipe))
        .route(
            "/api/recipes/import/photos",
            routing::post(import_photos)
                .layer(DefaultBodyLimit::max(crate::photos::MAX_BODY_BYTES)),
        )
        .route("/api/recipes/bulk-delete", routing::post(bulk_delete))
        .route("/api/recipes/random", routing::get(random_recipe))
        .route("/api/suggestions", routing::get(suggestions))
        .route(
            "/api/recipes/{id}",
            routing::get(get_recipe)
                .patch(patch_recipe)
                .delete(delete_recipe),
        )
        .route(
            "/api/recipes/{id}/cookbooks",
            routing::get(recipe_cookbooks),
        )
        .route("/api/recipes/{id}/viewed", routing::post(recipe_viewed))
        .route("/api/recipes/{id}/checks", routing::get(recipe_checks))
        .route("/api/recipes/{id}/checks/undo", routing::post(undo_checks))
        .route(
            "/api/recipes/{id}/flags/{flag}/dismiss",
            routing::post(dismiss_flag),
        )
        .route("/api/checks", routing::get(checks_status).post(check_all))
        .route("/api/checks/review", routing::get(checks_review))
        .route(
            "/api/recipes/{id}/cooked",
            routing::post(recipe_cooked).delete(undo_cooked),
        )
        .route(
            "/api/cookbooks",
            routing::get(list_cookbooks).post(create_cookbook),
        )
        .route(
            "/api/cookbooks/{id}",
            routing::get(get_cookbook)
                .patch(patch_cookbook)
                .delete(delete_cookbook),
        )
        .route(
            "/api/cookbooks/{id}/recipes",
            routing::post(add_to_cookbook),
        )
        .route(
            "/api/cookbooks/{id}/recipes/{recipe_id}",
            routing::delete(remove_from_cookbook),
        )
}

pub fn id_param(raw: &str, name: &str) -> AppResult<i64> {
    raw.parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| AppError::bad_request(format!("Invalid {name}")))
}

fn json_body(body: &Bytes) -> AppResult<Value> {
    serde_json::from_slice(body).map_err(|_| AppError::bad_request("Invalid JSON body"))
}

fn ids_field(body: &Value, key: &str) -> AppResult<Vec<i64>> {
    let ids: Option<Vec<i64>> = body
        .get(key)
        .and_then(Value::as_array)
        .and_then(|a| a.iter().map(|v| v.as_i64().filter(|i| *i > 0)).collect());
    ids.filter(|ids| (1..=1000).contains(&ids.len()))
        .ok_or_else(|| {
            AppError::bad_request(format!("{key}: expected 1 to 1000 positive integer ids"))
        })
}

async fn health(State(state): State<AppState>) -> AppResult<Json<Value>> {
    state.db.lock().query_row("select 1", [], |_| Ok(()))?;
    Ok(Json(json!({"ok": true})))
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Response> {
    let body = json_body(&body)?;
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .filter(|p| !p.is_empty())
        .ok_or_else(|| AppError::bad_request("password: Password is required"))?;
    if !state.config.auth_enabled() {
        return Ok(Json(json!({"ok": true})).into_response());
    }
    if !auth::check_password(&state.config, password) {
        // Slow down guessing
        tokio::time::sleep(std::time::Duration::from_millis(750)).await;
        return Err(AppError::new(401, "Incorrect password"));
    }
    let mut res = Json(json!({"ok": true})).into_response();
    if let Some(cookie) = auth::login_cookie(&state.config, &headers) {
        res.headers_mut().append(header::SET_COOKIE, cookie);
    }
    Ok(res)
}

async fn logout(headers: HeaderMap) -> Response {
    let mut res = Json(json!({"ok": true})).into_response();
    res.headers_mut()
        .append(header::SET_COOKIE, auth::logout_cookie(&headers));
    res
}

pub fn connector_info(state: &AppState, headers: &HeaderMap) -> Value {
    json!({
        "mcpUrl": format!("{}/mcp", state.config.public_origin(headers)),
        "authEnabled": state.config.auth_enabled(),
        // Wee Chef (the AI helper) is on whenever a key is configured
        "weeChef": crate::llm::available(state),
        // Older name for weeChef
        "claudeParsing": crate::llm::available(state),
        // The API behind Wee Chef, for diagnostics; the UI says "Wee Chef"
        "aiProvider": crate::llm::provider_label(state),
        // Wee Chef reads recipe photos; without it the browser runs OCR itself
        "vision": crate::llm::available(state),
        "browserScraping": state.browser.available(),
        // Wee Chef checks imported recipes (a TypeSafe key is configured)
        "weeChefChecks": crate::checks::enabled(state),
    })
}

async fn connector(State(state): State<AppState>, headers: HeaderMap) -> Json<Value> {
    Json(connector_info(&state, &headers))
}

async fn export(State(state): State<AppState>) -> AppResult<Response> {
    let backup = recipes::export_backup(&state.db.lock())?;
    let date = chrono::Utc::now().format("%Y-%m-%d");
    let body = serde_json::to_string_pretty(&backup).map_err(AppError::internal)?;
    let mut res = body.into_response();
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"crumb-{date}.json\"")) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    Ok(res)
}

// Multipart upload of export files (JTR PDFs, Paprika, JSON, HTML, text, zip)
async fn import_files(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> AppResult<Json<Vec<ImportSummary>>> {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(format!("Invalid upload ({e})")))?
    {
        let Some(name) = field.file_name().map(String::from) else {
            continue;
        };
        let data = field
            .bytes()
            .await
            .map_err(|e| AppError::bad_request(format!("Invalid upload ({e})")))?;
        if !data.is_empty() {
            files.push((name, data.to_vec()));
        }
    }
    if files.is_empty() {
        return Err(AppError::bad_request("No files uploaded"));
    }
    let mut results = Vec::new();
    for (name, data) in files {
        if data.len() > MAX_FILE_BYTES {
            results.push(ImportSummary::failed(&name, "File is over 50 MB"));
            continue;
        }
        results.push(recipes::import_file(&state, &name, data).await);
    }
    Ok(Json(results))
}

async fn list_recipes(
    State(state): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> AppResult<Json<Value>> {
    let query = q.get("q").map(String::as_str);
    if query.is_some_and(|s| s.chars().count() > 200) {
        return Err(AppError::bad_request("q: Too long (200 characters max)"));
    }
    let limit = match q.get("limit") {
        None => None,
        Some(raw) => Some(
            raw.trim()
                .parse::<f64>()
                .ok()
                .filter(|n| n.fract() == 0.0 && (1.0..=500.0).contains(n))
                .map(|n| n as i64)
                .ok_or_else(|| AppError::bad_request("limit: expected an integer from 1 to 500"))?,
        ),
    };
    let rows = recipes::list_recipes(&state.db.lock(), query, limit, None)?;
    Ok(Json(recipes::to_value(&rows)))
}

async fn create_recipe(State(state): State<AppState>, body: Bytes) -> AppResult<Response> {
    let fields = RecipeFields::from_json(&json_body(&body)?)?;
    let (recipe, is_new) = recipes::create_recipe(&state.db.lock(), fields, "manual")?;
    let status = if is_new {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(recipes::with_is_new(&recipe, is_new))).into_response())
}

async fn import_recipe(State(state): State<AppState>, body: Bytes) -> AppResult<Json<Value>> {
    let body = json_body(&body)?;
    let (recipe, is_new) = if let Some(url) = body.get("url") {
        let url = url
            .as_str()
            .filter(|u| crate::model::is_valid_url(u))
            .ok_or_else(|| AppError::bad_request("Please enter a valid URL"))?;
        recipes::import_from_url(&state, url).await?
    } else if let Some(text) = body.get("text") {
        let text = text
            .as_str()
            .ok_or_else(|| AppError::bad_request("Paste a bit more of the recipe"))?
            .trim();
        if text.chars().count() < 10 {
            return Err(AppError::bad_request("Paste a bit more of the recipe"));
        }
        if text.chars().count() > 100_000 {
            return Err(AppError::bad_request(
                "That's too much text (100,000 characters max)",
            ));
        }
        recipes::import_from_text(&state, text, true).await?
    } else {
        return Err(AppError::bad_request("Paste a link or the recipe text"));
    };
    Ok(Json(
        json!({"id": recipe.id, "title": recipe.title, "isNew": is_new}),
    ))
}

/// A multipart error as the status axum gives it (413 past the body limit).
fn upload_error(err: axum::extract::multipart::MultipartError) -> AppError {
    let status = err.status();
    if status == StatusCode::PAYLOAD_TOO_LARGE {
        let mb = crate::photos::MAX_BODY_BYTES / (1024 * 1024);
        return AppError::new(
            413,
            format!("Those photos are too big together ({mb} MB max)"),
        );
    }
    AppError::new(
        status.as_u16(),
        format!("Invalid upload ({})", err.body_text()),
    )
}

// Multipart: 1 to 6 `photo` fields (the pages, in order) and an optional `text` note
async fn import_photos(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> AppResult<Json<Value>> {
    if !crate::llm::available(&state) {
        return Err(AppError::bad_request(
            "Wee Chef isn't set up here, so photos are read on your device instead.",
        ));
    }
    let mut photos: Vec<Vec<u8>> = Vec::new();
    let mut hint: Option<String> = None;
    while let Some(field) = multipart.next_field().await.map_err(upload_error)? {
        match field.name() {
            Some("photo") => {
                if photos.len() == crate::photos::MAX_PHOTOS {
                    return Err(AppError::bad_request(format!(
                        "Up to {} photos at a time",
                        crate::photos::MAX_PHOTOS
                    )));
                }
                let data = field.bytes().await.map_err(upload_error)?;
                if !data.is_empty() {
                    photos.push(data.to_vec());
                }
            }
            Some("text") => {
                let text = field.text().await.map_err(upload_error)?;
                hint = Some(text.trim().chars().take(2000).collect());
            }
            _ => {}
        }
    }
    let (recipe, is_new) = crate::photos::import(&state, photos, hint.as_deref()).await?;
    Ok(Json(
        json!({"id": recipe.id, "title": recipe.title, "isNew": is_new}),
    ))
}

async fn bulk_delete(State(state): State<AppState>, body: Bytes) -> AppResult<Json<Value>> {
    let ids = ids_field(&json_body(&body)?, "ids")?;
    let deleted = recipes::delete_recipes(&state.db.lock(), &ids)?;
    Ok(Json(json!({"deleted": deleted})))
}

async fn get_recipe(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let recipe = recipes::require_recipe(&state.db.lock(), id_param(&id, "id")?)?;
    Ok(Json(recipes::to_value(&recipe)))
}

async fn patch_recipe(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> AppResult<Json<Value>> {
    let id = id_param(&id, "id")?;
    let patch = RecipePatch::from_json(&json_body(&body)?)?;
    let recipe = recipes::update_recipe(&state.db.lock(), id, patch)?;
    Ok(Json(recipes::to_value(&recipe)))
}

async fn delete_recipe(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let id = id_param(&id, "id")?;
    if recipes::delete_recipes(&state.db.lock(), &[id])? == 0 {
        return Err(AppError::not_found("Recipe not found"));
    }
    Ok(Json(json!({"ok": true})))
}

async fn recipe_cookbooks(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<i64>>> {
    Ok(Json(recipes::recipe_cookbook_ids(
        &state.db.lock(),
        id_param(&id, "id")?,
    )?))
}

fn int_param(
    q: &HashMap<String, String>,
    key: &str,
    range: std::ops::RangeInclusive<u64>,
    default: u64,
) -> AppResult<u64> {
    match q.get(key) {
        None => Ok(default),
        Some(raw) => raw
            .trim()
            .parse::<u64>()
            .ok()
            .filter(|n| range.contains(n))
            .ok_or_else(|| {
                AppError::bad_request(format!(
                    "{key}: expected an integer from {} to {}",
                    range.start(),
                    range.end()
                ))
            }),
    }
}

async fn suggestions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> AppResult<Json<Value>> {
    let limit = int_param(&q, "limit", 1..=12, 4)? as usize;
    let seed = int_param(&q, "seed", 0..=1_000_000, 0)?;
    let ctx = suggestions::context(&state, &headers, seed);
    let opts = suggestions::Options {
        limit,
        exclude: suggestions::id_set(q.get("exclude")),
        may_call_ai: true,
        ..Default::default()
    };
    Ok(Json(recipes::to_value(&suggestions::suggestions(
        &state, ctx, &opts,
    )?)))
}

async fn random_recipe(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> AppResult<Json<Value>> {
    suggestions::zone(&state, &headers);
    let current = q.get("current").and_then(|c| c.parse().ok());
    let id = suggestions::random(
        &state,
        &suggestions::id_set(q.get("exclude")),
        current,
        &Default::default(),
    )?
    .ok_or_else(|| AppError::not_found("No recipes saved yet"))?;
    let summary = recipes::summaries_by_ids(&state.db.lock(), &[id])?;
    let first = summary
        .first()
        .ok_or_else(|| AppError::not_found("Recipe not found"))?;
    Ok(Json(recipes::to_value(first)))
}

async fn recipe_viewed(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let id = id_param(&id, "id")?;
    recipes::log_event(&state.db.lock(), id, EventKind::Viewed, now_secs())?;
    Ok(StatusCode::NO_CONTENT)
}

async fn recipe_cooked(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let id = id_param(&id, "id")?;
    let conn = state.db.lock();
    // eventId is null when this cook was already logged (nothing to undo)
    let event = recipes::log_event(&conn, id, EventKind::Cooked, now_secs())?;
    let mut stats = recipes::to_value(&recipes::cook_stats(&conn, id)?);
    stats["eventId"] = json!(event);
    Ok(Json(stats))
}

async fn undo_cooked(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> AppResult<Json<Value>> {
    let id = id_param(&id, "id")?;
    let event = q.get("event").map(|e| id_param(e, "event")).transpose()?;
    Ok(Json(recipes::to_value(&recipes::undo_cooked(
        &state.db.lock(),
        id,
        event,
    )?)))
}

async fn list_cookbooks(State(state): State<AppState>) -> AppResult<Json<Value>> {
    Ok(Json(recipes::to_value(&recipes::list_cookbooks(
        &state.db.lock(),
    )?)))
}

async fn create_cookbook(State(state): State<AppState>, body: Bytes) -> AppResult<Response> {
    let body = json_body(&body)?;
    let name = cookbook_name(body.get("name").unwrap_or(&Value::Null), "Name is required")?;
    let description = body
        .get("description")
        .map(cookbook_description)
        .transpose()?
        .flatten();
    let color = match body.get("color") {
        None | Some(Value::Null) => None,
        Some(c) => Some(cookbook_color(c)?),
    };
    let book = recipes::create_cookbook(
        &state.db.lock(),
        &name,
        description.as_deref(),
        color.as_deref(),
    )?;
    Ok((StatusCode::CREATED, Json(recipes::to_value(&book))).into_response())
}

async fn get_cookbook(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let book = recipes::get_cookbook(&state.db.lock(), id_param(&id, "id")?)?;
    Ok(Json(recipes::to_value(&book)))
}

async fn patch_cookbook(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> AppResult<Json<Value>> {
    let id = id_param(&id, "id")?;
    let body = json_body(&body)?;
    let patch = CookbookPatch {
        name: body
            .get("name")
            .map(|n| cookbook_name(n, "Name is required"))
            .transpose()?,
        description: body
            .get("description")
            .map(cookbook_description)
            .transpose()?,
        color: body.get("color").map(cookbook_color).transpose()?,
    };
    let book = recipes::update_cookbook(&state.db.lock(), id, patch)?;
    Ok(Json(recipes::to_value(&book)))
}

async fn delete_cookbook(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    recipes::delete_cookbook(&state.db.lock(), id_param(&id, "id")?)?;
    Ok(Json(json!({"ok": true})))
}

async fn add_to_cookbook(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> AppResult<Json<Value>> {
    let id = id_param(&id, "id")?;
    let ids = ids_field(&json_body(&body)?, "recipeIds")?;
    let added = recipes::add_to_cookbook(&state.db.lock(), id, &ids)?;
    Ok(Json(json!({"added": added})))
}

async fn remove_from_cookbook(
    State(state): State<AppState>,
    Path((id, recipe_id)): Path<(String, String)>,
) -> AppResult<Json<Value>> {
    let conn = state.db.lock();
    recipes::remove_from_cookbook(
        &conn,
        id_param(&id, "id")?,
        id_param(&recipe_id, "recipeId")?,
    )?;
    Ok(Json(json!({"ok": true})))
}

// ─── Wee Chef's import checks ───────────────────────────────────────────────

async fn recipe_checks(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let id = id_param(&id, "id")?;
    let conn = state.db.lock();
    recipes::require_recipe(&conn, id)?;
    Ok(Json(checks::for_recipe(&conn, id)?))
}

async fn undo_checks(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let id = id_param(&id, "id")?;
    let conn = state.db.lock();
    let checks = checks::undo(&conn, id)?;
    let recipe = recipes::require_recipe(&conn, id)?;
    Ok(Json(
        json!({"recipe": recipes::to_value(&recipe), "checks": checks}),
    ))
}

async fn dismiss_flag(
    State(state): State<AppState>,
    Path((id, flag)): Path<(String, String)>,
) -> AppResult<Json<Value>> {
    let (id, flag) = (id_param(&id, "id")?, id_param(&flag, "flag")?);
    Ok(Json(checks::dismiss(&state.db.lock(), id, flag)?))
}

async fn checks_status(State(state): State<AppState>) -> AppResult<Json<Value>> {
    Ok(Json(checks::status(&state, &state.db.lock())?))
}

async fn checks_review(State(state): State<AppState>) -> AppResult<Json<Value>> {
    Ok(Json(
        json!({"recipes": checks::to_review(&state.db.lock())?}),
    ))
}

async fn check_all(State(state): State<AppState>) -> AppResult<Json<Value>> {
    Ok(Json(checks::check_all(&state)?))
}
