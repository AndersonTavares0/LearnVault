//! Política de rescan: persiste o resultado de uma varredura e decide o que muda.
//!
//! Uma varredura nunca exclui registros. Arquivos encontrados são inseridos ou
//! atualizados; arquivos antes conhecidos e não encontrados só são marcados como
//! ausentes quando a varredura realmente cobriu a área onde eles estavam. Se uma
//! subárvore não pôde ser lida, os registros dela são preservados; se a localização do
//! erro é imprecisa, nenhuma ausência é registrada nessa varredura.
//!
//! A identidade da mídia é o `media_id`, nunca o caminho. Um arquivo que reaparece na
//! mesma chave normalizada reusa a linha existente e, com ela, o progresso associado: a
//! linha ausente continua no banco e volta a ficar disponível.
//!
//! Toda a aplicação acontece em uma única transação. Ou a varredura inteira é
//! registrada, ou nada muda — inclusive o histórico e o estado da biblioteca.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::db::{Database, now_unix_seconds};
use crate::error::{Error, Result};
use crate::ids::new_media_id;
use crate::paths::{KeyMode, relative_path_key};
use crate::scanner::{ScanOutcome, ScannedFile, scan};

/// Resumo do que uma varredura mudou no catálogo.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RescanSummary {
    /// Identificador do registro criado na tabela `scan`.
    pub scan_id: i64,
    /// Indica se a varredura cobriu toda a biblioteca.
    ///
    /// `false` significa que alguma área não pôde ser lida. Isso não impede o registro
    /// de ausências nas áreas que foram verificadas.
    pub complete: bool,
    /// Arquivos novos inseridos no catálogo.
    pub inserted: usize,
    /// Registros já conhecidos que a varredura reafirmou.
    pub updated: usize,
    /// Registros marcados como ausentes.
    pub marked_missing: usize,
    /// Registros preservados por estarem em uma área não verificada.
    pub preserved: usize,
    /// Chaves normalizadas disputadas por mais de um arquivo nesta varredura.
    pub ambiguous: Vec<String>,
}

