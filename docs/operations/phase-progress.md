# Progresso das fases do LearnVault

Este registro acompanha a execução sequencial das fases autorizadas. Cada fase usa uma
worktree própria, fixtures sintéticas e um commit separado; nenhum banco ou arquivo real de
curso é usado durante o desenvolvimento.

| Fase | Estado | Branch/commit | Decisões e validação |
| --- | --- | --- | --- |
| 1 — Ordenação manual | Concluída | `feat/phase1-catalog-order` / `c0e92a2` | Ordenação por usuário, curso e grupo irmão; migração V4→V5 preservadora; testes core, servidor, desktop e web. |
| 2 — Descoberta e prévia | Concluída | `feat/phase2-import-discovery` / `870642e` | Descoberta física somente leitura separada de interpretação heurística; prévia HTTP autenticada e confirmação explícita; testes Rust, servidor, desktop, frontend, build e auditoria passaram. |
| 3 — Catálogo lógico e correções | Concluída | `feat/phase3-logical-catalog` / commit da branch | Corrige diretórios mistos: mídia direta vira aula explícita e subdiretórios permanecem módulos aninhados, sem alterar caminhos ou identidades. |
| 4 — Materiais, capas e visualização segura | Concluída | `feat/phase4-materials-safe-view` / `3761124` | Visualização inline somente para tipos allowlisted; capas derivadas de materiais de imagem; arquivos e raízes continuam somente leitura. |
| 5 — Study Analytics | Concluída | `feat/phase5-study-analytics` / commit da branch | Agregação somente leitura por usuário, excluindo mídias removidas e sem alterar o estado de progresso; contratos HTTP e desktop mantêm isolamento por dono. |
| 6 — Pomodoro persistente | Pendente | — | Executar somente após o commit da Fase 5. |
| 7 — Redesign visual | Pendente | — | Auditar a implementação existente antes de alterar; executar somente após a Fase 6. |

## Registro técnico da Fase 2

- A descoberta conserva a hierarquia física e classifica cada item como encontrado, ignorado,
  inacessível ou não suportado, sem gravar no SQLite.
- A interpretação produz apenas sugestões com confiança, explicação e sinalização de ambiguidade.
- Pastas selecionadas explicitamente são a única entrada; nenhum caminho é inferido a partir de
  bibliotecas do usuário.
- A importação existente e o rescan continuam sendo o único caminho de persistência de cursos e
  mídias; a prévia não altera identidade, progresso, anotações ou ordenação manual.
