//! Every Crumb API call QML makes, through one singleton: `Api.call("recipe", {"id": 7})`
//! returns a request id, and `replied(id, ok, json, error)` brings the answer back on the Qt
//! thread. `qml/Requests.qml` wraps that in callbacks for the pages.
//!
//! [`dispatch`] is plain Rust over `crumb-client`'s typed methods, so tests drive it
//! against the real router. Answers are the client's own types serialized back to the
//! API's JSON, which the pages read with `JSON.parse`. Files (exports, imports, photos) are
//! read and written here, never in QML.

use std::sync::Arc;

use core::pin::Pin;

use crumb_client::{
    Client, Credentials, Error, ImportInput, Mode, Provider, RecipeFormat, ShareKind,
};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::session::{SessionCore, app_core};

fn bad(message: impl Into<String>) -> Error {
    Error::Api {
        status: 400,
        message: message.into(),
    }
}

fn to_json(value: impl Serialize) -> Result<Value, Error> {
    serde_json::to_value(value).map_err(|err| Error::Decode(err.to_string()))
}

fn int(args: &Value, key: &str) -> Result<i64, Error> {
    args.get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| bad(format!("`{key}` is missing")))
}

fn opt_int(args: &Value, key: &str) -> Option<i64> {
    args.get(key).and_then(Value::as_i64)
}

fn text<'a>(args: &'a Value, key: &str) -> Result<&'a str, Error> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| bad(format!("`{key}` is missing")))
}

fn opt_text<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

fn ids(args: &Value, key: &str) -> Vec<i64> {
    args.get(key)
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_i64).collect())
        .unwrap_or_default()
}

fn object(args: &Value, key: &str) -> Result<Value, Error> {
    args.get(key)
        .filter(|v| v.is_object())
        .cloned()
        .ok_or_else(|| bad(format!("`{key}` is missing")))
}

fn share_kind(args: &Value) -> Result<ShareKind, Error> {
    match text(args, "kind")? {
        "recipe" => Ok(ShareKind::Recipe),
        "cookbook" => Ok(ShareKind::Cookbook),
        other => Err(bad(format!("no such share kind: {other}"))),
    }
}

/// Reads the files at `paths` as `(file name, bytes)`.
async fn read_files(paths: &[String]) -> Result<Vec<(String, Vec<u8>)>, Error> {
    let mut files = Vec::new();
    for path in paths {
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|err| bad(format!("Couldn't read {path}: {err}")))?;
        let name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());
        files.push((name, bytes));
    }
    Ok(files)
}

fn paths(args: &Value) -> Vec<String> {
    args.get("paths")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(local_path)
                .collect()
        })
        .unwrap_or_default()
}

/// A `file://` URL from a QML file dialog as a local path; a path is kept as is.
pub fn local_path(input: &str) -> String {
    url::Url::parse(input)
        .ok()
        .filter(|u| u.scheme() == "file")
        .and_then(|u| u.to_file_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| input.to_string())
}

/// A photo's media type from its file name, for Wee Chef's photo import.
fn photo_type(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".heic") || lower.ends_with(".heif") {
        "image/heic"
    } else {
        "image/jpeg"
    }
}

/// Saves a download into `dir` (a folder the person picked) under its own file name, and
/// answers `{path}`.
async fn save(download: crumb_client::Download, dir: &str) -> Result<Value, Error> {
    let dir = local_path(dir);
    let name = std::path::Path::new(&download.file_name)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "crumb-export".into());
    let path = std::path::Path::new(&dir).join(name);
    tokio::fs::write(&path, &download.bytes)
        .await
        .map_err(|err| bad(format!("Couldn't save {}: {err}", path.display())))?;
    Ok(json!({ "path": path.to_string_lossy() }))
}

