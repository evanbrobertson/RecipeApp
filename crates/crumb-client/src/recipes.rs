//! Recipe routes beyond reading one: saving, editing, deleting, cook log, Try next,
//! exports and file imports.

use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    Client, CookStats, Download, Error, FileImport, Popular, PopularSetting, Recipe, RecipeFormat,
    RecipeSummary, Staples, Suggestions, Trashed,
};

/// The `{id, title, isNew}` the photo import answers with.
#[derive(Deserialize)]
struct Saved {
    id: i64,
    #[serde(rename = "isNew")]
    is_new: bool,
}

impl Client {
    /// `POST /api/recipes`: saves a recipe typed in by hand, as the API's camelCase recipe
    /// JSON (`title` required; the server validates the rest). The bool is the API's `isNew`
    /// (false when its URL was already saved, and that recipe is returned).
    pub async fn create_recipe(&self, fields: &Value) -> Result<(Recipe, bool), Error> {
        let res = self
            .send(self.http.post(self.endpoint("api/recipes")).json(fields))
            .await?;
        let is_new = res.status() == reqwest::StatusCode::CREATED;
        let recipe = self.json(res, false).await?;
        Ok((recipe, is_new))
    }

    /// `PATCH /api/recipes/{id}`: only the keys in `patch` change (camelCase, as the
    /// API's recipe JSON; `null` clears a field).
    pub async fn patch_recipe(&self, id: i64, patch: &Value) -> Result<Recipe, Error> {
        self.fetch(
            self.http
                .patch(self.endpoint(&format!("api/recipes/{id}")))
                .json(patch),
        )
        .await
    }

    /// `DELETE /api/recipes/{id}`.
    pub async fn delete_recipe(&self, id: i64) -> Result<(), Error> {
        self.call(
            self.http
                .delete(self.endpoint(&format!("api/recipes/{id}"))),
        )
        .await
    }

    /// `POST /api/recipes/bulk-delete`: how many were deleted.
    pub async fn bulk_delete(&self, ids: &[i64]) -> Result<u64, Error> {
        #[derive(Deserialize)]
        struct Deleted {
            deleted: u64,
        }
        let res: Deleted = self
            .fetch(
                self.http
                    .post(self.endpoint("api/recipes/bulk-delete"))
                    .json(&json!({ "ids": ids })),
            )
            .await?;
        Ok(res.deleted)
    }

    /// `GET /api/trash`: recipes deleted in the last 30 days, newest first.
    pub async fn trash(&self) -> Result<Vec<Trashed>, Error> {
        self.fetch(self.http.get(self.endpoint("api/trash"))).await
    }

    /// `POST /api/trash/{id}/restore`: puts a deleted recipe back. The bool is the API's
    /// `isNew`: false when its link had been saved again meanwhile, and that recipe is returned.
    pub async fn restore_recipe(&self, id: i64) -> Result<(Recipe, bool), Error> {
        let value: Value = self
            .fetch(
                self.http
                    .post(self.endpoint(&format!("api/trash/{id}/restore"))),
            )
            .await?;
        let is_new = value.get("isNew").and_then(Value::as_bool).unwrap_or(true);
        let recipe = serde_json::from_value(value).map_err(|e| Error::Decode(e.to_string()))?;
        Ok((recipe, is_new))
    }

    /// `DELETE /api/trash/{id}`: deletes one recipe from the trash for good.
    pub async fn purge_trashed(&self, id: i64) -> Result<(), Error> {
        self.call(self.http.delete(self.endpoint(&format!("api/trash/{id}"))))
            .await
    }

    /// `DELETE /api/trash`: empties the trash; how many went.
    pub async fn empty_trash(&self) -> Result<u64, Error> {
        #[derive(Deserialize)]
        struct Deleted {
            deleted: u64,
        }
        let res: Deleted = self
            .fetch(self.http.delete(self.endpoint("api/trash")))
            .await?;
        Ok(res.deleted)
    }

    /// `GET /api/popular`: links several other households saved that this one hasn't.
    pub async fn popular(&self) -> Result<Popular, Error> {
        self.fetch(self.http.get(self.endpoint("api/popular")))
            .await
    }

    /// `GET /api/popular/opt-out`: whether Popular runs here, and whether this household's
    /// saved links count toward it.
    pub async fn popular_setting(&self) -> Result<PopularSetting, Error> {
        self.fetch(self.http.get(self.endpoint("api/popular/opt-out")))
            .await
    }

    /// `POST /api/popular/opt-out`: leaves this household's saved links out of Popular, or
    /// counts them again.
    pub async fn set_popular_opt_out(&self, opted_out: bool) -> Result<(), Error> {
        self.call(
            self.http
                .post(self.endpoint("api/popular/opt-out"))
                .json(&json!({ "optedOut": opted_out })),
        )
        .await
    }

