# Roadmap do LearnVault

Este roadmap é o resumo durável do produto e é orientado a resultados. Detalhes de
implementação pertencem às Issues e pull requests relacionadas. Os requisitos funcionais e
não funcionais consolidados ficam em [requirements.md](requirements.md).

## Escopo do MVP (proposta)

O MVP é o conjunto mínimo que permite aposentar o serviço Go de referência (Fase 5) e publicar o
desktop:

- **Inclui:** RF-01 a RF-10 completos; RNF-01 a RNF-07 atendidos (segurança, privacidade,
  desempenho de mídia, operação, portabilidade e qualidade); RNF-08 e RNF-09 seguem como metas
  contínuas. RF-11 coberto por instalador Windows NSIS e imagem OCI ou binário nativo do
  servidor; estratégia para bancos Go legados resolvida ou suporte explicitamente delimitado,
  conforme o milestone
  [Migração Go → Rust](https://github.com/AndersonTavares0/LearnVault/milestone/3).
- **Não inclui:** Whisper e transcrições, notas, favoritos, busca avançada e cliente mobile
  nativo (seção "Mais tarde"); `.msi` e versão portátil para Windows (discussion
  [#131](https://github.com/AndersonTavares0/LearnVault/discussions/131)); Flatpak como bloqueio
  de release, já que a validação técnica pode seguir em paralelo.

Esta delimitação é uma proposta para revisão do mantenedor; mudanças de escopo atualizam esta
seção.

## Corrente — reescrita em Rust

A arquitetura aprovada está no [ADR 0001](../architecture/adr/0001-reescrita-em-rust.md).
A execução avança por fases, com um pull request por fase:

- [x] Fase 0 — ADR, arquitetura e roadmap atualizados
- [x] Fase 1 — `crates/core` com domínio, SQLite, catálogo, identidade e política de rescan
- [x] Fase 1 (complemento) — remoção explícita de mídia no núcleo
- [x] Fase 2 — `crates/server` com Axum, autenticação, sessões e mídia com Range
- [x] Fase 3 — SPA Svelte 5 + TypeScript + Vite com dois builds
- [x] Fase 3a — recusa segura de bancos Go legados e API explícita, limitada e backup-first para
  importar usuários, catálogo e estado; comando `learnvault-server migrate-go` executa a operação
  sem iniciar o listener HTTP
- [ ] Fase 4 — `crates/desktop` com Tauri 2, offline e sem portas de rede
  - [x] Fase 4 (fundação) — crate `crates/desktop` no workspace, `frontendDist` embutido, capability deny-by-default e CSP explícita; IPC de domínio, mídia e janelas reais ainda pendentes
  - [x] Fase 4 (IPC de catálogo) — comandos `desktop_config` e `desktop_catalog` offline com ACL app-defined mínima, DTOs snake_case do catálogo e erros IPC sanitizados; mídia, autenticação, seleção da raiz da biblioteca e janelas reais continuam pendentes
  - [x] Fase 4 (seleção da raiz) — comando `desktop_select_library` sem argumentos aciona o seletor nativo no Rust e persiste a raiz canonicalizada de forma versionada e atômica no diretório de configuração do aplicativo, com revalidação no startup e biblioteca somente leitura; mídia, autenticação e janelas reais continuam pendentes
  - [x] Fase 4 (rescan da biblioteca) — comando `desktop_rescan` sem argumentos e assíncrono (`spawn_blocking`) indexa explicitamente a raiz persistida reutilizando `learnvault-core` (`rescan`/`RescanSummary`), com estado ocupado que serializa chamadas concorrentes por um erro estável (`rescan_in_progress`), raiz não configurada (`library_not_configured`) e raiz inacessível (`rescan_unavailable`) sanitizados; o rescan nunca é automático e a biblioteca permanece somente leitura. Mídia, autenticação e janelas reais continuam pendentes
  - [x] Fase 4 (mídia local) — protocolo customizado `learnvault-media://localhost/<media-id>` entrega imagens/thumbnails, materiais pequenos, vídeo, áudio e legendas pelo `media_id` opaco, com parsing estrito, allowlist de tipo de conteúdo, faixas HTTP limitadas, limite explícito por resposta (`200`/`HEAD`/`206`/`413`/`416`) e revalidação pelo `learnvault-core` (`open_media`); no Windows (WebView2) a SPA pede `http://learnvault-media.localhost/<media-id>` e o runtime reverte para o esquema antes do handler (Linux/macOS usam o esquema direto); nenhuma capability nova
  - [x] Fase 4 (Study desktop) — comandos `desktop_study_progress`, `desktop_record_position`, `desktop_set_completed` e `desktop_recent_study`, assíncronos (`spawn_blocking`) e com ACL mínima, reutilizam `learnvault-core::study` com o dono local fixo resolvido no Rust — o webview nunca envia `owner_id` — e devolvem o mesmo contrato snake_case do HTTP (`recorded`, `updated_at`) com erros estáveis (`media_not_found`, `invalid_position`, `study_unavailable`). A SPA ativa Study nos dois transportes por `invoke`, sem `fetch`, e sincroniza posição real da reprodução local; autenticação e janelas reais continuam pendentes
- [ ] Fase 5 — distribuição por plataforma e remoção do serviço Go — ver o [Gate de elegibilidade para a remoção do serviço Go](../operations/migration-go-to-rust.md#gate-de-elegibilidade-para-a-remoção-do-serviço-go)

## Atual — fundação e catálogo

Esta seção descreve a implementação atual em Go, mantida como referência funcional e visual
durante a reescrita.

- [x] Serviço Go com SPA SvelteKit embutida
- [x] Bootstrap do administrador e sessões seguras
- [x] Migrações SQLite e persistência do catálogo
- [x] Scan explícito e somente leitura da biblioteca
- [x] Catálogo autenticado e endpoint de mídia com suporte a Range
- [x] Base reproduzível de deploy com Docker/Podman
- [x] CI, automação de releases e governança do repositório

## Entregue — ciclo de estudo compatível

As entregas mescladas #86, #88, #90 e #91 alinham o contrato de Study entre o servidor Rust,
a SPA web e o serviço Go de referência: progresso, posição de retomada, conclusão e histórico
recente usam o mesmo contrato e os mesmos limites públicos. O desktop expõe esse contrato por IPC,
com o dono local fixo resolvido no Rust, e o player reproduz mídia local pelo protocolo customizado,
mantendo retomada e conclusão no mesmo fluxo de estudo.

A entrega #182 reorganiza a biblioteca por **cursos importados individualmente** — no desktop pelo
seletor nativo, no servidor por caminho validado — com a página inicial em cards de capa gerada,
navegação por curso e remoção sem apagar arquivos. A biblioteca deixa de ser uma raiz única: cada
pasta é um curso, registrado com identidade estável, e a interface não conhece caminhos.

## Próximo — ciclo de estudo utilizável

- [ ] Detalhes do curso e navegação de aulas
  - [x] Navegação anterior/próxima entre aulas e índice de aulas do curso no player (`apps/web`)
  - [ ] Página de detalhes do curso
- [ ] Player direto com links de legendas e materiais
- [x] Progresso individual e posição de retomada
- [x] Marcar aula como concluída e painel de histórico recente
- [ ] Permissões de acesso a cursos para múltiplos usuários locais

## Interface — mesa de estudo

- [ ] Estabelecer sidebar persistente no desktop e navegação mobile
- [ ] Priorizar “Continuar estudando” na tela inicial
- [ ] Construir o player como canvas principal de estudo com índice de aulas
- [ ] Adicionar capas manuais, estados vazios, carregamento e recuperação
  - [x] Capas geradas a partir do título e estados vazios da página inicial
- [ ] Validar navegação por teclado, foco, contraste, movimento reduzido e labels para leitores de tela
- [ ] Manter a identidade visual original do projeto

## Depois — experiência de biblioteca ampliada

- [ ] Capas de cursos manuais (por imagem da pasta)
- [ ] Cache de remux sob demanda para mídias incompatíveis
- [ ] Busca no catálogo, materiais e notas
- [ ] Favoritos e notas com timestamp
- [ ] Navegação responsiva e melhorias de acessibilidade

## Distribuição — portabilidade primeiro

A matriz aprovada está no [ADR 0001](../architecture/adr/0001-reescrita-em-rust.md).

- [ ] Manter o núcleo compilável sem Docker ou Podman
- [x] Publicar instalador Windows `.exe` via NSIS (por usuário, pt-BR/en, não assinado)
- [ ] Publicar Flatpak como formato Linux preferencial do desktop — a seleção usa o backend
  oficial `xdg-portal` no Linux e o manifesto não concede filesystem amplo; ainda faltam build e
  smoke test reais no sandbox Flatpak, inclusive validação do acesso persistente ao diretório
  escolhido
- [ ] Publicar imagem OCI e binário nativo para instalação self-hosted
- [ ] Assinar artefatos desktop quando houver credenciais de plataforma
- [x] Gerar `SHA256SUMS` para cada release
- [ ] Manter Docker Compose e Podman rootless/Quadlet como adaptadores opcionais de servidor

O Flatpak continua explicitamente bloqueado: o build e a publicação automática permanecem
desativados. A seleção Linux usa o backend oficial `xdg-portal` e o manifesto não concede acesso
amplo ao filesystem, mas o build Flatpak real e o smoke test no sandbox, incluindo acesso
persistente ao diretório escolhido e reprodução de mídia, ainda estão pendentes.

## Mais tarde — inteligência e operações opcionais

- [ ] Integração opcional com Whisper como capacidade separada
- [ ] Indexação de transcrições e legendas pesquisáveis
- [ ] Automação de backup com retenção e verificação de restauração
- [ ] Deploy em ambiente doméstico atrás de Caddy
- [ ] Cliente mobile nativo depois que a experiência web estiver estável

## Regras de entrega

- Cada marco deve ter resultado claro para o usuário, testes, documentação e notas de rollback.
- Alvos de distribuição são aditivos: falha de container não deve bloquear desenvolvimento nativo.
- Trabalho sensível a performance deve incluir medições de recursos e cache.
- Nenhum item pode modificar ou copiar a biblioteca física, que permanece externa e somente leitura.
- Mudanças de escopo são discutidas no GitHub e refletidas aqui por pull request.

## Decisões em aberto

- [Escopo e prioridades das próximas versões](https://github.com/AndersonTavares0/LearnVault/discussions/32)
- [Camada de análise e dados](https://github.com/AndersonTavares0/LearnVault/discussions/33)
- [Artefatos Windows além do instalador padrão (`.msi`, portátil)](https://github.com/AndersonTavares0/LearnVault/discussions/131)
