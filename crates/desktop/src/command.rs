//! Comandos IPC do desktop e as funções livres que eles embrulham.
//!
//! Cada wrapper `#[tauri::command]` é fino: delega para uma função livre que não conhece
//! o Tauri. Os testes exercitam as funções livres; o runtime do webview fica fora delas.

use learnvault_core::Database;
use tauri::State;

use crate::config::{DesktopConfigDto, config_status};
use crate::dto::CatalogDto;
use crate::error::IpcError;
use crate::state::DesktopState;

/// Identificador local da biblioteca, o mesmo usado pelo servidor.
///
/// O desktop lê a mesma tabela do domínio, então reusa a identidade para não criar um
/// catálogo paralelo quando a configuração da raiz chegar.
pub const LIBRARY_ID: &str = "library-1";

/// Constrói o DTO do catálogo a partir dos dados persistidos no banco local.
///
/// Função livre e testável: não conhece o Tauri e qualquer falha do núcleo é traduzida
/// para um erro IPC sanitizado, sem ecoar o erro de `Display` do domínio.
pub fn catalog(database: &Database) -> Result<CatalogDto, IpcError> {
    learnvault_core::build_catalog(database, LIBRARY_ID)
        .map(CatalogDto::from)
        .map_err(|_| IpcError::catalog_unavailable())
}

/// Comando IPC: devolve o catálogo do desktop.
#[tauri::command]
pub fn desktop_catalog(state: State<'_, DesktopState>) -> Result<CatalogDto, IpcError> {
    state.catalog()
}

/// Comando IPC: devolve a situação de configuração do desktop.
#[tauri::command]
pub fn desktop_config() -> DesktopConfigDto {
    config_status()
}
