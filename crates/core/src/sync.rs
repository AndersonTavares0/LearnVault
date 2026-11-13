//! Sincronização de estudo entre o desktop e o servidor.
//!
//! O contrato é um diário de operações somente de acréscimo. Cada operação tem um
//! `op_id` estável gerado por quem a produziu, um `device_id` e um instante de cliente.
//! O servidor aplica a operação e grava a linha na mesma transação: reenviar o mesmo
//! `op_id` responde como duplicata e não repete o efeito. `seq` é o cursor monotônico da
//! leitura incremental — as linhas nunca são apagadas, então um cursor menor sempre
//! encontra o que veio depois.
//!
//! # Donos
//!
//! O dono é sempre derivado do chamador (sessão HTTP ou dono local fixo do desktop) e
//! nunca vem da operação. Leitura, escrita e fila filtram por dono: nenhuma operação
//! cruza usuários.
//!
//! # Regras de conflito
//!
//! - progresso: última escrita vence pelo `client_at`, com a posição limitada a
//!   `[0, MAX_SYNC_POSITION_SECONDS]`; uma escrita antiga nunca desfaz uma conclusão já
//!   observada, mas a escrita mais nova pode desmarcá-la;
//! - anotações: cada versão sincronizada tem uma identidade estável (`op_id`). A versão
//!   mais recente (maior `op_id`, já que UUID v7 ordena por instante de criação) ocupa a
//!   linha principal; as outras viram cópias com id determinístico
//!   `note_id~op_version`. Assim dois dispositivos que viram o mesmo conjunto de versões
//!   convergem para os mesmos textos, sem apagar conteúdo;
//! - revisões de cartão: somente acréscimo, aplicadas na ordem do cursor;
//! - preferências: última escrita vence pelo `client_at`.
//!
//! A mídia não é sincronizada. Uma operação cuja mídia ainda não existe no banco local
//! continua no diário e é ignorada ao ser aplicada: o dado não se perde e a convergência
//! acontece quando a mídia aparecer.
//!
//! # Limites conhecidos
//!
//! - `client_at` vem do relógio do dispositivo; um relógio muito adiantado vence as
//!   escritas seguintes até o relógio real alcançá-lo.
//! - anotações gravadas antes da migração 17 têm `sync_version` nulo; elas nunca são
//!   substituídas por uma versão sincronizada (essa versão vira cópia), para não apagar
//!   trabalho local anterior ao contrato.

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::db::{Database, now_unix_seconds};
use crate::error::{Error, Result};
use crate::player::{
    MAX_PLAYBACK_SPEED_PERCENT, MIN_PLAYBACK_SPEED_PERCENT, PlayerPreference, valid_language,
};
use crate::study::{
    Flashcard, MediaBookmark, ReviewRating, StudyNote, StudyProgress, validate_owner,
};

/// Maior lote aceito por [`push_operations`].
pub const MAX_SYNC_BATCH: usize = 200;

/// Maior página devolvida por [`pull_operations`].
pub const MAX_PULL_BATCH: usize = 500;

/// Maior identificador aceito em `op_id` e `device_id`.
pub const MAX_SYNC_ID_CHARS: usize = 128;

/// Teto da posição de progresso sincronizada, em segundos (24 horas).
///
/// A duração real da mídia não é conhecida pelo domínio, então o limite é um teto
/// grosseiro contra valores corrompidos, e não a duração da aula.
pub const MAX_SYNC_POSITION_SECONDS: i64 = 24 * 60 * 60;

