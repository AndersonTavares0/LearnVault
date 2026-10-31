//! Contratos de persistência tipada para autenticação e sessões.

use learnvault_core::{
    AuthSession, AuthUser, Database, bootstrap_admin, create_session, delete_session,
    find_user_by_username, session_user, update_credentials,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const USER_ID: &str = "usr-admin";
const USERNAME: &str = "admin";
const PASSWORD_HASH: &str = "$argon2id$v=19$m=65536,t=2,p=1$c2FsdA$hash";
const TOKEN_HASH: &str = "token-hash-sha256";

#[test]
fn existing_auth_tables_are_available_without_a_new_migration() -> TestResult {
    let db = Database::open_in_memory()?;

    assert_eq!(db.schema_version()?, 8);
    for table in ["user", "session"] {
        let exists: i64 = db.connection().query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )?;
        assert_eq!(
            exists, 1,
            "a tabela {table} deve continuar sendo reutilizada"
        );
    }
    Ok(())
}

#[test]
fn bootstrap_admin_is_idempotent_and_lookup_is_case_insensitive() -> TestResult {
    let mut db = Database::open_in_memory()?;

    assert!(bootstrap_admin(
        &mut db,
        USER_ID,
        USERNAME,
        PASSWORD_HASH,
        1_700_000_000,
    )?);
    assert!(!bootstrap_admin(
        &mut db,
        "usr-other",
        "ADMIN",
        "another-hash",
        1_700_000_001,
    )?);

    let user: AuthUser = find_user_by_username(&db, "ADMIN")?.expect("admin deve existir");
    assert_eq!(user.user_id, USER_ID);
    assert_eq!(user.username, USERNAME);
    assert_eq!(user.password_hash, PASSWORD_HASH);
    assert!(user.is_admin);
    assert!(user.must_change_credentials);
    Ok(())
}

#[test]
fn changing_credentials_is_atomic_and_invalidates_previous_sessions() -> TestResult {
    let mut db = Database::open_in_memory()?;
    bootstrap_admin(&mut db, USER_ID, USERNAME, PASSWORD_HASH, 1_700_000_000)?;
    create_session(&mut db, TOKEN_HASH, USER_ID, 2_000)?;
    update_credentials(&mut db, USER_ID, "owner", "new-hash")?;
    let user = find_user_by_username(&db, "owner")?.unwrap();
    assert!(!user.must_change_credentials);
    assert!(session_user(&db, TOKEN_HASH, 1_999)?.is_none());
    Ok(())
}

#[test]
fn sessions_expire_and_store_only_the_token_hash() -> TestResult {
    let mut db = Database::open_in_memory()?;
    bootstrap_admin(&mut db, USER_ID, USERNAME, PASSWORD_HASH, 1_700_000_000)?;

    create_session(&mut db, TOKEN_HASH, USER_ID, 2_000)?;

    let active: AuthSession = session_user(&db, TOKEN_HASH, 1_999)?.expect("sessão ativa");
    assert_eq!(active.user_id, USER_ID);
    assert_eq!(active.username, USERNAME);
    assert!(active.is_admin);
    assert_eq!(active.expires_at, 2_000);
    assert!(session_user(&db, TOKEN_HASH, 2_000)?.is_none());

    let stored: String = db.connection().query_row(
        "SELECT token_hash FROM session WHERE user_id = ?1",
        [USER_ID],
        |row| row.get(0),
    )?;
    assert_eq!(stored, TOKEN_HASH);
    Ok(())
}

#[test]
fn deleting_a_session_invalidates_it() -> TestResult {
    let mut db = Database::open_in_memory()?;
    bootstrap_admin(&mut db, USER_ID, USERNAME, PASSWORD_HASH, 1_700_000_000)?;
    create_session(&mut db, TOKEN_HASH, USER_ID, 2_000)?;

    delete_session(&mut db, TOKEN_HASH)?;

    assert!(session_user(&db, TOKEN_HASH, 1_999)?.is_none());
    Ok(())
}
