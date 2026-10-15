//! Persistência tipada de usuários e sessões para adaptadores de autenticação.
//!
//! Este módulo não conhece hashing, cookies, HTTP ou políticas de autenticação. O servidor
//! fornece o hash da senha, o hash do token e os instantes já calculados; o core apenas
//! lê e grava os registros nas tabelas existentes do schema V1.

use rusqlite::{OptionalExtension, TransactionBehavior};

use crate::{Database, Result};

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
            "SELECT s.user_id, u.username, u.is_admin, u.must_change_credentials, s.expires_at
               FROM session s
               JOIN user u ON u.user_id = s.user_id
              WHERE s.token_hash = ?1
                AND s.expires_at > ?2",
            rusqlite::params![token_hash, now],
            |row| {
                Ok(AuthSession {
                    user_id: row.get(0)?,
                    username: row.get(1)?,
                    is_admin: row.get::<_, i64>(2)? != 0,
                    must_change_credentials: row.get::<_, i64>(3)? != 0,
                    expires_at: row.get(4)?,
                })
            },
        )
        .optional()?)
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
