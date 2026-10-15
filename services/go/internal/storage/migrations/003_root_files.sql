CREATE TABLE IF NOT EXISTS catalog_root_files (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    size INTEGER NOT NULL
);
INSERT OR IGNORE INTO schema_migrations(version) VALUES (3);
