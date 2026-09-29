//! Households: whose recipe box a request works on.
//!
//! Each household's recipes live in a SQLite file of their own, with the schema in
//! [`crate::db`] unchanged, so no query can reach another household's box. The home
//! household ([`HOME`]) is the database Crumb has always used (`DATABASE_PATH`, or the
//! Railway volume): with one password (`AUTH_MODE=password`, the default) it is the only
//! one. Other households live beside it in `households/{id}/recipes.db`.
//!
//! What a household keeps in memory beside its file (its time zone, the cached Try next
//! picks and the queue of recipes waiting for a check) travels with it in [`Household`].

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::db::{self, Db};
use crate::error::{AppError, AppResult};
use crate::suggestions::{AiState, Zone};

/// A household's id: `households.id` in the accounts database.
pub type HouseholdId = i64;

/// The household whose box is the original database.
pub const HOME: HouseholdId = 1;

/// One household's box: its database and what's kept in memory beside it.
#[derive(Clone)]
pub struct Household {
    pub id: HouseholdId,
    pub db: Db,
    /// The cook's last-seen time zone, for the Try next context.
    pub zone: Arc<Zone>,
    /// Cached AI picks for Try next.
    pub ai: Arc<AiState>,
    /// Recipes waiting for Wee Chef's check.
    pub queued: Arc<Mutex<HashSet<i64>>>,
}

impl Household {
    fn new(id: HouseholdId, db: Db) -> Self {
        Self {
            id,
            db,
            zone: Arc::default(),
            ai: Arc::default(),
            queued: Arc::default(),
        }
    }
}

/// Every household's box, opened on first use and kept open (one connection each).
pub struct Households {
    home: Household,
    /// Where other households' files go; None keeps them in memory (tests).
    dir: Option<PathBuf>,
    open: Mutex<HashMap<HouseholdId, Household>>,
}

impl Households {
    /// `home` is the original database; `dir` is the folder beside it (see
    /// [`households_dir`]), or None for in-memory boxes.
    pub fn new(home: Db, dir: Option<PathBuf>) -> Self {
        Self {
            home: Household::new(HOME, home),
            dir,
            open: Mutex::default(),
        }
    }

    pub fn home(&self) -> &Household {
        &self.home
    }

    /// Household `id`'s box, opening (and creating) its file on first use.
    pub fn get(&self, id: HouseholdId) -> AppResult<Household> {
        if id == HOME {
            return Ok(self.home.clone());
        }
        if id < 1 {
            return Err(AppError::internal(format!("no household {id}")));
        }
        let mut open = self.open.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(h) = open.get(&id) {
            return Ok(h.clone());
        }
        let db = match &self.dir {
            Some(dir) => db::open(&dir.join(id.to_string()).join("recipes.db")),
            None => db::open_in_memory(),
        }
        .map_err(AppError::internal)?;
        let h = Household::new(id, db);
        open.insert(id, h.clone());
        Ok(h)
    }
}

impl Households {
    /// Household `id`'s box if it has one already: never creates a file (Popular reads every
    /// household's box, and one that never saved anything has nothing to add).
    pub fn existing(&self, id: HouseholdId) -> Option<Household> {
        if id == HOME {
            return Some(self.home.clone());
        }
        if let Some(h) = self.open.lock().unwrap_or_else(|e| e.into_inner()).get(&id) {
            return Some(h.clone());
        }
        let dir = self.dir.as_ref()?;
        dir.join(id.to_string())
            .join("recipes.db")
            .exists()
            .then(|| self.get(id).ok())
            .flatten()
    }

    /// Deletes household `id`'s box (never the home one's): closes it and removes its
    /// folder, database and all.
    pub fn remove(&self, id: HouseholdId) -> AppResult<()> {
        if id == HOME {
            return Err(AppError::internal(
                "the home household's file is never removed",
            ));
        }
        self.open
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&id);
        if let Some(dir) = &self.dir {
            match std::fs::remove_dir_all(dir.join(id.to_string())) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(AppError::internal(err)),
            }
        }
        Ok(())
    }
}

/// The folder other households' files go in: `households/` beside the home database.
pub fn households_dir(home_db: &std::path::Path) -> PathBuf {
    home_db
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."))
        .join("households")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_household_has_its_own_file() {
        let dir = tempfile::tempdir().unwrap();
        let home = db::open(&dir.path().join("recipes.db")).unwrap();
        let all = Households::new(home, Some(households_dir(&dir.path().join("recipes.db"))));
        let now = 1;
        let add = |h: &Household, title: &str| {
            h.db.lock()
                .execute(
                    "INSERT INTO recipes (title, ingredients, instructions, created_at, updated_at)
                     VALUES (?1, '[]', '[]', ?2, ?2)",
                    rusqlite::params![title, now],
                )
                .unwrap();
        };
        let count = |h: &Household| -> i64 {
            h.db.lock()
                .query_row("SELECT count(*) FROM recipes", [], |r| r.get(0))
                .unwrap()
        };
        add(&all.get(HOME).unwrap(), "Home soup");
        let two = all.get(2).unwrap();
        add(&two, "Their pie");
        add(&two, "Their tart");
        assert_eq!(count(&all.get(HOME).unwrap()), 1);
        assert_eq!(count(&all.get(2).unwrap()), 2, "reopened from the registry");
        assert!(dir.path().join("households/2/recipes.db").exists());
        assert!(all.get(0).is_err());
        // The same household shares its in-memory state between requests
        assert!(Arc::ptr_eq(&all.get(2).unwrap().ai, &two.ai));
    }
}
