//! Desktop IPC error: serializable, stable and sanitized.
//!
//! Each failure becomes a `{ code, message }` object. The `code` is the frontend's contract and does not
//! change without versioning; the `message` is short and never contains file-system
//! paths or secrets. The core's diagnostic message (`Display`) is local and is not
//! sent to the webview.

use serde::Serialize;

/// Stable IPC error code consumed by the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IpcErrorCode {
    /// The catalog could not be derived from the local data.
    CatalogUnavailable,
    /// The local database could not be opened or read.
    DatabaseUnavailable,
    /// The folder chosen for a course is invalid.
    CoursePathInvalid,
    /// The application's course configuration could not be written.
    CoursesConfigUnavailable,
    /// No course is registered to index.
    NoCoursesRegistered,
    /// A rescan is already in progress.
    RescanInProgress,
    /// The library scan could not be completed.
    RescanUnavailable,
    /// The local owner's study progress could not be read or written.
    StudyUnavailable,
    /// The queried media does not exist in the local catalog.
    MediaNotFound,
    /// The Flatpak sandbox blocks opening media in the system player.
    SystemPlayerUnavailable,
    /// The resume position sent is invalid.
    InvalidPosition,
    /// A Pomodoro session sent is invalid.
    InvalidFocusSession,
    /// The key of a session was already used with other content.
    FocusSessionConflict,
    /// The daily mission is not yet ready to be redeemed.
    MissionNotComplete,
    /// The weekly goal is outside the range of 1 to 7 days.
    InvalidWeeklyGoal,
    /// The removal requires confirmation because there is associated progress.
    MediaRemovalNeedsConfirmation,
    /// The media removal could not be completed.
    MediaRemovalUnavailable,
    /// The course given is not registered in the application.
    CourseNotFound,
    /// The image chosen for the cover is not PNG, JPEG or WebP, or exceeds the limit.
    InvalidCover,
    /// The course's cover could not be written or removed.
    CoverUnavailable,
    /// The order given is not a valid permutation of the siblings.
    InvalidOrder,
    /// The course's manual order could not be written or removed.
    OrderUnavailable,
    /// The update check failed because of a network error or an unreachable endpoint.
    UpdateCheckFailed,
    /// The update check or installation could not be completed.
    UpdateUnavailable,
    /// The requested note was not found or does not belong to the user.
    NoteNotFound,
    /// The content of the note is invalid or exceeds the accepted limit.
    InvalidNote,
    /// The PDF annotation was not found or does not belong to the user.
    PdfAnnotationNotFound,
    /// The PDF annotation has an invalid page, kind, color, rectangle or text.
    InvalidPdfAnnotation,
    /// The requested memorization card was not found or does not belong to the user.
    FlashcardNotFound,
    /// The content of the memorization card is invalid or exceeds the accepted limit.
    InvalidFlashcard,
    /// The rating given in the card review is invalid.
    InvalidReviewRating,
    /// The favourite target (course or media) is invalid.
    InvalidFavoriteTarget,
    /// The time marker was not found or does not belong to the user.
    BookmarkNotFound,
    /// The marker's label is empty or exceeds the accepted limit.
    InvalidBookmarkLabel,
    /// The playback preferences are outside the accepted range.
    InvalidPlayerPreference,
    /// The local sync queue could not be read or written.
    SyncUnavailable,
    /// The sync server's URL is invalid.
    InvalidSyncServer,
    /// The server refused the sync credentials.
    SyncLoginFailed,
}

/// Serializable IPC error returned to the webview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IpcError {
    /// Stable code consumed by the frontend.
    pub code: IpcErrorCode,
    /// Short message, with no paths or secrets.
    pub message: &'static str,
}

impl IpcError {
    /// Catalog unavailable: the derivation failed over the local data.
    pub fn catalog_unavailable() -> Self {
        Self {
            code: IpcErrorCode::CatalogUnavailable,
            message: "catálogo indisponível",
        }
    }

    /// Local database unavailable.
    pub fn database_unavailable() -> Self {
        Self {
            code: IpcErrorCode::DatabaseUnavailable,
            message: "banco de dados local indisponível",
        }
    }