/// Operação de estudo trocada entre os dispositivos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SyncPayload {
    /// Posição de retomada e conclusão de uma mídia.
    Progress {
        /// Identificador opaco da mídia.
        media_id: String,
        /// Ponto de retomada em segundos.
        position_seconds: i64,
        /// Marca de conclusão da aula.
        completed: bool,
    },
    /// Anotação criada ou editada no dispositivo.
    Note {
        /// Identificador opaco da anotação.
        note_id: String,
        /// Identificador opaco da mídia.
        media_id: String,
        /// Posição no player em segundos.
        position_seconds: i64,
        /// Texto da anotação.
        content: String,
        /// Instante de criação informado pelo dispositivo.
        created_at: i64,
        /// Instante da última edição informado pelo dispositivo.
        updated_at: i64,
    },
    /// Marcador de tempo criado no dispositivo.
    Bookmark {
        /// Identificador opaco do marcador.
        bookmark_id: String,
        /// Identificador opaco da mídia.
        media_id: String,
        /// Instante da mídia em segundos.
        position_seconds: i64,
        /// Rótulo curto do marcador.
        label: String,
        /// Instante de criação informado pelo dispositivo.
        created_at: i64,
    },
    /// Revisão de um cartão, somente de acréscimo.
    FlashcardReview {
        /// Identificador opaco do cartão.
        card_id: String,
        /// Avaliação: `again` ou `good`.
        rating: String,
        /// Instante da revisão informado pelo dispositivo.
        reviewed_at: i64,
    },
    /// Preferências de reprodução do dono.
    Preference {
        /// Velocidade de vídeo em porcentagem da normal.
        video_speed: i64,
        /// Velocidade de áudio em porcentagem da normal.
        audio_speed: i64,
        /// Última língua de legenda escolhida; vazio significa sem preferência.
        subtitle_language: String,
    },
}

impl SyncPayload {
    /// Mídia referenciada pela operação, quando existe uma.
    pub fn media_id(&self) -> Option<&str> {
        match self {
            Self::Progress { media_id, .. }
            | Self::Note { media_id, .. }
            | Self::Bookmark { media_id, .. } => Some(media_id),
            Self::FlashcardReview { .. } | Self::Preference { .. } => None,
        }
    }
}

/// Operação enviada por um dispositivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncOperationInput {
    /// Identificador estável da operação; reenviar o mesmo valor é idempotente.
    pub op_id: String,
    /// Instante do dispositivo em segundos Unix.
    pub client_at: i64,
    /// Efeito da operação.
    pub payload: SyncPayload,
}

/// Operação já registrada no diário.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncOperation {
    /// Cursor monotônico atribuído pelo servidor.
    pub seq: i64,
    /// Identificador estável da operação.
    pub op_id: String,
    /// Dispositivo que produziu a operação.
    pub device_id: String,
    /// Instante do dispositivo em segundos Unix.
    pub client_at: i64,
    /// Efeito da operação.
    pub payload: SyncPayload,
}

/// Resultado de um envio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushOutcome {
    /// Operações aplicadas nesta chamada.
    pub applied: usize,
    /// Operações já conhecidas pelo `op_id` (replay).
    pub duplicated: usize,
    /// Maior cursor do dono depois do envio.
    pub cursor: i64,
}

/// Página de operações prontas para aplicação local.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncBatch {
    /// Cursor a usar na próxima leitura.
    pub cursor: i64,
    /// Indica que ainda há operações depois do cursor devolvido.
    pub has_more: bool,
    /// Operações em ordem crescente de `seq`.
    pub items: Vec<SyncOperation>,
}

/// Estado local de sincronização de um dono.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncState {
    /// Identidade estável deste dispositivo.
    pub device_id: String,
    /// Último cursor do servidor já aplicado localmente.
    pub pull_cursor: i64,
}

