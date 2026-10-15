# Changelog

Todas as mudanças relevantes do LearnVault são documentadas neste arquivo.

O formato segue o Keep a Changelog e as versões usam Semantic Versioning.

## [Unreleased]

### Planejado

- Player focado em estudo e persistência de progresso.
- Curadoria de cursos, thumbnails, busca, anotações e favoritos.

## [0.1.0] - 2026-10-14

### Adicionado

- Governança inicial do projeto e documentação de contribuição.
- Políticas, playbooks, checklists e template de ADR em `.agents/`.
- Layout canônico da documentação e fluxo controlado de sincronização da Wiki.

### Alterado

- Padronização do repositório em torno de Go 1.26, Node.js 24, pnpm 11.28.5 e lockfile congelado do pnpm.
- Atualização de `golang.org/x/crypto` para v0.58.0 e `modernc.org/sqlite` para v1.60.1 junto com o toolchain Go 1.26.
- Manutenção da saída gerada da SPA, dados de runtime, caches, segredos e ferramentas locais fora do Git.

### Segurança

- A biblioteca externa de cursos permanece somente para leitura e fora do controle de versão,
  dos testes e dos backups.
