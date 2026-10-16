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
| `crates/` | Espaço de trabalho Rust; `crates/core` concentra o domínio compartilhado (SQLite, catálogo, scanner, identidade de mídia e estado de estudo) e `crates/server` concentra o servidor Axum |
| `apps/web/` | Código-fonte e configuração da SPA Svelte 5 + Vite (`src/`, `vite.config.ts`, `tsconfig.json`, `package.json`) |
| `services/go/` | Módulo Go de referência (executável, pacotes internos, migrações e saída embutida da SPA), em remoção gradual |
| `testdata/` | Fixtures sintéticas pequenas |
| `deploy/container/` | Definição da imagem OCI |
| `deploy/compose/` | Stack Docker Compose e exemplo de ambiente |
| `deploy/quadlet/` | Unidade systemd para Podman rootless |
| `scripts/` | Scripts de verificação local |
| `docs/` | Arquitetura, governança, operações, produto, releases e Wiki |
| `.github/` | Actions, configuração do Release Please e arquivos de comunidade |
| `.agents/` | Políticas, playbooks, checklists e templates para agentes |
| `.opencode/` | Perfis específicos do OpenCode (somente leitura) |

Arquivos de comunidade (`CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`) ficam em
`.github/`. O changelog fica em `docs/releases/CHANGELOG.md`. A configuração do Release Please
fica em `.github/release-please/`. Os perfis específicos do OpenCode ficam em `.opencode/`.
A raiz mantém apenas `AGENTS.md`, `README.md`, `LICENSE`, `.dockerignore`, `.gitignore`,
`Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `package.json`, `pnpm-lock.yaml` e
`pnpm-workspace.yaml`. O crate `desktop` (Tauri) pertence à Fase 4 e já existe, mas a fase
permanece incompleta: mídia, autenticação e janelas reais ainda são gates. A distribuição por
plataforma e a remoção do serviço Go pertencem à Fase 5 e permanecem incompletas.

Saída gerada (`services/go/cmd/learnvault/web/build`,
`services/go/cmd/learnvault/web/desktop-build`, `target`, `node_modules` e dados/cache locais)
deve permanecer ignorada e não pode ser commitada.

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

### Automação segura do Dependabot

O workflow `Dependabot safe auto-merge` usa a configuração confiável da base por
`pull_request_target`, sem checkout, instalação de dependências ou execução de código da pull
request. Ele só considera PRs do `dependabot[bot]` para `main`, no mesmo repositório, com uma
atualização individual patch/minor não relacionada a alerta de segurança, com o head atual
confirmado pela API. Atualizações major, de segurança, agrupadas, desconhecidas, conflitantes ou
desatualizadas permanecem para análise humana; o auto-merge nativo aguarda ou recusa qualquer
estado que não satisfaça a proteção da branch.

Quando elegível, o workflow apenas solicita o auto-merge nativo do GitHub com squash. Ele não
aprova PRs, não usa `--admin`, não faz force-push, rebase ou fechamento automático. A proteção da
branch continua sendo a autoridade para reviews e checks; a solicitação também é reavaliada pelo
GitHub quando o head ou os checks mudam. Os checks protegidos atualmente são `SPA checks`,
`Go tests and vet` e `Container build`; este último inclui a varredura Trivy de vulnerabilidades
HIGH/CRITICAL. Os jobs Rust e Desktop continuam executando no CI, mas ainda não são checks
protegidos e portanto não são anunciados como gates da automação.

O workflow usa somente `contents: write` e `pull-requests: write` no `GITHUB_TOKEN` do workflow.
Para consultar alertas do Dependabot com `alert-lookup`, ele exige o segredo
`DEPENDABOT_AUTOMERGE_TOKEN`, configurado como um fine-grained PAT ou token de GitHub App com
apenas `Dependabot alerts: read`, `Contents: write` e `Pull requests: write`. Sem esse segredo,
o job falha fechado e nenhum auto-merge é solicitado. Workflows disparados diretamente por PRs do
Dependabot recebem normalmente um `GITHUB_TOKEN` somente leitura e não devem receber segredos; por
isso esta automação usa a base confiável, o token dedicado e uma chamada de API sem executar o
conteúdo da PR. Se a proteção de branch for alterada,
ou uma merge queue for adotada, os gates devem ser reavaliados antes de manter a automação ativa.
Como defesa adicional, a automação só considera alterações nos manifests e lockfiles aprovados
(`package.json`, `apps/web/package.json`, `pnpm-lock.yaml`, `Cargo.toml`, manifests de crates,
`Cargo.lock`, `services/go/go.mod` e `services/go/go.sum`); workflows, scripts, Containerfiles,
Compose e outros caminhos permanecem manuais. Ela confirma o SHA da base e do head e rejeita PRs
que ficaram atrás da base, sem fazer polling, rebase ou checkout.
Para rollback, desabilite o workflow e cancele auto-merges pendentes; não há mutação automática de
branches ou lockfiles.

Os três nomes de jobs obrigatórios permanecem estáveis: `SPA checks`, `Go tests and vet` e
`Container build`. O CI roda em pull requests, cancela execuções obsoletas, constrói a SPA uma
vez para o embed do Go, usa cache e mantém timeouts curtos.

O job `Rust tests and lints` roda junto e verifica formatação, lints, testes e a cobertura do
núcleo em Rust. Ele ainda não é um check obrigatório; a promoção a obrigatório exige decisão
explícita do mantenedor.

O Wiki sync não é check obrigatório: ele roda após mudanças mapeadas chegarem à `main` ou por
execução manual, escreve somente páginas gerenciadas e não reage a eventos da própria Wiki.

## Labels automáticas

`.github/labeler.yml` e o workflow `Labeler` classificam pull requests por arquivos
alterados: `area:web`, `area:rust`, `area:desktop`, `area:go`, `area:docs`, `area:ci` e
`area:deploy`, além de `tests` e `dependencies`. O workflow usa `actions/labeler` fixado em
SHA completo, roda em `pull_request_target`, não faz checkout, não usa segredos e não executa
código da pull request; a configuração é lida da base confiável e execuções vindas de forks
são ignoradas.

Limites explícitos:

- A classificação é informacional. Não é check obrigatório, não aprova e não bloqueia merge.
- Tipo de trabalho (`bug`, `enhancement`, `documentation`) e segurança continuam decisões
  humanas; a automação não infere essas categorias por nome de arquivo ou de branch.
- `sync-labels: false`: nenhuma label é removida automaticamente e correções manuais seguem
  permitidas, inclusive para labels antigas que deixarem de corresponder aos arquivos atuais.
- As labels referenciadas precisam existir no repositório. O workflow tem apenas
  `pull-requests: write`, não cria labels e falha ao tentar aplicar uma label inexistente.
- Testes unitários inline em Rust (`#[cfg(test)]` dentro de `src/`) não são detectáveis por
  caminho e podem não receber `tests`.

## Dependências e testes

O Dependabot agrupa atualizações compatíveis do frontend, módulos Go e Actions. Atualizações
major permanecem individuais e exigem revisão de compatibilidade e checks atuais.

Os testes mantêm cobertura real de Argon2id e autenticação. Fixtures são isoladas e não acessam
dados externos.
