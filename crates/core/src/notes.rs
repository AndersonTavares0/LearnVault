//! Notas pessoais em texto simples, isoladas por dono e mídia.

use rusqlite::{OptionalExtension, params};

use crate::{Database, Error, Result, db::now_unix_seconds};

/// Tamanho máximo em caracteres para o texto de uma nota.
///
/// A unidade é o caractere, e não o byte UTF-8: é a mesma contagem que a interface
/// mostra ao usuário e a que o banco valida com `length(text)`.
pub const MAX_NOTE_TEXT_CHARS: usize = 20_000;

/// Teto de notas por dono e mídia.
///
/// A criação recusa passar do teto e a listagem traz `LIMIT`: assim a lista nunca
/// trunca notas em silêncio, e a resposta não cresce sem limite.
pub const MAX_NOTES_PER_MEDIA: usize = 500;

/// Teto de notas por dono na listagem de todas as anotações.
///
/// A listagem global avisa quando truncou, para que a tela nunca esconda uma nota
/// sem dizer isso.
pub const MAX_NOTES_PER_OWNER: usize = 2_000;

/// Nota pessoal vinculada a uma mídia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudyNote {
    /// Identificador estável da nota.
    pub note_id: i64,
    /// Mídia associada.
    pub media_id: String,
    /// Texto simples, nunca HTML.
    pub text: String,
    /// Posição opcional de reprodução, em segundos.
    pub position_seconds: Option<i64>,
    /// Instante de criação em segundos Unix.
    pub created_at: i64,
    /// Instante da última alteração em segundos Unix.
    pub updated_at: i64,
}

/// Lista as notas de um dono para a mídia, em ordem de criação.
pub fn list_notes(db: &Database, owner_id: &str, media_id: &str) -> Result<Vec<StudyNote>> {
    validate_owner(owner_id)?;
    require_media(&db.conn, media_id)?;
    let mut statement = db.conn.prepare(
        "SELECT note_id, media_id, text, position_seconds, created_at, updated_at
         FROM study_note WHERE owner_id = ?1 AND media_id = ?2 ORDER BY created_at, note_id
         LIMIT ?3",
    )?;
    Ok(statement
        .query_map(
            params![owner_id, media_id, MAX_NOTES_PER_MEDIA as i64],
            note_from_row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Lista todas as notas de um dono, das mais recentes para as mais antigas.
///
/// A listagem global tem teto próprio: quando truncou, o segundo valor devolvido é
/// `true` e a interface avisa, em vez de esconder notas em silêncio.
pub fn list_all_notes(db: &Database, owner_id: &str) -> Result<(Vec<StudyNote>, bool)> {
    validate_owner(owner_id)?;
    let mut statement = db.conn.prepare(
        "SELECT note_id, media_id, text, position_seconds, created_at, updated_at
         FROM study_note WHERE owner_id = ?1
         ORDER BY updated_at DESC, note_id DESC
         LIMIT ?2",
    )?;
    let mut notes = statement
        .query_map(
            params![owner_id, MAX_NOTES_PER_OWNER as i64 + 1],
            note_from_row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let truncated = notes.len() > MAX_NOTES_PER_OWNER;
    notes.truncate(MAX_NOTES_PER_OWNER);
    Ok((notes, truncated))
}

/// Cria uma nota pessoal para o dono e mídia informados.
pub fn create_note(
    db: &Database,
    owner_id: &str,
    media_id: &str,
    text: &str,
    position_seconds: Option<i64>,
) -> Result<StudyNote> {
    validate(owner_id, text, position_seconds)?;
    require_media(&db.conn, media_id)?;
    // O teto é do par (dono, mídia): acima dele a criação é recusada, para que a
    // listagem com `LIMIT` nunca esconda notas sem avisar. A contagem e a inserção
    // formam uma única instrução: contagem e inserção separadas deixariam passar
    // duas criações simultâneas acima do teto.
    let now = now_unix_seconds();
    let inserted = db.conn.execute(
        "INSERT INTO study_note (owner_id, media_id, text, position_seconds, created_at, updated_at)
         SELECT ?1, ?2, ?3, ?4, ?5, ?5
         WHERE (SELECT COUNT(*) FROM study_note WHERE owner_id = ?1 AND media_id = ?2) < ?6",
        params![owner_id, media_id, text, position_seconds, now, MAX_NOTES_PER_MEDIA as i64],
    )?;
    if inserted == 0 {
        return Err(Error::TooManyNotes);
    }
    let note_id = db.conn.last_insert_rowid();
    get_note(&db.conn, owner_id, note_id)?.ok_or(Error::StudyNoteNotFound)
}

/// Atualiza texto e posição de uma nota pertencente ao dono informado.
pub fn update_note(
    db: &Database,
    owner_id: &str,
    note_id: i64,
    text: &str,
    position_seconds: Option<i64>,
) -> Result<StudyNote> {
    validate(owner_id, text, position_seconds)?;
    if note_id <= 0 {
        return Err(Error::StudyNoteNotFound);
    }
    let changed = db.conn.execute(
        "UPDATE study_note SET text = ?1, position_seconds = ?2,
             updated_at = CASE WHEN text = ?1 AND position_seconds IS ?2 THEN updated_at ELSE ?3 END
         WHERE owner_id = ?4 AND note_id = ?5",
        params![
            text,
            position_seconds,
            now_unix_seconds(),
            owner_id,
            note_id
        ],
    )?;
    if changed == 0 {
        return Err(Error::StudyNoteNotFound);
    }
    get_note(&db.conn, owner_id, note_id)?.ok_or(Error::StudyNoteNotFound)
}

/// Exclui uma nota pertencente ao dono informado.
pub fn delete_note(db: &Database, owner_id: &str, note_id: i64) -> Result<()> {
    validate_owner(owner_id)?;
    let changed = db.conn.execute(
        "DELETE FROM study_note WHERE owner_id = ?1 AND note_id = ?2",
        params![owner_id, note_id],
    )?;
    if changed == 0 {
        return Err(Error::StudyNoteNotFound);
    }
    Ok(())
}

fn get_note(
    conn: &rusqlite::Connection,
    owner_id: &str,
    note_id: i64,
) -> Result<Option<StudyNote>> {
    Ok(conn
        .query_row(
            "SELECT note_id, media_id, text, position_seconds, created_at, updated_at
         FROM study_note WHERE owner_id = ?1 AND note_id = ?2",
            params![owner_id, note_id],
            note_from_row,
        )
        .optional()?)
}

fn note_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StudyNote> {
    Ok(StudyNote {
        note_id: row.get(0)?,
        media_id: row.get(1)?,
        text: row.get(2)?,
        position_seconds: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn validate(owner_id: &str, text: &str, position_seconds: Option<i64>) -> Result<()> {
    validate_owner(owner_id)?;
    if text.trim().is_empty() || text.chars().count() > MAX_NOTE_TEXT_CHARS {
        return Err(Error::InvalidStudyNote);
    }
    if let Some(position) = position_seconds
        && position < 0
    {
        return Err(Error::InvalidStudyPosition {
            position_seconds: position,
        });
    }
    Ok(())
}

fn validate_owner(owner_id: &str) -> Result<()> {
    if owner_id.is_empty() {
        Err(Error::InvalidOwnerId)
    } else {
        Ok(())
    }
}

fn require_media(conn: &rusqlite::Connection, media_id: &str) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM media WHERE media_id = ?1)",
        [media_id],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(Error::MediaNotFound {
            media_id: media_id.to_owned(),
        })
    }
}
