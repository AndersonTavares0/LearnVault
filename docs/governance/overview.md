# Governança do repositório

## Fluxo obrigatório

1. Abra uma Discussion quando ainda houver dúvida ou alternativas de design.
2. Abra uma issue para uma feature ou mudança de comportamento.
3. Para typo, dependência ou ajuste simples de CI, a issue é opcional.
4. Crie uma branch focada a partir de `main`.
5. Abra uma pull request usando o template do repositório.
6. Passe pelos checks obrigatórios e obtenha a revisão necessária.
7. Resolva as conversas e faça squash merge pelo GitHub.

Push direto, force-push e exclusão de branch em `main` não são permitidos. A proteção da
branch é configurada no GitHub; este documento registra a regra do projeto.

O caminho de colaboração é:

```text
Discussion (se necessário) → issue (quando necessário) → branch → PR → revisão → squash merge
```

Forks não fazem parte do fluxo atual. Colaboradores com permissão de escrita trabalham em
branches dentro deste repositório. O `.github/CODEOWNERS` documenta ownership, mas não é
uma aprovação obrigatória para merge.

Cada issue deve ter uma pessoa responsável, definida pelo `assignee`.

## Política de templates

- Issues em branco estão desabilitadas.
- Bugs usam `.github/ISSUE_TEMPLATE/bug.yml`.
- Features usam `.github/ISSUE_TEMPLATE/feature.yml`.
- Documentação e typos usam `.github/ISSUE_TEMPLATE/documentation.yml`.
- Dependências e CI usam `.github/ISSUE_TEMPLATE/maintenance.yml`.
- Vulnerabilidades usam a política privada de segurança, nunca uma issue pública.
- Pull requests usam `.github/pull_request_template.md`.
- Dúvidas e decisões abertas devem usar GitHub Discussions.

## Organização do código

Mantenha a raiz limitada a configurações de ferramentas, políticas do projeto e pontos de
entrada de deploy. O código da aplicação pertence a estas pastas:

| Caminho | Responsabilidade |
| --- | --- |
| `crates/` | Espaço de trabalho Rust; `crates/core` concentra o domínio compartilhado (SQLite, catálogo, scanner, identidade de mídia e estado de estudo) |
| `web/src/` | Código-fonte da SPA |
| `cmd/learnvault/` | Implementação Go de referência (executável e saída embutida da SPA), em remoção gradual |
| `internal/` | Implementação Go de referência (pacotes internos, backend e migrações), em remoção gradual |
| `testdata/` | Fixtures sintéticas pequenas |
| `deploy/container/` | Definição da imagem OCI |
| `deploy/compose/` | Stack Docker Compose e exemplo de ambiente |
| `deploy/quadlet/` | Unidade systemd para Podman rootless |
| `scripts/` | Scripts de verificação local |
| `docs/` | Arquitetura, governança, operações, produto, releases e Wiki |
| `.github/` | Actions, configuração do Release Please e arquivos de comunidade |
| `.agents/` | Políticas, playbooks, checklists e templates para agentes |

Arquivos de comunidade (`CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`) ficam em
`.github/`. O changelog fica em `docs/releases/CHANGELOG.md`. A configuração do Release Please
fica em `.github/release-please/`. A raiz mantém apenas `AGENTS.md`, `README.md`, `LICENSE`,
`.dockerignore`, `.gitignore`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `go.mod`,
`go.sum`, `package.json`, `pnpm-lock.yaml`, `pnpm-workspace.yaml`, `tsconfig.json` e
`vite.config.ts`. Os crates `server` e `desktop` surgem nas suas fases correspondentes; só
existe `crates/core` até lá.

Saída gerada (`cmd/learnvault/web/build`, `target`, `.svelte-kit`, `node_modules` e dados/cache
locais) deve permanecer ignorada e não pode ser commitada.

## Limites do repositório

- Nunca faça commit de mídia de cursos, credenciais, endereços privados, bancos de dados ou saída gerada.
- Mudanças de segurança, armazenamento, deploy, migrações ou arquitetura de UX exigem revisão explícita.
- Pesquisa clean-room pode orientar comportamento, mas não código, assets, marca, texto ou layout copiado.
- Docker e Podman são adaptadores opcionais; o comportamento deve continuar testável sem runtime de container.

## Fontes documentais

O repositório é a fonte canônica. O índice em [`docs/README.md`](../README.md) explica onde
cada material deve ser colocado. A Wiki é uma superfície derivada; apenas as páginas em
`docs/wiki/mapping.md` são sincronizadas automaticamente.

`README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md` e `SECURITY.md` são
mantidos em pt-BR, assim como a governança, os templates, a Wiki e a documentação editorial.
Apenas a `LICENSE` permanece em inglês. Prefixos de Conventional Commits permanecem em inglês;
as descrições ficam em pt-BR para manter o Release Please consistente.

## Releases e CI

O Release Please controla versão, changelog, tags e metadados do GitHub Release. A publicação
de artefatos acontece em workflow separado.

Os três nomes de jobs obrigatórios permanecem estáveis: `SPA checks`, `Go tests and vet` e
`Container build`. O CI roda em pull requests, cancela execuções obsoletas, constrói a SPA uma
vez para o embed do Go, usa cache e mantém timeouts curtos.

O job `Rust tests and lints` roda junto e verifica formatação, lints, testes e a cobertura do
núcleo em Rust. Ele ainda não é um check obrigatório; a promoção a obrigatório exige decisão
explícita do mantenedor.

O Wiki sync não é check obrigatório: ele roda após mudanças mapeadas chegarem à `main` ou por
execução manual, escreve somente páginas gerenciadas e não reage a eventos da própria Wiki.

## Dependências e testes

O Dependabot agrupa atualizações compatíveis do frontend, módulos Go e Actions. Atualizações
major permanecem individuais e exigem revisão de compatibilidade e checks atuais.

Os testes mantêm cobertura real de Argon2id e autenticação. Fixtures são isoladas e não acessam
dados externos.
