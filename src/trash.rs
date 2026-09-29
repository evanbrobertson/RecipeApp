//! Deleted recipes, kept for [`RETENTION_DAYS`] so they can be put back. The safeguard for a
//! delete that shouldn't have happened, whoever asked for it: the cook, or Claude talked into
//! a confirmed delete by text in a recipe (the connector can restore, never empty the trash).
//!
//! A deleted recipe leaves `recipes` as before (so nothing else has to skip it) and its row is
//! kept here whole, as JSON, with the cookbooks it was in and when it was cooked. Restoring
//! puts the row back under its own id (`recipes` ids are AUTOINCREMENT, so never reused), in
//! the cookbooks that still exist. Views, Wee Chef's checks and flags, and its share link are
//! not kept: the checks are re-derived, and a public link shouldn't come back by itself.

use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde_json::{Map, Value, json};

use crate::error::{AppError, AppResult};
use crate::model::{Recipe, now_secs};

/// How long a deleted recipe can be put back.
pub const RETENTION_DAYS: i64 = 30;
const RETENTION_SECS: i64 = RETENTION_DAYS * 86_400;

pub const TABLE_SQL: &str = "
  -- Deleted recipes (src/trash.rs), put back within 30 days. Not in backups.
  CREATE TABLE IF NOT EXISTS recipe_trash (
    id integer PRIMARY KEY NOT NULL,
    title text NOT NULL,
    url text,
    image text,
    row text NOT NULL,
    cookbooks text DEFAULT '[]' NOT NULL,
    cooked text DEFAULT '[]' NOT NULL,
    deleted_at integer NOT NULL
  );
  CREATE INDEX IF NOT EXISTS recipe_trash_deleted_idx ON recipe_trash (deleted_at);";

/// A recipe in the trash, as listed.
#[derive(Debug, Clone, PartialEq)]
pub struct Trashed {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub image: Option<String>,
    pub deleted_at: i64,
}

impl Trashed {
    /// When it goes for good.
    pub fn purge_at(&self) -> i64 {
        self.deleted_at + RETENTION_SECS
    }

    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "title": self.title,
            "url": self.url,
            "image": self.image,
            "deletedAt": crate::model::iso(self.deleted_at),
            "purgeAt": crate::model::iso(self.purge_at()),
        })
    }
}

fn to_json(v: SqlValue) -> Value {
    match v {
        SqlValue::Null | SqlValue::Blob(_) => Value::Null,
        SqlValue::Integer(i) => json!(i),
        SqlValue::Real(f) => json!(f),
        SqlValue::Text(s) => json!(s),
    }
}

fn to_sql(v: &Value) -> SqlValue {
    match v {
        Value::Number(n) => n
            .as_i64()
            .map(SqlValue::Integer)
            .or_else(|| n.as_f64().map(SqlValue::Real))
            .unwrap_or(SqlValue::Null),
        Value::String(s) => SqlValue::Text(s.clone()),
        Value::Bool(b) => SqlValue::Integer(i64::from(*b)),
        _ => SqlValue::Null,
    }
}

/// Every column of recipe `id`, by name; None when there's no such recipe.
fn recipe_row(conn: &Connection, id: i64) -> AppResult<Option<Map<String, Value>>> {
    let mut stmt = conn.prepare("SELECT * FROM recipes WHERE id = ?1")?;
    let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    Ok(stmt
        .query_row([id], |r| {
            let mut row = Map::new();
            for (i, name) in names.iter().enumerate() {
                row.insert(name.clone(), to_json(r.get::<_, SqlValue>(i)?));
            }
            Ok(row)
        })
        .optional()?)
}

