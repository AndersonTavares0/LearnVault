//! Ordenação manual de módulos e aulas no desktop (issue #239).
//!
//! Como no servidor, a ordem escolhida pelo usuário é um estado do LearnVault: ela vive no
//! arquivo versionado `catalog-order.json`, dentro do diretório de dados do aplicativo (ao
//! lado do banco e das capas), nunca dentro das pastas de curso, que permanecem externas e
//! somente leitura.
//!
//! A preferência é aplicada por sobreposição em [`learnvault_core::apply_order`] sobre a
//! ordem automática, derivada a cada leitura; remover a preferência restaura a ordem
//! automática sem tocar em nenhum arquivo do curso.

use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use learnvault_core::CatalogOrder;

/// Nome do arquivo de ordenação dentro do diretório de dados do aplicativo.
pub const ORDER_FILE_NAME: &str = "catalog-order.json";

/// Versão do formato do arquivo de ordenação.
pub const ORDER_VERSION: u32 = 1;

/// Preferências de ordenação de todos os cursos, por identidade opaca do curso.
pub type CourseOrders = BTreeMap<String, CatalogOrder>;

/// Falha estável da persistência da ordenação. Nunca carrega caminho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderError {
    /// A ordenação não pôde ser gravada com segurança.
    WriteFailed,
}

/// Formato serializado do arquivo de ordenação.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct OrderFile {
    version: u32,
    /// Curso -> (caminho do pai -> ordem desejada dos filhos).
    courses: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

/// Caminho do arquivo de ordenação dentro do diretório de dados.
pub fn order_path(data_dir: &Path) -> PathBuf {
    data_dir.join(ORDER_FILE_NAME)
}

/// Carrega as preferências persistidas.
///
/// Arquivo ausente, corrompido ou em versão desconhecida resulta em nenhuma preferência
/// (ordem automática); nenhuma mensagem carrega caminho.
pub fn load_orders(data_dir: &Path) -> CourseOrders {
    let Ok(raw) = std::fs::read(order_path(data_dir)) else {
        return CourseOrders::new();
    };
    let Ok(stored) = serde_json::from_slice::<OrderFile>(&raw) else {
        return CourseOrders::new();
    };
    if stored.version != ORDER_VERSION {
        return CourseOrders::new();
    }
    stored
        .courses
        .into_iter()
        .map(|(course_id, parents)| (course_id, CatalogOrder::from_map(parents)))
        .filter(|(_, order)| !order.is_empty())
        .collect()
}

/// Grava as preferências de forma serializada e atômica.
pub fn store_orders(data_dir: &Path, orders: &CourseOrders) -> Result<(), OrderError> {
    let stored = OrderFile {
        version: ORDER_VERSION,
        courses: orders
            .iter()
            .filter(|(_, order)| !order.is_empty())
            .map(|(course_id, order)| (course_id.clone(), order.as_map().clone()))
            .collect(),
    };
    let payload = serde_json::to_vec(&stored).map_err(|_| OrderError::WriteFailed)?;
    write_atomically(&order_path(data_dir), &payload)
}

/// Escreve `bytes` no destino de forma atômica, criando o diretório de dados se preciso.
fn write_atomically(dest: &Path, bytes: &[u8]) -> Result<(), OrderError> {
    let directory = dest.parent().ok_or(OrderError::WriteFailed)?;
    std::fs::create_dir_all(directory).map_err(|_| OrderError::WriteFailed)?;

    let temporary = directory.join(format!(".{ORDER_FILE_NAME}.{}.tmp", std::process::id()));
    let write_result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if write_result.is_err() {
        let _ = std::fs::remove_file(&temporary);
        return Err(OrderError::WriteFailed);
    }
    if replace_file(&temporary, dest).is_err() {
        let _ = std::fs::remove_file(&temporary);
        return Err(OrderError::WriteFailed);
    }
    Ok(())
}

/// Substitui o destino usando apenas APIs padrão.
#[cfg(windows)]
fn replace_file(source: &Path, dest: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(dest) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    std::fs::rename(source, dest)
}

/// Substitui o destino usando apenas APIs padrão.
#[cfg(not(windows))]
fn replace_file(source: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::rename(source, dest)
}
