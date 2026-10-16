//! Configuração de caminhos do desktop.
//!
//! Nesta fatia não existe fonte canônica de configuração de biblioteca: escolher e
//! persistir a raiz da biblioteca exige uma decisão de produto (por exemplo um seletor de
//! diretórios) que permanece fora do escopo. Por isso o comando `desktop_config` responde
//! de forma determinística que o desktop ainda não está configurado, sem inventar um
//! setter nem um caminho no frontend.
//!
//! O único caminho resolvido aqui é o do banco local, que vive no diretório de dados do
//! próprio aplicativo — nunca na biblioteca, que permanece externa e somente leitura.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Nome do arquivo do banco local dentro do diretório de dados.
pub const DATABASE_FILE_NAME: &str = "learnvault.sqlite";

/// Situação de configuração do desktop exposta ao webview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DesktopConfigDto {
    /// Verdadeiro somente quando existe uma raiz de biblioteca configurada.
    pub configured: bool,
}

/// Situação de configuração atual.
///
/// Sem uma fonte canônica de configuração, o desktop está sempre não configurado nesta
/// fatia. Quando a configuração existir (fatia posterior), esta função passará a refletir
/// a raiz resolvida.
pub fn config_status() -> DesktopConfigDto {
    DesktopConfigDto { configured: false }
}

/// Caminho do banco local dentro do diretório de dados.
pub fn database_path(data_dir: &Path) -> PathBuf {
    data_dir.join(DATABASE_FILE_NAME)
}