/// Runs one API operation. `mode` is how the server signs people in (for the account
/// calls); `progress` hears a video import's queue lines.
pub async fn dispatch(
    client: &Client,
    mode: Mode,
    op: &str,
    args: &Value,
    progress: impl FnMut(&str),
) -> Result<Value, Error> {
    let accounts = client.accounts(mode);
    match op {
        "health" => client.health().await.map(|()| Value::Null),
        "connector" => to_json(client.connector().await?),

        // Recipes
        "recipes" => to_json(
            client
                .recipes(
                    opt_text(args, "query"),
                    opt_int(args, "limit").map(|n| n as u32),
                )
                .await?,
        ),
        "recipe" => to_json(client.recipe(int(args, "id")?).await?),
        "import" => {
            let input = match (opt_text(args, "url"), opt_text(args, "text")) {
                (Some(url), _) => ImportInput::Url(url.to_string()),
                (None, Some(text)) => ImportInput::Text(text.to_string()),
                _ => return Err(bad("Paste a link or the recipe text")),
            };
            to_json(client.import_with_progress(input, progress).await?)
        }
        "importJob" => to_json(client.import_job(text(args, "id")?).await?),
        "importFiles" => to_json(client.import_files(read_files(&paths(args)).await?).await?),
        "importPhotos" => {
            let photos = read_files(&paths(args))
                .await?
                .into_iter()
                .map(|(name, bytes)| {
                    let kind = photo_type(&name).to_string();
                    (name, kind, bytes)
                })
                .collect();
            to_json(client.import_photos(photos, opt_text(args, "hint")).await?)
        }
        "createRecipe" => {
            let (recipe, is_new) = client.create_recipe(&object(args, "fields")?).await?;
            let mut out = to_json(recipe)?;
            out["isNew"] = json!(is_new);
            Ok(out)
        }
        "patchRecipe" => to_json(
            client
                .patch_recipe(int(args, "id")?, &object(args, "patch")?)
                .await?,
        ),
        "deleteRecipe" => client
            .delete_recipe(int(args, "id")?)
            .await
            .map(|()| Value::Null),
        "bulkDelete" => Ok(json!({ "deleted": client.bulk_delete(&ids(args, "ids")).await? })),
        "viewed" => client.viewed(int(args, "id")?).await.map(|()| Value::Null),
        "cooked" => to_json(client.cooked(int(args, "id")?).await?),
        "undoCooked" => to_json(
            client
                .undo_cooked(int(args, "id")?, opt_int(args, "event"))
                .await?,
        ),
        "random" => to_json(
            client
                .random_recipe(opt_int(args, "current"), &ids(args, "exclude"))
                .await?,
        ),
        "suggestions" => to_json(
            client
                .suggestions(
                    opt_int(args, "limit").unwrap_or(4) as u32,
                    opt_int(args, "seed").unwrap_or(0) as u32,
                    &ids(args, "exclude"),
                )
                .await?,
        ),
        "staples" => to_json(client.staples().await?),
        "recipeCookbooks" => to_json(client.recipe_cookbooks(int(args, "id")?).await?),
        "exportRecipe" => {
            let format = match opt_text(args, "format") {
                Some("markdown") => RecipeFormat::Markdown,
                _ => RecipeFormat::Json,
            };
            let download = client.export_recipe(int(args, "id")?, format).await?;
            save(download, text(args, "dir")?).await
        }
        "exportAll" => save(client.export_all().await?, text(args, "dir")?).await,

        // Cookbooks
        "cookbooks" => to_json(client.cookbooks().await?),
        "cookbook" => to_json(client.cookbook(int(args, "id")?).await?),
        "createCookbook" => to_json(
            client
                .create_cookbook(
                    text(args, "name")?,
                    opt_text(args, "description"),
                    opt_text(args, "color"),
                )
                .await?,
        ),
        "patchCookbook" => to_json(
            client
                .patch_cookbook(int(args, "id")?, &object(args, "patch")?)
                .await?,
        ),
        "deleteCookbook" => client
            .delete_cookbook(int(args, "id")?)
            .await
            .map(|()| Value::Null),
        "addToCookbook" => Ok(json!({
            "added": client
                .add_to_cookbook(int(args, "id")?, &ids(args, "recipeIds"))
                .await?
        })),
        "removeFromCookbook" => client
            .remove_from_cookbook(int(args, "id")?, int(args, "recipeId")?)
            .await
            .map(|()| Value::Null),
        "exportCookbook" => {
            save(
                client.export_cookbook(int(args, "id")?).await?,
                text(args, "dir")?,
            )
            .await
        }

        // Wee Chef's checks
        "recipeChecks" => to_json(client.recipe_checks(int(args, "id")?).await?),
        "checkRecipe" => to_json(client.check_recipe(int(args, "id")?).await?),
        "undoChecks" => to_json(client.undo_checks(int(args, "id")?).await?),
        "dismissFlag" => to_json(
            client
                .dismiss_flag(int(args, "id")?, int(args, "flag")?)
                .await?,
        ),
        "checksStatus" => to_json(client.checks_status().await?),
        "checkAll" => to_json(client.check_all().await?),
        "checksReview" => to_json(client.checks_review().await?),

        // Share links
        "createShare" => to_json(
            client
                .create_share(share_kind(args)?, int(args, "id")?)
                .await?,
        ),
        "updateShare" => to_json(
            client
                .update_share(
                    share_kind(args)?,
                    int(args, "id")?,
                    args.get("includeNotes")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                )
                .await?,
        ),
        "stopShare" => client
            .stop_share(share_kind(args)?, int(args, "id")?)
            .await
            .map(|()| Value::Null),
        "shares" => to_json(client.shares().await?),

        // Account and household
        "authStatus" => to_json(client.auth_status().await),
        "devices" => to_json(accounts.devices().await?),
        "signOutDevice" => accounts
            .sign_out_device(text(args, "id")?)
            .await
            .map(|()| Value::Null),
        "signOutOthers" => accounts.sign_out_others().await.map(|()| Value::Null),
        "household" => to_json(accounts.household().await?),
        "renameHousehold" => accounts
            .rename(text(args, "name")?)
            .await
            .map(|()| Value::Null),
        "invite" => to_json(accounts.invite(opt_text(args, "email")).await?),
        "cancelInvite" => accounts
            .cancel_invite(text(args, "id")?)
            .await
            .map(|()| Value::Null),
        "removeMember" => accounts
            .remove_member(text(args, "id")?)
            .await
            .map(|()| Value::Null),
        "leaveHousehold" => accounts
            .leave(text(args, "id")?)
            .await
            .map(|()| Value::Null),
        "switchHousehold" => accounts
            .switch_to(text(args, "id")?)
            .await
            .map(|()| Value::Null),
        "previewInvite" => to_json(accounts.preview_invite(text(args, "token")?).await?),
        "acceptInvite" => {
            let creds = opt_text(args, "email").map(|email| Credentials {
                name: opt_text(args, "name").map(str::to_string),
                email: email.to_string(),
                password: opt_text(args, "password").unwrap_or_default().to_string(),
            });
            to_json(
                accounts
                    .accept_invite(text(args, "token")?, creds.as_ref())
                    .await?,
            )
        }
        "requestReset" => accounts
            .request_reset(text(args, "email")?)
            .await
            .map(|()| Value::Null),
        "signInMethods" => to_json(accounts.sign_in_methods().await?),
        "unlink" => {
            let provider = match text(args, "provider")? {
                "google" => Provider::Google,
                "apple" => Provider::Apple,
                other => return Err(bad(format!("no such provider: {other}"))),
            };
            accounts.unlink(provider).await.map(|()| Value::Null)
        }
        "connectedApps" => to_json(client.connected_apps().await?),
        "disconnectApp" => client
            .disconnect_app(text(args, "id")?)
            .await
            .map(|()| Value::Null),
        "exportAccount" => save(client.export_account().await?, text(args, "dir")?).await,
        "deleteAccount" => client
            .delete_account(opt_text(args, "password"), opt_text(args, "confirm"))
            .await
            .map(|()| Value::Null),
        "changeEmail" => to_json(
            client
                .change_email(text(args, "email")?, text(args, "password")?)
                .await?,
        ),

        other => Err(bad(format!("unknown operation: {other}"))),
    }
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(i32, pending)]
        type Api = super::ApiRust;

        /// A call finished: `json` is the answer when `ok`, else `error` says why.
        #[qsignal]
        fn replied(self: Pin<&mut Api>, id: i32, ok: bool, json: QString, error: QString);

        /// A running call's progress line (a cooking video in the server's queue).
        #[qsignal]
        fn progress(self: Pin<&mut Api>, id: i32, text: QString);

        /// A call was refused with 401: the session is gone.
        #[qsignal]
        fn unauthorized(self: Pin<&mut Api>);

        /// Starts `op` with `args` (a JSON object) and returns the request id `replied` uses.
        #[qinvokable]
        fn call(self: Pin<&mut Api>, op: QString, args: QString) -> i32;

        /// A recipe photo's URL at a width, or "" when it has none.
        #[qinvokable]
        #[cxx_name = "photoUrl"]
        fn photo_url(self: &Api, recipe_id: i64, width: i32, image: QString) -> QString;

        /// The signed-in server's address, for links like a recipe's share page.
        #[qinvokable]
        #[cxx_name = "serverUrl"]
        fn server_url(self: &Api) -> QString;
    }

    impl cxx_qt::Threading for Api {}
}

