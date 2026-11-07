# Migrating SQLite from Go to Rust

The Rust core provides an explicit, scoped database import tool. It never runs automatically on startup and never modifies the original Go database or physical course library.

`learnvault_core::import_go_database(LegacyImportOptions)` takes `input`, `output`, `backup`, and `library_id`. A backup is created first; output is written to a temporary file and promoted only after foreign key checks, integrity checks, and row count validations succeed. Input, output, and backup paths must be distinct and non-overlapping.

Users, sessions, catalog files, valid media removals, and study progress are imported. Paths are normalized to `/`, and Rust-format user and media IDs are generated. Because the Rust schema strictly requires media records for each tombstone, orphan tombstones fail the import safely. Key collisions, unrecognized roles, or invalid timestamps also safely halt execution without promoting corrupted output.

Run the migration via the server CLI:

```text
learnvault-server migrate-go --input <go.sqlite> --output <rust.sqlite> --backup <go.sqlite.backup> --library-id <id>
```

This command does not start the HTTP listener and does not require library, network, or admin configuration. For a stable database snapshot, run it with the Go service stopped. Preserve the backup and verify the output with `Database::open` before deployment.

## Self-Hosted Cutover Gate

Before migrating, verify whether `${DATA_DIR}` contains a legacy Go v8 database or a Rust database. Formats are incompatible and cannot share the same directory volume.

1. Preserve the legacy Go volume and stop the Go service. Confirm quiescence before building or starting the new service.
2. Run `learnvault-server migrate-go` providing explicit paths for `--input`, `--backup`, `--output`, and `--library-id`. Prepare the output in a separate Rust volume.
3. Verify that the input file checksum is unchanged and output files exist and pass integrity checks.
4. Keep paired backups of both volumes. To roll back, stop Rust and restore the original Go volume.
