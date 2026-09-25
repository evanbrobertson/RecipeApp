//! SQLite connection and schema. Byte-compatible with the databases the Nuxt
//! version created (drizzle, unix-second timestamps, JSON text columns).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::Connection;

#[derive(Clone)]
pub struct Db(Arc<Mutex<Connection>>);

impl Db {
    pub fn lock(&self) -> MutexGuard<'_, Connection> {
        // A panic mid-query leaves SQLite itself consistent, so keep serving
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Resolves the SQLite file location. On Railway, attach a volume and the file
/// lands on it automatically via RAILWAY_VOLUME_MOUNT_PATH.
pub fn database_path() -> PathBuf {
    if let Some(p) = env_nonempty("DATABASE_PATH") {
        return PathBuf::from(p);
    }
    if let Some(dir) = env_nonempty("RAILWAY_VOLUME_MOUNT_PATH") {
        return Path::new(&dir).join("recipes.db");
    }
    PathBuf::from(".data/recipes.db")
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

const VIEW_RETENTION_SECS: i64 = 400 * 86_400;

fn recipes_table_sql(name: &str) -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS {name} (
    id integer PRIMARY KEY AUTOINCREMENT NOT NULL,
    url text,
    source text DEFAULT 'url' NOT NULL,
    title text NOT NULL,
    description text,
    image text,
    author text,
    prep_time text,
    cook_time text,
    total_time text,
    freeze_time text,
    recipe_yield text,
    recipe_category text,
    recipe_cuisine text,
    ingredients text NOT NULL,
    instructions text NOT NULL,
    nutrition text,
    notes text,
    created_at integer NOT NULL,
    updated_at integer NOT NULL
  );"
    )
}

fn bootstrap_sql() -> String {
    format!(
        "{}
  CREATE UNIQUE INDEX IF NOT EXISTS recipes_url_unique ON recipes (url);
  CREATE INDEX IF NOT EXISTS recipes_created_at_idx ON recipes (created_at);

  CREATE TABLE IF NOT EXISTS cookbooks (
    id integer PRIMARY KEY AUTOINCREMENT NOT NULL,
    name text NOT NULL,
    description text,
    color text,
    created_at integer NOT NULL
  );

  CREATE TABLE IF NOT EXISTS cookbook_recipes (
    id integer PRIMARY KEY AUTOINCREMENT NOT NULL,
    cookbook_id integer NOT NULL REFERENCES cookbooks(id) ON DELETE CASCADE,
    recipe_id integer NOT NULL REFERENCES recipes(id) ON DELETE CASCADE
  );
  CREATE UNIQUE INDEX IF NOT EXISTS cookbook_recipes_unique ON cookbook_recipes (cookbook_id, recipe_id);

  CREATE TABLE IF NOT EXISTS oauth_clients (
    id text PRIMARY KEY NOT NULL,
    name text,
    redirect_uris text NOT NULL,
    created_at integer NOT NULL
  );

  CREATE TABLE IF NOT EXISTS oauth_tokens (
    hash text PRIMARY KEY NOT NULL,
    kind text NOT NULL,
    client_id text NOT NULL,
    code_challenge text,
    redirect_uri text,
    expires_at integer NOT NULL,
    created_at integer NOT NULL
  );
  CREATE INDEX IF NOT EXISTS oauth_tokens_expires_idx ON oauth_tokens (expires_at);

  CREATE TABLE IF NOT EXISTS recipe_events (
    id integer PRIMARY KEY AUTOINCREMENT NOT NULL,
    recipe_id integer NOT NULL REFERENCES recipes(id) ON DELETE CASCADE,
    kind text NOT NULL,
    created_at integer NOT NULL
  );
  CREATE INDEX IF NOT EXISTS recipe_events_recipe_kind_idx ON recipe_events (recipe_id, kind, created_at);",
        recipes_table_sql("recipes")
    )
}

struct Column {
    name: String,
    notnull: bool,
}

fn columns(conn: &Connection, table: &str) -> rusqlite::Result<Vec<Column>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |r| {
        Ok(Column {
            name: r.get("name")?,
            notnull: r.get::<_, i64>("notnull")? == 1,
        })
    })?;
    rows.collect()
}

