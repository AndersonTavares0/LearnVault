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
    /// A pasta escolhida como raiz da biblioteca é inválida.
    LibraryRootInvalid,
    /// A configuração do aplicativo não pôde ser gravada.
    LibraryConfigUnavailable,
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

    /// A pasta escolhida como raiz da biblioteca é inválida.
    pub fn library_root_invalid() -> Self {
        Self {
            code: IpcErrorCode::LibraryRootInvalid,
            message: "a pasta escolhida não é uma biblioteca válida",
        }
    }

    /// A configuração do aplicativo não pôde ser gravada.
    pub fn library_config_unavailable() -> Self {
        Self {
            code: IpcErrorCode::LibraryConfigUnavailable,
            message: "não foi possível salvar a configuração do aplicativo",
        }
    }
}
