//! Accounts (`AUTH_MODE=accounts`): people who sign in with an email and password, the
//! households whose recipe boxes they share, and their sessions.
//!
//! Kept in `accounts.db` beside the home database; each household's recipes live in their
//! own file ([`crate::households`]). The first account made (the setup) owns household 1,
//! the box Crumb already had, so switching an install to accounts keeps every recipe.
//!
//! Sessions are random tokens in the `crumb_session` cookie; only their SHA-256 is stored, so
//! the table can't be replayed. They last 90 days from last use and can be listed and revoked.
//! Passwords are hashed with Argon2id (the `argon2` crate's defaults) off the async threads.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::auth::{random_token, sha256_hex};
use crate::error::{AppError, AppResult};
use crate::households::{HOME, HouseholdId};
use crate::model::now_secs;

/// How long a session lasts after it was last used.
pub const SESSION_SECS: i64 = 60 * 60 * 24 * 90;
/// A session's `last_seen_at` is only written this often, not on every request.
const TOUCH_EVERY_SECS: i64 = 60 * 60;
const MIN_PASSWORD: usize = 8;
const MAX_PASSWORD: usize = 256;

const SCHEMA: &str = "
  CREATE TABLE IF NOT EXISTS users (
    id integer PRIMARY KEY AUTOINCREMENT NOT NULL,
    email text NOT NULL,
    name text NOT NULL,
    password_hash text NOT NULL,
    created_at integer NOT NULL
  );
  CREATE UNIQUE INDEX IF NOT EXISTS users_email_unique ON users (lower(email));

  CREATE TABLE IF NOT EXISTS households (
    id integer PRIMARY KEY AUTOINCREMENT NOT NULL,
    name text NOT NULL,
    created_at integer NOT NULL
  );

  CREATE TABLE IF NOT EXISTS household_members (
    household_id integer NOT NULL REFERENCES households(id) ON DELETE CASCADE,
    user_id integer NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role text NOT NULL CHECK (role IN ('owner', 'member')),
    created_at integer NOT NULL,
    PRIMARY KEY (household_id, user_id)
  );

  CREATE TABLE IF NOT EXISTS sessions (
    id integer PRIMARY KEY AUTOINCREMENT NOT NULL,
    token_hash text NOT NULL,
    user_id integer NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    household_id integer NOT NULL REFERENCES households(id) ON DELETE CASCADE,
    user_agent text,
    created_at integer NOT NULL,
    last_seen_at integer NOT NULL,
    expires_at integer NOT NULL
  );
  CREATE UNIQUE INDEX IF NOT EXISTS sessions_token_unique ON sessions (token_hash);
  CREATE INDEX IF NOT EXISTS sessions_user_idx ON sessions (user_id);

  -- Which household's box a share link (by its token's hash) opens, for households other
  -- than the home one (whose links are found in its own database, as before accounts)
  CREATE TABLE IF NOT EXISTS share_tokens (
    token_hash text PRIMARY KEY NOT NULL,
    household_id integer NOT NULL REFERENCES households(id) ON DELETE CASCADE
  );";

/// A signed-in person, and the household the session works on.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    #[serde(skip)]
    pub id: i64,
    pub user_id: i64,
    pub email: String,
    pub name: String,
    pub household_id: HouseholdId,
    pub household_name: String,
    pub role: String,
}

/// One of a person's sessions, for the list they can revoke from.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: i64,
    pub user_agent: Option<String>,
    pub created_at: i64,
    pub last_seen_at: i64,
    pub current: bool,
}

/// `accounts.db`, next to the home database.
pub fn accounts_db_path(home_db: &Path) -> std::path::PathBuf {
    home_db
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .join("accounts.db")
}

pub struct Accounts(Arc<Mutex<Connection>>);

