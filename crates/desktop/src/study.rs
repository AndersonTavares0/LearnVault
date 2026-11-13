//! Progresso de estudo do desktop sobre o IPC.
//!
//! Este adaptador não reimplementa nenhuma regra: cada operação delega a
//! [`learnvault_core::study`] e apenas fixa o dono. O `owner_id` do desktop é um
//! identificador local fixo ([`LOCAL_OWNER_ID`]) resolvido pelo Rust; ele nunca é recebido
//! do webview, então o contrato de comandos não oferece — nem aceita — um `owner_id` como
//! argumento, e o webview não pode ler nem gravar o progresso de outro dono.
//!
//! Como o adaptador HTTP, o erro do núcleo é traduzido para um código estável e o
//! `Display` do domínio não é ecoado: o identificador de mídia consultado nunca aparece na
//! resposta enviada ao webview.
//!
//! Limitação conhecida desta fatia: o desktop atende um único dono local. Vários donos
//! locais e a seleção entre eles pertencem a uma fatia posterior — o núcleo já isola por
//! dono, mas aqui não existe sessão nem seletor local.

use learnvault_core::{Database, FocusSessionInput, ReviewRating};

use crate::dto::{
    FlashcardDto, FlashcardReviewDto, FlashcardReviewsDto, FocusSessionDto, FocusSummaryDto,
    MediaBookmarkDto, NewPdfAnnotationDto, PdfAnnotationDto, PlaybackPreferencesDto,
    RecentStudyDto, RewardSummaryDto, StudyFavoritesDto, StudyNoteDto, StudyProgressDto,
    StudySummaryDto,
};
use crate::error::IpcError;

/// Dono local fixo do progresso de estudo do desktop.
///
/// O núcleo trata `owner_id` como opaco, canônico e global: o desktop escolhe um valor
/// estável, em vez de derivá-lo de algo que possa variar entre execuções, para que o
/// progresso sobreviva a reinicializações. O valor não é um identificador de usuário do
/// servidor e não é aceito de fora do Rust.
pub const LOCAL_OWNER_ID: &str = "desktop-local";

/// Limite padrão do histórico recente quando o webview não informa um.
///
/// Acompanha o padrão do adaptador HTTP; o teto real continua sendo
/// [`learnvault_core::MAX_RECENT_STUDY`], aplicado pelo núcleo, e não uma configuração.
pub const DEFAULT_RECENT_LIMIT: usize = 20;

/// Limite padrão do histórico de revisões quando o webview não informa um.
///
/// O teto real continua sendo [`learnvault_core::MAX_FLASHCARD_REVIEWS`], aplicado pelo núcleo.
pub const DEFAULT_REVIEW_LIMIT: usize = 20;

/// Lê o progresso do dono local em uma mídia conhecida.
///
/// Ausência de progresso não é erro: uma mídia conhecida sem estado devolve o registro
/// neutro com `recorded: false`. Uma mídia desconhecida no catálogo vira
/// [`IpcError::media_not_found`].
pub fn progress(database: &Database, media_id: &str) -> Result<StudyProgressDto, IpcError> {
    match learnvault_core::study_progress(database, LOCAL_OWNER_ID, media_id) {
        Ok(Some(found)) => Ok(StudyProgressDto::from(found)),
        Ok(None) => Ok(StudyProgressDto::absent(media_id)),
        Err(error) => Err(study_error(error)),
    }
}

/// Registra a posição de retomada do dono local, preservando a conclusão registrada.
pub fn record_position(
    database: &Database,
    media_id: &str,
    position_seconds: i64,
) -> Result<StudyProgressDto, IpcError> {
    let device = crate::sync::device(database)?;
    learnvault_core::queue_position(
        database,
        LOCAL_OWNER_ID,
        &device,
        media_id,
        position_seconds,
    )
    .map(StudyProgressDto::from)
    .map_err(study_error)
}

/// Marca ou desmarca a conclusão do dono local, preservando a posição de retomada.
pub fn set_completed(
    database: &Database,
    media_id: &str,
    completed: bool,
) -> Result<StudyProgressDto, IpcError> {
    let device = crate::sync::device(database)?;
    learnvault_core::queue_completed(database, LOCAL_OWNER_ID, &device, media_id, completed)
        .map(StudyProgressDto::from)
        .map_err(study_error)
}

