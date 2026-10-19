# Governança do repositório

**Para quem é:** qualquer pessoa que abra issue, pull request, release ou mexa na CI. Se você só
quer rodar o projeto, comece por [Getting-Started](../wiki/getting-started.md).

**Resumo:** o repositório usa GitHub Flow. Mudanças de comportamento começam em uma issue ou
Discussion, todo código entra por branch e pull request, e `main` só recebe squash merge depois dos
checks e da revisão.

## Fluxo de contribuição

1. Abra uma Discussion quando o design ainda tiver dúvida ou alternativas.
2. Abra uma issue para uma feature ou mudança de comportamento.
3. Para typo, dependência ou ajuste simples de CI, a issue é opcional.
4. Crie uma branch focada a partir de `main`.
5. Abra uma pull request usando o template do repositório.
6. Passe pelos checks obrigatórios e obtenha a revisão necessária.
7. Resolva as conversas e faça squash merge (um único commit) pelo GitHub.

```text
Discussion (se necessário) → issue (quando necessário) → branch → PR → revisão → squash merge
```

Push direto, force-push e exclusão de branch em `main` não são permitidos. O GitHub aplica a
proteção da branch; este documento registra a regra do projeto.

Forks não fazem parte do fluxo atual. Quem tem permissão de escrita trabalha em branches neste
repositório. O `.github/CODEOWNERS` indica ownership, mas não é uma aprovação obrigatória para
merge. Toda issue precisa de uma pessoa responsável no `assignee`.

## Qual template usar

| Situação | Onde registrar |
| --- | --- |
| Bug | `.github/ISSUE_TEMPLATE/bug.yml` |
| Feature | `.github/ISSUE_TEMPLATE/feature.yml` |
| Documentação ou typo | `.github/ISSUE_TEMPLATE/documentation.yml` |
| Dependência ou CI | `.github/ISSUE_TEMPLATE/maintenance.yml` |
| Planejamento | `.github/ISSUE_TEMPLATE/planning.yml`, com a label `planning` |
| Vulnerabilidade | Política privada de segurança, nunca uma issue pública |
| Dúvida ou decisão aberta | GitHub Discussions |
| Pull request | `.github/pull_request_template.md` |

Issues em branco estão desabilitadas.

## Organização do código

Mantenha a raiz limitada a configurações de ferramentas, políticas do projeto e pontos de entrada de
deploy. O código da aplicação fica nestas pastas:

| Caminho | Responsabilidade |
| --- | --- |
| `crates/` | Workspace Rust. `crates/core` concentra o domínio compartilhado (SQLite, catálogo, scanner, identidade de mídia e estado de estudo); `crates/server` concentra o servidor Axum |
| `apps/web/` | Código-fonte e configuração da SPA Svelte 5 + Vite (`src/`, `vite.config.ts`, `tsconfig.json`, `package.json`) |
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
`.github/`. O changelog fica em `docs/releases/CHANGELOG.md`. A configuração do Release Please fica
em `.github/release-please/`.

A raiz mantém apenas `AGENTS.md`, `README.md`, `LICENSE`, `.dockerignore`, `.gitignore`, `Cargo.toml`,
`Cargo.lock`, `rust-toolchain.toml`, `package.json`, `pnpm-lock.yaml` e `pnpm-workspace.yaml`.

