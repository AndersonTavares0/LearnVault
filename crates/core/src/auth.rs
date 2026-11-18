//! Typed persistence of users and sessions for authentication adapters.
//!
//! This module knows nothing of hashing, cookies, HTTP or authentication policies. The server
//! supplies the password hash, the token hash and the instants already computed; the core only
//! reads and writes the records in the existing tables of the V1 schema.

use rusqlite::{OptionalExtension, TransactionBehavior};

use crate::{Database, Error, Result};

pub(crate) const NO_COURSE_ACCESS_SENTINEL: &str = "__none__";

/// Summary of a local user for administration and course permissions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalUserSummary {
    /// Stable identifier of the user.
    pub user_id: String,
    /// Persisted user name.
    pub username: String,
    /// Tells whether the user is an administrator.
    pub is_admin: bool,
    /// Unix seconds of the account's creation.
    pub created_at: i64,
    /// `None` when the user has unrestricted access to every course;
    /// `Some(ids)` when access is restricted to only the listed `course_id`s.
    pub allowed_course_ids: Option<Vec<String>>,
}

/// Persisted user needed for authentication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthUser {
    /// Stable identifier of the user.
    pub user_id: String,
    /// User name normalized by the schema (`COLLATE NOCASE`).
    pub username: String,
    /// PHC hash of the password, without the plain-text password.
    pub password_hash: String,
    /// Tells whether the user may perform administrative actions.
    pub is_admin: bool,
    /// Tells whether the user must change the temporary credentials.
    pub must_change_credentials: bool,
}

/// Identity derived from a valid session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSession {
    /// Identifier of the authenticated user.
    pub user_id: String,
    /// Name of the authenticated user.
    pub username: String,
    /// PHC hash of the persisted password, used only to validate the initial change.
    pub password_hash: String,
    /// Tells whether the authenticated user is an administrator.
    pub is_admin: bool,
    /// Tells whether the session is limited to changing the temporary credentials.
    pub must_change_credentials: bool,
    /// Expiry instant of the session in Unix seconds.
    pub expires_at: i64,
}

/// Creates the first administrator, without replacing an existing administrator.
///
/// The check and the insert use the same immediate transaction so that two processes that
/// start the server at the same time do not create concurrent administrators.
pub fn bootstrap_admin(
    db: &mut Database,
    user_id: &str,
    username: &str,
    password_hash: &str,
    created_at: i64,
) -> Result<bool> {
    bootstrap_admin_with_change_required(db, user_id, username, password_hash, created_at, true)
}

/// Creates the initial administrator, allowing a temporary credential to be told apart from a configured one.
pub fn bootstrap_admin_with_change_required(
    db: &mut Database,
    user_id: &str,
    username: &str,
    password_hash: &str,
    created_at: i64,
    must_change_credentials: bool,
) -> Result<bool> {
    let tx = db
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    let has_admin: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM user WHERE is_admin = 1)",
        [],
        |row| row.get(0),
    )?;

    if has_admin {
        tx.commit()?;
        return Ok(false);
    }

    tx.execute(
        "INSERT INTO user (user_id, username, password_hash, is_admin, created_at, must_change_credentials)
          VALUES (?1, ?2, ?3, 1, ?4, ?5)",
        rusqlite::params![user_id, username, password_hash, created_at, must_change_credentials as i64],
    )?;
    tx.commit()?;
    Ok(true)
}

/// Looks a user up by name, without telling the caller whether the user or the password is the one that is absent.
pub fn find_user_by_username(db: &Database, username: &str) -> Result<Option<AuthUser>> {
    Ok(db
        .connection()
        .query_row(
            "SELECT user_id, username, password_hash, is_admin, must_change_credentials
               FROM user
              WHERE username = ?1",
            [username],
            |row| {
                Ok(AuthUser {
                    user_id: row.get(0)?,
                    username: row.get(1)?,
                    password_hash: row.get(2)?,
                    is_admin: row.get::<_, i64>(3)? != 0,
                    must_change_credentials: row.get::<_, i64>(4)? != 0,
                })
            },
        )
        .optional()?)
}

/// Tells whether the database already has at least one administrator.
pub fn has_admin(db: &Database) -> Result<bool> {
    Ok(db.connection().query_row(
        "SELECT EXISTS (SELECT 1 FROM user WHERE is_admin = 1)",
        [],
        |row| row.get(0),
    )?)
}

/// Stores the hash of a session token until the given expiry instant.
pub fn create_session(
    db: &mut Database,
    token_hash: &str,
    user_id: &str,
    expires_at: i64,
) -> Result<()> {
    db.connection().execute(
        "INSERT INTO session (token_hash, user_id, expires_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![token_hash, user_id, expires_at],
    )?;
    Ok(())
}

