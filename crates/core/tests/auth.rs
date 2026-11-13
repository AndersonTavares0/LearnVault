//! Contratos de persistência tipada para autenticação e sessões.

use learnvault_core::{
    AuthSession, AuthUser, Database, Error, bootstrap_admin, can_user_access_course,
    create_session, create_user, delete_session, delete_user, find_user_by_username, list_users,
    prune_expired_sessions, session_user, set_user_course_access, touch_session,
    update_credentials,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const USER_ID: &str = "usr-admin";
const USERNAME: &str = "admin";
const PASSWORD_HASH: &str = "$argon2id$v=19$m=65536,t=2,p=1$c2FsdA$hash";
const TOKEN_HASH: &str = "token-hash-sha256";

#[test]
fn existing_auth_tables_are_available_without_a_new_migration() -> TestResult {
    let db = Database::open_in_memory()?;

    assert_eq!(db.schema_version()?, 18);
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
fn touches_a_session_forward_and_prunes_only_expired_rows() -> TestResult {
    let mut db = Database::open_in_memory()?;
    bootstrap_admin(&mut db, USER_ID, USERNAME, PASSWORD_HASH, 1_700_000_000)?;
    create_session(&mut db, TOKEN_HASH, USER_ID, 2_000)?;
    create_session(&mut db, "expired-hash", USER_ID, 900)?;

    assert_eq!(touch_session(&db, TOKEN_HASH, 3_000)?, 1);
    assert_eq!(
        session_user(&db, TOKEN_HASH, 2_999)?.map(|s| s.expires_at),
        Some(3_000)
    );

    assert_eq!(prune_expired_sessions(&db, 1_000)?, 1);
    assert!(session_user(&db, "expired-hash", 1_000)?.is_none());
    assert!(session_user(&db, TOKEN_HASH, 1_000)?.is_some());
    assert_eq!(prune_expired_sessions(&db, 1_000)?, 0);
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

#[test]
fn local_users_and_course_permissions_lifecycle() -> TestResult {
    let mut db = Database::open_in_memory()?;
    bootstrap_admin(&mut db, USER_ID, USERNAME, PASSWORD_HASH, 1_700_000_000)?;

    // Cria estudante com acesso irrestrito inicial (None)
    let alice = create_user(
        &mut db,
        "usr-alice",
        "alice",
        PASSWORD_HASH,
        1_700_000_100,
        None,
    )?;
    assert_eq!(alice.username, "alice");
    assert!(!alice.is_admin);
    assert_eq!(alice.allowed_course_ids, None);
    assert!(can_user_access_course(&db, "usr-alice", false, "course-a")?);
    assert!(can_user_access_course(&db, "usr-alice", false, "course-b")?);

    // Duplicidade case-insensitive é recusada
    let dup = create_user(
        &mut db,
        "usr-alice-2",
        "ALICE",
        PASSWORD_HASH,
        1_700_000_101,
        None,
    );
    assert!(matches!(dup, Err(Error::UsernameTaken)));

    // Restringe Alice apenas a course-a
    let restricted = set_user_course_access(&mut db, "usr-alice", Some(&["course-a".to_owned()]))?;
    assert_eq!(restricted, Some(vec!["course-a".to_owned()]));
    assert!(can_user_access_course(&db, "usr-alice", false, "course-a")?);
    assert!(!can_user_access_course(
        &db,
        "usr-alice",
        false,
        "course-b"
    )?);

    // Restringe Alice a zero cursos
    let zero_courses = set_user_course_access(&mut db, "usr-alice", Some(&[]))?;
    assert_eq!(zero_courses, Some(Vec::new()));
    assert!(!can_user_access_course(
        &db,
        "usr-alice",
        false,
        "course-a"
    )?);

    // Restaura acesso irrestrito (None)
    let unrestricted = set_user_course_access(&mut db, "usr-alice", None)?;
    assert_eq!(unrestricted, None);
    assert!(can_user_access_course(&db, "usr-alice", false, "course-b")?);

    // Não permite restringir nem remover administrador
    assert!(matches!(
        set_user_course_access(&mut db, USER_ID, Some(&[])),
        Err(Error::CannotModifyAdminUser)
    ));
    assert!(matches!(
        delete_user(&mut db, USER_ID),
        Err(Error::CannotModifyAdminUser)
    ));

    let users = list_users(&db)?;
    assert_eq!(users.len(), 2);

    assert!(delete_user(&mut db, "usr-alice")?);
    assert!(!delete_user(&mut db, "usr-alice")?);
    assert_eq!(list_users(&db)?.len(), 1);
    Ok(())
}
