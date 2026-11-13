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
//! Um arquivo que muda de caminho também mantém a identidade: quando a linha antiga está
//! ausente e o arquivo novo não é conhecido, tamanho, mtime e um hash amostral do
//! conteúdo reconhecem o mesmo arquivo. O hash é lido somente das pontas do arquivo, e
//! apenas para arquivos novos ou com metadados alterados; sem ele (arquivo ilegível ou
//! linha anterior à migração), a decisão cai para tamanho e mtime, e só é tomada quando o
//! candidato é único.
//!
//! Toda a aplicação acontece em uma única transação. Ou a varredura inteira é
//! registrada, ou nada muda — inclusive o histórico e o estado da biblioteca.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use sha2::{Digest, Sha256};

use crate::db::{Database, now_unix_seconds};
use crate::error::{Error, Result};
use crate::ids::new_media_id;
use crate::paths::{KeyMode, relative_path_key};
use crate::scanner::{ScanOutcome, ScannedFile, scan};

/// Bytes lidos em cada ponta do arquivo para o hash amostral.
const SAMPLE_BYTES: u64 = 64 * 1024;

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
    /// Registros cujo tamanho, mtime ou conteúdo mudou.
    pub changed: usize,
    /// Registros cujo arquivo mudou de caminho e manteve a identidade.
    pub moved: usize,
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
/// Como não há raiz, arquivos novos ou alterados não recebem hash amostral; a detecção de
/// movimento usa somente tamanho e mtime. [`apply_incremental_scan`] recebe a raiz e
/// confirma o movimento pelo conteúdo.
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
    apply(db, library_id, outcome, mode, None)
}

/// Aplica uma varredura lendo a raiz somente para o hash amostral dos arquivos novos ou
/// alterados.
///
/// A biblioteca continua somente leitura: o hash é lido das pontas do arquivo, nunca
/// escrito. É a forma usada por [`rescan`] e pela indexação automática.
pub fn apply_incremental_scan(
    db: &mut Database,
    library_id: &str,
    outcome: &ScanOutcome,
    mode: KeyMode,
    root: &Path,
) -> Result<RescanSummary> {
    apply(db, library_id, outcome, mode, Some(root))
}

/// Linha de mídia já conhecida, lida em um único snapshot antes das decisões.
struct KnownMedia {
    media_id: String,
    size: i64,
    mtime: i64,
    availability: String,
    content_hash: Option<String>,
}

fn apply(
    db: &mut Database,
    library_id: &str,
    outcome: &ScanOutcome,
    mode: KeyMode,
    root: Option<&Path>,
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
    let covered = |key: &str| freeze_absences || is_covered_by_unverified(key, &unverified);

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

    let known = known_media(&tx, library_id)?;
    let moves = MoveIndex::new(&known, &found, &covered);
    // Identidades já realocadas nesta varredura: a linha antiga não pode ser marcada como
    // ausente depois de o arquivo ter sido encontrado no caminho novo.
    let mut moved_ids: BTreeSet<String> = BTreeSet::new();

    for (key, file) in &found {
        if ambiguous.contains(*key) {
            continue;
        }

        let size = size_as_integer(file.size);
        match known.get(*key) {
            Some(row) => {
                let metadata_changed = row.size != size || row.mtime != file.mtime;
                let mut content_hash = row.content_hash.clone();
                let mut content_changed = false;
                if metadata_changed {
                    match root.and_then(|root| sampled_hash(root, &file.relative_path)) {
                        Some(hash) => {
                            content_changed = row.content_hash.as_deref() != Some(hash.as_str());
                            content_hash = Some(hash);
                        }
                        // Sem hash o metadado já provou a mudança.
                        None => content_changed = true,
                    }
                }
                if content_changed {
                    summary.changed += 1;
                }
                tx.execute(
                    "UPDATE media
                     SET relative_path = ?1, size = ?2, mtime = ?3, availability = 'available',
                         content_hash = ?4
                     WHERE media_id = ?5
                       AND (relative_path <> ?1 OR size <> ?2 OR mtime <> ?3
                            OR availability <> 'available' OR content_hash IS NOT ?4)",
                    rusqlite::params![
                        file.relative_path,
                        size,
                        file.mtime,
                        content_hash,
                        row.media_id
                    ],
                )?;
                summary.updated += 1;
            }
            None => {
                let hash = root.and_then(|root| sampled_hash(root, &file.relative_path));
                match moves.moved(&moved_ids, size, file.mtime, hash.as_deref()) {
                    Some(media_id) => {
                        // Mesmo arquivo em outro caminho: a identidade é preservada e,
                        // com ela, as notas, os marcadores e o progresso.
                        tx.execute(
                            "UPDATE media
                             SET relative_path = ?1, relative_path_key = ?2, size = ?3,
                                 mtime = ?4, availability = 'available', content_hash = ?5
                             WHERE media_id = ?6",
                            rusqlite::params![
                                file.relative_path,
                                key,
                                size,
                                file.mtime,
                                hash,
                                media_id
                            ],
                        )?;
                        moved_ids.insert(media_id.clone());
                        summary.moved += 1;
                        summary.updated += 1;
                    }
                    None => {
                        tx.execute(
                            "INSERT INTO media
                                 (media_id, library_id, relative_path, relative_path_key,
                                  size, mtime, availability, content_hash)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'available', ?7)",
                            rusqlite::params![
                                new_media_id(),
                                library_id,
                                file.relative_path,
                                key,
                                size,
                                file.mtime,
                                hash,
                            ],
                        )?;
                        summary.inserted += 1;
                    }
                }
            }
        }
    }

    for (key, row) in &known {
        if found.contains_key(key.as_str()) || moved_ids.contains(&row.media_id) {
            continue;
        }
        if row.availability != "available" {
            continue;
        }
        if covered(key) {
            summary.preserved += 1;
            continue;
        }

        tx.execute(
            "UPDATE media SET availability = 'missing' WHERE media_id = ?1",
            rusqlite::params![row.media_id],
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
        Ok(outcome) => apply_incremental_scan(db, library_id, &outcome, KeyMode::for_host(), root),
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

/// Lista as linhas conhecidas da biblioteca, indexadas pela chave normalizada.
fn known_media(
    tx: &rusqlite::Transaction<'_>,
    library_id: &str,
) -> rusqlite::Result<BTreeMap<String, KnownMedia>> {
    let mut statement = tx.prepare(
        "SELECT relative_path_key, media_id, size, mtime, availability, content_hash
         FROM media WHERE library_id = ?1 ORDER BY relative_path_key",
    )?;
    let rows = statement.query_map(rusqlite::params![library_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            KnownMedia {
                media_id: row.get(1)?,
                size: row.get(2)?,
                mtime: row.get(3)?,
                availability: row.get(4)?,
                content_hash: row.get(5)?,
            },
        ))
    })?;

    let mut collected = BTreeMap::new();
    for row in rows {
        let (key, media) = row?;
        collected.insert(key, media);
    }
    Ok(collected)
}

