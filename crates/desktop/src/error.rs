//! Erro IPC do desktop: serializável, estável e sanitizado.
//!
//! Cada falha vira um objeto `{ code, message }`. O `code` é o contrato do frontend e não
//! muda sem versionamento; a `message` é curta e nunca contém caminhos do sistema de
//! arquivos nem segredos. A mensagem de diagnóstico do núcleo (`Display`) é local e não é
//! enviada ao webview.

use serde::Serialize;

/// Código estável de erro IPC consumido pelo frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IpcErrorCode {
    /// O catálogo não pôde ser derivado dos dados locais.
    CatalogUnavailable,
    /// O banco de dados local não pôde ser aberto ou lido.
    DatabaseUnavailable,
    /// A pasta escolhida para um curso é inválida.
    CoursePathInvalid,
    /// A configuração de cursos do aplicativo não pôde ser gravada.
    CoursesConfigUnavailable,
    /// Nenhum curso está registrado para indexar.
    NoCoursesRegistered,
    /// Já existe um rescan em andamento.
    RescanInProgress,
    /// A varredura da biblioteca não pôde ser concluída.
    RescanUnavailable,
    /// O progresso de estudo do dono local não pôde ser lido nem gravado.
    StudyUnavailable,
    /// A mídia consultada não existe no catálogo local.
    MediaNotFound,
    /// A posição de retomada enviada é inválida.
    InvalidPosition,
    /// A duração solicitada para o Pomodoro está fora do intervalo permitido.
    InvalidPomodoroDuration,
    /// A remoção exige confirmação porque existe progresso associado.
    MediaRemovalNeedsConfirmation,
    /// A remoção de mídia não pôde ser concluída.
    MediaRemovalUnavailable,
    /// O curso informado não está registrado no aplicativo.
    CourseNotFound,
    /// A imagem escolhida para a capa não é PNG, JPEG ou WebP, ou excede o limite.
    InvalidCover,
    /// A capa do curso não pôde ser gravada nem removida.
    CoverUnavailable,
    /// A ordem informada não é uma permutação válida dos irmãos.
    InvalidOrder,
    /// A ordem manual do curso não pôde ser gravada nem removida.
    OrderUnavailable,
}

/// Erro IPC serializável devolvido ao webview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IpcError {
    /// Código estável consumido pelo frontend.
    pub code: IpcErrorCode,
    /// Mensagem curta, sem caminhos nem segredos.
    pub message: &'static str,
}

impl IpcError {
    /// Catálogo indisponível: a derivação falhou sobre os dados locais.
    pub fn catalog_unavailable() -> Self {
        Self {
            code: IpcErrorCode::CatalogUnavailable,
            message: "catálogo indisponível",
        }
    }

    /// Banco de dados local indisponível.
    pub fn database_unavailable() -> Self {
        Self {
            code: IpcErrorCode::DatabaseUnavailable,
            message: "banco de dados local indisponível",
        }
    }

    /// A pasta escolhida para um curso é inválida.
    pub fn course_path_invalid() -> Self {
        Self {
            code: IpcErrorCode::CoursePathInvalid,
            message: "a pasta escolhida não é um curso válido",
        }
    }

    /// A configuração de cursos do aplicativo não pôde ser gravada.
    pub fn courses_config_unavailable() -> Self {
        Self {
            code: IpcErrorCode::CoursesConfigUnavailable,
            message: "não foi possível salvar a configuração do aplicativo",
        }
    }

    /// Nenhum curso está registrado para indexar.
    pub fn no_courses_registered() -> Self {
        Self {
            code: IpcErrorCode::NoCoursesRegistered,
            message: "nenhum curso está importado",
        }
    }

    /// Já existe um rescan em andamento.
    pub fn rescan_in_progress() -> Self {
        Self {
            code: IpcErrorCode::RescanInProgress,
            message: "já existe uma indexação em andamento",
        }
    }

    /// A varredura da biblioteca não pôde ser concluída.
    pub fn rescan_unavailable() -> Self {
        Self {
            code: IpcErrorCode::RescanUnavailable,
            message: "não foi possível indexar a biblioteca",
        }
    }

    /// O progresso de estudo do dono local não pôde ser lido nem gravado.
    pub fn study_unavailable() -> Self {
        Self {
            code: IpcErrorCode::StudyUnavailable,
            message: "não foi possível sincronizar o progresso de estudo",
        }
    }

    /// A mídia consultada não existe no catálogo local.
    pub fn media_not_found() -> Self {
        Self {
            code: IpcErrorCode::MediaNotFound,
            message: "mídia não encontrada no catálogo",
        }
    }

    /// A posição de retomada enviada é inválida.
    pub fn invalid_position() -> Self {
        Self {
            code: IpcErrorCode::InvalidPosition,
            message: "posição de retomada inválida",
        }
    }

    /// Erro estável para uma duração de Pomodoro fora do intervalo aceito.
    pub fn invalid_pomodoro_duration() -> Self {
        Self {
            code: IpcErrorCode::InvalidPomodoroDuration,
            message: "duração do Pomodoro inválida",
        }
    }

    /// A remoção exige confirmação porque existe progresso associado.
    pub fn media_removal_needs_confirmation() -> Self {
        Self {
            code: IpcErrorCode::MediaRemovalNeedsConfirmation,
            message: "a remoção da mídia exige confirmação",
        }
    }

    /// A remoção de mídia não pôde ser concluída.
    pub fn media_removal_unavailable() -> Self {
        Self {
            code: IpcErrorCode::MediaRemovalUnavailable,
            message: "não foi possível remover a mídia",
        }
    }

    /// O curso informado não está registrado no aplicativo.
    pub fn course_not_found() -> Self {
        Self {
            code: IpcErrorCode::CourseNotFound,
            message: "curso não encontrado",
        }
    }

    /// A imagem escolhida para a capa não é PNG, JPEG ou WebP, ou excede o limite.
    pub fn invalid_cover() -> Self {
        Self {
            code: IpcErrorCode::InvalidCover,
            message: "a capa precisa ser PNG, JPEG ou WebP de até 4 MiB",
        }
    }

    /// A capa do curso não pôde ser gravada nem removida.
    pub fn cover_unavailable() -> Self {
        Self {
            code: IpcErrorCode::CoverUnavailable,
            message: "não foi possível atualizar a capa do curso",
        }
    }

    /// A ordem informada não é uma permutação válida dos irmãos.
    pub fn invalid_order() -> Self {
        Self {
            code: IpcErrorCode::InvalidOrder,
            message: "a ordem informada não é válida",
        }
    }

    /// A ordem manual do curso não pôde ser gravada nem removida.
    pub fn order_unavailable() -> Self {
        Self {
            code: IpcErrorCode::OrderUnavailable,
            message: "não foi possível salvar a ordem do curso",
        }
    }
}
