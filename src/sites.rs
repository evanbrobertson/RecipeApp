//! Site memory: what worked the last time a recipe was read from a host, so the next scrape
//! can go straight to it (see `scraper::scrape_with`).
//!
//! The same file keeps `terms_checks`, when Wee Chef last looked at a host's terms of service
//! (see `site_terms`).
//!
//! One server-wide file, `sites.db` beside the home database. These are facts about public
//! websites ("recipetineats.com is WordPress and its own API answers"), not about anyone's
//! box, so unlike recipes they are not kept per household: what one household's import
//! learned saves every other household's a probe. Nothing about a recipe, a cook or a
//! link's path is stored, only the host and how it was read.
//!
//! A fact is good for [`TTL_SECS`]; two scrapes in a row that got nothing forget the host
//! ([`MISSES_TO_FORGET`]). A host that turned Firefox and Safari away is remembered as such,
//! and the page fetch is skipped, except once a [`PROBE_EVERY_SECS`] so a site that lifts its
//! block is noticed. Timestamps are unix seconds.

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

/// How long a fact is trusted.
pub const TTL_SECS: i64 = 7 * 86_400;
/// How often a host known to block the server has its page fetch tried again.
pub const PROBE_EVERY_SECS: i64 = 86_400;
/// Consecutive scrapes that got nothing before the host is forgotten.
pub const MISSES_TO_FORGET: i64 = 2;

/// `sites.db`, next to the home database.
pub fn sites_db_path(home_db: &Path) -> PathBuf {
    home_db
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .join("sites.db")
}

/// Seconds since the epoch.
pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// What is known about a host.
#[derive(Debug, Clone, PartialEq)]
pub struct Facts {
    pub host: String,
    /// The recipe platform it runs ("wordpress").
    pub platform: Option<String>,
    /// Where its REST API lives ("https://food.test/wp-json/").
    pub api_root: Option<String>,
    /// How its API gave the recipe ("wprm", "mediavine", "post-markup", "recipe_schema").
    pub recipe_endpoint_kind: Option<String>,
    /// The [`Method`](crate::scraper::Method) label that read a recipe last.
    pub winning_method: Option<String>,
    /// Firefox and Safari were both turned away.
    pub blocks_server: bool,
    /// Anything else worth keeping (`probed_at`: when the page fetch was last tried).
    pub extra: Value,
    pub checked_at: i64,
    pub failures: i64,
}

impl Facts {
    /// When the page fetch of a host that blocks the server was last tried.
    fn probed_at(&self) -> i64 {
        self.extra
            .get("probed_at")
            .and_then(Value::as_i64)
            .unwrap_or(self.checked_at)
    }

    /// Whether the page fetch should be skipped: the host blocked the server, and it isn't
    /// time to look again.
    pub fn skip_page(&self, now: i64) -> bool {
        self.blocks_server && now - self.probed_at() < PROBE_EVERY_SECS
    }
}

/// What a scrape that read a recipe learned, to be remembered.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Learned {
    pub platform: Option<String>,
    pub api_root: Option<String>,
    pub recipe_endpoint_kind: Option<String>,
    pub winning_method: Option<String>,
    pub blocks_server: bool,
    /// The page fetch was tried in this scrape (not skipped on the strength of memory).
    pub page_tried: bool,
}

pub struct Sites(Mutex<Connection>);

