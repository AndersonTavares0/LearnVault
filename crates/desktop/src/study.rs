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
    FlashcardDto, FocusSessionDto, FocusSummaryDto, RecentStudyDto, RewardSummaryDto,
    StudyFavoritesDto, StudyNoteDto, StudyProgressDto, StudySummaryDto,
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
    learnvault_core::record_position(database, LOCAL_OWNER_ID, media_id, position_seconds)
        .map(StudyProgressDto::from)
        .map_err(study_error)
}

/// Marca ou desmarca a conclusão do dono local, preservando a posição de retomada.
pub fn set_completed(
    database: &Database,
    media_id: &str,
    completed: bool,
) -> Result<StudyProgressDto, IpcError> {
    learnvault_core::set_completed(database, LOCAL_OWNER_ID, media_id, completed)
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
    learnvault_core::create_study_note(
        database,
        LOCAL_OWNER_ID,
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
    learnvault_core::update_study_note(database, LOCAL_OWNER_ID, note_id, content)
        .map(StudyNoteDto::from)
        .map_err(study_error)
}

/// Exclui uma anotação de estudo do dono local. Devolve erro se a anotação não existir.
pub fn delete_note(database: &Database, note_id: &str) -> Result<(), IpcError> {
    let deleted = learnvault_core::delete_study_note(database, LOCAL_OWNER_ID, note_id)
        .map_err(study_error)?;
    if deleted {
        Ok(())
    } else {
        Err(IpcError::note_not_found())
    }
}

/// Lista os cartões de memorização do dono local para uma mídia específica.
pub fn list_flashcards(database: &Database, media_id: &str) -> Result<Vec<FlashcardDto>, IpcError> {
    learnvault_core::list_flashcards(database, LOCAL_OWNER_ID, media_id)
        .map(|cards| cards.into_iter().map(FlashcardDto::from).collect())
        .map_err(study_error)
}

/// Cria um novo cartão de memorização para o dono local.
pub fn create_flashcard(
    database: &Database,
    media_id: &str,
    front: &str,
    back: &str,
) -> Result<FlashcardDto, IpcError> {
    learnvault_core::create_flashcard(database, LOCAL_OWNER_ID, media_id, front, back)
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

/// Registra uma revisão ("again" ou "good") em um cartão de memorização do dono local.
pub fn review_flashcard(
    database: &Database,
    card_id: &str,
    rating: &str,
) -> Result<FlashcardDto, IpcError> {
    let Some(parsed) = ReviewRating::parse(rating) else {
        return Err(IpcError::invalid_review_rating());
    };
    learnvault_core::review_flashcard(database, LOCAL_OWNER_ID, card_id, parsed)
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
        Error::FlashcardNotFound { .. } => IpcError::flashcard_not_found(),
        Error::InvalidFlashcardContent => IpcError::invalid_flashcard(),
        Error::InvalidFavoriteTarget => IpcError::invalid_favorite_target(),
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
