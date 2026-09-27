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
/// How long an invite link works.
pub const INVITE_SECS: i64 = 60 * 60 * 24 * 7;
/// Pending invites a household may have at once.
const MAX_PENDING_INVITES: i64 = 20;
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
  );

  -- One-time links an owner hands someone to join their household (only the hash is kept)
  CREATE TABLE IF NOT EXISTS invitations (
    id integer PRIMARY KEY AUTOINCREMENT NOT NULL,
    token_hash text NOT NULL,
    household_id integer NOT NULL REFERENCES households(id) ON DELETE CASCADE,
    role text NOT NULL DEFAULT 'member' CHECK (role IN ('owner', 'member')),
    created_by integer REFERENCES users(id) ON DELETE SET NULL,
    created_at integer NOT NULL,
    expires_at integer NOT NULL,
    accepted_at integer,
    accepted_by integer REFERENCES users(id) ON DELETE SET NULL
  );
  CREATE UNIQUE INDEX IF NOT EXISTS invitations_token_unique ON invitations (token_hash);
  CREATE INDEX IF NOT EXISTS invitations_household_idx ON invitations (household_id);

  -- The hosted edition (AUTH_MODE=hosted): people are Better Auth users, known here only by
  -- a local id for the connector tokens they approve
  CREATE TABLE IF NOT EXISTS hosted_users (
    id integer PRIMARY KEY AUTOINCREMENT NOT NULL,
    external_id text NOT NULL,
    created_at integer NOT NULL
  );
  CREATE UNIQUE INDEX IF NOT EXISTS hosted_users_external_unique ON hosted_users (external_id);";

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

/// Someone in a household, for the members list.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub user_id: i64,
    pub name: String,
    pub email: String,
    pub role: String,
    pub joined_at: i64,
}

/// A household someone belongs to, for switching between them.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Membership {
    pub id: HouseholdId,
    pub name: String,
    pub role: String,
}

/// An invite link that hasn't been used yet.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Invite {
    pub id: i64,
    pub created_by: Option<String>,
    pub created_at: i64,
    pub expires_at: i64,
}

