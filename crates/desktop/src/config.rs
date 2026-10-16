//! Configuração de caminhos do desktop.
//!
//! A situação de configuração exposta ao webview é apenas um booleano: o desktop sabe se
//! uma raiz de biblioteca foi escolhida, mas nunca entrega o caminho. A raiz em si é
//! selecionada e persistida por [`crate::library`], fora da biblioteca e fora do banco.
//!
//! O único caminho resolvido aqui é o do banco local, que vive no diretório de dados do
//! próprio aplicativo — nunca na biblioteca, que permanece externa e somente leitura.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Nome do arquivo do banco local dentro do diretório de dados.
pub const DATABASE_FILE_NAME: &str = "learnvault.sqlite";

/// Situação de configuração do desktop exposta ao webview.
///
/// O shape é deliberadamente mínimo: o webview aprende apenas se há uma biblioteca
/// configurada, nunca onde ela está.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DesktopConfigDto {
    /// Verdadeiro somente quando existe uma raiz de biblioteca configurada e válida.
    pub configured: bool,
}

/// Caminho do banco local dentro do diretório de dados.
pub fn database_path(data_dir: &Path) -> PathBuf {
    data_dir.join(DATABASE_FILE_NAME)
}