/// Lista o progresso do dono local, do alterado mais recentemente ao mais antigo.
///
/// Sem limite informado, aplica [`DEFAULT_RECENT_LIMIT`]; o teto do núcleo continua valendo
/// para qualquer valor recebido.
pub fn recent(database: &Database, limit: Option<usize>) -> Result<RecentStudyDto, IpcError> {
    learnvault_core::recent_study(
        database,
        LOCAL_OWNER_ID,
        limit.unwrap_or(DEFAULT_RECENT_LIMIT),
    )
    .map(|states| RecentStudyDto {
        items: states.into_iter().map(StudyProgressDto::from).collect(),
    })
    .map_err(study_error)
}

/// Resumo de ofensiva e meta semanal do dono local.
pub fn summary(database: &Database) -> Result<StudySummaryDto, IpcError> {
    learnvault_core::study_summary(database, LOCAL_OWNER_ID)
        .map(StudySummaryDto::from)
        .map_err(study_error)
}

/// Grava sessão do timer de forma idempotente para o dono local.
pub fn record_focus_session(
    database: &Database,
    input: FocusSessionInput,
) -> Result<FocusSessionDto, IpcError> {
    learnvault_core::record_focus_session(database, LOCAL_OWNER_ID, input)
        .map(FocusSessionDto::from)
        .map_err(focus_error)
}

/// Lista sessões de foco persistidas do dono local, da mais recente para a mais antiga.
pub fn list_focus_sessions(
    database: &Database,
    limit: Option<usize>,
) -> Result<Vec<FocusSessionDto>, IpcError> {
    learnvault_core::list_focus_sessions(database, LOCAL_OWNER_ID, limit.unwrap_or(100))
        .map(|sessions| sessions.into_iter().map(FocusSessionDto::from).collect())
        .map_err(focus_error)
}

/// Lê estatísticas persistidas do timer do dono local.
pub fn focus_summary(database: &Database) -> Result<FocusSummaryDto, IpcError> {
    learnvault_core::focus_summary(database, LOCAL_OWNER_ID)
        .map(FocusSummaryDto::from)
        .map_err(focus_error)
}

/// Lê XP e progresso de missões persistidos do dono local.
pub fn reward_summary(database: &Database) -> Result<RewardSummaryDto, IpcError> {
    learnvault_core::reward_summary(database, LOCAL_OWNER_ID)
        .map(RewardSummaryDto::from)
        .map_err(focus_error)
}

/// Resgata uma missão diária para o dono local.
pub fn claim_daily_mission(database: &Database, mission_id: &str) -> Result<bool, IpcError> {
    learnvault_core::claim_daily_mission_today(database, LOCAL_OWNER_ID, mission_id)
        .map_err(focus_error)
}

/// Lê a meta semanal persistida do dono local.
pub fn weekly_goal(database: &Database) -> Result<i64, IpcError> {
    learnvault_core::weekly_goal(database, LOCAL_OWNER_ID).map_err(focus_error)
}

/// Atualiza a meta semanal persistida do dono local.
pub fn set_weekly_goal(database: &Database, goal: i64) -> Result<i64, IpcError> {
    learnvault_core::set_weekly_goal(database, LOCAL_OWNER_ID, goal).map_err(focus_error)
}

/// Lista anotações do dono local. Quando `media_id` é fornecido, filtra pela mídia;
/// caso contrário, devolve todas as anotações do usuário com caminhos relativos para busca global.
pub fn list_notes(
    database: &Database,
    media_id: Option<&str>,
) -> Result<Vec<StudyNoteDto>, IpcError> {
    if let Some(id) = media_id.filter(|s| !s.trim().is_empty()) {
        learnvault_core::list_study_notes(database, LOCAL_OWNER_ID, id)
            .map(|notes| notes.into_iter().map(StudyNoteDto::from).collect())
            .map_err(study_error)
    } else {
        learnvault_core::list_all_study_notes(database, LOCAL_OWNER_ID)
            .map(|notes| notes.into_iter().map(StudyNoteDto::from).collect())
            .map_err(study_error)
    }
}

/// Cria uma anotação de estudo vinculada a uma mídia e timestamp para o dono local.
pub fn create_note(
    database: &Database,
    media_id: &str,
    position_seconds: i64,
    content: &str,
) -> Result<StudyNoteDto, IpcError> {
    let device = crate::sync::device(database)?;
    learnvault_core::queue_note_create(
        database,
        LOCAL_OWNER_ID,
        &device,
        media_id,
        position_seconds,
        content,
    )
    .map(StudyNoteDto::from)
    .map_err(study_error)
}

