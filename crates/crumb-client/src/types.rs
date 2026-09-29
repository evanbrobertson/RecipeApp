//! Response shapes for routes whose JSON has no matching `crumb_core` type. Field names
//! follow the server's camelCase JSON; each type names the handler that builds it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crumb_core::suggest::ReasonKind;

use crate::{Recipe, RecipeSummary};

/// How often a recipe has been cooked (`recipes::CookStats`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CookStats {
    pub count: i64,
    /// ISO-8601, or None when never cooked.
    pub last_cooked_at: Option<String>,
}

/// `POST /api/recipes/{id}/cooked`: the new stats and the event to undo, which is None
/// when this cook was already logged within the last few hours.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cooked {
    #[serde(flatten)]
    pub stats: CookStats,
    pub event_id: Option<i64>,
}

/// `GET /api/suggestions` (`suggestions::Suggestions`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestions {
    pub items: Vec<Suggestion>,
    pub ai: AiStatus,
    /// Wee Chef's dish that isn't in the box yet, on idea days.
    pub idea: Option<Idea>,
}

/// One Try next pick and why it was chosen.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub recipe: RecipeSummary,
    pub reason: String,
    pub reason_kind: ReasonKind,
    /// The reason was written by Wee Chef.
    pub ai: bool,
}

/// Whether Wee Chef's re-rank is in the list, still running, or off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiStatus {
    Ready,
    /// Ask again in a few seconds.
    Pending,
    Off,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Idea {
    pub title: String,
    pub why: String,
    /// A web search for the dish, to find a page to import.
    pub search_url: String,
}

/// `GET /api/connector` (`api::connector_info`): what this server can do.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connector {
    pub mcp_url: String,
    pub auth_enabled: bool,
    pub wee_chef: bool,
    /// Wee Chef reads recipe photos (`POST /api/recipes/import/photos`).
    pub vision: bool,
    pub browser_scraping: bool,
    pub wee_chef_checks: bool,
}

/// A downloaded file: an export of a recipe, a cookbook or the whole box.
#[derive(Debug, Clone)]
pub struct Download {
    /// From `Content-Disposition` (the UTF-8 `filename*` when there is one).
    pub file_name: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

/// `GET /api/recipes/{id}/export?format=`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipeFormat {
    /// The backup format another Crumb imports.
    Json,
    Markdown,
}

/// One uploaded file's result from `POST /api/import/files` (`recipes::ImportSummary`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileImport {
    pub file: String,
    pub created: Vec<Created>,
    pub duplicates: usize,
    /// Recipes read from the file that couldn't be saved.
    pub skipped: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Created {
    pub id: i64,
    pub title: String,
}

/// A recipe's Wee Chef check (`checks::for_recipe`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeChecks {
    /// `pending`, `running`, `done`, `failed` or `tidied`.
    pub status: String,
    /// Wee Chef's fixes still stand, so Undo can put the import back.
    pub can_undo: bool,
    pub flags: Vec<CheckFlag>,
}

/// A line Wee Chef fixed (`state` `fixed`) or wants the cook to look at (`review`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckFlag {
    pub id: i64,
    /// `ingredients`, `instructions`, `notes`, ...
    pub field: String,
    pub item_text: Option<String>,
    pub kind: String,
    pub state: String,
    /// What was done, for `crumb_core::checks::fix_text` (`{"p": .., "fix": ..}`).
    pub detail: Value,
}

/// `POST /api/recipes/{id}/checks/undo`: the recipe as imported, and its check now.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoneChecks {
    pub recipe: Recipe,
    pub checks: Option<RecipeChecks>,
}

/// `GET /api/checks` (`checks::status`), and `POST /api/checks` with `queued` set.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecksStatus {
    pub enabled: bool,
    pub eligible: u32,
    pub checked: u32,
    pub pending: u32,
    pub failed: u32,
    pub tidied: u32,
    pub to_check: u32,
    pub due: u32,
    pub restored: u32,
    pub edited: u32,
    /// How many "Check all" just queued.
    #[serde(default)]
    pub queued: Option<u32>,
}

impl ChecksStatus {
    /// The counts `crumb_core::checks::checks_status_text` words.
    pub fn counts(&self) -> crumb_core::checks::ChecksCounts {
        crumb_core::checks::ChecksCounts {
            eligible: self.eligible,
            checked: self.checked,
            pending: self.pending,
            tidied: self.tidied,
            to_check: self.to_check,
            due: self.due,
            restored: self.restored,
            edited: self.edited,
        }
    }
}

/// A recipe with suggestions waiting (`checks::to_review`), newest first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub id: i64,
    pub title: String,
    pub image: Option<String>,
    pub count: i64,
    /// Suggestions per field: `{"instructions": 2}`.
    pub fields: BTreeMap<String, i64>,
}

/// A public link to a recipe or cookbook (`share::to_json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Share {
    pub token: String,
    pub url: String,
    pub include_notes: bool,
    pub created_at: String,
}

/// What a share link shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShareKind {
    Recipe,
    Cookbook,
}

/// One live link in `GET /api/shares` (`share::list`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedLink {
    pub kind: ShareKind,
    /// The recipe or cookbook id the share routes take.
    pub id: i64,
    pub title: String,
    pub url: String,
    pub include_notes: bool,
    pub created_at: String,
    pub last_opened_at: Option<String>,
}

/// A recipe in the trash (`GET /api/trash`, `trash::Trashed`): deleted, and restorable
/// until `purge_at`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trashed {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub image: Option<String>,
    /// ISO-8601.
    pub deleted_at: String,
    /// ISO-8601: when it goes for good.
    pub purge_at: String,
}
