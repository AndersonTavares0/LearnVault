//! Remoção explícita de mídia do catálogo.
//!
//! A remoção existe para o item sair do catálogo por decisão de quem usa o produto. Ela é
//! deliberada e persistente: nenhuma varredura cria ou desfaz uma remoção, e encontrar o
//! arquivo novamente no mesmo caminho não devolve a mídia ao catálogo.
//!
//! A remoção vale para a identidade atual da mídia (`media_id`), não para o conteúdo do
//! arquivo. Mover ou renomear o arquivo faz a varredura seguinte criar uma identidade nova
//! — é a política de rescan aprovada, em que o mesmo caminho reutiliza a identidade e um
//! caminho novo gera outra —, de modo que o item reaparece no catálogo como registro
//! distinto, sem a remoção anterior.
//!
//! # O que a remoção faz e o que ela não faz
//!
//! - o catálogo deixa de montar a mídia (`crate::build_catalog` filtra as removidas);
//! - a mídia deixa de ser entregue (`crate::open_media` devolve [`Error::MediaRemoved`]);
//! - os recentes deixam de listá-la (`crate::recent_study` filtra as removidas);
//! - a linha de `media`, o progresso em `study_state` e o arquivo físico permanecem
//!   exatamente como estavam.
//!
//! A distinção entre mídia ausente e mídia removida é explícita: `availability` continua
//! com dois estados e a remoção é um fato separado, guardado em `media_removal`. Uma
//! mídia pode estar ausente, removida, ou as duas coisas, sem que uma condição
//! contamine a outra.
//!
//! # Confirmação
//!
//! Retirar do catálogo uma mídia com progresso associado exige confirmação explícita,
//! porque o progresso é preservado e a mídia sai da vista de quem estudava. A recusa
//! devolve [`Error::MediaRemovalNeedsConfirmation`] sem alterar nada.
//!
//! # Autorização
//!
//! O núcleo não autoriza: ele não sabe quem está chamando. Decidir quem pode retirar uma
//! mídia do catálogo — e quem pode consultar o impacto, que revela quantos donos têm
//! progresso — é responsabilidade do adaptador de plataforma.
//!
//! # Consistência e concorrência
//!
//! Todas as operações recebem `library_id` e `media_id`; o par nunca é confiado ao
//! chamador. A inserção seleciona o par diretamente de `media`, então o `library_id`
//! gravado é o da própria mídia, e uma combinação inconsistente simplesmente não encontra
//! a linha. A confirmação e a checagem de progresso acontecem na mesma instrução, dentro
//! de uma transação `IMMEDIATE`, de modo que um progresso gravado por outro processo não
//! pode ser removido sem confirmação.

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior};

use crate::catalog::{Availability, availability_of};
use crate::db::{Database, now_unix_seconds};
use crate::error::{Error, Result};

/// O que a remoção de uma mídia afeta, para a interface decidir se pede confirmação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRemovalImpact {
    /// Identificador consultado.
    pub media_id: String,
    /// Caminho relativo atual da mídia, para confirmar o item certo.
    pub relative_path: String,
    /// Disponibilidade da mídia na última varredura.
    pub availability: Availability,
    /// Quantidade de donos distintos com progresso nesta mídia.
    pub progress_owners: usize,
    /// A mídia já estava removida do catálogo.
    pub already_removed: bool,
}

/// Resultado de uma remoção.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRemovalOutcome {
    /// Identificador removido.
    pub media_id: String,
    /// A remoção foi gravada agora; falso quando a mídia já estava removida.
    pub removed_now: bool,
    /// Quantidade de donos distintos cujo progresso foi preservado.
    pub preserved_owners: usize,
}

