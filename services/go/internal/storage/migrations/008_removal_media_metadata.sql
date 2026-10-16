-- Metadados de elegibilidade de mídia no tombstone de remoção.
--
-- O progresso de estudo só se aplica a mídia reproduzível. Ao remover um arquivo, o
-- tombstone guardava apenas o caminho e o instante, então a elegibilidade de um `.ts`
-- (que o scanner classifica como mídia só quando tem tamanho de mídia, senão material)
-- não podia ser decidida depois, quando o arquivo já não está no catálogo.
--
-- Esta coluna guarda a decisão tomada no momento da remoção, enquanto o arquivo ainda
-- está no catálogo: `1` mídia, `0` legenda/material, `NULL` desconhecido (linha legada
-- gravada antes desta coluna, ou caminho que não estava no catálogo). Para `NULL`, a
-- borda HTTP aceita apenas extensões inequivocamente de mídia e recusa `.ts` por ser
-- ambíguo — conservador, mas sem inverter a classificação do catálogo.
--
-- A migração é aditiva (`ALTER TABLE ADD COLUMN`): bancos Go já existentes, que param na
-- versão 7, continuam válidos; as linhas de remoção existentes ganham `NULL`.
ALTER TABLE media_removal ADD COLUMN is_media INTEGER CHECK (is_media IN (0, 1));
INSERT OR IGNORE INTO schema_migrations(version) VALUES (8);
