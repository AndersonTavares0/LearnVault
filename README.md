# LearnVault

[![CI](https://github.com/AndersonTavares0/LearnVault/actions/workflows/ci.yml/badge.svg)](https://github.com/AndersonTavares0/LearnVault/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.99-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Node.js](https://img.shields.io/badge/Node.js-24.x-339933?logo=node.js&logoColor=white)](https://nodejs.org/)
[![pnpm](https://img.shields.io/badge/pnpm-11.28.5-F69220?logo=pnpm&logoColor=white)](https://pnpm.io/)
[![Svelte](https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte&logoColor=white)](https://svelte.dev/)
[![Go](https://img.shields.io/badge/Go-1.26-00ADD8?logo=go&logoColor=white)](https://go.dev/)

O LearnVault transforma uma pasta de cursos existente em um espaço de estudo auto-hospedado.
Ele combina um núcleo de domínio em Rust (`crates/core`) e um servidor Axum (`crates/server`)
com uma SPA em Svelte 5 + Vite em torno de uma biblioteca somente leitura, contas locais,
indexação de catálogo e um fluxo de estudo focado. O serviço Go em `services/go` permanece
como implementação de referência até a Fase 5 da reescrita.

## Status

A reescrita em Rust está em andamento. `crates/core` (domínio, SQLite, catálogo, scanner,
identidade de mídia e estado de estudo) e `crates/server` (Axum, autenticação, sessões, mídia
com `Range` e API de estudo) já estão implementados, junto da SPA Svelte 5 + Vite com dois
builds. As entregas mescladas #86, #88, #90 e #91 alinham Study no servidor Rust, na SPA web e
no serviço Go de referência, além de avançarem o desktop com configuração, seleção da raiz,
 catálogo, rescan explícito e reprodução local de materiais, vídeo e áudio, além da entrega de
 legendas por protocolo com faixas limitadas. No desktop, o player renderiza legendas VTT; SRT,
 ASS e SSA ficam disponíveis para download, enquanto o build HTTP pode converter SRT sob demanda.
 O desktop Tauri e a distribuição por plataforma com a remoção do serviço Go
(Fase 5) **ainda não estão concluídos**; autenticação desktop, janelas reais, migração do banco e
distribuição continuam gates explícitos.

## Comece aqui

Requisitos: Rust 1.99 (definido em `rust-toolchain.toml`), Node.js 24.x e pnpm 11.28.5. Go 1.26
é necessário apenas para a implementação de referência. Docker é opcional.

```sh
pnpm install --frozen-lockfile   # uma vez por checkout
pnpm dev                         # API Rust + SPA com recarga automática
pnpm verify                      # todos os checks de PR antes de enviar
```

`pnpm dev` sobe a API em `http://127.0.0.1:8090` e a SPA em `http://localhost:5173`, com
proxy de `/api` para o servidor. Para o desktop Tauri (Fase 4), use `pnpm dev:tauri`. Antes
da pull request, `pnpm verify` reproduz os checks de PR (use `--fast` para pular builds e
audit durante a iteração); os comandos individuais continuam em `.github/CONTRIBUTING.md`.

A SPA é construída a partir de `apps/web/src` e gravada em
`crates/server/web/build`, que o servidor Rust serve com `--web-root` (`pnpm dev` dispensa esse
build: o Vite serve a SPA e faz
proxy de `/api`). Sem configuração, o primeiro acesso usa `admin`/`admin` e exige troca
imediata; credenciais fornecidas usam `ADMIN_USERNAME` e `ADMIN_PASSWORD` com pelo menos 12
caracteres. Veja `deploy/compose/.env.example` para as configurações locais.

Esse caminho é o contrato canônico preparatório para a futura cutover coordenada: o CI também
estagia a mesma saída para os testes Go. O serviço Go e a imagem Go padrão continuam sendo a
produção/referência estável; a imagem Rust permanece opt-in e experimental.

A implementação de referência e rollback em Go continua disponível:

```sh
go -C services/go test ./...
go -C services/go run ./cmd/learnvault
```

## Mapa do repositório

```text
crates/           Workspace Rust: `core` (domínio, SQLite, catálogo e identidade de mídia) e `server` (Axum)
apps/web/         SPA Svelte 5 + Vite (fonte, configuração e scripts)
services/go/      Módulo Go de referência (executável, pacotes internos, migrações e saída embutida da SPA)
deploy/           Adaptadores de contêiner, Compose e Quadlet rootless
docs/             Arquitetura, API, governança, operações, produto e releases
.agents/          Políticas, playbooks, checklists e templates para agentes
.github/          Actions, configuração do Release Please, Dependabot e formulários
scripts/          Automações auxiliares de verificação e release
testdata/         Fixtures sintéticas usadas pelos testes
```

A raiz fica limitada a manifestos, configuração de ferramentas, políticas do projeto e pontos
de entrada exigidos pelas toolchains Rust e Go, pelo pnpm e Vite, pelo GitHub ou pela
documentação. Arquivos de comunidade como `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md` e
`SECURITY.md` ficam em `.github/`. Saída gerada e dados de runtime são ignorados.

## Arquitetura

- Workspace Rust com um núcleo de domínio compartilhado em `crates/core` e um servidor Axum
  em `crates/server`.
- O servidor Rust expõe autenticação, sessões, catálogo, mídia com `Range` e Study; o desktop
  Tauri (`crates/desktop`, Fase 4) já tem fundação, configuração, catálogo, seleção de raiz,
  rescan explícito e Study por IPC com dono local fixo, mas ainda não é um produto desktop
  completo.
- SQLite no núcleo, com cadeia única de migrações.
- A SPA em Svelte 5 + TypeScript + Vite tem roteamento por hash e dois builds que escolhem o
  transporte em tempo de compilação.
- A biblioteca física é somente leitura e permanece fora do armazenamento da aplicação.
- O serviço Go em `services/go` permanece como referência funcional até a Fase 5.
- Docker Compose e Podman/Quadlet são adaptadores opcionais de deploy.
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
automaticamente binários Linux versionados e `SHA256SUMS` a partir da implementação de
referência em Go.

O instalador Windows via NSIS já é construído e publicado automaticamente: instala por usuário,
sem exigir administrador, com textos em português e inglês. O
[guia de distribuição](docs/operations/distribution.md) documenta os dados do usuário, o build
local e os avisos do SmartScreen, já que o binário ainda não é assinado. O Flatpak permanece
bloqueado e fora dos releases: a seleção Linux já usa o backend oficial `xdg-portal`, mas ainda
faltam o build e o smoke test reais no sandbox, incluindo acesso persistente e reprodução de
mídia. A publicação do servidor ainda pertence à Fase 5 da reescrita.

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

O LearnVault é distribuído sob a [Apache License 2.0](LICENSE).
