# Roadmap do LearnVault

Este roadmap registra entregas concluídas e trabalho pendente do produto. Os requisitos
funcionais e não funcionais ficam em [requirements.md](requirements.md); a arquitetura atual
está em [overview.md](../architecture/overview.md). Detalhes de implementação pertencem às
issues e pull requests.

## Entregue

- Backend consolidado em Rust: `crates/core` (domínio, SQLite, catálogo), `crates/server`
  (Axum) e a SPA Svelte 5 + Vite, conforme o [ADR 0001](../architecture/adr/0001-reescrita-em-rust.md).
  O serviço Go foi removido na PR #206 e o repositório não contém backend Go executável;
  resta apenas o importador explícito e limitado para bancos SQLite legados (`migrate-go`).
- Desktop Tauri 2 offline: catálogo, seleção da biblioteca pelo seletor nativo, importação e
  remoção por cursos individuais, rescan explícito, protocolo local de mídia por `media_id` e
  progresso de estudo por IPC reutilizando `learnvault-core::study` com o dono resolvido no
  Rust. O desktop não tem autenticação por decisão de arquitetura.
- Contrato de Study compartilhado entre servidor, SPA e desktop (#86, #88, #90, #91).
- Player com preferências: velocidade independente para áudio/vídeo, seleção de mídia e
  legenda por aula, atalhos limitados ao foco e avanço automático opcional.
- Capas de curso geradas a partir do título e capas manuais por upload do admin
  (`/api/admin/courses/{id}/cover`) e por seletor nativo no desktop, com validação
  PNG/JPEG/WebP, limite de 4 MiB e gravação atômica fora das pastas de curso.
- Distribuição publicada: instalador Windows `.exe` via NSIS (por usuário, pt-BR/en, não
  assinado), tarballs do servidor Linux e `SHA256SUMS` em cada release.

## Próximo — ciclo de estudo utilizável

- [ ] Página de detalhes do curso; a navegação anterior/próxima e o índice de aulas no player
  já existem
- [ ] Player direto com links de legendas e materiais
- [ ] Permissões de acesso a cursos para múltiplos usuários locais

## Interface — mesa de estudo

- [ ] Sidebar persistente no desktop e navegação mobile
- [ ] Priorizar “Continuar estudando” na tela inicial
- [ ] Player como canvas principal de estudo, com índice de aulas
- [ ] Acessibilidade: teclado, foco, contraste, movimento reduzido e leitores de tela
- [ ] Manter a identidade visual original do projeto

## Depois — experiência de biblioteca ampliada

- [ ] Capas por imagem dentro da pasta do curso
- [ ] Cache de remux sob demanda para mídias incompatíveis
- [ ] Busca no catálogo, materiais e notas
- [ ] Favoritos e notas com timestamp
- [ ] Navegação responsiva

## Distribuição — portabilidade primeiro

- [ ] Manter o núcleo compilável sem Docker ou Podman
- [ ] Publicar Flatpak como formato Linux preferencial do desktop — a seleção de diretório usa
  o backend `xdg-portal` e o manifesto não concede filesystem amplo; faltam build e smoke test
  reais no sandbox, incluindo acesso persistente ao diretório escolhido e reprodução de mídia
- [ ] Publicar imagem OCI e binário nativo para instalação self-hosted
- [ ] Assinar artefatos desktop quando houver credenciais de plataforma
- [ ] Manter Docker Compose e Podman rootless/Quadlet como adaptadores opcionais de servidor

## Mais tarde — inteligência e operações opcionais

- [ ] Whisper como capacidade separada e opcional
- [ ] Indexação de transcrições e legendas pesquisáveis
- [ ] Backup automatizado com retenção e verificação de restauração
- [ ] Cliente mobile nativo quando a experiência web estiver estável

## Regras de entrega

- Cada marco tem resultado claro para o usuário, testes, documentação e rollback documentado.
- Alvos de distribuição são aditivos: uma falha de container não bloqueia o caminho nativo.
- Nenhum item modifica ou copia a biblioteca física, que permanece externa e somente leitura.
- Mudanças de escopo são discutidas no GitHub e chegam aqui por pull request.

## Decisões em aberto

- [Escopo e prioridades das próximas versões](https://github.com/AndersonTavares0/LearnVault/discussions/32)
- [Camada de análise e dados](https://github.com/AndersonTavares0/LearnVault/discussions/33)
- [Artefatos Windows além do instalador padrão (`.msi`, portátil)](https://github.com/AndersonTavares0/LearnVault/discussions/131)