/// Relata o efeito de remover uma mídia do catálogo.
///
/// Não altera nada e não exige confirmação: é o dado que a interface usa para montar a
/// pergunta. A mídia precisa existir na biblioteca, mesmo que esteja ausente no disco.
pub fn media_removal_impact(
    db: &Database,
    library_id: &str,
    media_id: &str,
) -> Result<MediaRemovalImpact> {
    let impact = db
        .connection()
        .query_row(
            "SELECT m.relative_path,
                    m.availability,
                    (SELECT COUNT(DISTINCT s.owner_id)
                       FROM study_state s
                      WHERE s.media_id = m.media_id),
                    EXISTS (SELECT 1
                              FROM media_removal r
                             WHERE r.library_id = m.library_id
                               AND r.media_id = m.media_id)
               FROM media m
              WHERE m.library_id = ?1
                AND m.media_id = ?2",
            rusqlite::params![library_id, media_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()?;

    let Some((relative_path, availability, owners, removed)) = impact else {
        return Err(Error::MediaNotFound {
            media_id: media_id.to_owned(),
        });
    };

    Ok(MediaRemovalImpact {
        media_id: media_id.to_owned(),
        relative_path,
        availability: availability_of(&availability),
        progress_owners: as_usize(owners),
        already_removed: removed != 0,
    })
}

/// Remove uma mídia do catálogo, preservando o progresso e o arquivo.
///
/// Quando existe progresso associado, a remoção só acontece com `confirm_progress`; sem
/// ele, devolve [`Error::MediaRemovalNeedsConfirmation`] e nada é gravado. Repetir a
/// remoção de uma mídia já removida é inofensivo: devolve `removed_now` falso, sem erro.
pub fn remove_media(
    db: &mut Database,
    library_id: &str,
    media_id: &str,
    confirm_progress: bool,
) -> Result<MediaRemovalOutcome> {
    let tx = db
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)?;

    if !media_exists(&tx, library_id, media_id)? {
        return Err(Error::MediaNotFound {
            media_id: media_id.to_owned(),
        });
    }

    if removal_exists(&tx, library_id, media_id)? {
        let preserved_owners = owners_with_progress(&tx, media_id)?;
        tx.commit()?;
        return Ok(MediaRemovalOutcome {
            media_id: media_id.to_owned(),
            removed_now: false,
            preserved_owners,
        });
    }

    // A confirmação e a checagem de progresso ficam na mesma instrução: o par gravado vem
    // da própria linha de `media`, e um progresso que apareça antes do commit impede a
    // remoção sem confirmação.
    let inserted = tx.execute(
        "INSERT INTO media_removal (library_id, media_id, removed_at)
         SELECT m.library_id, m.media_id, ?3
           FROM media m
          WHERE m.library_id = ?1
            AND m.media_id = ?2
            AND (?4 = 1 OR NOT EXISTS (SELECT 1
                                         FROM study_state s
                                        WHERE s.media_id = m.media_id))
         ON CONFLICT (library_id, media_id) DO NOTHING",
        rusqlite::params![library_id, media_id, now_unix_seconds(), confirm_progress],
    )?;

    if inserted == 0 {
        let progress_owners = owners_with_progress(&tx, media_id)?;
        return Err(Error::MediaRemovalNeedsConfirmation {
            media_id: media_id.to_owned(),
            progress_owners,
        });
    }

    let preserved_owners = owners_with_progress(&tx, media_id)?;
    tx.commit()?;

    Ok(MediaRemovalOutcome {
        media_id: media_id.to_owned(),
        removed_now: true,
        preserved_owners,
    })
}

/// A mídia existe na biblioteca, independentemente da disponibilidade no disco.
fn media_exists(tx: &Transaction<'_>, library_id: &str, media_id: &str) -> rusqlite::Result<bool> {
    Ok(tx
        .query_row(
            "SELECT 1 FROM media WHERE library_id = ?1 AND media_id = ?2",
            rusqlite::params![library_id, media_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// A mídia já está registrada como removida.
fn removal_exists(
    tx: &Transaction<'_>,
    library_id: &str,
    media_id: &str,
) -> rusqlite::Result<bool> {
    Ok(tx
        .query_row(
            "SELECT 1 FROM media_removal WHERE library_id = ?1 AND media_id = ?2",
            rusqlite::params![library_id, media_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// Quantidade de donos distintos com progresso preservado nesta mídia.
fn owners_with_progress(tx: &Transaction<'_>, media_id: &str) -> rusqlite::Result<usize> {
    let owners = tx.query_row(
        "SELECT COUNT(DISTINCT owner_id) FROM study_state WHERE media_id = ?1",
        rusqlite::params![media_id],
        |row| row.get::<_, i64>(0),
    )?;

    Ok(as_usize(owners))
}

/// Converte uma contagem do SQLite, saturando em vez de falhar.
fn as_usize(value: i64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}