    /// The folder chosen for a course is invalid.
    pub fn course_path_invalid() -> Self {
        Self {
            code: IpcErrorCode::CoursePathInvalid,
            message: "a pasta escolhida não é um curso válido",
        }
    }

    /// The application's course configuration could not be written.
    pub fn courses_config_unavailable() -> Self {
        Self {
            code: IpcErrorCode::CoursesConfigUnavailable,
            message: "não foi possível salvar a configuração do aplicativo",
        }
    }

    /// No course is registered to index.
    pub fn no_courses_registered() -> Self {
        Self {
            code: IpcErrorCode::NoCoursesRegistered,
            message: "nenhum curso está importado",
        }
    }

    /// A rescan is already in progress.
    pub fn rescan_in_progress() -> Self {
        Self {
            code: IpcErrorCode::RescanInProgress,
            message: "já existe uma indexação em andamento",
        }
    }

    /// The library scan could not be completed.
    pub fn rescan_unavailable() -> Self {
        Self {
            code: IpcErrorCode::RescanUnavailable,
            message: "não foi possível indexar a biblioteca",
        }
    }

    /// The local owner's study progress could not be read or written.
    pub fn study_unavailable() -> Self {
        Self {
            code: IpcErrorCode::StudyUnavailable,
            message: "não foi possível sincronizar o progresso de estudo",
        }
    }

    /// The queried media does not exist in the local catalog.
    pub fn media_not_found() -> Self {
        Self {
            code: IpcErrorCode::MediaNotFound,
            message: "mídia não encontrada no catálogo",
        }
    }

    /// Opening the system player is unavailable in the Flatpak sandbox.
    pub fn system_player_unavailable() -> Self {
        Self {
            code: IpcErrorCode::SystemPlayerUnavailable,
            message: "Opening media in the system player is unavailable in Flatpak.",
        }
    }

    /// The resume position sent is invalid.
    pub fn invalid_position() -> Self {
        Self {
            code: IpcErrorCode::InvalidPosition,
            message: "posição de retomada inválida",
        }
    }

    /// Invalid Pomodoro session data.
    pub fn invalid_focus_session() -> Self {
        Self {
            code: IpcErrorCode::InvalidFocusSession,
            message: "sessão de foco inválida",
        }
    }

    /// The same session key was sent with different data.
    pub fn focus_session_conflict() -> Self {
        Self {
            code: IpcErrorCode::FocusSessionConflict,
            message: "sessão de foco já registrada com outros dados",
        }
    }

    /// The mission has not yet reached the required progress.
    pub fn mission_not_complete() -> Self {
        Self {
            code: IpcErrorCode::MissionNotComplete,
            message: "missão diária ainda não concluída",
        }
    }

    /// Weekly goal outside the accepted range.
    pub fn invalid_weekly_goal() -> Self {
        Self {
            code: IpcErrorCode::InvalidWeeklyGoal,
            message: "meta semanal inválida",
        }
    }

    /// The removal requires confirmation because there is associated progress.
    pub fn media_removal_needs_confirmation() -> Self {
        Self {
            code: IpcErrorCode::MediaRemovalNeedsConfirmation,
            message: "a remoção da mídia exige confirmação",
        }
    }

    /// The media removal could not be completed.
    pub fn media_removal_unavailable() -> Self {
        Self {
            code: IpcErrorCode::MediaRemovalUnavailable,
            message: "não foi possível remover a mídia",
        }
    }

    /// The course given is not registered in the application.
    pub fn course_not_found() -> Self {
        Self {
            code: IpcErrorCode::CourseNotFound,
            message: "curso não encontrado",
        }
    }

    /// The image chosen for the cover is not PNG, JPEG or WebP, or exceeds the limit.
    pub fn invalid_cover() -> Self {
        Self {
            code: IpcErrorCode::InvalidCover,
            message: "a capa precisa ser PNG, JPEG ou WebP de até 4 MiB",
        }
    }

    /// The course's cover could not be written or removed.
    pub fn cover_unavailable() -> Self {
        Self {
            code: IpcErrorCode::CoverUnavailable,
            message: "não foi possível atualizar a capa do curso",
        }
    }