/// Atualiza o texto de uma anotação pertencente ao dono local.
pub fn update_note(
    database: &Database,
    note_id: &str,
    content: &str,
) -> Result<StudyNoteDto, IpcError> {
    let device = crate::sync::device(database)?;
    learnvault_core::queue_note_update(database, LOCAL_OWNER_ID, &device, note_id, content)
        .map(StudyNoteDto::from)
        .map_err(study_error)
}

/// Exclui uma anotação de estudo do dono local. Devolve erro se a anotação não existir.
///
/// The deletion enters the outbox in the same transaction as the local delete, so another
/// device stops showing the note instead of pulling it back.
pub fn delete_note(database: &Database, note_id: &str) -> Result<(), IpcError> {
    let device = crate::sync::device(database)?;
    let deleted = learnvault_core::queue_note_delete(database, LOCAL_OWNER_ID, &device, note_id)
        .map_err(study_error)?;
    if deleted {
        Ok(())
    } else {
        Err(IpcError::note_not_found())
    }
}

/// Lists the local owner's annotations on one PDF.
pub fn list_pdf_annotations(
    database: &Database,
    media_id: &str,
) -> Result<Vec<PdfAnnotationDto>, IpcError> {
    learnvault_core::list_pdf_annotations(database, LOCAL_OWNER_ID, media_id)
        .map(|items| items.into_iter().map(PdfAnnotationDto::from).collect())
        .map_err(study_error)
}

/// Creates an annotation. An unknown kind or color is refused like any other bad input.
pub fn create_pdf_annotation(
    database: &Database,
    media_id: &str,
    input: NewPdfAnnotationDto,
) -> Result<PdfAnnotationDto, IpcError> {
    let (Some(kind), Some(color)) = (
        learnvault_core::PdfAnnotationKind::parse(&input.kind),
        learnvault_core::PdfAnnotationColor::parse(&input.color),
    ) else {
        return Err(IpcError::invalid_pdf_annotation());
    };
    learnvault_core::create_pdf_annotation(
        database,
        LOCAL_OWNER_ID,
        media_id,
        learnvault_core::NewPdfAnnotation {
            page: input.page,
            kind,
            color,
            rects: input
                .rects
                .into_iter()
                .map(|rect| learnvault_core::PdfRect {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                })
                .collect(),
            quote: input.quote,
            content: input.content,
        },
    )
    .map(PdfAnnotationDto::from)
    .map_err(study_error)
}

/// Changes an annotation's color and/or text.
pub fn update_pdf_annotation(
    database: &Database,
    annotation_id: &str,
    color: Option<&str>,
    content: Option<&str>,
) -> Result<PdfAnnotationDto, IpcError> {
    let color = match color {
        Some(value) => Some(
            learnvault_core::PdfAnnotationColor::parse(value)
                .ok_or_else(IpcError::invalid_pdf_annotation)?,
        ),
        None => None,
    };
    learnvault_core::update_pdf_annotation(database, LOCAL_OWNER_ID, annotation_id, color, content)
        .map(PdfAnnotationDto::from)
        .map_err(study_error)
}

/// Deletes an annotation; an unknown id is an error, like for notes.
pub fn delete_pdf_annotation(database: &Database, annotation_id: &str) -> Result<(), IpcError> {
    let deleted = learnvault_core::delete_pdf_annotation(database, LOCAL_OWNER_ID, annotation_id)
        .map_err(study_error)?;
    if deleted {
        Ok(())
    } else {
        Err(IpcError::pdf_annotation_not_found())
    }
}

/// Lista os cartões de memorização do dono local para uma mídia específica.
pub fn list_flashcards(database: &Database, media_id: &str) -> Result<Vec<FlashcardDto>, IpcError> {
    learnvault_core::list_flashcards(database, LOCAL_OWNER_ID, media_id)
        .map(|cards| cards.into_iter().map(FlashcardDto::from).collect())
        .map_err(study_error)
}

/// Lists every card of the local owner, soonest due first, for the review hub.
pub fn list_all_flashcards(database: &Database) -> Result<Vec<FlashcardDto>, IpcError> {
    learnvault_core::list_all_flashcards(database, LOCAL_OWNER_ID)
        .map(|cards| cards.into_iter().map(FlashcardDto::from).collect())
        .map_err(study_error)
}