impl Sites {
    pub fn open(path: &Path) -> crate::db::anyhow_like::Result<Self> {
        if let Some(dir) = path.parent()
            && !dir.as_os_str().is_empty()
        {
            std::fs::create_dir_all(dir)?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> crate::db::anyhow_like::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> crate::db::anyhow_like::Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS site_facts (
               host TEXT PRIMARY KEY,
               platform TEXT,
               api_root TEXT,
               recipe_endpoint_kind TEXT,
               winning_method TEXT,
               blocks_server INTEGER NOT NULL DEFAULT 0,
               extra_json TEXT NOT NULL DEFAULT '{}',
               checked_at INTEGER NOT NULL,
               failures INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS terms_checks (
               host TEXT PRIMARY KEY,
               checked_at INTEGER NOT NULL,
               result TEXT NOT NULL,
               terms_url TEXT,
               issue_url TEXT
             );",
        )?;
        Ok(Self(Mutex::new(conn)))
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// What is known about `host` as of `now`. A fact past its time is forgotten.
    pub fn get(&self, host: &str, now: i64) -> Option<Facts> {
        let conn = self.lock();
        let facts = conn
            .query_row(
                "SELECT platform, api_root, recipe_endpoint_kind, winning_method, blocks_server,
                        extra_json, checked_at, failures
                 FROM site_facts WHERE host = ?1",
                [host],
                |row| {
                    let extra: String = row.get(5)?;
                    Ok(Facts {
                        host: host.to_string(),
                        platform: row.get(0)?,
                        api_root: row.get(1)?,
                        recipe_endpoint_kind: row.get(2)?,
                        winning_method: row.get(3)?,
                        blocks_server: row.get::<_, i64>(4)? != 0,
                        extra: serde_json::from_str(&extra).unwrap_or(Value::Null),
                        checked_at: row.get(6)?,
                        failures: row.get(7)?,
                    })
                },
            )
            .optional()
            .inspect_err(|e| tracing::warn!("[sites] read failed: {e}"))
            .ok()
            .flatten()?;
        if now - facts.checked_at > TTL_SECS {
            let _ = conn.execute("DELETE FROM site_facts WHERE host = ?1", [host]);
            return None;
        }
        Some(facts)
    }

    /// Remembers that `host` was read, and how.
    pub fn record_win(&self, host: &str, learned: &Learned, now: i64) {
        let before = self.get(host, now);
        let probed_at = if learned.page_tried || !learned.blocks_server {
            now
        } else {
            before.as_ref().map_or(now, |f| f.probed_at())
        };
        // What an earlier scrape learned stays unless this one learned better
        let keep = |new: &Option<String>, old: Option<&Option<String>>| {
            new.clone().or_else(|| old.and_then(Clone::clone))
        };
        let platform = keep(&learned.platform, before.as_ref().map(|f| &f.platform));
        let api_root = keep(&learned.api_root, before.as_ref().map(|f| &f.api_root));
        let kind = keep(
            &learned.recipe_endpoint_kind,
            before.as_ref().map(|f| &f.recipe_endpoint_kind),
        );
        let saved = self.lock().execute(
            "INSERT INTO site_facts (host, platform, api_root, recipe_endpoint_kind, winning_method,
                                     blocks_server, extra_json, checked_at, failures)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)
             ON CONFLICT(host) DO UPDATE SET
               platform = excluded.platform, api_root = excluded.api_root,
               recipe_endpoint_kind = excluded.recipe_endpoint_kind,
               winning_method = excluded.winning_method, blocks_server = excluded.blocks_server,
               extra_json = excluded.extra_json, checked_at = excluded.checked_at, failures = 0",
            params![
                host,
                platform,
                api_root,
                kind,
                learned.winning_method,
                learned.blocks_server,
                json!({"probed_at": probed_at}).to_string(),
                now
            ],
        );
        if let Err(e) = saved {
            tracing::warn!("[sites] write failed: {e}");
        }
    }

    /// Remembers that a scrape of `host` got nothing. Past [`MISSES_TO_FORGET`] in a row the
    /// host is forgotten, so a redesign or a new block doesn't keep sending us the old way.
    pub fn record_miss(&self, host: &str, now: i64) {
        let conn = self.lock();
        let result = conn
            .execute(
                "UPDATE site_facts SET failures = failures + 1, checked_at = ?2 WHERE host = ?1",
                params![host, now],
            )
            .and_then(|_| {
                conn.execute(
                    "DELETE FROM site_facts WHERE host = ?1 AND failures >= ?2",
                    params![host, MISSES_TO_FORGET],
                )
            });
        if let Err(e) = result {
            tracing::warn!("[sites] write failed: {e}");
        }
    }
}