    /// The order given is not a valid permutation of the siblings.
    pub fn invalid_order() -> Self {
        Self {
            code: IpcErrorCode::InvalidOrder,
            message: "a ordem informada não é válida",
        }
    }

    /// The course's manual order could not be written or removed.
    pub fn order_unavailable() -> Self {
        Self {
            code: IpcErrorCode::OrderUnavailable,
            message: "não foi possível salvar a ordem do curso",
        }
    }

    /// The update check failed because of a network error or an unreachable endpoint.
    pub fn update_check_failed() -> Self {
        Self {
            code: IpcErrorCode::UpdateCheckFailed,
            message: "não foi possível conectar ao servidor de atualizações",
        }
    }

    /// The update check or installation could not be completed.
    pub fn update_unavailable() -> Self {
        Self {
            code: IpcErrorCode::UpdateUnavailable,
            message: "não foi possível verificar ou aplicar a atualização",
        }
    }

    /// The requested note was not found or does not belong to the user.
    pub fn note_not_found() -> Self {
        Self {
            code: IpcErrorCode::NoteNotFound,
            message: "anotação não encontrada",
        }
    }

    /// The content of the note is invalid.
    pub fn invalid_note() -> Self {
        Self {
            code: IpcErrorCode::InvalidNote,
            message: "conteúdo da anotação inválido",
        }
    }

    /// The requested PDF annotation was not found or is not the user's.
    pub fn pdf_annotation_not_found() -> Self {
        Self {
            code: IpcErrorCode::PdfAnnotationNotFound,
            message: "anotação de PDF não encontrada",
        }
    }

    /// The PDF annotation is invalid.
    pub fn invalid_pdf_annotation() -> Self {
        Self {
            code: IpcErrorCode::InvalidPdfAnnotation,
            message: "anotação de PDF inválida",
        }
    }

    /// The requested memorization card was not found.
    pub fn flashcard_not_found() -> Self {
        Self {
            code: IpcErrorCode::FlashcardNotFound,
            message: "cartão não encontrado",
        }
    }

    /// The content of the memorization card is invalid.
    pub fn invalid_flashcard() -> Self {
        Self {
            code: IpcErrorCode::InvalidFlashcard,
            message: "conteúdo do cartão inválido",
        }
    }

    /// The rating of the card review is invalid.
    pub fn invalid_review_rating() -> Self {
        Self {
            code: IpcErrorCode::InvalidReviewRating,
            message: "avaliação inválida",
        }
    }

    /// The favourite target (course or media) is invalid.
    pub fn invalid_favorite_target() -> Self {
        Self {
            code: IpcErrorCode::InvalidFavoriteTarget,
            message: "alvo de favorito inválido",
        }
    }

    /// The requested time marker was not found.
    pub fn bookmark_not_found() -> Self {
        Self {
            code: IpcErrorCode::BookmarkNotFound,
            message: "marcador não encontrado",
        }
    }

    /// The marker's label is invalid.
    pub fn invalid_bookmark_label() -> Self {
        Self {
            code: IpcErrorCode::InvalidBookmarkLabel,
            message: "rótulo do marcador inválido",
        }
    }

    /// The playback preferences are outside the accepted range.
    pub fn invalid_player_preference() -> Self {
        Self {
            code: IpcErrorCode::InvalidPlayerPreference,
            message: "preferência de reprodução inválida",
        }
    }

    /// The local sync queue could not be read or written.
    pub fn sync_unavailable() -> Self {
        Self {
            code: IpcErrorCode::SyncUnavailable,
            message: "não foi possível sincronizar o estudo",
        }
    }

    /// The sync server's URL is invalid.
    pub fn invalid_sync_server() -> Self {
        Self {
            code: IpcErrorCode::InvalidSyncServer,
            message: "endereço do servidor inválido",
        }
    }

    /// The server refused the sync credentials.
    pub fn sync_login_failed() -> Self {
        Self {
            code: IpcErrorCode::SyncLoginFailed,
            message: "não foi possível entrar no servidor",
        }
    }
}