/// Aplica um lote enviado por um dispositivo e registra cada operação no diário.
///
/// O lote inteiro roda em uma transação: ou todas as operações novas entram com o seu
/// efeito, ou nenhuma entra. Uma operação com `op_id` já conhecido conta como duplicata
/// e não é aplicada de novo. A aplicação normaliza dados fora da faixa em vez de
/// recusar o lote, para que um dispositivo não repita para sempre uma operação que
/// nunca poderá ser aceita.
pub fn push_operations(
    db: &Database,
    owner_id: &str,
    device_id: &str,
    operations: &[SyncOperationInput],
) -> Result<PushOutcome> {
    validate_owner(owner_id)?;
    validate_id(device_id, "device")?;
    if operations.len() > MAX_SYNC_BATCH {
        return Err(Error::InvalidSyncOperation {
            reason: "lote acima do limite",
        });
    }

    let conn = db.connection();
    let transaction = conn.unchecked_transaction()?;
    let mut applied = 0_usize;
    let mut duplicated = 0_usize;
    for operation in operations {
        validate_id(&operation.op_id, "op")?;
        // `sync_operation.op_id` is a global primary key, so a colliding id from another owner
        // must be refused instead of silently swallowing this operation as a duplicate.
        let existing_owner: Option<String> = transaction
            .query_row(
                "SELECT owner_id FROM sync_operation WHERE op_id = ?1",
                [&operation.op_id],
                |row| row.get(0),
            )
            .optional()?;
        match existing_owner.as_deref() {
            Some(owner) if owner == owner_id => {
                duplicated += 1;
                continue;
            }
            Some(_) => {
                return Err(Error::InvalidSyncOperation {
                    reason: "op_id already used by another owner",
                });
            }
            None => {}
        }
        apply_payload(
            &transaction,
            owner_id,
            &operation.payload,
            operation.client_at,
            &operation.op_id,
        )?;
        transaction.execute(
            "INSERT INTO sync_operation (op_id, owner_id, device_id, client_at, received_at, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                operation.op_id,
                owner_id,
                device_id,
                operation.client_at.max(0),
                now_unix_seconds(),
                encode(&operation.payload)?,
            ],
        )?;
        applied += 1;
    }
    let cursor: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(seq), 0) FROM sync_operation WHERE owner_id = ?1",
        [owner_id],
        |row| row.get(0),
    )?;
    transaction.commit()?;

    Ok(PushOutcome {
        applied,
        duplicated,
        cursor,
    })
}

