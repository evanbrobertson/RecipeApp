//! Wee Chef's checks of imported recipes. The server answers 409 when checks aren't
//! set up (see [`crate::Connector::wee_chef_checks`]).

use serde::Deserialize;

use crate::{ChecksStatus, Client, Error, RecipeChecks, Review, UndoneChecks};

impl Client {
    /// `GET /api/recipes/{id}/checks`: None when the recipe was never checked.
    pub async fn recipe_checks(&self, id: i64) -> Result<Option<RecipeChecks>, Error> {
        self.fetch(
            self.http
                .get(self.endpoint(&format!("api/recipes/{id}/checks"))),
        )
        .await
    }

    /// `POST /api/recipes/{id}/checks`: "Check with Wee Chef". The check comes back
    /// pending; poll [`Client::recipe_checks`] until it isn't.
    pub async fn check_recipe(&self, id: i64) -> Result<Option<RecipeChecks>, Error> {
        self.fetch(
            self.http
                .post(self.endpoint(&format!("api/recipes/{id}/checks"))),
        )
        .await
    }

    /// `POST /api/recipes/{id}/checks/undo`: puts the recipe back as it was imported.
    pub async fn undo_checks(&self, id: i64) -> Result<UndoneChecks, Error> {
        self.fetch(
            self.http
                .post(self.endpoint(&format!("api/recipes/{id}/checks/undo"))),
        )
        .await
    }

    /// `POST /api/recipes/{id}/flags/{flag}/dismiss`: "Keep as is".
    pub async fn dismiss_flag(&self, id: i64, flag: i64) -> Result<Option<RecipeChecks>, Error> {
        self.fetch(
            self.http
                .post(self.endpoint(&format!("api/recipes/{id}/flags/{flag}/dismiss"))),
        )
        .await
    }

    /// `GET /api/checks`: counts for the Suggestions page.
    pub async fn checks_status(&self) -> Result<ChecksStatus, Error> {
        self.fetch(self.http.get(self.endpoint("api/checks"))).await
    }

    /// `POST /api/checks`: "Check all recipes"; `queued` says how many started.
    pub async fn check_all(&self) -> Result<ChecksStatus, Error> {
        self.fetch(self.http.post(self.endpoint("api/checks")))
            .await
    }

    /// `GET /api/checks/review`: recipes with suggestions waiting, newest first.
    pub async fn checks_review(&self) -> Result<Vec<Review>, Error> {
        #[derive(Deserialize)]
        struct Reviews {
            recipes: Vec<Review>,
        }
        let res: Reviews = self
            .fetch(self.http.get(self.endpoint("api/checks/review")))
            .await?;
        Ok(res.recipes)
    }
}