/// Upgrades databases created by the first version of the app:
/// url was NOT NULL and there were no source/updated_at columns.
fn upgrade_legacy_schema(conn: &mut Connection) -> rusqlite::Result<()> {
    let cols = columns(conn, "recipes")?;
    if cols.is_empty() {
        return Ok(());
    }
    let find = |n: &str| cols.iter().find(|c| c.name == n);
    let needs_rebuild = find("url").is_some_and(|c| c.notnull)
        || find("source").is_none()
        || find("updated_at").is_none();
    if !needs_rebuild {
        return Ok(());
    }

    let legacy = [
        "id",
        "url",
        "title",
        "description",
        "image",
        "author",
        "prep_time",
        "cook_time",
        "total_time",
        "freeze_time",
        "recipe_yield",
        "recipe_category",
        "recipe_cuisine",
        "ingredients",
        "instructions",
        "nutrition",
        "notes",
        "created_at",
    ];
    let list = legacy
        .iter()
        .filter(|c| find(c).is_some())
        .copied()
        .collect::<Vec<_>>()
        .join(", ");

    conn.execute_batch("PRAGMA foreign_keys = OFF")?;
    let tx = conn.transaction()?;
    // SQLite's recommended table rebuild: create, copy, drop, rename
    tx.execute_batch(&recipes_table_sql("recipes_new"))?;
    tx.execute_batch(&format!(
        "INSERT INTO recipes_new ({list}, source, updated_at) SELECT {list}, 'url', created_at FROM recipes"
    ))?;
    tx.execute_batch("DROP TABLE recipes")?;
    tx.execute_batch("ALTER TABLE recipes_new RENAME TO recipes")?;
    tx.commit()?;
    conn.execute_batch("PRAGMA foreign_keys = ON")?;
    Ok(())
}

/// Additive column changes (new nullable columns) for existing databases.
fn add_missing_columns(conn: &Connection) -> rusqlite::Result<()> {
    if !columns(conn, "cookbooks")?
        .iter()
        .any(|c| c.name == "color")
    {
        conn.execute_batch("ALTER TABLE cookbooks ADD COLUMN color text")?;
    }
    Ok(())
}

pub fn open(path: &Path) -> anyhow_like::Result<Db> {
    if std::env::var_os("RAILWAY_ENVIRONMENT").is_some()
        && env_nonempty("RAILWAY_VOLUME_MOUNT_PATH").is_none()
        && env_nonempty("DATABASE_PATH").is_none()
    {
        tracing::warn!(
            "No Railway volume attached: recipes will be lost on every deploy. Add a volume to this service."
        );
    }
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir)?;
    }
    let conn = Connection::open(path)?;
    init(conn)
}

pub fn open_in_memory() -> anyhow_like::Result<Db> {
    init(Connection::open_in_memory()?)
}

fn init(mut conn: Connection) -> anyhow_like::Result<Db> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA busy_timeout = 5000;
         PRAGMA foreign_keys = ON;",
    )?;
    upgrade_legacy_schema(&mut conn)?;
    conn.execute_batch(&bootstrap_sql())?;
    add_missing_columns(&conn)?;
    // Views only feed suggestions for a few months; cooks are kept for good
    conn.execute(
        "DELETE FROM recipe_events WHERE kind = 'viewed' AND created_at < ?1",
        [crate::model::now_secs() - VIEW_RETENTION_SECS],
    )?;
    Ok(Db(Arc::new(Mutex::new(conn))))
}

/// A boxed error for startup paths.
pub mod anyhow_like {
    pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrades_the_first_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legacy.db");
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch(
                "CREATE TABLE recipes (id integer PRIMARY KEY AUTOINCREMENT NOT NULL, url text NOT NULL,
                 title text NOT NULL, ingredients text NOT NULL, instructions text NOT NULL, created_at integer NOT NULL);
                 INSERT INTO recipes (url, title, ingredients, instructions, created_at)
                 VALUES ('https://a.test/x', 'Old', '[\"1 egg\"]', '[\"Cook\"]', 1700000000);
                 CREATE TABLE cookbooks (id integer PRIMARY KEY AUTOINCREMENT NOT NULL, name text NOT NULL,
                 description text, created_at integer NOT NULL);",
            )
            .unwrap();
        }
        let db = open(&path).unwrap();
        let c = db.lock();
        let (source, updated): (String, i64) = c
            .query_row("SELECT source, updated_at FROM recipes", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(source, "url");
        assert_eq!(updated, 1_700_000_000);
        assert!(
            columns(&c, "cookbooks")
                .unwrap()
                .iter()
                .any(|col| col.name == "color")
        );
        // Opening an older database adds the event log, and deleting a recipe clears its events
        c.execute(
            "INSERT INTO recipe_events (recipe_id, kind, created_at) VALUES (1, 'cooked', 1)",
            [],
        )
        .unwrap();
        c.execute("DELETE FROM recipes", []).unwrap();
        let left: i64 = c
            .query_row("SELECT count(*) FROM recipe_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0);
    }
}
