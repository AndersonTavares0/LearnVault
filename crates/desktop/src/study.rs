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

use learnvault_core::Database;

use crate::dto::{RecentStudyDto, StudyProgressDto};
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
        _ => IpcError::study_unavailable(),
    }
}
