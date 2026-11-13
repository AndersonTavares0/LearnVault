//! Operações de backup e restauração do banco SQLite do LearnVault.

use std::{
    path::PathBuf,
    pin::Pin,
    task::{Context, Poll},
};

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use tokio::io::{AsyncRead, ReadBuf};

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

/// Arquivo temporário apagado quando a resposta termina, mesmo se o cliente desistir.
struct CleanupFile {
    file: tokio::fs::File,
    path: PathBuf,
}

impl AsyncRead for CleanupFile {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.file).poll_read(context, buffer)
    }
}

impl Drop for CleanupFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
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
    let base_dir = state
        .config
        .database_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(std::env::temp_dir);
    let backup_temp_file = base_dir.join(format!(
        ".learnvault-backup-{}-{}-{}.sqlite",
        std::process::id(),
        timestamp,
        getrandom::u32().unwrap_or(0)
    ));

    // The export runs on its own connection: the shared one stays free for requests, and
    // SQLite's backup API gives a consistent snapshot without holding our mutex.
    let database_path = state.config.database_path.clone();
    let destination = backup_temp_file.clone();
    let exported = tokio::task::spawn_blocking(move || {
        let db = learnvault_core::Database::open(&database_path)?;
        db.backup_to(&destination)
    })
    .await;
    match exported {
        Ok(Ok(())) => {}
        Ok(Err(_)) | Err(_) => {
            let _ = std::fs::remove_file(&backup_temp_file);
            return (StatusCode::INTERNAL_SERVER_ERROR, "backup unavailable").into_response();
        }
    }

    let file = match tokio::fs::File::open(&backup_temp_file).await {
        Ok(file) => file,
        Err(_) => {
            let _ = std::fs::remove_file(&backup_temp_file);
            return (StatusCode::INTERNAL_SERVER_ERROR, "backup unavailable").into_response();
        }
    };

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let filename = format!("learnvault-backup-{timestamp}.sqlite");

    let stream = tokio_util::io::ReaderStream::new(CleanupFile {
        file,
        path: backup_temp_file,
    });
    let mut response = (StatusCode::OK, Body::from_stream(stream)).into_response();
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
