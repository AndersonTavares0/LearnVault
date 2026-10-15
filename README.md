# LearnVault

[![CI](https://github.com/AndersonTavares0/LearnVault/actions/workflows/ci.yml/badge.svg)](https://github.com/AndersonTavares0/LearnVault/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/AndersonTavares0/LearnVault?display_name=tag&sort=semver)](https://github.com/AndersonTavares0/LearnVault/releases)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Go](https://img.shields.io/badge/Go-1.26-00ADD8?logo=go&logoColor=white)](https://go.dev/)
[![Node.js](https://img.shields.io/badge/Node.js-24.x-339933?logo=node.js&logoColor=white)](https://nodejs.org/)
[![pnpm](https://img.shields.io/badge/pnpm-11.28.5-F69220?logo=pnpm&logoColor=white)](https://pnpm.io/)
[![SvelteKit](https://img.shields.io/badge/SvelteKit-2-FF3E00?logo=svelte&logoColor=white)](https://svelte.dev/docs/kit)

O LearnVault transforma uma pasta de cursos existente em um espaço de estudo auto-hospedado.
Ele combina um serviço em Go, SQLite e uma SPA em SvelteKit em torno de uma biblioteca
somente leitura, contas locais, indexação de catálogo e um fluxo de estudo focado.

## Status

A base está em desenvolvimento ativo. Autenticação, migrações de SQLite, indexação de
catálogo, a SPA embutida e adaptadores opcionais de contêiner já estão disponíveis. Player,
progresso, busca, anotações, favoritos, thumbnails e processamento de mídia são os próximos
passos previstos.

## Comece aqui

Requisitos: Go 1.26, Node.js 24.x e pnpm 11.28.5. Docker é opcional.

```sh
pnpm install --frozen-lockfile
pnpm run check
pnpm run build
go test ./...
go run ./cmd/learnvault
```

A SPA é construída a partir de `web/src`, gravada em `cmd/learnvault/web/build` e embutida no
executável Go. A primeira execução exige `ADMIN_USERNAME` e `ADMIN_PASSWORD` com pelo menos
12 caracteres. Veja `.env.example` para as configurações locais.

## Mapa do repositório

```text
crates/           Núcleo de domínio em Rust (SQLite, catálogo, scanner e identidade de mídia)
cmd/learnvault/   Implementação Go de referência (executável e saída embutida da SPA)
internal/         Implementação Go de referência (backend, catálogo, biblioteca e API HTTP)
web/src/          Código-fonte da SPA SvelteKit
deploy/           Adaptadores de contêiner, Compose e Quadlet rootless
docs/             Arquitetura, API, governança, operações, produto e releases
.agents/          Políticas, playbooks, checklists e templates para agentes
.github/          Actions, configuração do Release Please, Dependabot e formulários
scripts/          Automações auxiliares de verificação e release
testdata/         Fixtures sintéticas usadas pelos testes
```

A raiz fica limitada a manifestos, configuração de ferramentas, políticas do projeto e pontos
de entrada exigidos por Go, pnpm, SvelteKit, GitHub ou pela documentação. Arquivos de
comunidade como `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md` e `SECURITY.md` ficam em `.github/`.
Saída gerada e dados de runtime são ignorados.

## Arquitetura

- Um único executável Go com a SPA estática embutida.
- SQLite por meio de `modernc.org/sqlite`.
- A biblioteca física é somente leitura e permanece fora do armazenamento da aplicação.
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
automaticamente binários Linux versionados e `SHA256SUMS`.

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