O crate `desktop` (Tauri) entrega a Fase 4: janela real, IPC, biblioteca somente leitura e
reprodução local, sem autenticação nem listener de rede por decisão de arquitetura. O serviço Go foi
removido na Fase 5 (PR #206). A distribuição Linux desktop/Flatpak ainda depende de validação; o
estado dos pacotes está em [`docs/operations/distribution.md`](../operations/distribution.md).

Saída gerada (`crates/server/web/build`, `crates/desktop/frontend-dist`, `target`, `node_modules` e
dados/cache locais) deve permanecer ignorada e não pode ser commitada.

## Limites do repositório

- Nunca faça commit de mídia de cursos, credenciais, endereços privados, bancos de dados ou saída
  gerada.
- Mudanças de segurança, armazenamento, deploy, migrações ou arquitetura de UX exigem revisão
  explícita.
- Pesquisa clean-room pode orientar comportamento, mas não código, assets, marca, texto ou layout
  copiado.
- Docker e Podman são adaptadores opcionais. O comportamento deve continuar testável sem runtime de
  container.

## Fontes documentais

O repositório é a fonte canônica. O índice em [`docs/README.md`](../README.md) explica onde cada
material deve ser colocado. A Wiki é uma superfície derivada; apenas as páginas listadas em
`docs/wiki/mapping.md` são sincronizadas automaticamente.

`README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md` e `SECURITY.md` são mantidos em
pt-BR, assim como a governança, os templates, a Wiki e a documentação editorial. Apenas a `LICENSE`
permanece em inglês. Prefixos de Conventional Commits permanecem em inglês; as descrições ficam em
pt-BR para manter o Release Please consistente.

## Releases e CI

O Release Please controla versão, changelog, tags e metadados do GitHub Release. A publicação de
artefatos acontece em um workflow separado.

O código e a documentação do LearnVault são distribuídos sob a licença MIT, conforme o arquivo
[`LICENSE`](../../LICENSE). Os metadados do workspace Rust, do pacote npm e do metainfo desktop
devem permanecer alinhados com essa licença. Registros no changelog descrevem o estado histórico das
versões e não substituem a licença vigente.

### Automação segura do Dependabot

O workflow `Dependabot safe auto-merge` existe para deixar o Dependabot atualizar dependências de
baixo risco sem transformar cada PR em trabalho manual. A automação roda no contexto da base por
`pull_request_target`, sem checkout, instalação de dependências ou execução de código da pull
request.

Ela só considera PRs do `dependabot[bot]` para `main`, no mesmo repositório, com uma atualização
individual patch/minor, alerta de segurança explicitamente `null` e head confirmado pela API.

O gate de alerta falha fechado. Se o estado do alerta estiver ausente, vazio ou desconhecido, a PR
fica de fora; o workflow nunca trata um dado ausente como "sem alerta". Atualizações major, de
segurança, agrupadas, conflitantes ou desatualizadas permanecem para análise humana.

Quando a PR é elegível, o workflow apenas solicita o auto-merge nativo com squash. Ele não aprova
PRs, não usa `--admin`, não faz force-push, rebase ou fechamento automático. A proteção da branch
define reviews e checks, e o GitHub reavalia a solicitação quando o head ou os checks mudam.

O workflow não concede permissões ao `GITHUB_TOKEN` (`permissions: {}`). Para consultar alertas com
`alert-lookup`, ele exige o segredo `DEPENDABOT_AUTOMERGE_TOKEN`, um fine-grained PAT ou token de
GitHub App com `Dependabot alerts: read`, `Contents: write` e `Pull requests: write`. Sem esse
segredo, o job falha fechado e nenhum auto-merge é solicitado.

Workflows disparados diretamente por PRs do Dependabot recebem `GITHUB_TOKEN` somente leitura e não
devem receber segredos. Por isso esta automação usa a base confiável, o token dedicado e uma chamada
de API sem executar o conteúdo da PR.

Só são elegíveis alterações nos manifests e lockfiles aprovados (`package.json`,
`apps/web/package.json`, `pnpm-lock.yaml`, `Cargo.toml`, manifests de crates e `Cargo.lock`).
Workflows, scripts, Containerfiles, Compose e outros caminhos permanecem manuais. A automação
confirma o SHA da base e do head e rejeita PRs que ficaram atrás da base, sem polling, rebase ou
checkout.

Para rollback, desabilite o workflow e cancele auto-merges pendentes. Se a proteção de branch mudar
ou uma merge queue for adotada, os gates devem ser reavaliados antes de manter a automação ativa.

### Checks e CI

Os nomes dos jobs obrigatórios são estáveis: `SPA checks`, `Rust tests and lints`,
`Desktop crate compile and guards`, `Container build (Rust)` e `CI gate`.

O CI roda em pull requests, cancela execuções obsoletas, constrói a SPA uma vez para a imagem Rust e
o desktop, usa cache e mantém timeouts curtos. Para alterações exclusivamente em documentação ou
metadados de release, os jobs E2E, Rust, desktop, container e auditoria de dependências podem ser
ignorados. O `CI gate` permanece estável e reprova falhas, cancelamentos e skips inesperados.

- `Rust tests and lints` verifica formatação, lints, testes e a cobertura do núcleo em Rust.
- `Desktop crate compile and guards` compila o crate desktop e roda seus testes.
- `Container build (Rust)` constrói e valida a imagem OCI Rust.
- `Rust dependency audit` audita o `Cargo.lock` com `cargo audit --deny unsound` e reprova
  vulnerabilidades e advisories `unsound`. Há uma única exceção estreita e documentada em
  `.cargo/audit.toml` (`RUSTSEC-2024-0429`, `glib`).

Toda promoção de check a obrigatório exige decisão explícita do mantenedor. Os checks acoplados ao Go
foram removidos na Fase 5 (PR #206).

O workflow `Wiki sync` não é check obrigatório. Ele roda depois que mudanças mapeadas chegam à `main`
ou por execução manual, escreve somente as páginas gerenciadas e não reage a eventos da própria
Wiki.

## Labels automáticas

`.github/labeler.yml` e o workflow `Labeler` classificam pull requests pelos arquivos alterados:
`area:web`, `area:rust`, `area:desktop`, `area:docs`, `area:ci` e `area:deploy`, além de `tests` e
`dependencies`.

O workflow usa `actions/labeler` fixado em SHA completo, roda em `pull_request_target`, não faz
checkout, não usa segredos e não executa código da pull request. Ele lê a configuração da base
confiável e ignora execuções vindas de forks.

Limites explícitos:

- A classificação é informacional. Não é check obrigatório, não aprova e não bloqueia merge.
- Tipo de trabalho (`bug`, `enhancement`, `documentation`) e segurança continuam decisões humanas. A
  automação não infere essas categorias por nome de arquivo ou de branch.
- `sync-labels: false`: nenhuma label é removida automaticamente. Correções manuais seguem
  permitidas, inclusive para labels antigas que deixarem de corresponder aos arquivos atuais.
- As labels referenciadas precisam existir no repositório. O workflow tem apenas
  `pull-requests: write`, não cria labels e falha ao tentar aplicar uma label inexistente.
- Testes unitários inline em Rust (`#[cfg(test)]` dentro de `src/`) não são detectáveis por caminho
  e podem não receber `tests`.

## Dependências e testes

O Dependabot segue o calendário e os grupos de `.github/dependabot.yml` para npm, Cargo, GitHub
Actions, Docker e Docker Compose. Atualizações minor e patch são agrupadas; atualizações major ficam
individuais e exigem revisão de compatibilidade e checks atuais.

Os testes mantêm cobertura real de Argon2id e autenticação. Fixtures são isoladas e não acessam dados
externos.
