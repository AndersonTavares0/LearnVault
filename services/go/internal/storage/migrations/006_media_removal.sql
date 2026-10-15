-- Remoção explícita de mídia no catálogo Go.
--
-- A identidade da mídia no serviço Go é o caminho relativo completo (a varredura não
-- gera identificadores estáveis próprios), então a linha de remoção é chaveada por esse
-- caminho, no mesmo formato canônico aceito pela autorização de mídia.
--
-- A presença da linha É o fato da remoção. Nenhuma varredura cria, apaga ou reescreve
-- esta tabela: um rescan não desfaz a remoção nem altera `removed_at`. A leitura do
-- catálogo filtra os caminhos removidos, de modo que a mídia sai do catálogo e de
-- `/api/media` sem que a linha de catálogo ou qualquer progresso sejam apagados.
--
-- A migração é aditiva (`CREATE TABLE IF NOT EXISTS`), então bancos Go já existentes, que
-- param na versão 5, continuam válidos e apenas ganham a tabela nova.
CREATE TABLE IF NOT EXISTS media_removal (
    relative_path TEXT PRIMARY KEY,
    removed_at INTEGER NOT NULL
);
INSERT OR IGNORE INTO schema_migrations(version) VALUES (6);
