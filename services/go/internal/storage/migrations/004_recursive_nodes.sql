CREATE TABLE IF NOT EXISTS catalog_nodes_json (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    value TEXT NOT NULL
);
INSERT OR IGNORE INTO schema_migrations(version) VALUES (4);