/// Aplica uma varredura já realizada ao catálogo, sem tocar o sistema de arquivos.
///
/// A função é pura em relação ao disco: recebe o resultado de [`scan`] e persiste as
/// decisões. Isso permite testar a política sem montar uma biblioteca real e mantém a
/// leitura do sistema de arquivos isolada na fronteira do scanner.
///
/// `mode` precisa ser o mesmo modo com que o resultado foi produzido. As chaves dos
/// arquivos já vêm prontas em [`ScanOutcome`], mas as áreas não verificadas são
/// normalizadas aqui: usar um modo diferente do scanner faria um diretório ilegível não
/// cobrir os registros que ele deveria proteger. [`rescan`] garante a coerência usando o
/// modo do próprio host, o mesmo de [`scan`].
pub fn apply_scan(
    db: &mut Database,
    library_id: &str,
    outcome: &ScanOutcome,
    mode: KeyMode,
) -> Result<RescanSummary> {
    let tx = db.conn.transaction()?;

    // A biblioteca precisa existir antes da mídia e do histórico, que a referenciam.
    tx.execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 0)
         ON CONFLICT (library_id) DO NOTHING",
        rusqlite::params![library_id],
    )?;

    let unverified = unverified_prefixes(outcome, mode);
    let freeze_absences = outcome.inaccessible.iter().any(|path| path == ".");

    let complete = outcome.inaccessible.is_empty();
    let scan_id = insert_scan(&tx, library_id, "completed", complete)?;
    // O histórico é um conjunto de áreas, não uma lista: repetir o mesmo caminho não pode
    // violar a chave primária e desfazer a varredura inteira por causa de uma duplicata.
    let unverified_areas: BTreeSet<&str> =
        outcome.inaccessible.iter().map(String::as_str).collect();
    for path in unverified_areas {
        tx.execute(
            "INSERT INTO scan_inaccessible (scan_id, relative_path) VALUES (?1, ?2)",
            rusqlite::params![scan_id, path],
        )?;
    }

    // Arquivos encontrados, indexados pela chave normalizada. Uma chave disputada por
    // mais de um arquivo é ambígua: nenhum dos dois é aplicado, porque escolher um
    // arbitrariamente associaria a identidade de um arquivo ao conteúdo do outro.
    let mut found: BTreeMap<&str, &ScannedFile> = BTreeMap::new();
    let mut ambiguous: BTreeSet<String> = BTreeSet::new();
    for file in &outcome.files {
        if found
            .insert(file.relative_path_key.as_str(), file)
            .is_some()
        {
            ambiguous.insert(file.relative_path_key.clone());
        }
    }

    let mut summary = RescanSummary {
        scan_id,
        complete,
        ambiguous: ambiguous.iter().cloned().collect(),
        ..RescanSummary::default()
    };

    for (key, file) in &found {
        if ambiguous.contains(*key) {
            continue;
        }

        match known_media(&tx, library_id, key)? {
            Some(media_id) => {
                // O arquivo continua no mesmo lugar: atualiza a localização e os
                // metadados e reafirma a disponibilidade, sem tocar a identidade.
                tx.execute(
                    "UPDATE media
                     SET relative_path = ?1, size = ?2, mtime = ?3, availability = 'available'
                     WHERE media_id = ?4
                       AND (relative_path <> ?1 OR size <> ?2 OR mtime <> ?3
                            OR availability <> 'available')",
                    rusqlite::params![
                        file.relative_path,
                        size_as_integer(file.size),
                        file.mtime,
                        media_id
                    ],
                )?;
                summary.updated += 1;
            }
            None => {
                tx.execute(
                    "INSERT INTO media
                         (media_id, library_id, relative_path, relative_path_key,
                          size, mtime, availability)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'available')",
                    rusqlite::params![
                        new_media_id(),
                        library_id,
                        file.relative_path,
                        key,
                        size_as_integer(file.size),
                        file.mtime,
                    ],
                )?;
                summary.inserted += 1;
            }
        }
    }

    for (key, media_id) in known_media_rows(&tx, library_id)? {
        if found.contains_key(key.as_str()) {
            continue;
        }
        if !is_available(&tx, &media_id)? {
            continue;
        }

        let covered = freeze_absences || is_covered_by_unverified(&key, &unverified);
        if covered {
            summary.preserved += 1;
            continue;
        }

        tx.execute(
            "UPDATE media SET availability = 'missing' WHERE media_id = ?1",
            rusqlite::params![media_id],
        )?;
        summary.marked_missing += 1;
    }

    // `available` significa apenas que a raiz foi alcançada nesta varredura; não afirma que
    // o catálogo está íntegro. Uma varredura parcial também alcança a raiz — quem precisa
    // saber se alguma área ficou fora da leitura consulta `complete`, no resumo.
    tx.execute(
        "UPDATE library SET available = 1 WHERE library_id = ?1",
        rusqlite::params![library_id],
    )?;

    tx.commit()?;
    Ok(summary)
}

/// Lê a biblioteca e aplica o resultado ao catálogo.
///
/// Se a raiz não puder ser lida, a tentativa é registrada como uma varredura com falha,
/// sem alterar a mídia, o estado de estudo ou a disponibilidade da biblioteca.
pub fn rescan(db: &mut Database, library_id: &str, root: &Path) -> Result<RescanSummary> {
    match scan(root) {
        // O modo vem do próprio host, exatamente como dentro do scanner: a chave usada
        // para interpretar as áreas não verificadas nunca pode divergir da chave gravada.
        Ok(outcome) => apply_scan(db, library_id, &outcome, KeyMode::for_host()),
        Err(scan_error) => Err(record_failed_scan(db, library_id, scan_error)),
    }
}

/// Registra a tentativa que falhou, preservando a falha original como causa.
fn record_failed_scan(db: &mut Database, library_id: &str, scan_error: Error) -> Error {
    match write_failed_scan(db, library_id) {
        Ok(()) => scan_error,
        Err(history_error) => Error::ScanHistoryNotRecorded {
            scan_error: Box::new(scan_error),
            history_error,
        },
    }
}

