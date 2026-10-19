# LearnVault

[![CI](https://github.com/AndersonTavares0/LearnVault/actions/workflows/ci.yml/badge.svg)](https://github.com/AndersonTavares0/LearnVault/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.99-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Node.js](https://img.shields.io/badge/Node.js-24.x-339933?logo=node.js&logoColor=white)](https://nodejs.org/)
[![pnpm](https://img.shields.io/badge/pnpm-11.28.5-F69220?logo=pnpm&logoColor=white)](https://pnpm.io/)
[![Svelte](https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte&logoColor=white)](https://svelte.dev/)

O LearnVault transforma suas pastas de cursos em um espaço de estudo auto-hospedado.
Ele combina um núcleo de domínio em Rust (`crates/core`) e um servidor Axum (`crates/server`)
com uma SPA em Svelte 5 + Vite em torno de cursos somente leitura, registrados individualmente,
contas locais, indexação de catálogo e um fluxo de estudo focado.

## Status

A migração para a stack Rust foi consolidada (ADR 0001). `crates/core` (domínio, SQLite, catálogo, scanner,
identidade de mídia e estado de estudo) e `crates/server` (Axum, autenticação, sessões, mídia
com `Range`, registro de cursos e API de estudo) são a stack backend oficial, junto da SPA Svelte 5 + Vite com dois
builds. O desktop Tauri (`crates/desktop`) possui catálogo, importação e remoção de
cursos pelo seletor nativo, rescan explícito e reprodução local de materiais, vídeo e áudio, além
da entrega de legendas por protocolo customizado com faixas parciais. O serviço Go foi removido na
PR #206. O importador de bancos SQLite Go legados permanece no servidor Rust.

## Comece aqui

Requisitos: Rust 1.99 (definido em `rust-toolchain.toml`), Node.js 24.x e pnpm 11.28.5. Docker é opcional.

```sh
pnpm install --frozen-lockfile   # uma vez por checkout
pnpm dev                         # API Rust + SPA com recarga automática
pnpm verify                      # todos os checks de PR antes de enviar
```

`pnpm dev` sobe a API em `http://127.0.0.1:8090` e a SPA em `http://localhost:5173`, com
proxy de `/api` para o servidor. Para o desktop Tauri, use `pnpm dev:tauri`. Antes
da pull request, `pnpm verify` reproduz os checks de PR (use `--fast` para pular builds e
audit durante a iteração); os comandos individuais continuam em `.github/CONTRIBUTING.md`.

A SPA é construída a partir de `apps/web/src` e gravada em
`crates/server/web/build`, que o servidor Rust serve com `--web-root` (`pnpm dev` dispensa esse
build: o Vite serve a SPA e faz
proxy de `/api`). Sem configuração, o primeiro acesso usa `admin`/`admin` e entra direto,
sem tela forçada de troca; a alteração de usuário e senha fica em **Configurações → Conta**,
sempre autenticada pela senha atual. Credenciais fornecidas usam `ADMIN_USERNAME` e `ADMIN_PASSWORD`
com pelo menos 12 caracteres. Veja `deploy/compose/.env.example` para as configurações locais.

Na tela de acesso e na Seção Conta das Configurações, **Mostrar/Ocultar** revela somente o campo
escolhido;
as senhas começam ocultas e continuam compatíveis com gerenciadores de senha. A nova senha
exige ao menos 12 caracteres. Durante o envio, o formulário permanece visível e bloqueia
reenvios; falhas de login não revelam se o usuário existe.

No catálogo, **Reproduzir** abre vídeos e áudios no player do LearnVault, inclusive arquivos
soltos. **Baixar** identifica materiais e legendas como downloads; arquivos marcados como
ausentes não oferecem ação de abertura. Essas ações preservam a navegação da aplicação.

## Mapa do repositório

```text
crates/           Workspace Rust: `core` (domínio, SQLite, catálogo e identidade de mídia), `server` (Axum) e `desktop` (Tauri)
apps/web/         SPA Svelte 5 + Vite (fonte, configuração e scripts)
deploy/           Adaptadores de contêiner, Compose e Quadlet rootless
docs/             Arquitetura, API, governança, operações, produto e releases
.agents/          Políticas, playbooks, checklists e templates para agentes
.github/          Actions, configuração do Release Please, Dependabot e formulários
scripts/          Automações auxiliares de verificação e release
testdata/         Fixtures sintéticas usadas pelos testes
```

A raiz fica limitada a manifestos, configuração de ferramentas, políticas do projeto e pontos
de entrada exigidos pelas toolchains Rust e Node/pnpm/Vite, pelo GitHub ou pela
documentação. Arquivos de comunidade como `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md` e
`SECURITY.md` ficam em `.github/`. Saída gerada e dados de runtime são ignorados.

## Arquitetura

- Workspace Rust com um núcleo de domínio compartilhado em `crates/core` e um servidor Axum
  em `crates/server`.
- O servidor Rust expõe autenticação, sessões, catálogo agregado de cursos, mídia com `Range` e
  Study. O desktop Tauri é offline, sem autenticação nem listener de rede, e implementa janela,
  catálogo, importação/remoção, rescan e Study por IPC com dono local fixo. A distribuição Linux
  desktop/Flatpak e sua validação em sandbox continuam pendentes.
- SQLite no núcleo, com cadeia única de migrações.
- A SPA em Svelte 5 + TypeScript + Vite tem roteamento por hash e dois builds que escolhem o
  transporte em tempo de compilação.
- Cada pasta de curso é somente leitura e permanece fora do armazenamento da aplicação; a página
  inicial mostra os cursos em cards com capa gerada.
- Builds nativos continuam sendo a fonte de verdade para instaladores futuros.

Veja a [visão de arquitetura](docs/architecture/overview.md) e a
[referência da API](docs/reference/api.md).

## Deploy

Para contêineres locais, use o adaptador Compose:

```sh
docker compose --env-file deploy/compose/.env.example \
  -f deploy/compose/compose.yaml up --build
```

Detalhes de contêiner e Quadlet estão em [`deploy/`](deploy/) e no
[guia de distribuição](docs/operations/distribution.md).

## Releases

O Release Please cuida do cálculo de versão, das PRs de release, do
[`CHANGELOG.md`](docs/releases/CHANGELOG.md), das tags e das notas do GitHub Release. Use
Conventional Commits como `feat:`, `fix:`, `docs:` e `chore:`. Releases publicados recebem
automaticamente tarballs Linux versionados do servidor e `SHA256SUMS` a partir da implementação
oficial em Rust.

O instalador Windows via NSIS já é construído e publicado automaticamente: instala por usuário,
sem exigir administrador, com textos em português e inglês. O
[guia de distribuição](docs/operations/distribution.md) documenta os dados do usuário, o build
local e os avisos do SmartScreen, já que o binário ainda não é assinado. O Flatpak permanece
bloqueado e fora dos releases: a seleção Linux já usa o backend oficial `xdg-portal`, mas ainda
faltam o build e o smoke test reais no sandbox, incluindo acesso persistente e reprodução de
mídia. Os tarballs Linux do servidor já são publicados; Flatpak e publicação OCI/binário self-hosted
continuam pendentes no roadmap.

Leia o [processo de release](docs/releases/process.md) antes de alterar arquivos de release.

## Documentação e contribuição

O [guia de documentação](docs/README.md) é o índice da documentação canônica do projeto. O
repositório é a fonte de verdade; a Wiki gerenciada é uma superfície derivada de leitura.

Antes de contribuir, leia [CONTRIBUTING.md](.github/CONTRIBUTING.md) e a
[visão de governança](docs/governance/overview.md). Mudanças na `main` passam por pull request
e pelos checks obrigatórios.

Questões de segurança devem ser reportadas de forma privada, conforme descrito em
[SECURITY.md](.github/SECURITY.md).

## Licença

O LearnVault é distribuído sob a [licença MIT](LICENSE).
