-- Progresso de estudo por usuário autenticado e por mídia do catálogo Go.
--
-- O serviço Go não possui um identificador de mídia próprio (a varredura não gera
-- identificadores estáveis): a identidade é o caminho relativo canônico, o mesmo usado
-- por `/api/media` e pelo tombstone de remoção. A chave é o par (usuário, caminho).
--
-- Posição e conclusão são independentes e cada escrita é um upsert de instrução única,
-- como no domínio Rust; a presença da linha é o fato do progresso. A linha nunca é
-- apagada por uma varredura, pela remoção explícita da mídia nem pela exclusão do usuário
-- — remover do catálogo não é apagar o progresso, e a chave estrangeira é `NO ACTION`
-- (preservação, como no domínio Rust), não `CASCADE`. Não há rota de exclusão de usuário.
--
-- A migração é aditiva (`CREATE TABLE IF NOT EXISTS`), então bancos Go já existentes, que
-- param na versão 6, continuam válidos e apenas ganham a tabela nova. Diferente do
-- domínio Rust, não há chave estrangeira para `media` (não existe uma tabela de mídia
-- estável no Go): a existência da mídia é garantida na borda HTTP, que só grava para um
-- caminho presente no catálogo persistido ou no tombstone de remoção.
CREATE TABLE IF NOT EXISTS study_state (
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE NO ACTION,
    media_path TEXT NOT NULL,
    position_seconds INTEGER NOT NULL DEFAULT 0 CHECK (position_seconds >= 0),
    completed INTEGER NOT NULL CHECK (completed IN (0, 1)),
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, media_path)
);
INSERT OR IGNORE INTO schema_migrations(version) VALUES (7);
