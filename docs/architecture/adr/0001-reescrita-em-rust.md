# ADR 0001: reescrever o LearnVault em Rust com núcleo compartilhado

## Status

Accepted — 2026-10-14

Este ADR registra a **arquitetura aprovada**. A **implementação atual ainda é Go com SvelteKit**
e permanece no repositório como referência funcional e visual até que o MVP em Rust cubra os
requisitos aprovados.

## Context

O LearnVault transforma pastas locais de cursos em uma plataforma pessoal de estudos, usada de
duas formas: como aplicativo desktop offline e como servidor self-hosted com WebUI. A
implementação atual é um serviço Go com SQLite e uma SPA SvelteKit embutida.

O mantenedor decidiu adotar Rust como stack de longo prazo, com um único núcleo de domínio
compartilhado entre servidor e desktop, e o Tauri 2 como tecnologia do desktop. A decisão é
**tecnológica e deliberada**: não existe limitação concreta do Go que force a troca, e Go com
Wails atenderia aos dois modos com menos trabalho.

## Decision

O LearnVault passa a ser um workspace Rust:

- `crates/core` — bibliotecas, catálogo, scanner, identidade de arquivos, política de rescan,
  persistência SQLite com cadeia única de migrações e estado de estudo. Expõe operações e erros
  tipados, não conhece HTTP nem Tauri, e recebe a raiz da biblioteca já resolvida, o `library_id`
  e o `owner_id` do estado de estudo.
- `crates/server` — servidor Axum com autenticação Argon2id, sessões, cookies, autorização de
  acesso, entrega de mídia com `Range` (206/416), cabeçalhos de segurança e entrega da SPA.
- `crates/desktop` — aplicativo Tauri 2 com comandos IPC, integração com o sistema de arquivos,
  seleção de diretórios e protocolo de reprodução de mídia. Sem HTTP, sem cookies, sem portas de
  rede e sem autenticação.
- `apps/web/` — uma única SPA em Svelte 5 + TypeScript + Vite, sem SvelteKit, com roteamento por
  hash e dois builds que selecionam o transporte em tempo de compilação.

Decisões de domínio que a reescrita mantém:

- Identidade de mídia (`media_id`) separada da localização física. `relative_path` é a
  localização atual, não a chave, e a comparação é normalizada conforme o sistema operacional.
- A raiz da biblioteca é configuração da plataforma, não dado do domínio.
- O rescan nunca exclui registros automaticamente: arquivo ausente é marcado como ausente e
  arquivo que reaparece no mesmo caminho reutiliza o `media_id`. Remoção só por ação explícita.
- A biblioteca física permanece externa e somente leitura.
- O estado de estudo é identificado por um `owner_id` opaco: o desktop usa um dono local fixo e
  o servidor usa o identificador do usuário autenticado.

Distribuição: instalador NSIS no Windows; **Flatpak como formato Linux preferencial**, sujeito à
validação técnica de permissões, acesso persistente às bibliotecas e reprodução de mídia; imagem
OCI mais binário nativo para instalação self-hosted. Uma única versão por release.

A execução acontece por fases: governança, `crates/core`, `crates/server`, `apps/web/`,
`crates/desktop` e distribuição, com a remoção do serviço Go em pull request próprio.

## Alternativas consideradas

### Permanecer em Go

- **Prós:** nenhum trabalho de reescrita; a implementação atual já cobre autenticação, catálogo,
  mídia com `Range` e deploy.
- **Contras:** não atende à decisão de stack do mantenedor nem à reutilização de um núcleo único
  com o desktop pelo Tauri.
- **Por que não:** a adoção de Rust é uma decisão deliberada e de longo prazo.

### Go com Wails no desktop

- **Prós:** reutilizaria o núcleo, os testes e o pipeline de release existentes, com uma única
  toolchain.
- **Contras:** escolheria a stack pelo atrito da migração, não pelo destino desejado.
- **Por que não:** o mantenedor prefere o Tauri 2 e aceita o custo inicial maior.

### Migração incremental do serviço Go

- **Prós:** reduziria o risco ao manter os dois backends em paralelo.
- **Contras:** exigiria manter duas pilhas e infraestrutura de testes duplicada durante toda a
  transição.
- **Por que não:** decidiu-se reescrever do zero, usando o Go apenas como referência.

## Consequences

### Positive

- Um único núcleo de domínio para os dois modos de uso.
- Desktop offline sem expor portas de rede.
- Stack única de longo prazo para servidor e desktop.

### Negative

- Reescrita completa antes de existir paridade funcional.
- O domínio, a persistência e a API não são aproveitados da implementação atual.
- O aplicativo desktop no Linux depende do webview do sistema.

### Risks

- **Empacotamento Flatpak:** tratado como formato preferencial, não definitivo; permissões,
  acesso persistente às bibliotecas e reprodução de mídia precisam de validação técnica.
- **Reprodução de mídia no desktop:** depende de streaming parcial no protocolo escolhido;
  será investigado na fase do desktop.
- **Versão única entre binários:** exige uma fonte única de verdade para o número de versão.

## Validation and rollback

Validação prevista: cobertura de testes Rust de pelo menos 80% medida com `cargo-llvm-cov`,
`cargo fmt --check`, `cargo clippy` sem avisos, builds da SPA nos dois modos e execução dos
artefatos de distribuição antes de cada release.

Rollback: enquanto a reescrita não estiver concluída, o serviço Go permanece no repositório e no
histórico, de modo que a implementação anterior pode ser retomada. A remoção do Go acontece em
pull request próprio, somente quando o MVP em Rust cobrir os requisitos aprovados.
