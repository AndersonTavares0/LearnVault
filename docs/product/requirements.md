# Requisitos do produto

Este documento define os requisitos funcionais (RF) e não funcionais (RNF) do LearnVault nos dois
modos de uso aprovados no [ADR 0001](../architecture/adr/0001-reescrita-em-rust.md): o **servidor
self-hosted** com WebUI e o **desktop offline**.

Cada requisito cita a fonte canônica de onde veio, e os identificadores são estáveis para citação
em issues e pull requests. O escopo do MVP e o estado de entrega ficam no
[roadmap](roadmap.md); o acompanhamento das pendências de implementação fica nas issues.

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
- **RNF-05 — Operação.** O servidor expõe a saúde do processo e a prontidão do banco em
  `/api/healthz`; deploys expostos usam proxy HTTPS com limites de conexão; dados e cache ficam
  em diretórios separados; o rollback reinstala a release anterior publicada, com artefatos e
  checksums verificáveis, e, no cutover self-hosted, exige volumes pareados com RPO no instante
  do backup da importação — procedimento documentado, ainda não comprovado (G7).
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