/// Cria um novo cartão de memorização para o dono local.
pub fn create_flashcard(
    database: &Database,
    media_id: &str,
    front: &str,
    back: &str,
    position_seconds: Option<i64>,
) -> Result<FlashcardDto, IpcError> {
    learnvault_core::create_flashcard(
        database,
        LOCAL_OWNER_ID,
        media_id,
        front,
        back,
        position_seconds,
    )
    .map(FlashcardDto::from)
    .map_err(study_error)
}

/// Atualiza a frente e o verso de um cartão de memorização do dono local.
pub fn update_flashcard(
    database: &Database,
    card_id: &str,
    front: &str,
    back: &str,
) -> Result<FlashcardDto, IpcError> {
    learnvault_core::update_flashcard(database, LOCAL_OWNER_ID, card_id, front, back)
        .map(FlashcardDto::from)
        .map_err(study_error)
}

/// Exclui um cartão de memorização pertencente ao dono local.
pub fn delete_flashcard(database: &Database, card_id: &str) -> Result<(), IpcError> {
    let deleted = learnvault_core::delete_flashcard(database, LOCAL_OWNER_ID, card_id)
        .map_err(study_error)?;
    if deleted {
        Ok(())
    } else {
        Err(IpcError::flashcard_not_found())
    }
}

/// Lista o histórico recente de um cartão do dono local, da revisão mais nova para a mais
/// antiga, junto da contagem completa.
pub fn list_flashcard_reviews(
    database: &Database,
    card_id: &str,
    limit: Option<usize>,
) -> Result<FlashcardReviewsDto, IpcError> {
    let limit = limit.unwrap_or(DEFAULT_REVIEW_LIMIT);
    let items = learnvault_core::list_flashcard_reviews(database, LOCAL_OWNER_ID, card_id, limit)
        .map_err(study_error)?;
    let total = learnvault_core::count_flashcard_reviews(database, LOCAL_OWNER_ID, card_id)
        .map_err(study_error)?;
    Ok(FlashcardReviewsDto {
        items: items.into_iter().map(FlashcardReviewDto::from).collect(),
        total,
    })
}

/// Conta os cartões do dono local vencidos agora.
pub fn count_due_flashcards(database: &Database) -> Result<i64, IpcError> {
    learnvault_core::count_due_flashcards(database, LOCAL_OWNER_ID).map_err(study_error)
}

/// Registra uma revisão ("again" ou "good") em um cartão de memorização do dono local.
pub fn review_flashcard(
    database: &Database,
    card_id: &str,
    rating: &str,
) -> Result<FlashcardDto, IpcError> {
    let Some(parsed) = ReviewRating::parse(rating) else {
        return Err(IpcError::invalid_review_rating());
    };
    let device = crate::sync::device(database)?;
    learnvault_core::queue_review(database, LOCAL_OWNER_ID, &device, card_id, parsed)
        .map(FlashcardDto::from)
        .map_err(study_error)
}

/// Lista os cursos e aulas favoritados pelo dono local.
pub fn list_favorites(database: &Database) -> Result<StudyFavoritesDto, IpcError> {
    learnvault_core::list_favorites(database, LOCAL_OWNER_ID)
        .map(StudyFavoritesDto::from)
        .map_err(study_error)
}

/// Marca ou desmarca um curso como favorito para o dono local.
pub fn set_course_favorite(
    database: &Database,
    course_id: &str,
    favorite: bool,
) -> Result<bool, IpcError> {
    learnvault_core::set_course_favorite(database, LOCAL_OWNER_ID, course_id, favorite)
        .map_err(study_error)
}

/// Marca ou desmarca uma aula (mídia) como favorita para o dono local.
pub fn set_lesson_favorite(
    database: &Database,
    media_id: &str,
    favorite: bool,
) -> Result<bool, IpcError> {
    learnvault_core::set_lesson_favorite(database, LOCAL_OWNER_ID, media_id, favorite)
        .map_err(study_error)
}

/// Lists the local owner's timestamp bookmarks for one lesson.
pub fn list_bookmarks(
    database: &Database,
    media_id: &str,
) -> Result<Vec<MediaBookmarkDto>, IpcError> {
    learnvault_core::list_media_bookmarks(database, LOCAL_OWNER_ID, media_id)
        .map(|items| items.into_iter().map(MediaBookmarkDto::from).collect())
        .map_err(study_error)
}

