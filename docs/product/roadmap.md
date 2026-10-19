# Roadmap do LearnVault

Este roadmap registra entregas concluídas e trabalho pendente. Os requisitos funcionais e não
funcionais ficam em [requirements.md](requirements.md); a arquitetura atual está em
[overview.md](../architecture/overview.md).

## Escopo do MVP (proposta)

O marco de migração do stack terminou com a remoção do serviço Go na Fase 5. Isso não significa
que todos os alvos de distribuição ou o backlog do produto estejam concluídos:

- **Entregue na migração:** núcleo Rust compartilhado, servidor Axum, SPA, desktop Tauri offline,
  remoção do serviço Go e importador explícito e limitado para bancos Go legados.
- **Distribuição disponível:** tarballs do servidor para Linux e instalador Windows NSIS.
- **Pendente:** Flatpak e validação em sandbox, publicação OCI/binário self-hosted, assinatura dos
  artefatos e demais itens listados em Distribuição. Eles continuam no roadmap e não bloqueiam o
  registro da migração do stack como concluída.

Esta seção distingue a migração concluída das entregas de produto e distribuição ainda pendentes.

## Corrente — reescrita em Rust

A arquitetura aprovada está no [ADR 0001](../architecture/adr/0001-reescrita-em-rust.md). As fases
0 a 5 da migração do stack estão concluídas; a Fase 5 removeu o serviço Go na PR #206. Esta
conclusão cobre o código e a CI do stack Rust, não cutover de dados reais nem todos os pacotes de
distribuição.

- [x] Fase 0 — ADR, arquitetura e roadmap atualizados
- [x] Fase 1 — `crates/core` com domínio, SQLite, catálogo, identidade e política de rescan
- [x] Fase 1 (complemento) — remoção explícita de mídia no núcleo
- [x] Fase 2 — `crates/server` com Axum, autenticação, sessões e mídia com Range
- [x] Fase 3 — SPA Svelte 5 + TypeScript + Vite com dois builds
- [x] Fase 3a — recusa segura de bancos Go legados e API explícita, limitada e backup-first para
  importar usuários, catálogo e estado; o comando legado `learnvault-server migrate-go` continua executando a operação
  sem iniciar o listener HTTP
- [x] Fase 4 — `crates/desktop` com Tauri 2, offline e sem portas de rede; janela real, IPC de cursos, catálogo e estudo, importação/remoção explícitas, rescan e protocolo local de mídia. O desktop não tem autenticação por decisão de arquitetura. A distribuição Linux/Flatpak e sua validação real permanecem no backlog.
  - [x] Fase 4 (fundação) — crate Tauri 2 no workspace, SPA embutida, capability deny-by-default, CSP explícita e janela `main` em `crates/desktop/tauri.conf.json`
  - [x] Fase 4 (IPC e biblioteca) — catálogo, seleção nativa, persistência atômica, importação/remoção e rescan explícito pelo núcleo compartilhado
  - [x] Fase 4 (mídia local) — protocolo por `media_id` opaco, `GET`/`HEAD`, faixas limitadas (`200`/`206`/`416`), allowlist e revalidação pelo núcleo; WebView2 usa o host local mapeado pelo runtime
  - [x] Fase 4 (Study desktop) — IPC reutiliza `learnvault-core::study`; o dono local é resolvido no Rust e o webview não envia `owner_id`
- [x] Fase 5 — distribuição por plataforma e remoção do serviço Go — executada em 2026-10-17 (PR #206), consolidando o Rust como backend único e eliminando `services/go`

## Histórico — implementação de referência em Go (removida)

Esta seção documenta o escopo original atendido pelo backend Go, mantido durante a transição
como referência funcional e integralmente substituído pelo ecossistema Rust:

O serviço e os arquivos de runtime Go foram removidos na PR #206. O suporte restante é um
importador Rust explícito para bancos SQLite legados e documentação histórica; o repositório não
contém backend Go executável.

- [x] Serviço Go com SPA SvelteKit embutida (substituído por `crates/server` e `apps/web`)
- [x] Bootstrap do administrador e sessões seguras (substituído por Argon2id e sessões Axum em `crates/server`)
- [x] Migrações SQLite e persistência do catálogo (substituído por `crates/core`)
- [x] Scan explícito e somente leitura da biblioteca (substituído por `crates/core`)
- [x] Catálogo autenticado e endpoint de mídia com suporte a Range (substituído por `crates/server` e protocolo Tauri)
- [x] Base reproduzível de deploy com Docker/Podman (substituído por `deploy/container/Containerfile` Rust)
- [x] CI, automação de releases e governança do repositório (atualizados para a stack nativa Rust)

## Entregue — ciclo de estudo compatível

As entregas mescladas #86, #88, #90 e #91 alinharam o contrato de Study entre o servidor Rust,
a SPA web e o serviço Go de referência, então removido: progresso, posição de retomada, conclusão e histórico
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
- [x] Preferências do player: velocidade independente para áudio/vídeo, seleção de mídia e legenda por aula, atalhos limitados ao foco e avanço automático opcional
- [x] Progresso individual e posição de retomada
- [x] Marcar aula como concluída e painel de histórico recente
- [ ] Permissões de acesso a cursos para múltiplos usuários locais

## Interface — mesa de estudo

- [ ] Estabelecer sidebar persistente no desktop e navegação mobile
- [ ] Priorizar “Continuar estudando” na tela inicial
- [ ] Construir o player como canvas principal de estudo com índice de aulas
- [ ] Adicionar capas manuais, estados vazios, carregamento e recuperação
  - [x] Capas geradas a partir do título e estados vazios da página inicial
  - [x] Capas manuais por upload do admin no servidor (`/api/admin/courses/{id}/cover`) e por seletor nativo de imagens no desktop (`desktop_import_course_cover`), com validação por bytes iniciais (PNG, JPEG ou WebP), limite de 4 MiB por capa, gravação atômica fora das pastas de curso, versão exposta no catálogo (`cover_version`) e entrega local por `learnvault-cover://` no desktop
- [ ] Validar navegação por teclado, foco, contraste, movimento reduzido e labels para leitores de tela
- [ ] Manter a identidade visual original do projeto

## Depois — experiência de biblioteca ampliada

- [ ] Capas de cursos manuais (por imagem da pasta)
- [ ] Cache de remux sob demanda para mídias incompatíveis
- [ ] Busca no catálogo, materiais e notas
- [ ] Favoritos e notas com timestamp
- [ ] Navegação responsiva e melhorias de acessibilidade

## Distribuição — portabilidade primeiro

A matriz aprovada está no [ADR 0001](../architecture/adr/0001-reescrita-em-rust.md). Esta frente
de distribuição é separada da migração do stack; os itens sem marca permanecem pendentes.

- [ ] Manter o núcleo compilável sem Docker ou Podman
- [x] Publicar instalador Windows `.exe` via NSIS (por usuário, pt-BR/en, não assinado)
- [ ] Publicar Flatpak como formato Linux preferencial do desktop — a seleção usa o backend
  oficial `xdg-portal` no Linux e o manifesto não concede filesystem amplo; ainda faltam build e
  smoke test reais no sandbox Flatpak, inclusive validação do acesso persistente ao diretório
  escolhido
- [ ] Publicar imagem OCI e binário nativo para instalação self-hosted (Compose OCI permanece opt-in local; não é artefato publicado)
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