fn ints(conn: &Connection, sql: &str, id: i64) -> AppResult<Vec<i64>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([id], |r| r.get(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Moves these recipes to the trash; how many there were. Ids that aren't recipes are skipped.
pub fn delete(conn: &Connection, ids: &[i64]) -> AppResult<usize> {
    let tx = conn.unchecked_transaction()?;
    let now = now_secs();
    let mut moved = 0;
    for &id in ids {
        let Some(row) = recipe_row(&tx, id)? else {
            continue;
        };
        let text = |k: &str| row.get(k).and_then(Value::as_str).map(str::to_string);
        let cookbooks = ints(
            &tx,
            "SELECT cookbook_id FROM cookbook_recipes WHERE recipe_id = ?1 ORDER BY id",
            id,
        )?;
        let cooked = ints(
            &tx,
            "SELECT created_at FROM recipe_events WHERE recipe_id = ?1 AND kind = 'cooked' ORDER BY created_at",
            id,
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO recipe_trash (id, title, url, image, row, cookbooks, cooked, deleted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                text("title").unwrap_or_default(),
                text("url"),
                text("image"),
                Value::Object(row).to_string(),
                json!(cookbooks).to_string(),
                json!(cooked).to_string(),
                now
            ],
        )?;
        moved += tx.execute("DELETE FROM recipes WHERE id = ?1", [id])?;
    }
    purge_expired(&tx, now)?;
    tx.commit()?;
    Ok(moved)
}

/// Removes what has been in the trash longer than [`RETENTION_DAYS`].
pub fn purge_expired(conn: &Connection, now: i64) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM recipe_trash WHERE deleted_at <= ?1",
        [now - RETENTION_SECS],
    )
}

