//! Operações de backup e restauração do banco SQLite do LearnVault.

use std::path::PathBuf;

use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};

use crate::{
    AppState,
    config::{BackupOptions, RestoreOptions},
    study::user_or_response,
};

/// Executa a criação de um backup consistente do banco via CLI.
pub fn execute_backup(options: &BackupOptions) -> Result<PathBuf, learnvault_core::Error> {
    let db = learnvault_core::Database::open(&options.database_path)?;
    db.backup_to(&options.output_path)?;
    Ok(options.output_path.clone())
}

/// Restaura o banco de dados a partir de um backup via CLI.
pub fn execute_restore(options: &RestoreOptions) -> Result<(), learnvault_core::Error> {
    learnvault_core::Database::restore_from(&options.backup_path, &options.database_path)
}

/// Handler HTTP autenticado (administrador) para download do backup transacional do banco.
pub(crate) async fn download_backup(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    if !user.is_admin {
        return (StatusCode::FORBIDDEN, "admin required").into_response();
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let backup_temp_file = std::env::temp_dir().join(format!(
        "learnvault-backup-{}-{}-{}.sqlite",
        std::process::id(),
        timestamp,
        getrandom::u32().unwrap_or(0)
    ));

    {
        let db = state.database.lock().await;
        if let Err(error) = db.backup_to(&backup_temp_file) {
            let _ = std::fs::remove_file(&backup_temp_file);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("backup failed: {error}"),
            )
                .into_response();
        }
    }

    let bytes = match std::fs::read(&backup_temp_file) {
        Ok(bytes) => {
            let _ = std::fs::remove_file(&backup_temp_file);
            bytes
        }
        Err(_) => {
            let _ = std::fs::remove_file(&backup_temp_file);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not read generated backup file",
            )
                .into_response();
        }
    };

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let filename = format!("learnvault-backup-{timestamp}.sqlite");

    let mut response = (StatusCode::OK, bytes).into_response();
    let headers_mut = response.headers_mut();
    headers_mut.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/x-sqlite3"),
    );
    headers_mut.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    if let Ok(disposition) = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
    {
        headers_mut.insert(header::CONTENT_DISPOSITION, disposition);
    }

    response
}