/// What an invite link's page shows before it's accepted.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitePreview {
    pub household_name: String,
    pub invited_by: Option<String>,
    pub expires_at: i64,
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
        // Hosted households are Better Auth organizations: `external_id` is the organization's
        let has_external: bool = conn
            .prepare("SELECT 1 FROM pragma_table_info('households') WHERE name = 'external_id'")?
            .exists([])?;
        if !has_external {
            conn.execute_batch("ALTER TABLE households ADD COLUMN external_id text;")?;
        }
        conn.execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS households_external_unique
               ON households (external_id) WHERE external_id IS NOT NULL;",
        )?;
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

    /// A new invite link to the signed-in owner's household: its token (shown once, only
    /// its hash is kept) and the invite.
    pub fn create_invite(&self, signed: &Session) -> AppResult<(String, Invite)> {
        require_owner(signed)?;
        let token = random_token(24);
        let now = now_secs();
        let conn = self.lock();
        // Used and expired links are only kept until the next one is made
        conn.execute(
            "DELETE FROM invitations WHERE household_id = ?1
               AND (accepted_at IS NOT NULL OR expires_at <= ?2)",
            params![signed.household_id, now],
        )?;
        let pending: i64 = conn.query_row(
            "SELECT count(*) FROM invitations WHERE household_id = ?1",
            [signed.household_id],
            |r| r.get(0),
        )?;
        if pending >= MAX_PENDING_INVITES {
            return Err(AppError::new(
                429,
                "There are a lot of unused invites. Cancel some first.",
            ));
        }
        conn.execute(
            "INSERT INTO invitations (token_hash, household_id, role, created_by, created_at,
               expires_at)
             VALUES (?1, ?2, 'member', ?3, ?4, ?5)",
            params![
                sha256_hex(&token),
                signed.household_id,
                signed.user_id,
                now,
                now + INVITE_SECS
            ],
        )?;
        let invite = Invite {
            id: conn.last_insert_rowid(),
            created_by: Some(signed.name.clone()),
            created_at: now,
            expires_at: now + INVITE_SECS,
        };
        Ok((token, invite))
    }

    /// The owner's household's invite links that can still be used, newest first.
    pub fn invites(&self, signed: &Session) -> AppResult<Vec<Invite>> {
        require_owner(signed)?;
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT i.id, u.name, i.created_at, i.expires_at FROM invitations i
             LEFT JOIN users u ON u.id = i.created_by
             WHERE i.household_id = ?1 AND i.accepted_at IS NULL AND i.expires_at > ?2
             ORDER BY i.created_at DESC, i.id DESC",
        )?;
        let rows = stmt.query_map(params![signed.household_id, now_secs()], |r| {
            Ok(Invite {
                id: r.get(0)?,
                created_by: r.get(1)?,
                created_at: r.get(2)?,
                expires_at: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Stops an invite link of the owner's household. False when there's no such invite.
    pub fn cancel_invite(&self, signed: &Session, id: i64) -> AppResult<bool> {
        require_owner(signed)?;
        let n = self.lock().execute(
            "DELETE FROM invitations WHERE id = ?1 AND household_id = ?2 AND accepted_at IS NULL",
            params![id, signed.household_id],
        )?;
        Ok(n > 0)
    }

    /// What an invite link opens, while it can still be used.
    pub fn preview_invite(&self, token: &str) -> AppResult<Option<InvitePreview>> {
        Ok(self
            .lock()
            .query_row(
                "SELECT h.name, u.name, i.expires_at FROM invitations i
                 JOIN households h ON h.id = i.household_id
                 LEFT JOIN users u ON u.id = i.created_by
                 WHERE i.token_hash = ?1 AND i.accepted_at IS NULL AND i.expires_at > ?2",
                params![sha256_hex(token), now_secs()],
                |r| {
                    Ok(InvitePreview {
                        household_name: r.get(0)?,
                        invited_by: r.get(1)?,
                        expires_at: r.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    /// Joins `user` to the household an invite link is for, using up the link. Someone
    /// already in it just gets its id back (and the link stays unused).
    pub fn accept_invite(&self, token: &str, user: i64) -> AppResult<HouseholdId> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let household = join_by_invite(&tx, token, user)?;
        tx.commit()?;
        Ok(household)
    }

    /// A new account that joins the household an invite link is for (and gets no household
    /// of its own), whether or not sign-up is open: the account's id and the household's.
    pub async fn sign_up_invited(
        &self,
        token: &str,
        email: &str,
        name: &str,
        password: &str,
    ) -> AppResult<(i64, HouseholdId)> {
        let (email, name) = (valid_email(email)?, valid_name(name)?);
        if self.preview_invite(token)?.is_none() {
            return Err(invite_gone());
        }
        let hash = hash_password(password).await?;
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let user = insert_user(&tx, &email, &name, &hash, now_secs())?;
        let household = join_by_invite(&tx, token, user)?;
        tx.commit()?;
        Ok((user, household))
    }

    /// Everyone in the signed-in household, owners first.
    pub fn members(&self, signed: &Session) -> AppResult<Vec<Member>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT u.id, u.name, u.email, m.role, m.created_at FROM household_members m
             JOIN users u ON u.id = m.user_id
             WHERE m.household_id = ?1
             ORDER BY m.role = 'owner' DESC, m.created_at, u.id",
        )?;
        let rows = stmt.query_map([signed.household_id], |r| {
            Ok(Member {
                user_id: r.get(0)?,
                name: r.get(1)?,
                email: r.get(2)?,
                role: r.get(3)?,
                joined_at: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The owner takes a member out of their household. The member's sessions there move to
    /// another household of theirs (a new, empty one if they have none), and connectors they
    /// approved for it stop working (see `oauth::access_household`).
    pub fn remove_member(&self, signed: &Session, user: i64) -> AppResult<()> {
        require_owner(signed)?;
        if user == signed.user_id {
            return Err(AppError::bad_request("You can't remove yourself"));
        }
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let role: Option<String> = tx
            .query_row(
                "SELECT role FROM household_members WHERE household_id = ?1 AND user_id = ?2",
                params![signed.household_id, user],
                |r| r.get(0),
            )
            .optional()?;
        match role.as_deref() {
            None => return Err(AppError::not_found("Member not found")),
            Some("owner") => return Err(AppError::new(403, "An owner can't be removed")),
            Some(_) => {}
        }
        move_out(&tx, user, signed.household_id)?;
        tx.commit()?;
        Ok(())
    }

    /// A member leaves the signed-in household; their sessions carry on in another household
    /// of theirs (a new, empty one if they have none), which is returned. Owners can't leave.
    pub fn leave(&self, signed: &Session) -> AppResult<HouseholdId> {
        if signed.role == "owner" {
            return Err(AppError::new(
                403,
                "The owner can't leave their own household",
            ));
        }
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let next = move_out(&tx, signed.user_id, signed.household_id)?;
        tx.commit()?;
        Ok(next)
    }

    /// Every household a person is in, the first joined first.
    pub fn memberships(&self, user: i64) -> AppResult<Vec<Membership>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT h.id, h.name, m.role FROM household_members m
             JOIN households h ON h.id = m.household_id
             WHERE m.user_id = ?1 ORDER BY m.created_at, h.id",
        )?;
        let rows = stmt.query_map([user], |r| {
            Ok(Membership {
                id: r.get(0)?,
                name: r.get(1)?,
                role: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Points the signed-in session at another of the person's households.
    pub fn switch_household(&self, signed: &Session, household: HouseholdId) -> AppResult<()> {
        let n = self.lock().execute(
            "UPDATE sessions SET household_id = ?1 WHERE id = ?2 AND EXISTS (
               SELECT 1 FROM household_members WHERE user_id = ?3 AND household_id = ?1)",
            params![household, signed.id, signed.user_id],
        )?;
        if n == 0 {
            return Err(AppError::new(403, "Not a member of that household"));
        }
        Ok(())
    }

    /// The local id for a hosted (Better Auth) user, made on first sight.
    pub fn hosted_user(&self, external: &str) -> AppResult<i64> {
        let conn = self.lock();
        conn.execute(
            "INSERT OR IGNORE INTO hosted_users (external_id, created_at) VALUES (?1, ?2)",
            params![external, now_secs()],
        )?;
        Ok(conn.query_row(
            "SELECT id FROM hosted_users WHERE external_id = ?1",
            [external],
            |r| r.get(0),
        )?)
    }

    /// The local id (and so the recipe file) for a hosted household (a Better Auth
    /// organization), made on first sight and renamed when its name changes. Household 1,
    /// the database Crumb already had, only ever goes to the organization `claim_home` names
    /// (see `Config::hosted_home_owner`); every other organization starts with an empty box.
    pub fn hosted_household(
        &self,
        external: &str,
        name: &str,
        claim_home: bool,
    ) -> AppResult<HouseholdId> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let now = now_secs();
        // Keep id 1 from ever being handed out by AUTOINCREMENT
        tx.execute(
            "INSERT OR IGNORE INTO households (id, name, created_at) VALUES (?1, 'Home', ?2)",
            params![HOME, now],
        )?;
        let found: Option<(HouseholdId, String)> = tx
            .query_row(
                "SELECT id, name FROM households WHERE external_id = ?1",
                [external],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let id = match found {
            Some((id, old)) => {
                if old != name {
                    tx.execute(
                        "UPDATE households SET name = ?1 WHERE id = ?2",
                        params![name, id],
                    )?;
                }
                id
            }
            None => {
                let claimed = claim_home
                    && tx.execute(
                        "UPDATE households SET external_id = ?1, name = ?2
                         WHERE id = ?3 AND external_id IS NULL",
                        params![external, name, HOME],
                    )? > 0;
                if claimed {
                    HOME
                } else {
                    tx.execute(
                        "INSERT INTO households (name, created_at, external_id) VALUES (?1, ?2, ?3)",
                        params![name, now, external],
                    )?;
                    tx.last_insert_rowid()
                }
            }
        };
        tx.commit()?;
        Ok(id)
    }

    /// The Better Auth ids behind a hosted user and household, to ask the auth service
    /// whether one is still in the other. None when either isn't a hosted one.
    pub fn hosted_ids(
        &self,
        user: i64,
        household: HouseholdId,
    ) -> AppResult<Option<(String, String)>> {
        let conn = self.lock();
        let user: Option<String> = conn
            .query_row(
                "SELECT external_id FROM hosted_users WHERE id = ?1",
                [user],
                |r| r.get(0),
            )
            .optional()?;
        let household: Option<String> = conn
            .query_row(
                "SELECT external_id FROM households WHERE id = ?1",
                [household],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        Ok(user.zip(household))
    }

    /// Renames the signed-in owner's household.
    pub fn rename_household(&self, signed: &Session, name: &str) -> AppResult<String> {
        require_owner(signed)?;
        let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
        if name.is_empty() {
            return Err(AppError::bad_request("name: Name is required"));
        }
        if name.chars().count() > 80 {
            return Err(AppError::bad_request(
                "name: Name is too long (80 characters max)",
            ));
        }
        self.lock().execute(
            "UPDATE households SET name = ?1 WHERE id = ?2",
            params![name, signed.household_id],
        )?;
        Ok(name)
    }
}

fn require_owner(signed: &Session) -> AppResult<()> {
    if signed.role != "owner" {
        return Err(AppError::new(403, "Only the household's owner can do that"));
    }
    Ok(())
}

fn invite_gone() -> AppError {
    AppError::new(
        410,
        "This invite has expired or was already used. Ask for a new one.",
    )
}

/// Adds `user` to an invite's household and uses the invite up (see
/// [`Accounts::accept_invite`]).
fn join_by_invite(tx: &rusqlite::Transaction, token: &str, user: i64) -> AppResult<HouseholdId> {
    let now = now_secs();
    let found: Option<(i64, HouseholdId, String)> = tx
        .query_row(
            "SELECT id, household_id, role FROM invitations
             WHERE token_hash = ?1 AND accepted_at IS NULL AND expires_at > ?2",
            params![sha256_hex(token), now],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((invite, household, role)) = found else {
        return Err(invite_gone());
    };
    let joined = tx.execute(
        "INSERT OR IGNORE INTO household_members (household_id, user_id, role, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![household, user, role, now],
    )?;
    if joined > 0 {
        tx.execute(
            "UPDATE invitations SET accepted_at = ?1, accepted_by = ?2 WHERE id = ?3",
            params![now, user, invite],
        )?;
    }
    Ok(household)
}

/// Takes `user` out of `household`, moving their sessions there to another household of
/// theirs, or a new one of their own so they can still sign in. Returns where they went.
fn move_out(
    tx: &rusqlite::Transaction,
    user: i64,
    household: HouseholdId,
) -> AppResult<HouseholdId> {
    tx.execute(
        "DELETE FROM household_members WHERE household_id = ?1 AND user_id = ?2",
        params![household, user],
    )?;
    let other: Option<HouseholdId> = tx
        .query_row(
            "SELECT household_id FROM household_members WHERE user_id = ?1
             ORDER BY created_at, household_id LIMIT 1",
            [user],
            |r| r.get(0),
        )
        .optional()?;
    let next = match other {
        Some(h) => h,
        None => {
            let name: String =
                tx.query_row("SELECT name FROM users WHERE id = ?1", [user], |r| r.get(0))?;
            let now = now_secs();
            tx.execute(
                "INSERT INTO households (name, created_at) VALUES (?1, ?2)",
                params![household_name(&name), now],
            )?;
            let own = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO household_members (household_id, user_id, role, created_at)
                 VALUES (?1, ?2, 'owner', ?3)",
                params![own, user, now],
            )?;
            own
        }
    };
    tx.execute(
        "UPDATE sessions SET household_id = ?1 WHERE user_id = ?2 AND household_id = ?3",
        params![next, user, household],
    )?;
    Ok(next)
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