/// The trash, most recently deleted first.
pub fn list(conn: &Connection) -> AppResult<Vec<Trashed>> {
    purge_expired(conn, now_secs())?;
    let mut stmt = conn.prepare(
        "SELECT id, title, url, image, deleted_at FROM recipe_trash ORDER BY deleted_at DESC, id DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Trashed {
            id: r.get(0)?,
            title: r.get(1)?,
            url: r.get(2)?,
            image: r.get(3)?,
            deleted_at: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// A deleted recipe's photo and link, for `/img` (the trash page shows its photo).
pub fn image_and_url(conn: &Connection, id: i64) -> Option<(Option<String>, Option<String>)> {
    conn.query_row(
        "SELECT image, url FROM recipe_trash WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .ok()
    .flatten()
}

/// What putting a recipe back did.
#[derive(Debug)]
pub enum Restored {
    /// Back in the box, as it was.
    Back(Recipe),
    /// Its link had been saved again meanwhile: that recipe, and the deleted copy is gone.
    AlreadySaved(Recipe),
}

/// Puts recipe `id` back: 404 when it isn't in the trash (or was purged).
pub fn restore(conn: &Connection, id: i64) -> AppResult<Restored> {
    purge_expired(conn, now_secs())?;
    let found: Option<(String, String, String)> = conn
        .query_row(
            "SELECT row, cookbooks, cooked FROM recipe_trash WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((row, cookbooks, cooked)) = found else {
        return Err(AppError::not_found("That recipe isn't in the trash"));
    };
    let mut row: Map<String, Value> = serde_json::from_str(&row)
        .map_err(|_| AppError::internal("A deleted recipe couldn't be read"))?;
    let tx = conn.unchecked_transaction()?;
    if let Some(url) = row.get("url").and_then(Value::as_str)
        && let Some(existing) = crate::recipes::find_by_url(&tx, url)?
    {
        tx.execute("DELETE FROM recipe_trash WHERE id = ?1", [id])?;
        let recipe = crate::recipes::require_recipe(&tx, existing)?;
        tx.commit()?;
        return Ok(Restored::AlreadySaved(recipe));
    }
    // Only columns the table still has (a later version may have dropped one)
    let mut columns = Vec::new();
    {
        let mut stmt = tx.prepare("SELECT name FROM pragma_table_info('recipes')")?;
        let names = stmt.query_map([], |r| r.get::<_, String>(0))?;
        for name in names {
            columns.push(name?);
        }
    }
    row.retain(|k, _| columns.contains(k));
    // Its id is its own unless something else took it (a backup restored with ids)
    let taken: bool = tx
        .query_row("SELECT 1 FROM recipes WHERE id = ?1", [id], |_| Ok(true))
        .optional()?
        .unwrap_or(false);
    if taken {
        row.remove("id");
    }
    let names: Vec<&String> = row.keys().collect();
    let sql = format!(
        "INSERT INTO recipes ({}) VALUES ({})",
        names
            .iter()
            .map(|n| format!("\"{n}\""))
            .collect::<Vec<_>>()
            .join(", "),
        vec!["?"; names.len()].join(", ")
    );
    tx.execute(&sql, params_from_iter(row.values().map(to_sql)))?;
    let new_id = tx.last_insert_rowid();
    let list = |text: &str| serde_json::from_str::<Vec<i64>>(text).unwrap_or_default();
    for book in list(&cookbooks) {
        tx.execute(
            "INSERT OR IGNORE INTO cookbook_recipes (cookbook_id, recipe_id)
             SELECT ?1, ?2 WHERE EXISTS (SELECT 1 FROM cookbooks WHERE id = ?1)",
            params![book, new_id],
        )?;
    }
    for at in list(&cooked) {
        tx.execute(
            "INSERT INTO recipe_events (recipe_id, kind, created_at) VALUES (?1, 'cooked', ?2)",
            params![new_id, at],
        )?;
    }
    tx.execute("DELETE FROM recipe_trash WHERE id = ?1", [id])?;
    let recipe = crate::recipes::require_recipe(&tx, new_id)?;
    tx.commit()?;
    Ok(Restored::Back(recipe))
}

/// Deletes one recipe from the trash for good; false when it wasn't there.
pub fn purge(conn: &Connection, id: i64) -> AppResult<bool> {
    Ok(conn.execute("DELETE FROM recipe_trash WHERE id = ?1", [id])? > 0)
}

/// Empties the trash; how many went.
pub fn empty(conn: &Connection) -> AppResult<usize> {
    Ok(conn.execute("DELETE FROM recipe_trash", [])?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn add(conn: &Connection, title: &str, url: Option<&str>) -> i64 {
        conn.execute(
            "INSERT INTO recipes (url, title, ingredients, instructions, notes, created_at, updated_at)
             VALUES (?1, ?2, '[]', '[]', 'my note', 100, 200)",
            params![url, title],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn a_deleted_recipe_comes_back_whole() {
        let db = db::open_in_memory().unwrap();
        let conn = db.lock();
        let id = add(&conn, "Soup", Some("https://a.test/soup"));
        conn.execute(
            "INSERT INTO cookbooks (name, created_at) VALUES ('Winter', 1)",
            [],
        )
        .unwrap();
        let book = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO cookbook_recipes (cookbook_id, recipe_id) VALUES (?1, ?2)",
            [book, id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO recipe_events (recipe_id, kind, created_at) VALUES (?1, 'cooked', 555)",
            [id],
        )
        .unwrap();

        assert_eq!(delete(&conn, &[id, 999]).unwrap(), 1);
        assert!(crate::recipes::get_recipe(&conn, id).unwrap().is_none());
        let listed = list(&conn).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].title, "Soup");
        assert_eq!(listed[0].purge_at() - listed[0].deleted_at, RETENTION_SECS);

        let Restored::Back(r) = restore(&conn, id).unwrap() else {
            panic!("expected it back")
        };
        assert_eq!((r.id, r.title.as_str()), (id, "Soup"));
        assert_eq!(r.notes.as_deref(), Some("my note"));
        assert_eq!(r.updated_at, 200);
        let books: i64 = conn
            .query_row(
                "SELECT count(*) FROM cookbook_recipes WHERE recipe_id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(books, 1);
        let cooked: i64 = conn
            .query_row(
                "SELECT created_at FROM recipe_events WHERE recipe_id = ?1 AND kind = 'cooked'",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cooked, 555);
        assert!(list(&conn).unwrap().is_empty());
        assert_eq!(restore(&conn, id).unwrap_err().status, 404);
    }

    #[test]
    fn a_link_saved_again_meanwhile_wins() {
        let db = db::open_in_memory().unwrap();
        let conn = db.lock();
        let id = add(&conn, "Soup", Some("https://a.test/soup"));
        delete(&conn, &[id]).unwrap();
        let again = add(&conn, "Soup again", Some("https://a.test/soup"));
        let Restored::AlreadySaved(r) = restore(&conn, id).unwrap() else {
            panic!("expected the saved one")
        };
        assert_eq!(r.id, again);
        assert!(list(&conn).unwrap().is_empty());
    }

    #[test]
    fn the_trash_empties_itself_after_thirty_days() {
        let db = db::open_in_memory().unwrap();
        let conn = db.lock();
        let old = add(&conn, "Old", None);
        let new = add(&conn, "New", None);
        delete(&conn, &[old, new]).unwrap();
        conn.execute(
            "UPDATE recipe_trash SET deleted_at = ?1 WHERE id = ?2",
            params![now_secs() - RETENTION_SECS - 1, old],
        )
        .unwrap();
        let left: Vec<i64> = list(&conn).unwrap().iter().map(|t| t.id).collect();
        assert_eq!(left, vec![new]);
        assert!(purge(&conn, new).unwrap());
        assert_eq!(empty(&conn).unwrap(), 0);
    }
}
