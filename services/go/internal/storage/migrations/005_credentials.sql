ALTER TABLE users ADD COLUMN must_change_credentials INTEGER NOT NULL DEFAULT 1 CHECK (must_change_credentials IN (0, 1));
INSERT OR IGNORE INTO schema_migrations(version) VALUES (5);