/// Returns the identity associated with a session that is still valid.
pub fn session_user(db: &Database, token_hash: &str, now: i64) -> Result<Option<AuthSession>> {
    Ok(db
        .connection()
        .query_row(
            "SELECT s.user_id, u.username, u.password_hash, u.is_admin, u.must_change_credentials, s.expires_at
               FROM session s
               JOIN user u ON u.user_id = s.user_id
              WHERE s.token_hash = ?1
                AND s.expires_at > ?2",
            rusqlite::params![token_hash, now],
            |row| {
                Ok(AuthSession {
                    user_id: row.get(0)?,
                    username: row.get(1)?,
                    password_hash: row.get(2)?,
                    is_admin: row.get::<_, i64>(3)? != 0,
                    must_change_credentials: row.get::<_, i64>(4)? != 0,
                    expires_at: row.get(5)?,
                })
            },
        )
        .optional()?)
}

/// Renews the deadline of an existing session.
pub fn touch_session(db: &Database, token_hash: &str, expires_at: i64) -> Result<usize> {
    Ok(db.connection().execute(
        "UPDATE session SET expires_at = ?2 WHERE token_hash = ?1",
        rusqlite::params![token_hash, expires_at],
    )?)
}

/// Removes expired sessions, keeping the table limited to active logins.
pub fn prune_expired_sessions(db: &Database, now: i64) -> Result<usize> {
    Ok(db
        .connection()
        .execute("DELETE FROM session WHERE expires_at <= ?1", [now])?)
}

/// Updates credentials and creates the replacement session in the same transaction.
pub fn update_credentials_and_session(
    db: &mut Database,
    user_id: &str,
    username: &str,
    password_hash: &str,
    token_hash: &str,
    expires_at: i64,
) -> Result<()> {
    let tx = db
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute("UPDATE user SET username = ?1, password_hash = ?2, must_change_credentials = 0 WHERE user_id = ?3", rusqlite::params![username, password_hash, user_id])?;
    tx.execute("DELETE FROM session WHERE user_id = ?1", [user_id])?;
    tx.execute(
        "INSERT INTO session (token_hash, user_id, expires_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![token_hash, user_id, expires_at],
    )?;
    tx.commit()?;
    Ok(())
}

/// Updates the credentials and invalidates the user's existing sessions.
pub fn update_credentials(
    db: &mut Database,
    user_id: &str,
    username: &str,
    password_hash: &str,
) -> Result<()> {
    let tx = db
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute("UPDATE user SET username = ?1, password_hash = ?2, must_change_credentials = 0 WHERE user_id = ?3", rusqlite::params![username, password_hash, user_id])?;
    tx.execute("DELETE FROM session WHERE user_id = ?1", [user_id])?;
    tx.commit()?;
    Ok(())
}

/// Invalidates a session by the persisted hash of its token.
pub fn delete_session(db: &mut Database, token_hash: &str) -> Result<()> {
    db.connection()
        .execute("DELETE FROM session WHERE token_hash = ?1", [token_hash])?;
    Ok(())
}

