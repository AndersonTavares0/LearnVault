//! Persistência tipada de usuários e sessões para adaptadores de autenticação.
//!
//! Este módulo não conhece hashing, cookies, HTTP ou políticas de autenticação. O servidor
//! fornece o hash da senha, o hash do token e os instantes já calculados; o core apenas
//! lê e grava os registros nas tabelas existentes do schema V1.

use rusqlite::{OptionalExtension, TransactionBehavior};

use crate::{Database, Error, Result};

pub(crate) const NO_COURSE_ACCESS_SENTINEL: &str = "__none__";

/// Resumo de um usuário local para administração e permissões de cursos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalUserSummary {
    /// Identificador estável do usuário.
    pub user_id: String,
    /// Nome de usuário persistido.
    pub username: String,
    /// Indica se o usuário é administrador.
    pub is_admin: bool,
    /// Segundos Unix de criação da conta.
    pub created_at: i64,
    /// `None` quando o usuário possui acesso irrestrito a todos os cursos;
    /// `Some(ids)` quando o acesso é restrito apenas aos `course_id` listados.
    pub allowed_course_ids: Option<Vec<String>>,
}

/// Usuário persistido necessário para autenticação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthUser {
    /// Identificador estável do usuário.
    pub user_id: String,
    /// Nome de usuário normalizado pelo schema (`COLLATE NOCASE`).
    pub username: String,
    /// Hash PHC da senha, sem a senha em texto claro.
    pub password_hash: String,
    /// Indica se o usuário pode executar ações administrativas.
    pub is_admin: bool,
    /// Indica se o usuário precisa trocar as credenciais temporárias.
    pub must_change_credentials: bool,
}

/// Identidade derivada de uma sessão válida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSession {
    /// Identificador do usuário autenticado.
    pub user_id: String,
    /// Nome de usuário autenticado.
    pub username: String,
    /// Hash PHC da senha persistida, usado apenas para validar a troca inicial.
    pub password_hash: String,
    /// Indica se o usuário autenticado é administrador.
    pub is_admin: bool,
    /// Indica se a sessão está limitada à troca das credenciais temporárias.
    pub must_change_credentials: bool,
    /// Instante de expiração da sessão em segundos Unix.
    pub expires_at: i64,
}

/// Cria o primeiro administrador, sem substituir um administrador existente.
///
/// A verificação e a inserção usam a mesma transação imediata para que dois processos que
/// inicializem o servidor ao mesmo tempo não criem administradores concorrentes.
pub fn bootstrap_admin(
    db: &mut Database,
    user_id: &str,
    username: &str,
    password_hash: &str,
    created_at: i64,
) -> Result<bool> {
    bootstrap_admin_with_change_required(db, user_id, username, password_hash, created_at, true)
}

/// Cria o administrador inicial, permitindo distinguir credencial temporária de configurada.
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

/// Procura um usuário pelo nome, sem revelar se a ausência é de usuário ou senha ao caller.
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

/// Informa se o banco já possui pelo menos um administrador.
pub fn has_admin(db: &Database) -> Result<bool> {
    Ok(db.connection().query_row(
        "SELECT EXISTS (SELECT 1 FROM user WHERE is_admin = 1)",
        [],
        |row| row.get(0),
    )?)
}

/// Grava o hash de um token de sessão até o instante de expiração informado.
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

/// Retorna a identidade associada a uma sessão ainda válida.
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

/// Renova o prazo de uma sessão existente.
pub fn touch_session(db: &Database, token_hash: &str, expires_at: i64) -> Result<usize> {
    Ok(db.connection().execute(
        "UPDATE session SET expires_at = ?2 WHERE token_hash = ?1",
        rusqlite::params![token_hash, expires_at],
    )?)
}

/// Remove as sessões expiradas, mantendo a tabela limitada aos logins ativos.
pub fn prune_expired_sessions(db: &Database, now: i64) -> Result<usize> {
    Ok(db
        .connection()
        .execute("DELETE FROM session WHERE expires_at <= ?1", [now])?)
}

/// Atualiza credenciais e cria a sessão substituta na mesma transação.
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

/// Atualiza as credenciais e invalida as sessões existentes do usuário.
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

/// Invalida uma sessão pelo hash persistido do token.
pub fn delete_session(db: &mut Database, token_hash: &str) -> Result<()> {
    db.connection()
        .execute("DELETE FROM session WHERE token_hash = ?1", [token_hash])?;
    Ok(())
}

/// Lista as permissões explícitas de cursos para um usuário.
///
/// Retorna `Ok(None)` quando o usuário possui acesso irrestrito a todos os cursos, ou
/// `Ok(Some(ids))` quando o acesso está restrito aos cursos listados.
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

/// Verifica se o usuário tem permissão para acessar um curso específico.
///
/// Administradores e usuários sem restrição explícita em `user_course_access` podem acessar
/// qualquer curso; usuários com restrição ativa só acessam os cursos concedidos.
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

/// Lista todos os usuários locais com suas permissões de acesso a cursos.
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

/// Cria uma nova conta local de estudante, aplicando opcionalmente restrições de acesso a cursos.
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

/// Atualiza as permissões de acesso a cursos de um usuário estudante.
///
/// Passar `None` concede acesso irrestrito a todos os cursos; passar `Some(ids)` restringe
/// o acesso apenas aos identificadores informados.
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

/// Remove uma conta local de estudante e invalida suas sessões e permissões.
///
/// Contas de administrador não podem ser removidas por esta operação.
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
