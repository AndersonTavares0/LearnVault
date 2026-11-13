//! Pre-migration backup of the local SQLite database.
//!
//! The desktop opens its database with [`learnvault_core::Database::open`], which applies the
//! migration chain compiled into the binary. Before that first open on a new version, the file
//! is copied next to itself so a failed migration can be rolled back by hand. The copy is best
//! effort: startup never fails because of it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use learnvault_core::CURRENT_SCHEMA_VERSION;

/// Number of pre-migration backups kept next to the database; older ones are pruned.
pub const MAX_DATABASE_BACKUPS: usize = 3;

/// Copies the database before migrations when its schema is older than this binary's.
///
/// Returns the backup path when a copy was made, and `None` when the file is absent, already
/// current, newer than this binary (the core refuses it instead of migrating), or unreadable.
pub fn backup_before_migration(database: &Path) -> io::Result<Option<PathBuf>> {
    backup_if_older(database, CURRENT_SCHEMA_VERSION)
}

fn backup_if_older(database: &Path, target_version: usize) -> io::Result<Option<PathBuf>> {
    if !database.is_file() {
        return Ok(None);
    }
    let Ok(found) = learnvault_core::file_schema_version(database) else {
        // An unreadable file is no usable rollback point; opening it still reports the real error.
        return Ok(None);
    };
    if found >= target_version {
        return Ok(None);
    }

    // shortcut: only the database and its `-wal` are copied, never the `-shm` index; SQLite
    // rebuilds that index on open and the backup runs before this process opens the file.
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let name = database
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let directory = database.parent().unwrap_or_else(|| Path::new("."));
    // A suffix keeps two backups taken in the same second from overwriting each other.
    let mut target = directory.join(format!("{name}.backup-{found}-{timestamp}"));
    let mut suffix = 1;
    while target.exists() {
        target = directory.join(format!("{name}.backup-{found}-{timestamp}-{suffix}"));
        suffix += 1;
    }

    if let Err(error) = copy_database(database, &target) {
        remove_backup(&target);
        return Err(error);
    }
    prune_backups(database, MAX_DATABASE_BACKUPS)?;
    Ok(Some(target))
}

/// Copies the database plus its pending WAL pages, so the backup opens with the same contents.
fn copy_database(database: &Path, target: &Path) -> io::Result<()> {
    fs::copy(database, target)?;
    let source_wal = sidecar(database, "-wal");
    if source_wal.is_file() {
        fs::copy(&source_wal, sidecar(target, "-wal"))?;
    }
    Ok(())
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Removes one backup and the sidecars a read may have created next to it.
fn remove_backup(target: &Path) {
    for path in [
        target.to_path_buf(),
        sidecar(target, "-wal"),
        sidecar(target, "-shm"),
    ] {
        let _ = fs::remove_file(path);
    }
}

/// Keeps the newest `keep` backups of `database` and deletes the oldest ones.
fn prune_backups(database: &Path, keep: usize) -> io::Result<()> {
    let Some(name) = database.file_name() else {
        return Ok(());
    };
    let directory = database.parent().unwrap_or_else(|| Path::new("."));
    let prefix = format!("{}.backup-", name.to_string_lossy());
    let mut backups = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        // `-wal`/`-shm` belong to a backup; they are removed with it, never counted as one.
        if !path.is_file()
            || !file_name.starts_with(&prefix)
            || file_name.ends_with("-wal")
            || file_name.ends_with("-shm")
        {
            continue;
        }
        // Oldest first; the name breaks a tie for backups written in the same instant.
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(UNIX_EPOCH);
        backups.push((modified, path));
    }
    backups.sort();
    let remove = backups.len().saturating_sub(keep);
    for (_, path) in backups.into_iter().take(remove) {
        remove_backup(&path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use learnvault_core::Database;

    fn database(directory: &Path) -> PathBuf {
        let path = directory.join("learnvault.sqlite");
        Database::open(&path).expect("o banco de teste precisa abrir");
        path
    }

    fn backups(directory: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(directory)
            .expect("o diretório de teste precisa ser legível")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with("learnvault.sqlite.backup-"))
            .filter(|name| !name.ends_with("-wal") && !name.ends_with("-shm"))
            .collect();
        names.sort();
        names
    }

    #[test]
    fn copies_the_database_when_the_schema_is_older() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = database(directory.path());

        let backup = backup_if_older(&path, CURRENT_SCHEMA_VERSION + 1)
            .expect("o backup não pode falhar")
            .expect("um schema anterior precisa de backup");

        assert_eq!(backup.parent(), Some(directory.path()));
        assert_eq!(
            fs::read(&backup).expect("backup legível"),
            fs::read(&path).expect("banco legível")
        );
        assert_eq!(
            learnvault_core::file_schema_version(&backup).expect("versão do backup"),
            CURRENT_SCHEMA_VERSION
        );
        assert_eq!(backups(directory.path()).len(), 1);
    }

    #[test]
    fn skips_the_copy_when_the_schema_is_current_or_missing() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = database(directory.path());

        assert!(
            backup_if_older(&path, CURRENT_SCHEMA_VERSION)
                .expect("a decisão não pode falhar")
                .is_none()
        );
        assert!(backups(directory.path()).is_empty());

        let missing = directory.path().join("absent.sqlite");
        assert!(
            backup_if_older(&missing, CURRENT_SCHEMA_VERSION)
                .expect("a decisão não pode falhar")
                .is_none()
        );
    }

    #[test]
    fn keeps_only_the_newest_three_backups() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = database(directory.path());
        for version in 0..5 {
            fs::copy(
                &path,
                directory
                    .path()
                    .join(format!("learnvault.sqlite.backup-{version}-{version}")),
            )
            .expect("backup sintético");
        }

        prune_backups(&path, MAX_DATABASE_BACKUPS).expect("a poda não pode falhar");

        assert_eq!(
            backups(directory.path()),
            vec![
                "learnvault.sqlite.backup-2-2".to_owned(),
                "learnvault.sqlite.backup-3-3".to_owned(),
                "learnvault.sqlite.backup-4-4".to_owned(),
            ]
        );
    }
}