/// Lists the explicit course permissions for a user.
///
/// Returns `Ok(None)` when the user has unrestricted access to every course, or
/// `Ok(Some(ids))` when access is restricted to the listed courses.
pub fn list_user_course_access(db: &Database, user_id: &str) -> Result<Option<Vec<String>>> {
    let mut stmt = db.connection().prepare(
        "SELECT course_id FROM user_course_access WHERE user_id = ?1 ORDER BY course_id ASC",
    )?;
    let rows: Vec<String> = stmt
        .query_map([user_id], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if rows.is_empty() {
        return Ok(None);
    }
    let filtered = rows
        .into_iter()
        .filter(|id| id != NO_COURSE_ACCESS_SENTINEL)
        .collect();
    Ok(Some(filtered))
}

/// Checks whether the user is allowed to access a specific course.
///
/// Administrators and users with no explicit restriction in `user_course_access` can access
/// any course; users with an active restriction only access the courses granted.
pub fn can_user_access_course(
    db: &Database,
    user_id: &str,
    is_admin: bool,
    course_id: &str,
) -> Result<bool> {
    if is_admin {
        return Ok(true);
    }
    match list_user_course_access(db, user_id)? {
        None => Ok(true),
        Some(allowed) => Ok(allowed.iter().any(|item| item == course_id)),
    }
}

/// Lists all local users with their course-access permissions.
pub fn list_users(db: &Database) -> Result<Vec<LocalUserSummary>> {
    let mut stmt = db.connection().prepare(
        "SELECT user_id, username, is_admin, created_at
           FROM user
          ORDER BY is_admin DESC, created_at ASC, username ASC",
    )?;
    let base: Vec<(String, String, bool, i64)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get::<_, i64>(2)? != 0,
                row.get(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut users = Vec::with_capacity(base.len());
    for (user_id, username, is_admin, created_at) in base {
        let allowed_course_ids = if is_admin {
            None
        } else {
            list_user_course_access(db, &user_id)?
        };
        users.push(LocalUserSummary {
            user_id,
            username,
            is_admin,
            created_at,
            allowed_course_ids,
        });
    }
    Ok(users)
}

fn normalize_course_ids(ids: &[String]) -> Vec<String> {
    let mut normalized: Vec<String> = ids
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && s.len() <= 256 && *s != NO_COURSE_ACCESS_SENTINEL)
        .map(str::to_owned)
        .collect();
    normalized.sort();
    normalized.dedup();
    normalized
}

/// Creates a new local student account, optionally applying course-access restrictions.
pub fn create_user(
    db: &mut Database,
    user_id: &str,
    username: &str,
    password_hash: &str,
    created_at: i64,
    allowed_course_ids: Option<&[String]>,
) -> Result<LocalUserSummary> {
    let tx = db
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    let exists: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM user WHERE username = ?1)",
        [username],
        |row| row.get(0),
    )?;
    if exists {
        return Err(Error::UsernameTaken);
    }

    tx.execute(
        "INSERT INTO user (user_id, username, password_hash, is_admin, created_at, must_change_credentials)
          VALUES (?1, ?2, ?3, 0, ?4, 0)",
        rusqlite::params![user_id, username, password_hash, created_at],
    )?;

    let final_allowed = if let Some(ids) = allowed_course_ids {
        let normalized = normalize_course_ids(ids);
        if normalized.is_empty() {
            tx.execute(
                "INSERT INTO user_course_access (user_id, course_id) VALUES (?1, ?2)",
                rusqlite::params![user_id, NO_COURSE_ACCESS_SENTINEL],
            )?;
        } else {
            for course_id in &normalized {
                tx.execute(
                    "INSERT INTO user_course_access (user_id, course_id) VALUES (?1, ?2)",
                    rusqlite::params![user_id, course_id],
                )?;
            }
        }
        Some(normalized)
    } else {
        None
    };

    tx.commit()?;
    Ok(LocalUserSummary {
        user_id: user_id.to_owned(),
        username: username.to_owned(),
        is_admin: false,
        created_at,
        allowed_course_ids: final_allowed,
    })
}

/// Updates the course-access permissions of a student user.
///
/// Passing `None` grants unrestricted access to every course; passing `Some(ids)` restricts
/// access to only the given identifiers.
pub fn set_user_course_access(
    db: &mut Database,
    user_id: &str,
    allowed_course_ids: Option<&[String]>,
) -> Result<Option<Vec<String>>> {
    let tx = db
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    let is_admin: Option<bool> = tx
        .query_row(
            "SELECT is_admin FROM user WHERE user_id = ?1",
            [user_id],
            |row| Ok(row.get::<_, i64>(0)? != 0),
        )
        .optional()?;
    let Some(is_admin) = is_admin else {
        return Err(Error::UserNotFound);
    };
    if is_admin {
        return Err(Error::CannotModifyAdminUser);
    }

    tx.execute(
        "DELETE FROM user_course_access WHERE user_id = ?1",
        [user_id],
    )?;

    let final_allowed = if let Some(ids) = allowed_course_ids {
        let normalized = normalize_course_ids(ids);
        if normalized.is_empty() {
            tx.execute(
                "INSERT INTO user_course_access (user_id, course_id) VALUES (?1, ?2)",
                rusqlite::params![user_id, NO_COURSE_ACCESS_SENTINEL],
            )?;
        } else {
            for course_id in &normalized {
                tx.execute(
                    "INSERT INTO user_course_access (user_id, course_id) VALUES (?1, ?2)",
                    rusqlite::params![user_id, course_id],
                )?;
            }
        }
        Some(normalized)
    } else {
        None
    };

    tx.commit()?;
    Ok(final_allowed)
}

/// Removes a local student account and invalidates its sessions and permissions.
///
/// Administrator accounts cannot be removed by this operation.
pub fn delete_user(db: &mut Database, user_id: &str) -> Result<bool> {
    let tx = db
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    let is_admin: Option<bool> = tx
        .query_row(
            "SELECT is_admin FROM user WHERE user_id = ?1",
            [user_id],
            |row| Ok(row.get::<_, i64>(0)? != 0),
        )
        .optional()?;
    let Some(is_admin) = is_admin else {
        return Ok(false);
    };
    if is_admin {
        return Err(Error::CannotModifyAdminUser);
    }

    tx.execute("DELETE FROM session WHERE user_id = ?1", [user_id])?;
    tx.execute(
        "DELETE FROM user_course_access WHERE user_id = ?1",
        [user_id],
    )?;
    tx.execute("DELETE FROM user WHERE user_id = ?1", [user_id])?;
    tx.commit()?;
    Ok(true)
}
