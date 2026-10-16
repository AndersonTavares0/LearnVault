# Requisitos do produto

Este documento consolida os requisitos funcionais (RF) e não funcionais (RNF) do LearnVault nos
dois modos de uso aprovados no [ADR 0001](../architecture/adr/0001-reescrita-em-rust.md): o
**servidor self-hosted** com WebUI e o **desktop offline**. Ele define o alvo do produto; o
status de entrega continua no [roadmap](roadmap.md), e funcionalidades pendentes não aparecem
aqui como concluídas.

Cada requisito cita a fonte de onde veio. Os identificadores são estáveis e podem ser citados em
issues e pull requests. Decisões ainda abertas aparecem em "Decisões abertas", nunca como
requisito fechado.

O **checklist de implementação** abaixo mostra o que já está entregue e o que falta; ele orienta
a abertura de novas issues para as lacunas sem acompanhamento. O roadmap continua sendo a visão
por fases; este documento define o alvo e o estado de cada requisito.

## Escopo do MVP (proposta)

O MVP é o conjunto mínimo que permite aposentar o serviço Go de referência (Fase 5 do
[roadmap](roadmap.md)) e publicar o desktop. Proposta:

- **Inclui:** RF-01 a RF-10 completos e RNF-01 a RNF-09 atendidos; RF-11 coberto por instalador
  Windows NSIS e imagem OCI ou binário nativo do servidor; estratégia para bancos Go legados
  resolvida ou suporte explicitamente delimitado, conforme o milestone
  [Migração Go → Rust](https://github.com/AndersonTavares0/LearnVault/milestone/3).
- **Não inclui:** Whisper e transcrições, notas, favoritos, busca avançada e cliente mobile
  nativo (seção "Mais tarde" do roadmap); `.msi` e versão portátil para Windows (discussion
  [#131](https://github.com/AndersonTavares0/LearnVault/discussions/131)); e Flatpak como
  bloqueio de release, já que a validação técnica pode seguir em paralelo.

Esta delimitação é uma proposta para revisão do mantenedor. O MVP só muda com atualização deste
documento.

## Modos de uso e atores

| Modo | Quem usa | Autenticação |
| --- | --- | --- |
| Servidor self-hosted | Administrador e usuários locais, pela rede ou WebUI | Contas locais com sessão por cookie |
| Desktop offline | Usuário único na própria máquina, sem rede | Sem login; dono local fixo resolvido no Rust |

Fonte: [visão de arquitetura](../architecture/overview.md); ADR 0001.

## Requisitos funcionais

- **RF-01 — Biblioteca somente leitura.** Indexar uma pasta de cursos existente sem mover,
  alterar ou apagar nada na pasta física.
- **RF-02 — Catálogo.** Persistir e servir a árvore de categorias, cursos, módulos e aulas, com
  materiais e arquivos soltos reconhecidos pelo scanner.
- **RF-03 — Identidade de mídia.** Cada arquivo tem um `media_id` opaco, separado do caminho
  físico; um arquivo que reaparece no mesmo caminho reutiliza a identidade.
- **RF-04 — Rescan explícito e preservador.** O scan só roda por ação do usuário; insere novos,
  reafirma conhecidos e marca ausentes, sem excluir registros nem progresso.
- **RF-05 — Remoção explícita de mídia.** A remoção é uma ação explícita e persistente
  (tombstone); nenhum rescan a desfaz e o progresso associado sobrevive.
- **RF-06 — Contas e sessões no servidor.** Login, logout, leitura da conta atual e troca de
  credenciais com invalidação das sessões anteriores; o primeiro acesso exige a troca.
- **RF-07 — Entrega de mídia autenticada.** Transmitir arquivos aprovados da biblioteca com
  suporte a `Range` (`206`/`416`) e recusar mídia removida ou ausente.
- **RF-08 — Estudo.** Progresso por usuário com posição de retomada, conclusão independente da
  posição e histórico recente com limites públicos.
- **RF-09 — Desktop offline.** Escolher a pasta da biblioteca por seletor nativo, indexar,
  navegar no catálogo e registrar estudo por IPC, sem login, sem cookies e sem portas de rede.
- **RF-10 — Interface de estudo.** Navegação de cursos e aulas, player, retomada, estados vazios
  e de erro, mantendo a identidade visual própria do projeto.
- **RF-11 — Distribuição.** Instalador NSIS no Windows, Flatpak como formato Linux preferencial
  (sujeito a validação técnica), imagem OCI e binário nativo para instalação self-hosted, com
  uma única versão por release.

Fontes: RF-01, RF-03, RF-04, RF-05, RF-06, RF-07, RF-09 e RF-11 no ADR 0001 e nos invariantes da
[visão de arquitetura](../architecture/overview.md); RF-02, RF-07 e RF-08 na
[referência da API](../reference/api.md); RF-10 nas seções de Interface do [roadmap](roadmap.md);
RF-11 também na [distribuição](../operations/distribution.md).

## Requisitos não funcionais

- **RNF-01 — Segurança dos dados.** A biblioteca fica externa ao aplicativo e é somente leitura;
  o SQLite guarda apenas metadados e estado pessoal, nunca a mídia dos cursos.
- **RNF-02 — Fronteira de confiança do desktop.** O webview nunca envia nem recebe caminhos
  arbitrários: a seleção usa o seletor nativo no Rust e a mídia é endereçada pelo `media_id`,
  com capabilities mínimas e CSP explícita.
- **RNF-03 — Privacidade e rede.** O desktop não expõe portas de rede nem usa autenticação HTTP;
  o servidor usa cookie HttpOnly `SameSite=Strict` e `Secure` por padrão, com `--insecure-http`
  restrito ao desenvolvimento local.
- **RNF-04 — Desempenho de mídia.** Reprodução com streaming parcial e seek, sem carregar
  arquivos inteiros em memória; mudanças sensíveis a desempenho incluem medições.
- **RNF-05 — Operação.** O servidor expõe saúde do processo e do banco em `/api/healthz`; deploys
  expostos usam proxy HTTPS com limites de conexão; dados e cache ficam em diretórios separados;
  o rollback permanece possível enquanto o serviço Go de referência existir.
- **RNF-06 — Portabilidade.** O núcleo compila sem Docker ou Podman; containers são adaptadores
  opcionais; o CI constrói cada artefato de plataforma antes de montar instaladores.
- **RNF-07 — Qualidade.** Cobertura de testes Rust de pelo menos 80% medida com `cargo-llvm-cov`,
  `fmt` e `clippy` sem avisos, e os passos do playbook de testes por tipo de mudança.
- **RNF-08 — Acessibilidade.** Navegação por teclado, foco visível, contraste, movimento reduzido
  e rótulos para leitores de tela.
- **RNF-09 — Idioma e documentação.** Produto e documentação editorial em pt-BR; o repositório é
  a fonte canônica e a Wiki é uma superfície derivada de leitura.

Fontes: RNF-01, RNF-03, RNF-05 na [visão de arquitetura](../architecture/overview.md), no ADR
0001 e na [referência da API](../reference/api.md); RNF-02 nos riscos do plano da issue #69 e na
visão de arquitetura; RNF-04 nas regras de entrega do [roadmap](roadmap.md) e no plano da #69;
RNF-06 na [distribuição](../operations/distribution.md) e no roadmap; RNF-07 no ADR 0001 e no
[playbook de testes](../../.agents/playbooks/testing.md); RNF-08 no roadmap; RNF-09 em
[`.agents/policies/governance.md`](../../.agents/policies/governance.md).

## Checklist de implementação

Status por requisito: **Implementado**, **Parcial** ou **Faltante**. A coluna de lacuna aponta
onde o trabalho pendente já é acompanhado; lacunas sem acompanhamento aparecem na seção
seguinte como candidatas a nova issue.

| Requisito | Status | Lacuna | Acompanhamento |
| --- | --- | --- | --- |
| RF-01 | Implementado | — | — |
| RF-02 | Implementado | Shape canônico recursivo em revisão | PR #121 |
| RF-03 | Implementado | — | — |
| RF-04 | Implementado | — | — |
| RF-05 | Parcial | Remoção explícita existe no núcleo e no Go, mas não é exposta no servidor Rust nem no desktop | Candidata a issue |
| RF-06 | Implementado | — | — |
| RF-07 | Implementado | — | — |
| RF-08 | Implementado | Capacidades de estudo documentadas na API | PR #125, issue #124 |
| RF-09 | Parcial | Sem reprodução de vídeo/áudio no desktop; mídia pequena já é entregue | Issue #69 (F4.5) |
| RF-10 | Parcial | Detalhes do curso, player com legendas/materiais e navegação de aulas pendentes | Roadmap (Interface) |
| RF-11 | Parcial | OCI do Go disponível, OCI Rust experimental; instaladores desktop faltantes | Discussion #131, issue #69 (C2) |
| RNF-01 | Implementado | — | — |
| RNF-02 | Implementado | Novas fatias de mídia devem manter a fronteira | Issue #69 (riscos) |
| RNF-03 | Implementado | — | — |
| RNF-04 | Parcial | Seek no desktop pendente; medições de recursos ainda não registradas | Issue #69 (F4.5) |
| RNF-05 | Parcial | Readiness do healthz em revisão; publicação do servidor Rust não validada | PR #128, issue #69 (P0.4) |
| RNF-06 | Parcial | CI ainda não constrói artefatos desktop por plataforma | Issue #69 (C2) |
| RNF-07 | Implementado | Gate de cobertura no núcleo ativo no CI | — |
| RNF-08 | Parcial | Itens de acessibilidade do roadmap pendentes | Roadmap (Interface) |
| RNF-09 | Implementado | — | — |

## Lacunas para novas issues

Itens sem issue própria hoje; cada um referencia o requisito que o origina. Devem virar issues
somente com a aprovação do mantenedor, uma por unidade de trabalho:

1. **Remoção explícita no servidor Rust e no desktop** (RF-05): expor a ação já implementada no
   núcleo, com tombstone e testes, mantendo a biblioteca somente leitura.
2. **Publicação do servidor Rust** (RNF-05, P0.4 da #69): exercitar binário Axum e imagem OCI
   Rust com healthz, persistência, volume somente leitura e rollback.
3. **Artefatos desktop por plataforma no CI/release** (RNF-06, RF-11): construir o instalador
   NSIS no runner Windows e anexar com checksums, depois da decisão da discussion #131.

As lacunas de mídia (RF-09, RNF-04), interface (RF-10, RNF-08), catálogo (RF-02) e healthz
(RNF-05) já têm acompanhamento aberto nas referências da tabela.

## Decisões abertas

Estas decisões ainda não fechadas afetam a distribuição e a migração; nenhuma delas é um
requisito fechado:

- **Artefatos Windows além do instalador padrão** (`.msi`, versão portátil): em discussão na
  [discussion #131](https://github.com/AndersonTavares0/LearnVault/discussions/131).
- **Estratégia para bancos legados do serviço Go**: item P0.2 da
  [issue #69](https://github.com/AndersonTavares0/LearnVault/issues/69). Até existir migração
  testada, a conversão automática está fora de escopo e o servidor Rust recusa o banco antigo
  com mensagem estável.
- **Flatpak como formato definitivo do desktop Linux**: preferencial no ADR, sujeito a
  validação de permissões, acesso persistente às bibliotecas e reprodução de mídia.

## Fora de escopo por ora

- Whisper e transcrições pesquisáveis, busca avançada, notas, favoritos e cliente mobile nativo,
  listados como trabalhos posteriores no [roadmap](roadmap.md).
- Conversão automática de bancos Go legados, conforme a [referência da API](../reference/api.md)
  e a decisão aberta acima.

## Rastreabilidade

| Grupo | Fonte principal |
| --- | --- |
| RF-01, RF-03, RF-04, RF-05, RF-06, RF-09, RF-11 e RNF-01, RNF-02, RNF-05 | [ADR 0001](../architecture/adr/0001-reescrita-em-rust.md) e [visão de arquitetura](../architecture/overview.md) |
| RF-02, RF-07, RF-08 e RNF-03 | [Referência da API](../reference/api.md) |
| RF-10, RNF-04, RNF-06, RNF-08 | [Roadmap](roadmap.md) |
| RF-11 e RNF-06 | [Distribuição](../operations/distribution.md) |
| RNF-07 e RNF-09 | [Playbook de testes](../../.agents/playbooks/testing.md) e [política de governança](../../.agents/policies/governance.md) |
