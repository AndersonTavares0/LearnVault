//! Typed errors of the domain.
//!
//! Each variant names a concrete domain failure. No error path uses
//! `panic` or `unwrap`: failures are propagated as values and translated by the
//! adapters (HTTP server, Tauri commands) to each platform's format.
//!
//! The `Display` messages are for logs and local diagnostics. The adapters choose the
//! text presented to the client and must not echo file-system paths.

use std::path::PathBuf;

/// LearnVault domain error.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The database file could not be opened or created.
    #[error("não foi possível abrir o banco de dados em {path}")]
    OpenDatabase {
        /// Path of the database file.
        path: PathBuf,
        /// Original cause reported by SQLite.
        #[source]
        source: rusqlite::Error,
    },

    /// The version recorded in the database could not be read.
    ///
    /// It is a read, not a migration: separating the two variants avoids reporting a
    /// migration that was never attempted.
    #[error("não foi possível ler a versão do schema do banco de dados")]
    ReadSchemaVersion {
        /// Original cause reported by SQLite.
        #[source]
        source: rusqlite::Error,
    },

    /// The migration chain could not be applied.
    #[error("não foi possível aplicar as migrações do banco de dados")]
    Migrations(#[source] rusqlite::Error),

    /// The database was created by a newer version of this program.
    ///
    /// The schema is never downgraded automatically: proceeding could corrupt data
    /// written by a version this binary does not know.
    #[error(
        "o schema do banco está na versão {found}, acima da versão {known} conhecida por este programa"
    )]
    SchemaTooNew {
        /// Version stored in the database.
        found: usize,
        /// Version known to this program.
        known: usize,
    },

    /// The file belongs to the legacy schema of the Go service.
    ///
    /// The conversion is not automatic: opening that file as Rust could create a second
    /// administrator and leave the legacy data invisible.
    #[error(
        "legacy_go_database_requires_migration: back up the database and migrate it before using the Rust server"
    )]
    LegacyGoDatabase,

    /// I/O failure during the legacy import.
    #[error("falha de {operation} no arquivo de importação {path}")]
    LegacyImportIo {
        /// Operation that failed.
        operation: String,
        /// Path involved.
        path: PathBuf,
        /// Cause from the operating system.
        #[source]
        source: std::io::Error,
    },

    /// I/O failure during backup or restore.
    #[error("falha de {operation} em {path}")]
    BackupIo {
        /// Operation that failed.
        operation: String,
        /// Path involved.
        path: PathBuf,
        /// Cause from the operating system.
        #[source]
        source: std::io::Error,
    },

    /// The backup file was not found.
    #[error("arquivo de backup não encontrado em {0}")]
    BackupFileNotFound(PathBuf),

    /// The backup does not contain a recognized LearnVault schema.
    #[error("backup does not contain a valid LearnVault database schema")]
    RestoreInvalidSchema,

    /// The current destination could not be checkpointed before replacement.
    #[error("could not checkpoint the destination database before restore")]
    RestoreCheckpointFailed,

    /// The backup's integrity check failed.
    #[error("falha na integridade do backup SQLite: {0}")]
    BackupIntegrityFailed(String),

    /// The database stayed busy and the backup hit the time limit.
    #[error("tempo limite excedido ao realizar o backup do banco de dados")]
    BackupTimeout,

    /// The backup or restore process failed.
    #[error("falha durante o backup: {0}")]
    BackupFailed(String),

    /// A value in the Go database does not satisfy the migration contract.
    #[error("valor inválido na importação legada: {field} = {value}")]
    LegacyImportInvalidValue {
        /// Invalid field.
        field: String,
        /// Refused value.
        value: String,
    },

    /// Two different records represent the same Rust key.
    #[error("colisão na importação legada para o caminho {path}")]
    LegacyImportCollision {
        /// Normalized path in a collision.
        path: String,
    },

    /// A legacy row points to an entity that does not exist.
    #[error("referência órfã na importação legada: {relation} = {key}")]
    LegacyImportOrphan {
        /// Relation that contains the reference.
        relation: String,
        /// Chave apontada.
        key: String,
    },

    /// The import would be ambiguous because one path is used as more than one key.
    #[error("caminho inválido na importação legada: {path}")]
    LegacyImportInvalidPath {
        /// Refused path.
        path: String,
    },

    /// SQL failure that does not fit the previous variants.
    #[error("erro de banco de dados")]
    Database(#[from] rusqlite::Error),

    /// The library root does not exist.
    #[error("a raiz da biblioteca não existe em {path}")]
    LibraryRootMissing {
        /// Path given as the library root.
        path: PathBuf,
    },

    /// The library root exists, but is not a directory.
    #[error("a raiz da biblioteca em {path} não é um diretório")]
    LibraryRootNotDirectory {
        /// Path given as the library root.
        path: PathBuf,
    },

    /// The library root exists, but could not be read.
    #[error("não foi possível ler a raiz da biblioteca em {path}")]
    LibraryRootUnreadable {
        /// Path given as the library root.
        path: PathBuf,
        /// Original cause reported by the file system.
        #[source]
        source: std::io::Error,
    },

    /// The configured library root was replaced by a link, junction or reparse
    /// point after it was resolved.
    ///
    /// The root is already delivered canonicalized by the platform integration. If the final
    /// component became a link, following it would authorize files outside the library, so the
    /// replacement is refused instead of resolved.
    #[error("a raiz da biblioteca é um link e não é seguida em {path}")]
    LibraryRootIsLink {
        /// Path given as the library root.
        path: PathBuf,
    },

    /// A relative path of the library is invalid or would try to escape the root.
    #[error("caminho relativo inválido na biblioteca: {path}")]
    InvalidRelativePath {
        /// Refused path.
        path: String,
    },

    /// A remote (or non-local) course source could not be read.
    #[error("não foi possível ler a fonte do curso")]
    SourceUnavailable {
        /// Sanitized failure of the source.
        #[source]
        source: crate::source::SourceError,
    },

    /// The scan attempt could not be recorded in the history.
    ///
    /// Recording the history is secondary: the scan's original failure is preserved
    /// as the cause, so that the reason the library could not be read is never
    /// hidden by a database write error.
    #[error("não foi possível registrar a varredura com falha no histórico")]
    ScanHistoryNotRecorded {
        /// Original failure of the scan.
        #[source]
        scan_error: Box<Error>,
        /// Original cause reported by SQLite when writing the history.
        history_error: rusqlite::Error,
    },

    /// The media identifier does not exist in the library.
    #[error("mídia desconhecida na biblioteca: {media_id}")]
    MediaNotFound {
        /// Identificador consultado.
        media_id: String,
    },

    /// The catalog row exists, but is marked as missing.
    #[error("a mídia está marcada como ausente: {media_id}")]
    MediaUnavailable {
        /// Identificador consultado.
        media_id: String,
    },

    /// The media was available in the last scan and is no longer on the disk.
    #[error("a mídia desapareceu do disco: {media_id}")]
    MediaMissingOnDisk {
        /// Identificador consultado.
        media_id: String,
    },

    /// The resolved path left the library root.
    #[error("o caminho resolvido está fora da biblioteca: {media_id}")]
    MediaOutsideLibrary {
        /// Identificador consultado.
        media_id: String,
    },

    /// The final component of the path is a symbolic link.
    ///
    /// The library is indexed without following links, so a link is not a known media.
    #[error("a mídia é um link simbólico e não é seguida: {media_id}")]
    MediaIsSymlink {
        /// Identificador consultado.
        media_id: String,
    },

    /// The path points to something that is not a regular file.
    #[error("a mídia não é um arquivo comum: {media_id}")]
    MediaNotAFile {
        /// Identificador consultado.
        media_id: String,
    },

    /// The file could not be opened or inspected.
    #[error("não foi possível abrir a mídia: {media_id}")]
    MediaUnreadable {
        /// Identificador consultado.
        media_id: String,
        /// Original cause reported by the file system.
        #[source]
        source: std::io::Error,
    },

    /// The opened file is not the file that was validated at the path.
    ///
    /// Only verifiable on Unix, where the system exposes the identity of the open descriptor.
    #[error("o arquivo aberto não corresponde ao caminho validado: {media_id}")]
    MediaIdentityMismatch {
        /// Identificador consultado.
        media_id: String,
    },

    /// The size recorded in the database is invalid (negative or outside `u64`).
    ///
    /// Such a value indicates catalog corruption, not an empty file: saturating
    /// to zero would hide the failure and falsify change detection.
    #[error("o tamanho registrado é inválido no banco: {media_id}")]
    MediaCorruptSize {
        /// Identificador consultado.
        media_id: String,
    },

    /// The owner of the study progress was not identified.
    ///
    /// The owner identifier is opaque to the domain, but it must exist: progress is
    /// always someone's, and an empty owner would mix data of different owners.
    #[error("o dono do progresso precisa ser identificado")]
    InvalidOwnerId,

    /// The resume position is invalid.
    #[error("posição de retomada inválida: {position_seconds}")]
    InvalidStudyPosition {
        /// Rejected position, in seconds.
        position_seconds: i64,
    },

    /// The media was explicitly removed from the catalog.
    ///
    /// The `media` row and the progress keep existing: the removal takes the media out of the
    /// catalog, but does not delete data. A removed media is neither delivered nor listed.
    #[error("a mídia foi removida do catálogo: {media_id}")]
    MediaRemoved {
        /// Identificador consultado.
        media_id: String,
    },

    /// The removal requires confirmation because there is associated progress.
    ///
    /// Progress is never deleted by a removal; when it exists, the caller must
    /// explicitly confirm that they want to take the media out of the catalog anyway.
    #[error(
        "a mídia tem progresso de {progress_owners} dono(s) e exige confirmação para ser removida: {media_id}"
    )]
    MediaRemovalNeedsConfirmation {
        /// Identificador consultado.
        media_id: String,
        /// Number of distinct owners with progress on the media.
        progress_owners: usize,
    },

    /// The day used in the streak calculation is not a valid ISO date.
    #[error("dia de estudo inválido: {day}")]
    InvalidStudyDay {
        /// Data recusada, no formato `YYYY-MM-DD`.
        day: String,
    },

    /// The focus session is invalid or exceeds the timer's limits.
    #[error("sessão de foco inválida: {reason}")]
    InvalidFocusSession {
        /// Stable reason for the rejection.
        reason: &'static str,
    },

    /// The same session identifier was resent with different content.
    #[error("o identificador da sessão já foi usado com outros dados")]
    FocusSessionConflict,

    /// The daily mission identifier is not known.
    #[error("missão diária desconhecida: {mission_id}")]
    InvalidMission {
        /// Refused identifier.
        mission_id: String,
    },

    /// The content of the study note is invalid or exceeds the limit.
    #[error("conteúdo da anotação de estudo é inválido ou excede o limite")]
    InvalidStudyNoteContent,

    /// The study note was not found.
    #[error("anotação de estudo não encontrada: {note_id}")]
    StudyNoteNotFound {
        /// Identificador consultado.
        note_id: String,
    },

    /// A PDF annotation has an invalid page, kind, colour, rectangles or text.
    #[error("anotação de PDF inválida")]
    InvalidPdfAnnotation,

    /// The PDF annotation was not found.
    #[error("anotação de PDF não encontrada: {annotation_id}")]
    PdfAnnotationNotFound {
        /// Identificador consultado.
        annotation_id: String,
    },

    /// The content of the card's front or back is invalid or exceeds the limit.
    #[error("conteúdo do cartão é inválido ou excede o limite")]
    InvalidFlashcardContent,

    /// The card's review rating is invalid.
    #[error("avaliação de revisão do cartão é inválida")]
    InvalidFlashcardRating,

    /// The card was not found.
    #[error("cartão não encontrado: {card_id}")]
    FlashcardNotFound {
        /// Identificador consultado.
        card_id: String,
    },

    /// The identifier of the favourite target (course or lesson) is invalid.
    #[error("identificador de favorito inválido")]
    InvalidFavoriteTarget,

    /// The label of a time marker is empty or exceeds the limit.
    #[error("rótulo do marcador inválido")]
    InvalidBookmarkLabel,

    /// The time marker was not found for the owner.
    #[error("marcador não encontrado: {bookmark_id}")]
    BookmarkNotFound {
        /// Identificador consultado.
        bookmark_id: String,
    },

    /// The requested user does not exist in the local database.
    #[error("usuário não encontrado")]
    UserNotFound,

    /// The given user name is already in use.
    #[error("nome de usuário já em uso")]
    UsernameTaken,

    /// An operation restricted to student accounts tried to change or remove an administrator.
    #[error("não é permitido remover ou restringir uma conta de administrador")]
    CannotModifyAdminUser,

    /// The mission has not yet met the requirement needed for redemption.
    #[error("a missão diária ainda não foi concluída")]
    MissionNotComplete,

    /// Weekly goal outside the supported range.
    #[error("meta semanal inválida: {weekly_goal}")]
    InvalidWeeklyGoal {
        /// Refused value.
        weekly_goal: i64,
    },

    /// A playback speed or subtitle language is outside the accepted bounds.
    #[error("preferência de reprodução inválida")]
    InvalidPlayerPreference,

    /// A synchronization operation is malformed.
    ///
    /// Covers an empty identifier, an oversized batch and an unreadable stored payload.
    /// Domain data inside an operation is normalized or skipped instead of rejected, so a
    /// device never retries an operation that can never be applied.
    #[error("invalid synchronization operation: {reason}")]
    InvalidSyncOperation {
        /// Stable rejection reason.
        reason: &'static str,
    },

    /// The requested course roots cannot be applied to the source.
    #[error(transparent)]
    SourceRoot(#[from] crate::roots::RootError),
}

/// Result of the domain operations.
pub type Result<T> = std::result::Result<T, Error>;
