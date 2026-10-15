-- Usuários que já existiam antes desta coluna já possuem credenciais configuradas.
ALTER TABLE users ADD COLUMN must_change_credentials INTEGER NOT NULL DEFAULT 0 CHECK (must_change_credentials IN (0, 1));
INSERT OR IGNORE INTO schema_migrations(version) VALUES (5);