/// The inner Rust struct behind the `Api` singleton.
pub struct ApiRust {
    core: Arc<Mutex<SessionCore>>,
    pending: i32,
    next_id: i32,
}

impl Default for ApiRust {
    fn default() -> Self {
        Self {
            core: app_core(),
            pending: 0,
            next_id: 0,
        }
    }
}

impl qobject::Api {
    pub fn call(mut self: Pin<&mut Self>, op: QString, args: QString) -> i32 {
        let id = self.next_id.wrapping_add(1);
        self.as_mut().rust_mut().next_id = id;
        let pending = self.pending + 1;
        self.as_mut().set_pending(pending);

        let op = op.to_string();
        let args: Value = serde_json::from_str(&args.to_string()).unwrap_or(Value::Null);
        let core = self.core.clone();
        let qt_thread = self.as_mut().qt_thread();
        drop(crate::runtime::spawn(async move {
            let (client, mode) = {
                let core = core.lock().await;
                (core.client(), core.mode())
            };
            let progress_thread = qt_thread.clone();
            let progress = move |line: &str| {
                let line = QString::from(line);
                let _ = progress_thread.queue(move |object| object.progress(id, line));
            };
            let result = match client {
                Some(client) => dispatch(&client, mode, &op, &args, progress).await,
                None => Err(Error::Unauthorized),
            };
            let _ = qt_thread.queue(move |mut object| {
                let pending = (object.pending - 1).max(0);
                object.as_mut().set_pending(pending);
                match result {
                    Ok(value) => {
                        let json = QString::from(&value.to_string());
                        object.replied(id, true, json, QString::default());
                    }
                    Err(err) => {
                        let unauthorized = matches!(err, Error::Unauthorized);
                        let message = QString::from(&err.to_string());
                        object
                            .as_mut()
                            .replied(id, false, QString::default(), message);
                        if unauthorized {
                            object.unauthorized();
                        }
                    }
                }
            });
        }));
        id
    }