/// Grava o histórico da tentativa que falhou em uma transação própria.
fn write_failed_scan(db: &mut Database, library_id: &str) -> rusqlite::Result<()> {
    let tx = db.conn.transaction()?;

    // A linha da biblioteca é necessária para a chave estrangeira do histórico. Se ela
    // já existir, nada muda: uma varredura com falha não altera a disponibilidade.
    tx.execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 0)
         ON CONFLICT (library_id) DO NOTHING",
        rusqlite::params![library_id],
    )?;
    tx.execute(
        "INSERT INTO scan (library_id, status, completed_at, complete)
         VALUES (?1, 'failed', ?2, 0)",
        rusqlite::params![library_id, now_unix_seconds()],
    )?;

    tx.commit()
}

/// Insere o registro da varredura e devolve o identificador gerado.
fn insert_scan(
    tx: &rusqlite::Transaction<'_>,
    library_id: &str,
    status: &str,
    complete: bool,
) -> rusqlite::Result<i64> {
    tx.execute(
        "INSERT INTO scan (library_id, status, completed_at, complete)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![library_id, status, now_unix_seconds(), complete],
    )?;
    Ok(tx.last_insert_rowid())
}

/// Devolve o `media_id` já conhecido para a chave, se existir.
fn known_media(
    tx: &rusqlite::Transaction<'_>,
    library_id: &str,
    key: &str,
) -> rusqlite::Result<Option<String>> {
    tx.query_row(
        "SELECT media_id FROM media WHERE library_id = ?1 AND relative_path_key = ?2",
        rusqlite::params![library_id, key],
        |row| row.get(0),
    )
    .optional_media_id()
}

/// Lista as chaves conhecidas da biblioteca com a identidade correspondente.
fn known_media_rows(
    tx: &rusqlite::Transaction<'_>,
    library_id: &str,
) -> rusqlite::Result<Vec<(String, String)>> {
    let mut statement = tx.prepare(
        "SELECT relative_path_key, media_id FROM media WHERE library_id = ?1 ORDER BY relative_path_key",
    )?;
    let rows = statement.query_map(rusqlite::params![library_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut collected = Vec::new();
    for row in rows {
        collected.push(row?);
    }
    Ok(collected)
}

/// Indica se a mídia está marcada como disponível.
fn is_available(tx: &rusqlite::Transaction<'_>, media_id: &str) -> rusqlite::Result<bool> {
    let availability: String = tx.query_row(
        "SELECT availability FROM media WHERE media_id = ?1",
        rusqlite::params![media_id],
        |row| row.get(0),
    )?;
    Ok(availability == "available")
}

/// Normaliza as áreas não verificadas em prefixos de chave.
///
/// Uma entrada que não pode ser normalizada não pode corresponder a nenhuma linha
/// existente; ela só torna a varredura parcial.
fn unverified_prefixes(outcome: &ScanOutcome, mode: KeyMode) -> Vec<String> {
    outcome
        .inaccessible
        .iter()
        .filter(|path| path.as_str() != ".")
        .filter_map(|path| relative_path_key(path, mode).ok())
        .collect()
}

/// Indica se a chave está dentro de uma área que a varredura não verificou.
fn is_covered_by_unverified(key: &str, unverified: &[String]) -> bool {
    unverified.iter().any(|prefix| {
        key == prefix
            || (key.len() > prefix.len()
                && key.starts_with(prefix.as_str())
                && key.as_bytes()[prefix.len()] == b'/')
    })
}

/// Converte o tamanho em bytes para o inteiro assinado usado pelo SQLite.
///
/// Tamanhos acima de `i64::MAX` (inviáveis em qualquer sistema de arquivos real)
/// saturam no limite em vez de falhar ou entrar em pânico.
fn size_as_integer(size: u64) -> i64 {
    i64::try_from(size).unwrap_or(i64::MAX)
}

/// Extensão local para transformar "nenhuma linha" em `None` sem `unwrap`.
trait OptionalMediaId {
    /// Converte o erro de ausência de linha em `None`.
    fn optional_media_id(self) -> rusqlite::Result<Option<String>>;
}

impl OptionalMediaId for rusqlite::Result<String> {
    fn optional_media_id(self) -> rusqlite::Result<Option<String>> {
        match self {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(other) => Err(other),
        }
    }
}
