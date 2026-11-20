# Migrating SQLite from Go to Rust

The Rust core can import a legacy Go database. The import runs only when requested, never at startup, and never modifies the Go database or the course library.

`learnvault_core::import_go_database(LegacyImportOptions)` takes `input`, `output`, `backup`, and `library_id`. A backup is created first; output is written to a temporary file and promoted only after foreign key checks, integrity checks, and row count validations succeed. Input, output, and backup paths must be distinct and non-overlapping.

The import covers users, sessions, catalog files, valid media removals, and study progress. Paths are normalized to `/`, and Rust-format user and media IDs are generated. The Rust schema requires a media record for each tombstone, so an orphan tombstone fails the import. Key collisions, unrecognized roles and invalid timestamps also stop it before any output is promoted.

Run the migration via the server CLI:

```text
learnvault-server migrate-go --input <go.sqlite> --output <rust.sqlite> --backup <go.sqlite.backup> --library-id <id>
```

The command does not start the HTTP listener and needs no library, network, or admin configuration. Run it with the Go service stopped so the database snapshot is stable. Keep the backup and open the output with `Database::open` before deploying.

## Self-Hosted Cutover Gate

Before migrating, check whether `${DATA_DIR}` holds a legacy Go v8 database or a Rust database. The formats are incompatible and cannot share a volume.

1. Keep the legacy Go volume and stop the Go service. Confirm it is idle before building or starting the new service.
2. Run `learnvault-server migrate-go` with `--input`, `--backup`, `--output`, and `--library-id`, writing the output to a separate Rust volume.
3. Check that the input checksum is unchanged and that the output exists and passes integrity checks.
4. Keep backups of both volumes. To roll back, stop the Rust server and restore the original Go volume.