/// Candidatos ausentes indexados por hash e por metadados.
///
/// A varredura de uma biblioteca grande não pode comparar cada arquivo novo com cada linha
/// antiga: o índice deixa a decisão de movimento em tempo constante e mantém a regra de
/// recusar qualquer escolha ambígua.
struct MoveIndex<'a> {
    by_hash: BTreeMap<&'a str, Vec<&'a str>>,
    by_metadata: BTreeMap<(i64, i64), Vec<&'a str>>,
}

impl<'a> MoveIndex<'a> {
    fn new(
        known: &'a BTreeMap<String, KnownMedia>,
        found: &BTreeMap<&str, &ScannedFile>,
        covered: &impl Fn(&str) -> bool,
    ) -> Self {
        let mut index = Self {
            by_hash: BTreeMap::new(),
            by_metadata: BTreeMap::new(),
        };
        for (key, row) in known {
            if found.contains_key(key.as_str()) || covered(key) {
                continue;
            }
            if let Some(hash) = row.content_hash.as_deref() {
                index.by_hash.entry(hash).or_default().push(&row.media_id);
            }
            index
                .by_metadata
                .entry((row.size, row.mtime))
                .or_default()
                .push(&row.media_id);
        }
        index
    }

    /// Devolve a identidade ausente que corresponde ao arquivo novo, quando a escolha é
    /// única. O hash confirmado vence a comparação de metadados; duas linhas com o mesmo
    /// conteúdo ou os mesmos metadados tornam a movimentação ambígua, porque escolher uma
    /// associaria notas e progresso ao arquivo errado.
    fn moved(
        &self,
        moved_ids: &BTreeSet<String>,
        size: i64,
        mtime: i64,
        hash: Option<&str>,
    ) -> Option<String> {
        if let Some(ids) = hash.and_then(|hash| self.by_hash.get(hash)) {
            return unique_unclaimed(ids, moved_ids).map(str::to_owned);
        }
        unique_unclaimed(self.by_metadata.get(&(size, mtime))?, moved_ids).map(str::to_owned)
    }
}

/// Devolve o único identificador ainda não realocado, ou `None` quando há zero ou vários.
fn unique_unclaimed<'a>(ids: &[&'a str], moved_ids: &BTreeSet<String>) -> Option<&'a str> {
    let mut fresh = ids
        .iter()
        .copied()
        .filter(|media_id| !moved_ids.contains(*media_id));
    match (fresh.next(), fresh.next()) {
        (Some(media_id), None) => Some(media_id),
        _ => None,
    }
}

/// Hash SHA-256 do tamanho e das pontas do arquivo.
///
/// Arquivos de até duas amostras são lidos inteiros. Um arquivo ilegível não tem hash:
/// a decisão de movimento cai para metadados e nunca falha a varredura por causa dele.
///
/// shortcut: um arquivo alterado no meio sem mudar tamanho nem mtime não é detectado,
/// porque o hash amostral só é calculado quando o metadado muda; revisitar quando houver
/// uma origem de eventos com confiança suficiente para dispensar a comparação.
fn sampled_hash(root: &Path, relative_path: &str) -> Option<String> {
    let mut file = File::open(root.join(relative_path)).ok()?;
    let length = file.metadata().ok()?.len();
    let sample_len = usize::try_from(SAMPLE_BYTES).ok()?;
    let mut sample = Vec::with_capacity(sample_len.saturating_mul(2));

    if length <= SAMPLE_BYTES.saturating_mul(2) {
        file.read_to_end(&mut sample).ok()?;
    } else {
        let mut head = vec![0; sample_len];
        file.read_exact(&mut head).ok()?;
        file.seek(SeekFrom::End(-(SAMPLE_BYTES as i64))).ok()?;
        let mut tail = vec![0; sample_len];
        file.read_exact(&mut tail).ok()?;
        sample.extend_from_slice(&head);
        sample.extend_from_slice(&tail);
    }

    let mut digest = Sha256::new();
    digest.update(length.to_le_bytes());
    digest.update(&sample);
    Some(
        digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
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