    /// `GET /api/recipes/random`: Surprise me. `current` is the recipe on screen (never
    /// picked again straight away); `exclude` are ones already shown this round. An empty
    /// box is an [`Error::Api`] with status 404.
    pub async fn random_recipe(
        &self,
        current: Option<i64>,
        exclude: &[i64],
    ) -> Result<RecipeSummary, Error> {
        let mut query: Vec<(&str, String)> = Vec::new();
        if let Some(current) = current {
            query.push(("current", current.to_string()));
        }
        if !exclude.is_empty() {
            query.push(("exclude", id_list(exclude)));
        }
        self.fetch(
            self.http
                .get(self.endpoint("api/recipes/random"))
                .query(&query),
        )
        .await
    }

    /// `GET /api/suggestions?limit=&seed=&exclude=`: Try next. `seed` 0 is today's order;
    /// another seed shuffles.
    pub async fn suggestions(
        &self,
        limit: u32,
        seed: u32,
        exclude: &[i64],
    ) -> Result<Suggestions, Error> {
        let mut query = vec![("limit", limit.to_string()), ("seed", seed.to_string())];
        if !exclude.is_empty() {
            query.push(("exclude", id_list(exclude)));
        }
        self.fetch(
            self.http
                .get(self.endpoint("api/suggestions"))
                .query(&query),
        )
        .await
    }

    /// `GET /api/staples`: Wee Chef's "keep these on hand" tip, the ingredients most of
    /// the box's recipes use.
    pub async fn staples(&self) -> Result<Staples, Error> {
        self.fetch(self.http.get(self.endpoint("api/staples")))
            .await
    }

    /// `GET /api/recipes/{id}/cooked`: how often it's been cooked, and when last.
    pub async fn cook_stats(&self, id: i64) -> Result<CookStats, Error> {
        self.fetch(
            self.http
                .get(self.endpoint(&format!("api/recipes/{id}/cooked"))),
        )
        .await
    }

    /// `DELETE /api/recipes/{id}/cooked?event=`: takes back a "Cooked it" (the event from
    /// [`Client::cooked`]; without one, the latest cook).
    pub async fn undo_cooked(&self, id: i64, event: Option<i64>) -> Result<CookStats, Error> {
        let mut req = self
            .http
            .delete(self.endpoint(&format!("api/recipes/{id}/cooked")));
        if let Some(event) = event {
            req = req.query(&[("event", event)]);
        }
        self.fetch(req).await
    }

    /// `GET /api/recipes/{id}/cookbooks`: the ids of the cookbooks it's in.
    pub async fn recipe_cookbooks(&self, id: i64) -> Result<Vec<i64>, Error> {
        self.fetch(
            self.http
                .get(self.endpoint(&format!("api/recipes/{id}/cookbooks"))),
        )
        .await
    }

    /// `GET /api/recipes/{id}/export?format=json|md`.
    pub async fn export_recipe(&self, id: i64, format: RecipeFormat) -> Result<Download, Error> {
        let format = match format {
            RecipeFormat::Json => "json",
            RecipeFormat::Markdown => "md",
        };
        self.download(
            self.http
                .get(self.endpoint(&format!("api/recipes/{id}/export")))
                .query(&[("format", format)]),
        )
        .await
    }

    /// `GET /api/export`: the whole box as a backup file.
    pub async fn export_all(&self) -> Result<Download, Error> {
        self.download(self.http.get(self.endpoint("api/export")))
            .await
    }

    /// `POST /api/import/files`: backups and other apps' exports (`(file name, bytes)`),
    /// one result per file.
    pub async fn import_files(
        &self,
        files: Vec<(String, Vec<u8>)>,
    ) -> Result<Vec<FileImport>, Error> {
        let mut form = Form::new();
        for (name, bytes) in files {
            form = form.part("files", Part::bytes(bytes).file_name(name));
        }
        self.fetch(
            self.http
                .post(self.endpoint("api/import/files"))
                .multipart(form),
        )
        .await
    }

    /// `POST /api/recipes/import/photos`: Wee Chef reads a recipe from photos (`(file name,
    /// mime type, bytes)`), with optional typed `hint`. Needs [`crate::Connector::vision`].
    pub async fn import_photos(
        &self,
        photos: Vec<(String, String, Vec<u8>)>,
        hint: Option<&str>,
    ) -> Result<crate::Imported, Error> {
        let mut form = Form::new();
        for (name, mime, bytes) in photos {
            let part = Part::bytes(bytes)
                .file_name(name)
                .mime_str(&mime)
                .map_err(|err| Error::Network(err.to_string()))?;
            form = form.part("photo", part);
        }
        if let Some(hint) = hint {
            form = form.text("text", hint.to_string());
        }
        let saved: Saved = self
            .fetch(
                self.http
                    .post(self.endpoint("api/recipes/import/photos"))
                    .multipart(form),
            )
            .await?;
        Ok(crate::Imported {
            recipe: self.recipe(saved.id).await?,
            is_new: saved.is_new,
            from_video: false,
            dropped_photo: false,
            cookbook: None,
        })
    }
}

/// Ids as the comma-separated list the API's `exclude` parameter reads.
fn id_list(ids: &[i64]) -> String {
    ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
}