    pub fn photo_url(&self, recipe_id: i64, width: i32, image: QString) -> QString {
        let image = image.to_string();
        if image.is_empty() {
            return QString::default();
        }
        let Ok(core) = self.core.try_lock() else {
            return QString::default();
        };
        core.client()
            .map(|client| QString::from(&client.image_url(recipe_id, width.max(1) as u32, &image)))
            .unwrap_or_default()
    }

    pub fn server_url(&self) -> QString {
        self.core
            .try_lock()
            .map(|core| QString::from(core.server()))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_server::TestServer;

    async fn call(client: &Client, op: &str, args: Value) -> Value {
        dispatch(client, Mode::Password, op, &args, |_| {})
            .await
            .unwrap_or_else(|err| panic!("{op}: {err}"))
    }

    #[tokio::test]
    async fn recipes_and_cookbooks_round_trip_as_the_apis_json() {
        let server = TestServer::start(None).await;
        let client = Client::new(&server.origin).unwrap();

        let made = call(
            &client,
            "createRecipe",
            json!({"fields": {
                "title": "Soup",
                "ingredients": [{"name": null, "items": ["1 onion"]}],
                "instructions": [{"name": null, "items": ["Boil."]}],
            }}),
        )
        .await;
        assert_eq!(made["isNew"], true);
        let id = made["id"].as_i64().unwrap();
        assert_eq!(
            call(&client, "recipe", json!({"id": id})).await["title"],
            "Soup"
        );
        let list = call(&client, "recipes", json!({"query": "soup"})).await;
        assert_eq!(list[0]["id"], id);

        let cooked = call(&client, "cooked", json!({"id": id})).await;
        assert_eq!(cooked["count"], 1);
        let undone = call(
            &client,
            "undoCooked",
            json!({"id": id, "event": cooked["eventId"]}),
        )
        .await;
        assert_eq!(undone["count"], 0);

        let book = call(&client, "createCookbook", json!({"name": "Weeknights"})).await;
        let book_id = book["id"].as_i64().unwrap();
        call(
            &client,
            "addToCookbook",
            json!({"id": book_id, "recipeIds": [id]}),
        )
        .await;
        let open = call(&client, "cookbook", json!({"id": book_id})).await;
        assert_eq!(open["recipes"][0]["id"], id);
        assert_eq!(
            call(&client, "recipeCookbooks", json!({"id": id})).await,
            json!([book_id])
        );

        let dir = tempfile::tempdir().unwrap();
        let saved = call(
            &client,
            "exportRecipe",
            json!({"id": id, "format": "markdown", "dir": dir.path()}),
        )
        .await;
        let path = saved["path"].as_str().unwrap();
        assert!(std::fs::read_to_string(path).unwrap().contains("Soup"));

        let status = call(&client, "authStatus", json!({})).await;
        assert_eq!(status["mode"], "password");
        call(&client, "deleteRecipe", json!({"id": id})).await;
        assert_eq!(call(&client, "recipes", json!({})).await, json!([]));
    }

    #[tokio::test]
    async fn bad_calls_say_what_is_missing() {
        let server = TestServer::start(None).await;
        let client = Client::new(&server.origin).unwrap();
        let err = dispatch(&client, Mode::Password, "recipe", &json!({}), |_| {})
            .await
            .unwrap_err();
        assert_eq!(err.to_string(), "`id` is missing");
        let err = dispatch(&client, Mode::Password, "nope", &json!({}), |_| {})
            .await
            .unwrap_err();
        assert_eq!(err.to_string(), "unknown operation: nope");
    }

    #[test]
    fn dialog_urls_become_paths() {
        assert_eq!(local_path("file:///home/a/b%20c.json"), "/home/a/b c.json");
        assert_eq!(local_path("/home/a/x"), "/home/a/x");
        assert_eq!(photo_type("IMG_1.HEIC"), "image/heic");
        assert_eq!(photo_type("a.jpg"), "image/jpeg");
    }
}
