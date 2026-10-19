use learnvault_core::paths::relative_path_key;
use learnvault_core::{
    Database, Error, KeyMode, MAX_NOTES_PER_MEDIA, create_note, delete_note, list_notes,
    update_note,
};
use rusqlite::params;

const OWNER: &str = "owner-a";
const OTHER: &str = "owner-b";

fn database() -> Database {
    let db = Database::open_in_memory().unwrap();
    db.connection()
        .execute(
            "INSERT INTO library (library_id, available) VALUES ('lib', 1)",
            [],
        )
        .unwrap();
    db.connection().execute("INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability) VALUES ('m1', 'lib', 'a.mp4', ?1, 1, 1, 'available')", [relative_path_key("a.mp4", KeyMode::for_host()).unwrap()]).unwrap();
    db
}

#[test]
fn notes_support_owner_scoped_crud_validation_and_persistence() {
    let db = database();
    let created = create_note(&db, OWNER, "m1", "Initial note", Some(42)).unwrap();
    assert_eq!(
        (created.text.as_str(), created.position_seconds),
        ("Initial note", Some(42))
    );
    assert_eq!(list_notes(&db, OTHER, "m1").unwrap(), []);
    assert_eq!(
        list_notes(&db, OWNER, "m1").unwrap(),
        std::slice::from_ref(&created)
    );

    let edited = update_note(&db, OWNER, created.note_id, "Edited note", None).unwrap();
    assert_eq!(
        (edited.text.as_str(), edited.position_seconds),
        ("Edited note", None)
    );
    assert_eq!(
        update_note(&db, OTHER, created.note_id, "steal", None)
            .unwrap_err()
            .to_string(),
        "a nota não existe"
    );
    assert!(create_note(&db, OWNER, "m1", "  ", None).is_err());
    let boundary = create_note(&db, OWNER, "m1", &"x".repeat(20_000), None).unwrap();
    delete_note(&db, OWNER, boundary.note_id).unwrap();
    assert!(create_note(&db, OWNER, "m1", &"x".repeat(20_001), None).is_err());
    assert!(create_note(&db, OWNER, "m1", "negative", Some(-1)).is_err());

    let path = tempfile::NamedTempFile::new().unwrap();
    let persisted = Database::open(path.path()).unwrap();
    persisted
        .connection()
        .execute(
            "INSERT INTO library (library_id, available) VALUES ('lib', 1)",
            [],
        )
        .unwrap();
    persisted.connection().execute("INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability) VALUES ('m1', 'lib', 'a.mp4', ?1, 1, 1, 'available')", [relative_path_key("a.mp4", KeyMode::for_host()).unwrap()]).unwrap();
    let note = create_note(&persisted, OWNER, "m1", "Durable", None).unwrap();
    drop(persisted);
    let reopened = Database::open(path.path()).unwrap();
    assert_eq!(
        list_notes(&reopened, OWNER, "m1").unwrap(),
        std::slice::from_ref(&note)
    );
    delete_note(&reopened, OWNER, note.note_id).unwrap();
    assert!(list_notes(&reopened, OWNER, "m1").unwrap().is_empty());
    assert!(delete_note(&reopened, OTHER, note.note_id).is_err());
}

#[test]
fn notes_reject_unknown_media_and_empty_owner() {
    let db = database();

    // Mídia desconhecida: a criação e a leitura falham com o mesmo erro.
    let missing = create_note(&db, OWNER, "m-inexistente", "Nota", None).unwrap_err();
    assert!(matches!(missing, Error::MediaNotFound { .. }));
    let missing_list = list_notes(&db, OWNER, "m-inexistente").unwrap_err();
    assert!(matches!(missing_list, Error::MediaNotFound { .. }));

    // Dono vazio é recusado: é a barreira contra misturar donos.
    assert!(matches!(
        create_note(&db, "", "m1", "Nota", None).unwrap_err(),
        Error::InvalidOwnerId
    ));
    assert!(matches!(
        list_notes(&db, "", "m1").unwrap_err(),
        Error::InvalidOwnerId
    ));
    assert!(matches!(
        delete_note(&db, "", 1).unwrap_err(),
        Error::InvalidOwnerId
    ));
}

#[test]
fn notes_report_missing_note_ids() {
    let db = database();

    // Uma nota que não existe é indistinguível de uma nota de outro dono: o erro é o
    // mesmo e a resposta não revela se o identificador existe.
    for note_id in [999, 0, -1] {
        assert!(matches!(
            update_note(&db, OWNER, note_id, "Nota", None).unwrap_err(),
            Error::StudyNoteNotFound
        ));
        assert!(matches!(
            delete_note(&db, OWNER, note_id).unwrap_err(),
            Error::StudyNoteNotFound
        ));
    }
}

