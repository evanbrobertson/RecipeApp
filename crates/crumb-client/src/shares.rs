//! Public share links for a recipe or a cookbook.

use serde_json::json;

use crate::{Client, Error, Share, ShareKind, SharedLink};

impl Client {
    /// `POST /api/{recipes|cookbooks}/{id}/share`: the link, made on first use (asking
    /// again returns the same one).
    pub async fn create_share(&self, kind: ShareKind, id: i64) -> Result<Share, Error> {
        self.fetch(self.http.post(self.endpoint(&share_path(kind, id))))
            .await
    }

    /// `PATCH /api/{recipes|cookbooks}/{id}/share`: whether the cook's notes are shown.
    pub async fn update_share(
        &self,
        kind: ShareKind,
        id: i64,
        include_notes: bool,
    ) -> Result<Share, Error> {
        self.fetch(
            self.http
                .patch(self.endpoint(&share_path(kind, id)))
                .json(&json!({ "includeNotes": include_notes })),
        )
        .await
    }

    /// `DELETE /api/{recipes|cookbooks}/{id}/share`: the link stops working.
    pub async fn stop_share(&self, kind: ShareKind, id: i64) -> Result<(), Error> {
        self.call(self.http.delete(self.endpoint(&share_path(kind, id))))
            .await
    }

    /// `GET /api/shares`: every live link, newest first.
    pub async fn shares(&self) -> Result<Vec<SharedLink>, Error> {
        self.fetch(self.http.get(self.endpoint("api/shares"))).await
    }
}

fn share_path(kind: ShareKind, id: i64) -> String {
    match kind {
        ShareKind::Recipe => format!("api/recipes/{id}/share"),
        ShareKind::Cookbook => format!("api/cookbooks/{id}/share"),
    }
}