/// Creates a timestamp bookmark for the local owner.
pub fn create_bookmark(
    database: &Database,
    media_id: &str,
    position_seconds: i64,
    label: &str,
) -> Result<MediaBookmarkDto, IpcError> {
    let device = crate::sync::device(database)?;
    learnvault_core::queue_bookmark_create(
        database,
        LOCAL_OWNER_ID,
        &device,
        media_id,
        position_seconds,
        label,
    )
    .map(MediaBookmarkDto::from)
    .map_err(study_error)
}

/// Deletes one of the local owner's timestamp bookmarks.
///
/// The deletion enters the outbox in the same transaction as the local delete, so another
/// device stops showing the bookmark instead of pulling it back.
pub fn delete_bookmark(database: &Database, bookmark_id: &str) -> Result<(), IpcError> {
    let device = crate::sync::device(database)?;
    let deleted =
        learnvault_core::queue_bookmark_delete(database, LOCAL_OWNER_ID, &device, bookmark_id)
            .map_err(study_error)?;
    if deleted {
        Ok(())
    } else {
        Err(IpcError::bookmark_not_found())
    }
}

/// Reads the local owner's playback preferences, falling back to the neutral defaults.
///
/// Preferences are part of the offline queue: every save is applied locally and enqueued
/// with its own `op_id`, so two devices converge by last write.
pub fn playback_preference(database: &Database) -> Result<PlaybackPreferencesDto, IpcError> {
    learnvault_core::player_preference(database, LOCAL_OWNER_ID)
        .map(PlaybackPreferencesDto::from)
        .map_err(study_error)
}

/// Saves every playback field for the local owner and enqueues the change.
pub fn save_playback_preference(
    database: &Database,
    video_speed: i64,
    audio_speed: i64,
    subtitle_language: &str,
) -> Result<PlaybackPreferencesDto, IpcError> {
    let device = crate::sync::device(database)?;
    learnvault_core::queue_preference(
        database,
        LOCAL_OWNER_ID,
        &device,
        video_speed,
        audio_speed,
        subtitle_language,
    )
    .map(PlaybackPreferencesDto::from)
    .map_err(study_error)
}

/// Traduz o erro do núcleo para o erro IPC estável, sem ecoar o identificador consultado.
///
/// Uma mídia desconhecida e uma posição negativa são falhas de entrada com código próprio,
/// como no adaptador HTTP. Qualquer outro erro é operacional e não pode virar "mídia
/// desconhecida", para não confundir ausência legítima com banco indisponível.
fn study_error(error: learnvault_core::Error) -> IpcError {
    use learnvault_core::Error;

    match error {
        Error::MediaNotFound { .. } => IpcError::media_not_found(),
        Error::InvalidStudyPosition { .. } => IpcError::invalid_position(),
        Error::StudyNoteNotFound { .. } => IpcError::note_not_found(),
        Error::InvalidStudyNoteContent => IpcError::invalid_note(),
        Error::PdfAnnotationNotFound { .. } => IpcError::pdf_annotation_not_found(),
        Error::InvalidPdfAnnotation => IpcError::invalid_pdf_annotation(),
        Error::FlashcardNotFound { .. } => IpcError::flashcard_not_found(),
        Error::InvalidFlashcardContent => IpcError::invalid_flashcard(),
        Error::InvalidFavoriteTarget => IpcError::invalid_favorite_target(),
        Error::BookmarkNotFound { .. } => IpcError::bookmark_not_found(),
        Error::InvalidBookmarkLabel => IpcError::invalid_bookmark_label(),
        Error::InvalidPlayerPreference => IpcError::invalid_player_preference(),
        _ => IpcError::study_unavailable(),
    }
}

fn focus_error(error: learnvault_core::Error) -> IpcError {
    use learnvault_core::Error;

    match error {
        Error::InvalidFocusSession { .. } => IpcError::invalid_focus_session(),
        Error::FocusSessionConflict => IpcError::focus_session_conflict(),
        Error::MissionNotComplete => IpcError::mission_not_complete(),
        Error::InvalidWeeklyGoal { .. } => IpcError::invalid_weekly_goal(),
        _ => IpcError::study_unavailable(),
    }
}
