# Changelog

Todas as mudanças relevantes do LearnVault são documentadas neste arquivo.

O formato segue o Keep a Changelog e as versões usam Semantic Versioning.

## [0.2.0](https://github.com/AndersonTavares0/LearnVault/compare/v0.1.0...v0.2.0) (2026-10-15)


### Funcionalidades

* automate releases with Release Please ([#26](https://github.com/AndersonTavares0/LearnVault/issues/26)) ([2584724](https://github.com/AndersonTavares0/LearnVault/commit/2584724d3bbbe58c354e204034f761e8583d33dd))


### Correções

* remove invalid Release Please input ([#38](https://github.com/AndersonTavares0/LearnVault/issues/38)) ([a9aff27](https://github.com/AndersonTavares0/LearnVault/commit/a9aff27e54db4d6952088be2fbbc73c21882d3c2))

## [Unreleased]

### Adicionado

- Fundação do aplicativo desktop em `crates/desktop` (Tauri 2.12.x): shell da SPA embutido por `frontendDist`, capability deny-by-default para a janela principal, CSP explícita e nenhum listener de rede ou plugin privilegiado.
- IPC de catálogo e configuração no desktop: comandos `desktop_config` e `desktop_catalog` offline sobre o `learnvault-core`, com ACL app-defined mínima, DTOs snake_case do catálogo e estado offline determinístico ("biblioteca não configurada") que nunca oferece login no desktop. Entrega de mídia, rescan, autenticação e seleção da raiz da biblioteca continuam pendentes.
- Seleção e persistência segura da raiz da biblioteca no desktop: o comando `desktop_select_library`, sem argumentos, aciona o seletor nativo via `tauri-plugin-dialog` interno ao Rust (não concedido ao webview) e devolve apenas `{ configured }`. A raiz é validada (existência, diretório, canonicalização, recusa de link/junction e leitura) e gravada de forma versionada e atômica no diretório de configuração do aplicativo, fora da biblioteca e do SQLite; a biblioteca permanece somente leitura e revalidada no startup. O webview nunca envia nem recebe caminho. Rescan, mídia e autenticação continuam pendentes.

### Correções

- Catálogo recursivo preserva aulas e arquivos em qualquer profundidade, com rescan seguro contra resultados parciais.

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