#[test]
fn notes_keep_updated_at_stable_when_the_content_does_not_change() {
    let db = database();
    let created = create_note(&db, OWNER, "m1", "Estável", Some(10)).unwrap();

    // A resolução é de um segundo: sem esperar, o teste não distinguiria as duas
    // semânticas possíveis de `updated_at`.
    std::thread::sleep(std::time::Duration::from_secs(1));

    let same = update_note(&db, OWNER, created.note_id, "Estável", Some(10)).unwrap();
    assert_eq!(
        same.updated_at, created.updated_at,
        "reenviar o mesmo conteúdo não move updated_at"
    );

    let changed = update_note(&db, OWNER, created.note_id, "Mudou", Some(10)).unwrap();
    assert!(
        changed.updated_at > created.updated_at,
        "uma alteração real move updated_at"
    );
    assert_eq!(
        changed.created_at, created.created_at,
        "created_at é imutável"
    );
}

#[test]
fn notes_enforce_the_per_media_ceiling() {
    let db = database();
    for index in 0..MAX_NOTES_PER_MEDIA {
        create_note(&db, OWNER, "m1", &format!("nota {index}"), None).unwrap();
    }

    // A criação acima do teto é recusada: o `LIMIT` da leitura nunca esconde nota em
    // silêncio.
    assert!(matches!(
        create_note(&db, OWNER, "m1", "acima do teto", None).unwrap_err(),
        Error::TooManyNotes
    ));
    assert_eq!(
        list_notes(&db, OWNER, "m1").unwrap().len(),
        MAX_NOTES_PER_MEDIA
    );

    // O teto é por par dono + mídia: outro dono tem o próprio orçamento.
    assert!(create_note(&db, OTHER, "m1", "nota de outro dono", None).is_ok());
}

#[test]
fn notes_list_never_returns_more_than_the_ceiling() {
    let db = database();

    // Linhas inseridas direto no banco, contornando a criação: a listagem precisa
    // respeitar o teto mesmo quando o banco tem mais notas do que o limite.
    for index in 0..MAX_NOTES_PER_MEDIA + 1 {
        db.connection()
            .execute(
                "INSERT INTO study_note (owner_id, media_id, text, position_seconds, created_at, updated_at)
                 VALUES (?1, ?2, ?3, NULL, 1, 1)",
                params![OWNER, "m1", format!("nota {index}")],
            )
            .unwrap();
    }

    assert_eq!(
        list_notes(&db, OWNER, "m1").unwrap().len(),
        MAX_NOTES_PER_MEDIA,
        "a listagem nunca devolve mais que o teto"
    );
    // O teto é por par dono + mídia: o outro dono não é afetado.
    assert!(list_notes(&db, OTHER, "m1").unwrap().is_empty());
}

#[test]
fn notes_list_follows_creation_order() {
    let db = database();
    let first = create_note(&db, OWNER, "m1", "primeira", None).unwrap();
    let second = create_note(&db, OWNER, "m1", "segunda", None).unwrap();
    let third = create_note(&db, OWNER, "m1", "terceira", None).unwrap();

    let ids = |notes: Vec<learnvault_core::StudyNote>| {
        notes
            .into_iter()
            .map(|note| note.note_id)
            .collect::<Vec<i64>>()
    };
    assert_eq!(
        ids(list_notes(&db, OWNER, "m1").unwrap()),
        vec![first.note_id, second.note_id, third.note_id]
    );

    // A ordenação é por `created_at` e, no empate, por `note_id`.
    db.connection()
        .execute(
            "UPDATE study_note SET created_at = 1 WHERE note_id = ?1",
            [third.note_id],
        )
        .unwrap();
    assert_eq!(
        ids(list_notes(&db, OWNER, "m1").unwrap()),
        vec![third.note_id, first.note_id, second.note_id]
    );
}

#[test]
fn notes_store_the_text_verbatim() {
    let db = database();

    // O texto é preservado como o usuário digitou: a validação recusa o vazio, mas não
    // reescreve o conteúdo.
    let created = create_note(&db, OWNER, "m1", "  texto com espaços  ", None).unwrap();
    assert_eq!(created.text, "  texto com espaços  ");
    assert_eq!(
        list_notes(&db, OWNER, "m1").unwrap()[0].text,
        "  texto com espaços  "
    );
}