impl Sites {
    /// When `host`'s terms were last looked at, and what came of it.
    pub fn terms_check(&self, host: &str) -> Option<(i64, String)> {
        self.lock()
            .query_row(
                "SELECT checked_at, result FROM terms_checks WHERE host = ?1",
                [host],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .inspect_err(|e| tracing::warn!("[sites] terms read failed: {e}"))
            .ok()
            .flatten()
    }

    /// The result of the last look at `host`'s terms and the issue opened for it.
    pub fn terms_recorded(&self, host: &str) -> Option<(String, Option<String>)> {
        self.lock()
            .query_row(
                "SELECT result, issue_url FROM terms_checks WHERE host = ?1",
                [host],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .inspect_err(|e| tracing::warn!("[sites] terms read failed: {e}"))
            .ok()
            .flatten()
    }

    /// Remembers a look at `host`'s terms.
    pub fn record_terms_check(
        &self,
        host: &str,
        now: i64,
        result: &str,
        terms_url: Option<&str>,
        issue_url: Option<&str>,
    ) {
        let saved = self.lock().execute(
            "INSERT INTO terms_checks (host, checked_at, result, terms_url, issue_url)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(host) DO UPDATE SET checked_at = excluded.checked_at,
               result = excluded.result, terms_url = excluded.terms_url,
               issue_url = excluded.issue_url",
            params![host, now, result, terms_url, issue_url],
        );
        if let Err(e) = saved {
            tracing::warn!("[sites] terms write failed: {e}");
        }
    }

    /// Makes every recorded terms check `secs` older.
    #[cfg(test)]
    pub fn backdate_terms_checks(&self, secs: i64) {
        self.lock()
            .execute(
                "UPDATE terms_checks SET checked_at = checked_at - ?1",
                [secs],
            )
            .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400;

    fn won(method: &str) -> Learned {
        Learned {
            platform: Some("wordpress".into()),
            api_root: Some("https://food.test/wp-json/".into()),
            recipe_endpoint_kind: Some("wprm".into()),
            winning_method: Some(method.into()),
            blocks_server: false,
            page_tried: true,
        }
    }

    #[test]
    fn a_win_is_remembered_across_opens_of_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("sites.db");
        Sites::open(&path)
            .unwrap()
            .record_win("food.test", &won("wordpress-api"), 1_000);
        let facts = Sites::open(&path).unwrap().get("food.test", 2_000).unwrap();
        assert_eq!(facts.platform.as_deref(), Some("wordpress"));
        assert_eq!(
            facts.api_root.as_deref(),
            Some("https://food.test/wp-json/")
        );
        assert_eq!(facts.recipe_endpoint_kind.as_deref(), Some("wprm"));
        assert_eq!(facts.winning_method.as_deref(), Some("wordpress-api"));
        assert!(!facts.blocks_server);
        assert_eq!((facts.checked_at, facts.failures), (1_000, 0));
        assert!(
            Sites::open(&path)
                .unwrap()
                .get("other.test", 2_000)
                .is_none()
        );
    }

    #[test]
    fn facts_expire_after_the_ttl() {
        let sites = Sites::open_in_memory().unwrap();
        sites.record_win("food.test", &won("wreq-firefox"), 0);
        assert!(sites.get("food.test", TTL_SECS).is_some());
        assert!(sites.get("food.test", TTL_SECS + 1).is_none());
        // and it stays gone
        assert!(sites.get("food.test", 10).is_none());
    }

    #[test]
    fn two_misses_in_a_row_forget_the_host_and_a_win_resets_the_count() {
        let sites = Sites::open_in_memory().unwrap();
        sites.record_win("food.test", &won("wreq-firefox"), 0);
        sites.record_miss("food.test", 10);
        assert_eq!(sites.get("food.test", 10).unwrap().failures, 1);
        sites.record_win("food.test", &won("wreq-firefox"), 20);
        assert_eq!(sites.get("food.test", 20).unwrap().failures, 0);
        sites.record_miss("food.test", 30);
        sites.record_miss("food.test", 40);
        assert!(sites.get("food.test", 40).is_none());
        // A miss for a host never seen doesn't invent an entry
        sites.record_miss("new.test", 50);
        assert!(sites.get("new.test", 50).is_none());
    }

    #[test]
    fn a_later_win_keeps_what_an_earlier_one_learned() {
        let sites = Sites::open_in_memory().unwrap();
        sites.record_win("food.test", &won("wordpress-api"), 0);
        let bare = Learned {
            winning_method: Some("wreq-safari".into()),
            page_tried: true,
            ..Default::default()
        };
        sites.record_win("food.test", &bare, 10);
        let facts = sites.get("food.test", 10).unwrap();
        assert_eq!(facts.winning_method.as_deref(), Some("wreq-safari"));
        assert_eq!(facts.platform.as_deref(), Some("wordpress"));
        assert_eq!(
            facts.api_root.as_deref(),
            Some("https://food.test/wp-json/")
        );
    }

    #[test]
    fn a_blocking_host_skips_the_page_fetch_until_the_daily_probe() {
        let sites = Sites::open_in_memory().unwrap();
        let blocked = Learned {
            blocks_server: true,
            ..won("relay")
        };
        sites.record_win("food.test", &blocked, 1_000);
        let facts = sites.get("food.test", 1_000).unwrap();
        assert!(facts.skip_page(1_000));
        assert!(facts.skip_page(1_000 + DAY - 1));
        assert!(!facts.skip_page(1_000 + DAY));

        // A scrape that skipped the page doesn't push the probe back
        let skipped = Learned {
            page_tried: false,
            ..blocked.clone()
        };
        sites.record_win("food.test", &skipped, 1_000 + DAY / 2);
        let facts = sites.get("food.test", 1_000 + DAY / 2).unwrap();
        assert!(!facts.skip_page(1_000 + DAY));

        // A probe that was blocked again does
        sites.record_win("food.test", &blocked, 1_000 + DAY);
        let facts = sites.get("food.test", 1_000 + DAY).unwrap();
        assert!(facts.skip_page(1_000 + DAY + 100));

        // And one that got through clears the block
        let open = Learned {
            blocks_server: false,
            ..won("wreq-firefox")
        };
        sites.record_win("food.test", &open, 1_000 + 2 * DAY);
        assert!(
            !sites
                .get("food.test", 1_000 + 2 * DAY)
                .unwrap()
                .skip_page(1_000 + 2 * DAY)
        );
    }

    #[test]
    fn terms_checks_share_the_file_and_are_overwritten_per_host() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sites.db");
        let sites = Sites::open(&path).unwrap();
        sites.record_win("food.test", &won("wreq-firefox"), 10);
        sites.record_terms_check("food.test", 100, "error", None, None);
        sites.record_terms_check(
            "food.test",
            200,
            "flagged",
            Some("https://t"),
            Some("https://i"),
        );
        let reopened = Sites::open(&path).unwrap();
        assert_eq!(
            reopened.terms_check("food.test"),
            Some((200, "flagged".into()))
        );
        assert_eq!(
            reopened.terms_recorded("food.test"),
            Some(("flagged".into(), Some("https://i".into())))
        );
        assert!(reopened.terms_check("other.test").is_none());
        // the site facts are in the same file
        assert!(reopened.get("food.test", 20).is_some());
    }

    #[test]
    fn the_file_sits_beside_the_home_database() {
        assert_eq!(
            sites_db_path(Path::new("/data/recipes.db")),
            Path::new("/data/sites.db")
        );
        assert_eq!(
            sites_db_path(Path::new("recipes.db")),
            Path::new("./sites.db")
        );
    }
}