impl Accounts {
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
             PRAGMA busy_timeout = 5000;
             PRAGMA foreign_keys = ON;",
        )?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self(Arc::new(Mutex::new(conn))))
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Whether nobody has an account yet (the first visit sets one up).
    pub fn needs_setup(&self) -> AppResult<bool> {
        let n: i64 = self
            .lock()
            .query_row("SELECT count(*) FROM users", [], |r| r.get(0))?;
        Ok(n == 0)
    }

    /// The first account: its owner gets household 1, the box that was already here.
    /// Fails once anyone has an account.
    pub async fn set_up(&self, email: &str, name: &str, password: &str) -> AppResult<i64> {
        let (email, name) = (valid_email(email)?, valid_name(name)?);
        let hash = hash_password(password).await?;
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let users: i64 = tx.query_row("SELECT count(*) FROM users", [], |r| r.get(0))?;
        if users > 0 {
            return Err(AppError::new(
                409,
                "Crumb is already set up. Sign in instead.",
            ));
        }
        let now = now_secs();
        tx.execute(
            "INSERT OR IGNORE INTO households (id, name, created_at) VALUES (?1, ?2, ?3)",
            params![HOME, household_name(&name), now],
        )?;
        let user = insert_user(&tx, &email, &name, &hash, now)?;
        tx.execute(
            "INSERT INTO household_members (household_id, user_id, role, created_at)
             VALUES (?1, ?2, 'owner', ?3)",
            params![HOME, user, now],
        )?;
        tx.commit()?;
        Ok(user)
    }

    /// A new account with a household (and so an empty recipe box) of its own: the
    /// account's id and its household's.
    pub async fn sign_up(
        &self,
        email: &str,
        name: &str,
        password: &str,
    ) -> AppResult<(i64, HouseholdId)> {
        let (email, name) = (valid_email(email)?, valid_name(name)?);
        let hash = hash_password(password).await?;
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let users: i64 = tx.query_row("SELECT count(*) FROM users", [], |r| r.get(0))?;
        if users == 0 {
            // Household 1 is the existing box: only the setup may take it
            return Err(AppError::new(409, "Crumb isn't set up yet."));
        }
        let now = now_secs();
        let user = insert_user(&tx, &email, &name, &hash, now)?;
        tx.execute(
            "INSERT INTO households (name, created_at) VALUES (?1, ?2)",
            params![household_name(&name), now],
        )?;
        let household = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO household_members (household_id, user_id, role, created_at)
             VALUES (?1, ?2, 'owner', ?3)",
            params![household, user, now],
        )?;
        tx.commit()?;
        Ok((user, household))
    }

    /// The account with this email and password, and the household it signs in to. The
    /// same work is done for an unknown email, so timing doesn't tell which emails exist.
    pub async fn authenticate(
        &self,
        email: &str,
        password: &str,
    ) -> AppResult<Option<(i64, HouseholdId)>> {
        let found: Option<(i64, String)> = self
            .lock()
            .query_row(
                "SELECT id, password_hash FROM users WHERE lower(email) = lower(?1)",
                [email.trim()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (user, hash) = match found {
            Some((user, hash)) => (Some(user), hash),
            None => (None, DUMMY_HASH.clone()),
        };
        let ok = verify_password(password, &hash).await;
        let Some(user) = user.filter(|_| ok) else {
            return Ok(None);
        };
        Ok(self.default_household(user)?.map(|h| (user, h)))
    }

    /// The household a person signs in to: the first one they joined.
    fn default_household(&self, user: i64) -> AppResult<Option<HouseholdId>> {
        Ok(self
            .lock()
            .query_row(
                "SELECT household_id FROM household_members WHERE user_id = ?1
                 ORDER BY created_at, household_id LIMIT 1",
                [user],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Notes that a share link's token opens `household`'s box.
    pub fn index_share(&self, token: &str, household: HouseholdId) -> AppResult<()> {
        self.lock().execute(
            "INSERT INTO share_tokens (token_hash, household_id) VALUES (?1, ?2)
             ON CONFLICT(token_hash) DO UPDATE SET household_id = ?2",
            params![sha256_hex(token), household],
        )?;
        Ok(())
    }

    /// Forgets a stopped share link.
    pub fn unindex_share(&self, token: &str) -> AppResult<()> {
        self.lock().execute(
            "DELETE FROM share_tokens WHERE token_hash = ?1",
            [sha256_hex(token)],
        )?;
        Ok(())
    }

    /// The household whose box a share token opens, when it isn't the home one.
    pub fn share_household(&self, token: &str) -> AppResult<Option<HouseholdId>> {
        Ok(self
            .lock()
            .query_row(
                "SELECT household_id FROM share_tokens WHERE token_hash = ?1",
                [sha256_hex(token)],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Whether `user` is (still) in `household`.
    pub fn is_member(&self, user: i64, household: HouseholdId) -> AppResult<bool> {
        Ok(self
            .lock()
            .query_row(
                "SELECT 1 FROM household_members WHERE user_id = ?1 AND household_id = ?2",
                params![user, household],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    /// Starts a session for `user` in `household`; returns the cookie's token.
    pub fn start_session(
        &self,
        user: i64,
        household: HouseholdId,
        user_agent: Option<&str>,
    ) -> AppResult<String> {
        let token = random_token(32);
        let now = now_secs();
        let agent: Option<String> = user_agent.map(|a| a.chars().take(200).collect());
        let conn = self.lock();
        conn.execute(
            "INSERT INTO sessions (token_hash, user_id, household_id, user_agent, created_at,
               last_seen_at, expires_at)
             SELECT ?1, ?2, ?3, ?4, ?5, ?5, ?6 WHERE EXISTS (
               SELECT 1 FROM household_members WHERE user_id = ?2 AND household_id = ?3)",
            params![
                sha256_hex(&token),
                user,
                household,
                agent,
                now,
                now + SESSION_SECS
            ],
        )?;
        if conn.changes() == 0 {
            return Err(AppError::new(403, "Not a member of that household"));
        }
        // Opportunistically clear out expired sessions
        conn.execute("DELETE FROM sessions WHERE expires_at < ?1", [now])?;
        Ok(token)
    }

    /// The live session for a cookie token, if any; keeps it alive while it's used.
    pub fn session(&self, token: &str) -> AppResult<Option<Session>> {
        let now = now_secs();
        let conn = self.lock();
        let found = conn
            .query_row(
                "SELECT s.id, s.user_id, u.email, u.name, s.household_id, h.name, m.role,
                        s.last_seen_at
                 FROM sessions s
                 JOIN users u ON u.id = s.user_id
                 JOIN households h ON h.id = s.household_id
                 JOIN household_members m ON m.household_id = s.household_id AND m.user_id = s.user_id
                 WHERE s.token_hash = ?1 AND s.expires_at > ?2",
                params![sha256_hex(token), now],
                |r| {
                    Ok((
                        Session {
                            id: r.get(0)?,
                            user_id: r.get(1)?,
                            email: r.get(2)?,
                            name: r.get(3)?,
                            household_id: r.get(4)?,
                            household_name: r.get(5)?,
                            role: r.get(6)?,
                        },
                        r.get::<_, i64>(7)?,
                    ))
                },
            )
            .optional()?;
        let Some((session, last_seen)) = found else {
            return Ok(None);
        };
        if now - last_seen >= TOUCH_EVERY_SECS {
            conn.execute(
                "UPDATE sessions SET last_seen_at = ?1, expires_at = ?2 WHERE id = ?3",
                params![now, now + SESSION_SECS, session.id],
            )?;
        }
        Ok(Some(session))
    }

    /// Signs a cookie's session out.
    pub fn end_session(&self, token: &str) -> AppResult<()> {
        self.lock().execute(
            "DELETE FROM sessions WHERE token_hash = ?1",
            [sha256_hex(token)],
        )?;
        Ok(())
    }

    /// A person's live sessions, newest use first.
    pub fn sessions(&self, current: &Session) -> AppResult<Vec<SessionInfo>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT id, user_agent, created_at, last_seen_at FROM sessions
             WHERE user_id = ?1 AND expires_at > ?2 ORDER BY last_seen_at DESC, id DESC",
        )?;
        let rows = stmt.query_map(params![current.user_id, now_secs()], |r| {
            let id: i64 = r.get(0)?;
            Ok(SessionInfo {
                id,
                user_agent: r.get(1)?,
                created_at: r.get(2)?,
                last_seen_at: r.get(3)?,
                current: id == current.id,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Signs out one of the person's own sessions. False when there's no such session.
    pub fn revoke(&self, current: &Session, id: i64) -> AppResult<bool> {
        let n = self.lock().execute(
            "DELETE FROM sessions WHERE id = ?1 AND user_id = ?2",
            params![id, current.user_id],
        )?;
        Ok(n > 0)
    }

    /// Signs the person out everywhere but here. Returns how many sessions ended.
    pub fn revoke_others(&self, current: &Session) -> AppResult<usize> {
        Ok(self.lock().execute(
            "DELETE FROM sessions WHERE user_id = ?1 AND id != ?2",
            params![current.user_id, current.id],
        )?)
    }
}

fn insert_user(
    tx: &rusqlite::Transaction,
    email: &str,
    name: &str,
    hash: &str,
    now: i64,
) -> AppResult<i64> {
    let taken: bool = tx
        .query_row(
            "SELECT 1 FROM users WHERE lower(email) = lower(?1)",
            [email],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if taken {
        return Err(AppError::new(
            409,
            "email: There's already an account with that email",
        ));
    }
    tx.execute(
        "INSERT INTO users (email, name, password_hash, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![email, name, hash, now],
    )?;
    Ok(tx.last_insert_rowid())
}

fn household_name(owner: &str) -> String {
    let first = owner.split_whitespace().next().unwrap_or(owner);
    format!("{first}'s kitchen")
}

fn valid_email(email: &str) -> AppResult<String> {
    let email = email.trim();
    let well_formed = email.len() <= 254
        && !email.chars().any(char::is_whitespace)
        && email.split_once('@').is_some_and(|(user, domain)| {
            !user.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
        });
    if !well_formed {
        return Err(AppError::bad_request("email: Enter a valid email address"));
    }
    Ok(email.to_string())
}

fn valid_name(name: &str) -> AppResult<String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err(AppError::bad_request("name: Name is required"));
    }
    if name.chars().count() > 80 {
        return Err(AppError::bad_request(
            "name: Name is too long (80 characters max)",
        ));
    }
    Ok(name)
}

fn valid_password(password: &str) -> AppResult<()> {
    let n = password.chars().count();
    if n < MIN_PASSWORD {
        return Err(AppError::bad_request(format!(
            "password: Use at least {MIN_PASSWORD} characters"
        )));
    }
    if n > MAX_PASSWORD {
        return Err(AppError::bad_request(format!(
            "password: Use at most {MAX_PASSWORD} characters"
        )));
    }
    Ok(())
}

/// Argon2id with the crate's defaults, on a blocking thread (it takes tens of ms).
async fn hash_password(password: &str) -> AppResult<String> {
    valid_password(password)?;
    let password = password.to_string();
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(AppError::internal)
    })
    .await
    .map_err(AppError::internal)?
}

async fn verify_password(password: &str, hash: &str) -> bool {
    let (password, hash) = (password.to_string(), hash.to_string());
    tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash).is_ok_and(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
    })
    .await
    .unwrap_or(false)
}

/// A real Argon2id hash (of a random password) checked against when an email is unknown, so
/// a miss takes as long as a wrong password.
static DUMMY_HASH: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(random_token(24).as_bytes(), &salt)
        .map(|h| h.to_string())
        .unwrap_or_default()
});

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn setup_owns_the_home_box_and_later_accounts_get_their_own() {
        let a = Accounts::open_in_memory().unwrap();
        assert!(a.needs_setup().unwrap());
        assert!(a.sign_up("x@a.test", "X", "long enough").await.is_err());
        let owner = a
            .set_up("Ann@Example.com", "Ann Cook", "correct horse")
            .await
            .unwrap();
        assert!(!a.needs_setup().unwrap());
        assert!(a.set_up("b@a.test", "B", "long enough").await.is_err());
        let (user, household) = a
            .authenticate("ann@example.COM", "correct horse")
            .await
            .unwrap()
            .unwrap();
        assert_eq!((user, household), (owner, HOME));
        assert_eq!(
            a.authenticate("ann@example.com", "wrong horse")
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            a.authenticate("nobody@example.com", "correct horse")
                .await
                .unwrap(),
            None
        );

        let (bob, bobs) = a
            .sign_up("bob@example.com", "Bob", "another pass")
            .await
            .unwrap();
        assert_eq!(
            a.authenticate("bob@example.com", "another pass")
                .await
                .unwrap(),
            Some((bob, bobs))
        );
        assert_ne!(bobs, HOME);
        assert_ne!(bob, owner);
        let err = a
            .sign_up("ANN@example.com", "Imposter", "long enough")
            .await
            .unwrap_err();
        assert_eq!(err.status, 409);
    }

    #[tokio::test]
    async fn sessions_are_stored_hashed_and_can_be_revoked() {
        let a = Accounts::open_in_memory().unwrap();
        let ann = a
            .set_up("ann@example.com", "Ann", "correct horse")
            .await
            .unwrap();
        let phone = a.start_session(ann, HOME, Some("Phone")).unwrap();
        let laptop = a.start_session(ann, HOME, Some("Laptop")).unwrap();
        let stored: i64 = a
            .lock()
            .query_row(
                "SELECT count(*) FROM sessions WHERE token_hash = ?1",
                [&phone],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored, 0, "the raw token is never stored");
        let s = a.session(&phone).unwrap().unwrap();
        assert_eq!(
            (s.user_id, s.household_id, s.role.as_str()),
            (ann, HOME, "owner")
        );
        assert_eq!(s.household_name, "Ann's kitchen");
        assert_eq!(a.session("made-up").unwrap(), None);
        let list = a.sessions(&s).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list.iter().filter(|i| i.current).count(), 1);
        assert_eq!(a.revoke_others(&s).unwrap(), 1);
        assert_eq!(a.session(&laptop).unwrap(), None);
        a.end_session(&phone).unwrap();
        assert_eq!(a.session(&phone).unwrap(), None);
        // Not into a household you don't belong to
        assert!(a.start_session(ann, 99, None).is_err());
    }

    #[test]
    fn the_dummy_hash_is_a_real_argon2id_hash() {
        assert!(PasswordHash::new(&DUMMY_HASH).is_ok());
    }

    #[test]
    fn emails_names_and_passwords_are_checked() {
        assert!(valid_email(" ann@example.com ").is_ok());
        for bad in [
            "",
            "ann",
            "@example.com",
            "ann@",
            "ann@example",
            "a nn@example.com",
            "ann@.com",
        ] {
            assert!(valid_email(bad).is_err(), "{bad}");
        }
        assert_eq!(valid_name("  Ann   Cook ").unwrap(), "Ann Cook");
        assert!(valid_name("   ").is_err());
        assert!(valid_password("short").is_err());
        assert!(valid_password("long enough").is_ok());
    }
}
