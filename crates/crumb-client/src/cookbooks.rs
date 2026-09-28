//! Cookbook routes: one book, creating, editing and deleting books, filing recipes.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{Client, Cookbook, CookbookWithRecipes, Download, Error};

impl Client {
    /// `GET /api/cookbooks/{id}`: the book and its recipes.
    pub async fn cookbook(&self, id: i64) -> Result<CookbookWithRecipes, Error> {
        self.fetch(self.http.get(self.endpoint(&format!("api/cookbooks/{id}"))))
            .await
    }

    /// `POST /api/cookbooks`. `color` is one of `crumb_core::model::BOOK_COLORS`.
    pub async fn create_cookbook(
        &self,
        name: &str,
        description: Option<&str>,
        color: Option<&str>,
    ) -> Result<Cookbook, Error> {
        self.fetch(
            self.http
                .post(self.endpoint("api/cookbooks"))
                .json(&json!({ "name": name, "description": description, "color": color })),
        )
        .await
    }

    /// `PATCH /api/cookbooks/{id}`: only the keys in `patch` change (`name`,
    /// `description`, `color`; a null description clears it).
    pub async fn patch_cookbook(&self, id: i64, patch: &Value) -> Result<Cookbook, Error> {
        self.fetch(
            self.http
                .patch(self.endpoint(&format!("api/cookbooks/{id}")))
                .json(patch),
        )
        .await
    }

    /// `DELETE /api/cookbooks/{id}`. Its recipes stay in the box.
    pub async fn delete_cookbook(&self, id: i64) -> Result<(), Error> {
        self.call(
            self.http
                .delete(self.endpoint(&format!("api/cookbooks/{id}"))),
        )
        .await
    }

    /// `POST /api/cookbooks/{id}/recipes`: how many weren't in the book already.
    pub async fn add_to_cookbook(&self, id: i64, recipe_ids: &[i64]) -> Result<u64, Error> {
        #[derive(Deserialize)]
        struct Added {
            added: u64,
        }
        let res: Added = self
            .fetch(
                self.http
                    .post(self.endpoint(&format!("api/cookbooks/{id}/recipes")))
                    .json(&json!({ "recipeIds": recipe_ids })),
            )
            .await?;
        Ok(res.added)
    }

    /// `DELETE /api/cookbooks/{id}/recipes/{recipe_id}`.
    pub async fn remove_from_cookbook(&self, id: i64, recipe_id: i64) -> Result<(), Error> {
        self.call(
            self.http
                .delete(self.endpoint(&format!("api/cookbooks/{id}/recipes/{recipe_id}"))),
        )
        .await
    }

    /// `GET /api/cookbooks/{id}/export`: the book in the backup format.
    pub async fn export_cookbook(&self, id: i64) -> Result<Download, Error> {
        self.download(
            self.http
                .get(self.endpoint(&format!("api/cookbooks/{id}/export"))),
        )
        .await
    }
}