/// Lê as operações do dono depois de `cursor`, exceto as do próprio dispositivo.
///
/// O dispositivo já aplicou localmente tudo o que enfileirou; devolver as próprias
/// operações faria a mesma escrita ser aplicada duas vezes. As operações são devolvidas
/// em ordem crescente de `seq`, e o cursor devolvido é o último item da página.
pub fn pull_operations(
    db: &Database,
    owner_id: &str,
    device_id: &str,
    cursor: i64,
    limit: usize,
) -> Result<SyncBatch> {
    validate_owner(owner_id)?;
    validate_id(device_id, "device")?;
    let limit = limit.clamp(1, MAX_PULL_BATCH) as i64;

    let conn = db.connection();
    let mut statement = conn.prepare(
        "SELECT seq, op_id, device_id, client_at, payload
           FROM sync_operation
          WHERE owner_id = ?1 AND seq > ?2 AND device_id <> ?3
          ORDER BY seq ASC
          LIMIT ?4",
    )?;
    let rows = statement.query_map(params![owner_id, cursor.max(0), device_id, limit], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;

    let mut items = Vec::new();
    for row in rows {
        let (seq, op_id, device_id, client_at, payload) = row?;
        items.push(SyncOperation {
            seq,
            op_id,
            device_id,
            client_at,
            payload: decode(&payload)?,
        });
    }
    let next_cursor = items.last().map_or(cursor.max(0), |item| item.seq);
    let has_more = conn
        .query_row(
            "SELECT 1 FROM sync_operation
              WHERE owner_id = ?1 AND seq > ?2 AND device_id <> ?3
              LIMIT 1",
            params![owner_id, next_cursor, device_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some();

    Ok(SyncBatch {
        cursor: next_cursor,
        has_more,
        items,
    })
}

/// Aplica localmente as operações recebidas e grava o cursor, em uma transação.
///
/// O cursor e os efeitos entram juntos: uma sincronização interrompida antes do commit
/// não avança o cursor e pode ser retomada do ponto anterior sem duplicar operações.
/// `cursor` precisa ser o maior `seq` aplicado; um valor menor que o já gravado nunca
/// retrocede o cursor.
pub fn apply_operations(
    db: &Database,
    owner_id: &str,
    operations: &[SyncOperation],
    cursor: i64,
) -> Result<usize> {
    validate_owner(owner_id)?;
    let conn = db.connection();
    let transaction = conn.unchecked_transaction()?;
    let mut applied = 0_usize;
    for operation in operations {
        apply_payload(
            &transaction,
            owner_id,
            &operation.payload,
            operation.client_at,
            &operation.op_id,
        )?;
        applied += 1;
    }
    transaction.execute(
        "INSERT INTO sync_state (owner_id, device_id, pull_cursor, updated_at)
         VALUES (?1, ?4, MAX(?2, 0), ?3)
         ON CONFLICT (owner_id) DO UPDATE SET
             pull_cursor = MAX(sync_state.pull_cursor, MAX(excluded.pull_cursor, 0)),
             updated_at = excluded.updated_at",
        params![
            owner_id,
            cursor,
            now_unix_seconds(),
            crate::ids::new_sync_id()
        ],
    )?;
    transaction.commit()?;
    Ok(applied)
}

/// Lê o estado local do dono, criando a identidade do dispositivo na primeira chamada.
pub fn sync_state(db: &Database, owner_id: &str) -> Result<SyncState> {
    validate_owner(owner_id)?;
    let conn = db.connection();
    conn.execute(
        "INSERT INTO sync_state (owner_id, device_id, pull_cursor, updated_at)
         VALUES (?1, ?2, 0, ?3)
         ON CONFLICT (owner_id) DO NOTHING",
        params![owner_id, crate::ids::new_sync_id(), now_unix_seconds()],
    )?;
    conn.query_row(
        "SELECT device_id, pull_cursor FROM sync_state WHERE owner_id = ?1",
        [owner_id],
        |row| {
            Ok(SyncState {
                device_id: row.get(0)?,
                pull_cursor: row.get(1)?,
            })
        },
    )
    .map_err(Error::from)
}

/// Enfileira uma operação no outbox local e devolve o `op_id` usado.
///
/// A operação é aplicada localmente por quem chama (a escrita de estudo já aconteceu);
/// o outbox só guarda o que ainda precisa chegar ao servidor. Reenviar a operação com o
/// mesmo `op_id` é seguro: o servidor a reconhece como duplicata.
pub fn enqueue_operation(
    db: &Database,
    owner_id: &str,
    device_id: &str,
    payload: &SyncPayload,
    client_at: i64,
) -> Result<String> {
    validate_owner(owner_id)?;
    enqueue_in(db.connection(), owner_id, device_id, payload, client_at)
}

fn enqueue_in(
    conn: &Connection,
    owner_id: &str,
    device_id: &str,
    payload: &SyncPayload,
    client_at: i64,
) -> Result<String> {
    validate_id(device_id, "device")?;
    let op_id = crate::ids::new_sync_id();
    conn.execute(
        "INSERT INTO sync_outbox (op_id, owner_id, device_id, client_at, payload, queued_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            op_id,
            owner_id,
            device_id,
            client_at.max(0),
            encode(payload)?,
            now_unix_seconds(),
        ],
    )?;
    Ok(op_id)
}

/// Registra a posição de retomada e enfileira a mesma mudança numa única transação.
///
/// A transação é a garantia de durabilidade da fila: se a inserção no outbox falhar, a
/// escrita de estudo também não é confirmada, e um crash nunca deixa a escrita local sem
/// a operação correspondente.
pub fn queue_position(
    db: &Database,
    owner_id: &str,
    device_id: &str,
    media_id: &str,
    position_seconds: i64,
) -> Result<StudyProgress> {
    queue_change(db, owner_id, device_id, |conn| {
        let progress =
            crate::study::record_position_on(conn, owner_id, media_id, position_seconds)?;
        let payload = SyncPayload::Progress {
            media_id: media_id.to_owned(),
            position_seconds,
            completed: progress.completed,
        };
        Ok((progress, payload, now_unix_seconds()))
    })
}

/// Marca a conclusão e enfileira a mesma mudança numa única transação.
pub fn queue_completed(
    db: &Database,
    owner_id: &str,
    device_id: &str,
    media_id: &str,
    completed: bool,
) -> Result<StudyProgress> {
    queue_change(db, owner_id, device_id, |conn| {
        let progress = crate::study::set_completed_on(conn, owner_id, media_id, completed)?;
        let payload = SyncPayload::Progress {
            media_id: media_id.to_owned(),
            position_seconds: progress.position_seconds,
            completed,
        };
        Ok((progress, payload, now_unix_seconds()))
    })
}

/// Cria uma anotação, marca a versão local e enfileira a operação numa transação.
pub fn queue_note_create(
    db: &Database,
    owner_id: &str,
    device_id: &str,
    media_id: &str,
    position_seconds: i64,
    content: &str,
) -> Result<StudyNote> {
    queue_note(db, owner_id, device_id, |conn| {
        crate::study::create_study_note_on(conn, owner_id, media_id, position_seconds, content)
    })
}

/// Edita uma anotação, marca a versão local e enfileira a operação numa transação.
pub fn queue_note_update(
    db: &Database,
    owner_id: &str,
    device_id: &str,
    note_id: &str,
    content: &str,
) -> Result<StudyNote> {
    queue_note(db, owner_id, device_id, |conn| {
        crate::study::update_study_note_on(conn, owner_id, note_id, content)
    })
}

fn queue_note(
    db: &Database,
    owner_id: &str,
    device_id: &str,
    work: impl FnOnce(&Connection) -> Result<StudyNote>,
) -> Result<StudyNote> {
    validate_owner(owner_id)?;
    let conn = db.connection();
    let transaction = conn.unchecked_transaction()?;
    let note = work(&transaction)?;
    let payload = SyncPayload::Note {
        note_id: note.note_id.clone(),
        media_id: note.media_id.clone(),
        position_seconds: note.position_seconds,
        content: note.content.clone(),
        created_at: note.created_at,
        updated_at: note.updated_at,
    };
    let op_id = enqueue_in(&transaction, owner_id, device_id, &payload, note.updated_at)?;
    transaction.execute(
        "UPDATE study_note SET sync_version = ?1
          WHERE owner_id = ?2 AND note_id = ?3",
        params![op_id, owner_id, note.note_id],
    )?;
    transaction.commit()?;
    Ok(note)
}

/// Cria um marcador e enfileira a operação numa única transação.
pub fn queue_bookmark_create(
    db: &Database,
    owner_id: &str,
    device_id: &str,
    media_id: &str,
    position_seconds: i64,
    label: &str,
) -> Result<MediaBookmark> {
    queue_change(db, owner_id, device_id, |conn| {
        let bookmark = crate::study::create_media_bookmark_on(
            conn,
            owner_id,
            media_id,
            position_seconds,
            label,
        )?;
        Ok((
            bookmark.clone(),
            SyncPayload::Bookmark {
                bookmark_id: bookmark.bookmark_id,
                media_id: bookmark.media_id,
                position_seconds: bookmark.position_seconds,
                label: bookmark.label,
                created_at: bookmark.created_at,
            },
            bookmark.created_at,
        ))
    })
}

/// Registra a revisão de um cartão e enfileira a operação numa única transação.
pub fn queue_review(
    db: &Database,
    owner_id: &str,
    device_id: &str,
    card_id: &str,
    rating: ReviewRating,
) -> Result<Flashcard> {
    queue_change(db, owner_id, device_id, |conn| {
        let card = crate::study::review_flashcard_on(conn, owner_id, card_id, rating)?;
        Ok((
            card.clone(),
            SyncPayload::FlashcardReview {
                card_id: card.card_id,
                rating: rating.as_str().to_owned(),
                reviewed_at: card.updated_at,
            },
            card.updated_at,
        ))
    })
}

/// Grava as preferências de reprodução e enfileira a operação numa única transação.
pub fn queue_preference(
    db: &Database,
    owner_id: &str,
    device_id: &str,
    video_speed: i64,
    audio_speed: i64,
    subtitle_language: &str,
) -> Result<PlayerPreference> {
    queue_change(db, owner_id, device_id, |conn| {
        let preference = crate::player::set_player_preference_on(
            conn,
            owner_id,
            video_speed,
            audio_speed,
            subtitle_language,
        )?;
        Ok((
            preference.clone(),
            SyncPayload::Preference {
                video_speed: preference.video_speed,
                audio_speed: preference.audio_speed,
                subtitle_language: preference.subtitle_language,
            },
            preference.updated_at,
        ))
    })
}

fn queue_change<T>(
    db: &Database,
    owner_id: &str,
    device_id: &str,
    work: impl FnOnce(&Connection) -> Result<(T, SyncPayload, i64)>,
) -> Result<T> {
    validate_owner(owner_id)?;
    let conn = db.connection();
    let transaction = conn.unchecked_transaction()?;
    let (value, payload, client_at) = work(&transaction)?;
    enqueue_in(&transaction, owner_id, device_id, &payload, client_at)?;
    transaction.commit()?;
    Ok(value)
}

/// Lista as operações pendentes do outbox, da mais antiga para a mais nova.
pub fn pending_operations(
    db: &Database,
    owner_id: &str,
    limit: usize,
) -> Result<Vec<SyncOperationInput>> {
    validate_owner(owner_id)?;
    let limit = limit.clamp(1, MAX_SYNC_BATCH) as i64;
    let conn = db.connection();
    let mut statement = conn.prepare(
        "SELECT op_id, client_at, payload FROM sync_outbox
          WHERE owner_id = ?1
          ORDER BY queued_at ASC, op_id ASC
          LIMIT ?2",
    )?;
    let rows = statement.query_map(params![owner_id, limit], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut operations = Vec::new();
    for row in rows {
        let (op_id, client_at, payload) = row?;
        operations.push(SyncOperationInput {
            op_id,
            client_at,
            payload: decode(&payload)?,
        });
    }
    Ok(operations)
}

/// Conta as operações ainda pendentes no outbox do dono.
pub fn pending_count(db: &Database, owner_id: &str) -> Result<i64> {
    validate_owner(owner_id)?;
    db.connection()
        .query_row(
            "SELECT COUNT(*) FROM sync_outbox WHERE owner_id = ?1",
            [owner_id],
            |row| row.get(0),
        )
        .map_err(Error::from)
}

/// Remove do outbox as operações aceitas pelo servidor e devolve quantas saíram.
pub fn acknowledge_operations(db: &Database, owner_id: &str, op_ids: &[String]) -> Result<usize> {
    validate_owner(owner_id)?;
    let conn = db.connection();
    let transaction = conn.unchecked_transaction()?;
    let mut removed = 0_usize;
    for op_id in op_ids {
        removed += transaction.execute(
            "DELETE FROM sync_outbox WHERE owner_id = ?1 AND op_id = ?2",
            params![owner_id, op_id],
        )?;
    }
    transaction.commit()?;
    Ok(removed)
}

fn apply_payload(
    conn: &Connection,
    owner_id: &str,
    payload: &SyncPayload,
    client_at: i64,
    op_id: &str,
) -> Result<()> {
    match payload {
        SyncPayload::Progress {
            media_id,
            position_seconds,
            completed,
        } => apply_progress(
            conn,
            owner_id,
            media_id,
            *position_seconds,
            *completed,
            client_at,
        ),
        SyncPayload::Note {
            note_id,
            media_id,
            position_seconds,
            content,
            created_at,
            updated_at,
        } => apply_note(
            conn,
            owner_id,
            note_id,
            media_id,
            *position_seconds,
            content,
            *created_at,
            *updated_at,
            op_id,
        ),
        SyncPayload::Bookmark {
            bookmark_id,
            media_id,
            position_seconds,
            label,
            created_at,
        } => apply_bookmark(
            conn,
            owner_id,
            bookmark_id,
            media_id,
            *position_seconds,
            label,
            *created_at,
        ),
        SyncPayload::FlashcardReview {
            card_id,
            rating,
            reviewed_at,
        } => apply_review(conn, owner_id, card_id, rating, *reviewed_at),
        SyncPayload::Preference {
            video_speed,
            audio_speed,
            subtitle_language,
        } => apply_preference(
            conn,
            owner_id,
            *video_speed,
            *audio_speed,
            subtitle_language,
            client_at,
        ),
    }
}

fn apply_progress(
    conn: &Connection,
    owner_id: &str,
    media_id: &str,
    position_seconds: i64,
    completed: bool,
    client_at: i64,
) -> Result<()> {
    if !media_known(conn, media_id)? {
        return Ok(());
    }
    let position = position_seconds.clamp(0, MAX_SYNC_POSITION_SECONDS);
    let client_at = client_at.max(0);
    let existing: Option<(i64, i64, i64)> = conn
        .query_row(
            "SELECT position_seconds, completed, updated_at FROM study_state
              WHERE owner_id = ?1 AND media_id = ?2",
            params![owner_id, media_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;

    let Some((current_position, current_completed, current_updated_at)) = existing else {
        conn.execute(
            "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                owner_id,
                media_id,
                position,
                i64::from(completed),
                client_at
            ],
        )?;
        return Ok(());
    };

    // Posição: a escrita mais nova vence. Conclusão: uma vez observada, nunca é desfeita —
    // nem por uma escrita mais nova nem por uma antiga que chegue atrasada.
    let newer = client_at >= current_updated_at;
    let next_position = if newer { position } else { current_position };
    let next_completed = current_completed != 0 || completed;
    if next_position == current_position && next_completed == (current_completed != 0) {
        return Ok(());
    }
    conn.execute(
        "UPDATE study_state
            SET position_seconds = ?1, completed = ?2, updated_at = ?3
          WHERE owner_id = ?4 AND media_id = ?5",
        params![
            next_position,
            i64::from(next_completed),
            current_updated_at.max(client_at),
            owner_id,
            media_id
        ],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_note(
    conn: &Connection,
    owner_id: &str,
    note_id: &str,
    media_id: &str,
    position_seconds: i64,
    content: &str,
    created_at: i64,
    updated_at: i64,
    op_id: &str,
) -> Result<()> {
    let content = content.trim();
    if note_id.trim().is_empty()
        || content.is_empty()
        || content.chars().count() > 10_000
        || position_seconds < 0
        || !media_known(conn, media_id)?
    {
        return Ok(());
    }
    let created_at = created_at.clamp(0, updated_at.max(0));
    let updated_at = updated_at.max(created_at);

    let existing: Option<(String, String, Option<String>, i64, i64, i64)> = conn
        .query_row(
            "SELECT owner_id, content, sync_version, position_seconds, created_at, updated_at
               FROM study_note WHERE note_id = ?1",
            [note_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()?;

    let Some((row_owner, row_content, row_version, row_position, row_created, row_updated)) =
        existing
    else {
        insert_note(
            conn,
            note_id,
            owner_id,
            media_id,
            position_seconds,
            content,
            created_at,
            updated_at,
            op_id,
        )?;
        return Ok(());
    };

    if row_owner != owner_id {
        // A identidade de anotação é opaca e global; um id já usado por outro dono nunca
        // é sobrescrito. A versão recebida vira uma cópia do dono que a enviou.
        insert_note(
            conn,
            &copy_id(note_id, op_id),
            owner_id,
            media_id,
            position_seconds,
            content,
            created_at,
            updated_at,
            op_id,
        )?;
        return Ok(());
    }

    if row_content == content {
        return Ok(());
    }

    match row_version {
        // Linha anterior ao contrato: o trabalho local nunca é substituído.
        None => insert_note(
            conn,
            &copy_id(note_id, op_id),
            owner_id,
            media_id,
            position_seconds,
            content,
            created_at,
            updated_at,
            op_id,
        ),
        // A versão recebida é mais nova (UUID v7 ordena por tempo): ela assume a linha
        // principal e a versão atual é preservada como cópia.
        Some(version) if version.as_str() < op_id => {
            insert_note(
                conn,
                &copy_id(note_id, &version),
                owner_id,
                media_id,
                row_position,
                &row_content,
                row_created,
                row_updated,
                &version,
            )?;
            conn.execute(
                "UPDATE study_note
                    SET position_seconds = ?1, content = ?2, created_at = ?3,
                        updated_at = ?4, sync_version = ?5
                  WHERE note_id = ?6",
                params![
                    position_seconds,
                    content,
                    created_at,
                    updated_at,
                    op_id,
                    note_id
                ],
            )?;
            Ok(())
        }
        // A versão recebida é mais antiga e só sobrevive como cópia.
        Some(_) => insert_note(
            conn,
            &copy_id(note_id, op_id),
            owner_id,
            media_id,
            position_seconds,
            content,
            created_at,
            updated_at,
            op_id,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn insert_note(
    conn: &Connection,
    note_id: &str,
    owner_id: &str,
    media_id: &str,
    position_seconds: i64,
    content: &str,
    created_at: i64,
    updated_at: i64,
    sync_version: &str,
) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO study_note
             (note_id, owner_id, media_id, position_seconds, content, created_at, updated_at, sync_version)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            note_id,
            owner_id,
            media_id,
            position_seconds,
            content,
            created_at,
            updated_at,
            sync_version
        ],
    )?;
    Ok(())
}

/// Identidade determinística da cópia de uma versão perdedora de anotação.
fn copy_id(note_id: &str, version: &str) -> String {
    format!("{note_id}~{version}")
}

fn apply_bookmark(
    conn: &Connection,
    owner_id: &str,
    bookmark_id: &str,
    media_id: &str,
    position_seconds: i64,
    label: &str,
    created_at: i64,
) -> Result<()> {
    let label = label.trim();
    if bookmark_id.trim().is_empty()
        || label.is_empty()
        || label.chars().count() > crate::study::MAX_BOOKMARK_LABEL_CHARS
        || position_seconds < 0
        || !media_known(conn, media_id)?
    {
        return Ok(());
    }
    conn.execute(
        "INSERT OR IGNORE INTO media_bookmark
             (bookmark_id, owner_id, media_id, position_seconds, label, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            bookmark_id,
            owner_id,
            media_id,
            position_seconds,
            label,
            created_at.max(0)
        ],
    )?;
    Ok(())
}

fn apply_review(
    conn: &Connection,
    owner_id: &str,
    card_id: &str,
    rating: &str,
    reviewed_at: i64,
) -> Result<()> {
    let Some(rating) = crate::study::ReviewRating::parse(rating) else {
        return Ok(());
    };
    let reviewed_at = reviewed_at.max(0);
    let previous: Option<i64> = conn
        .query_row(
            "SELECT interval_days FROM flashcard WHERE owner_id = ?1 AND card_id = ?2",
            params![owner_id, card_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(previous_interval) = previous else {
        // Cards are not synced, so a review of an unknown card cannot be applied anywhere;
        // it stays only in the journal for audit. Tracked in #472.
        return Ok(());
    };
    let (next_interval, next_due) =
        crate::study::schedule_review(previous_interval, rating, reviewed_at);
    conn.execute(
        "UPDATE flashcard
            SET interval_days = ?1, next_due = ?2, updated_at = ?3
          WHERE owner_id = ?4 AND card_id = ?5",
        params![next_interval, next_due, reviewed_at, owner_id, card_id],
    )?;
    conn.execute(
        "INSERT INTO flashcard_review
             (owner_id, card_id, rating, previous_interval_days, next_due, reviewed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            owner_id,
            card_id,
            rating.as_str(),
            previous_interval,
            next_due,
            reviewed_at
        ],
    )?;
    Ok(())
}

fn apply_preference(
    conn: &Connection,
    owner_id: &str,
    video_speed: i64,
    audio_speed: i64,
    subtitle_language: &str,
    client_at: i64,
) -> Result<()> {
    if !valid_language(subtitle_language) {
        return Ok(());
    }
    let client_at = client_at.max(0);
    let video_speed = video_speed.clamp(MIN_PLAYBACK_SPEED_PERCENT, MAX_PLAYBACK_SPEED_PERCENT);
    let audio_speed = audio_speed.clamp(MIN_PLAYBACK_SPEED_PERCENT, MAX_PLAYBACK_SPEED_PERCENT);
    conn.execute(
        "INSERT INTO player_preference
             (owner_id, video_speed, audio_speed, subtitle_language, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (owner_id) DO UPDATE SET
             video_speed = excluded.video_speed,
             audio_speed = excluded.audio_speed,
             subtitle_language = excluded.subtitle_language,
             updated_at = excluded.updated_at
         WHERE excluded.updated_at >= player_preference.updated_at",
        params![
            owner_id,
            video_speed,
            audio_speed,
            subtitle_language,
            client_at
        ],
    )?;
    Ok(())
}

fn media_known(conn: &Connection, media_id: &str) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM media WHERE media_id = ?1",
            [media_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn validate_id(value: &str, field: &'static str) -> Result<()> {
    if value.trim().is_empty() || value.chars().count() > MAX_SYNC_ID_CHARS {
        return Err(Error::InvalidSyncOperation { reason: field });
    }
    Ok(())
}

fn encode(payload: &SyncPayload) -> Result<String> {
    serde_json::to_string(payload).map_err(|_| Error::InvalidSyncOperation {
        reason: "payload ilegível",
    })
}

fn decode(payload: &str) -> Result<SyncPayload> {
    serde_json::from_str(payload).map_err(|_| Error::InvalidSyncOperation {
        reason: "payload ilegível",
    })
}
