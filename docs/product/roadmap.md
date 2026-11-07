# Roadmap do LearnVault

**Para quem é:** qualquer pessoa que queira saber onde o produto está e para onde vai. Requisitos
detalhados ficam em [requirements.md](requirements.md); decisões técnicas em
[overview.md](../architecture/overview.md).

## Como ler

- **Entregue:** já está na `main` e disponível para uso.
- **Agora:** o foco atual do time.
- **Em seguida:** planejado, sem data fechada.
- **Depois:** trabalho previsto para os próximos ciclos.
- **Mais tarde:** ideias em avaliação, não compromisso.

Detalhes de implementação ficam nas issues e pull requests.

## Entregue

- **Núcleo Rust:** domínio, SQLite e catálogo em `crates/core`; servidor Axum em `crates/server`;
  interface web em Svelte 5 + Vite em `apps/web`. O serviço Go foi removido na PR #206; resta o
  importador `migrate-go` para bancos SQLite antigos.
- **Desktop offline (Tauri 2):** catálogo, seleção da biblioteca pelo seletor nativo, importação e
  remoção por curso, releitura explícita da biblioteca, mídia local por identificador e progresso de
  estudo por comandos internos (IPC). O desktop não tem autenticação e não abre portas de rede.
- **Regras de estudo** compartilhadas entre servidor, interface web e desktop (#86, #88, #90, #91).
- **Player** com seleção de mídia e legenda por aula, atalhos de teclado (`J`/`K`/`L`, `0-9`, `M`,
  `N`, `F`, `P`, `[`/`]`), velocidade persistida, Picture-in-Picture, linha do tempo interativa com
  marcações de anotações, avanço automático opcional, remux/transcodificação sob demanda via FFmpeg
  no servidor para formatos estendidos (`.mkv`, `.avi`, `.wmv`, `.ts`, HEVC, AC3) e abertura no
  reprodutor padrão do sistema no desktop.
- **Tela de estudo do curso:** player e lista de aulas na mesma tela, retomada da última aula
  assistida, conclusão rápida por aula na árvore do curso, módulo da aula atual aberto, materiais do
  módulo fora da playlist e ações administrativas em um menu do curso.
- **Anotações e cartões de revisão (Flashcards):** anotações com marcador de tempo clicável, busca
  global por anotações com filtro por curso e cartões de repetição espaçada por aula no Web e no
  desktop.
- **Foco, evolução e Bosque de Estudo 2.5D:** timer Pomodoro idempotente com fila offline, ofensiva
  de estudo, missões diárias, meta semanal e bosque isométrico 2.5D alimentado pelas sessões de foco
  concluídas.
- **Capas** geradas a partir do título, detecção automática de imagens locais na raiz do curso
  (`cover.*`, `folder.*`, `poster.*`, `thumb.*`) e capas manuais por upload do admin ou seletor
  nativo no desktop, com validação PNG/JPEG/WebP, limite de 4 MiB e gravação atômica fora das pastas
  de curso.
- **Backup e restauração administrativa:** download e restauração segura do banco SQLite com
  validação de integridade pela interface administrativa do servidor.
- **Distribuição** com instalador Windows NSIS, updater assinado (Ed25519), tarballs Linux do
  servidor e `SHA256SUMS` em cada release.

## Agora: uma rotina de estudo completa

- [x] Fluxo biblioteca → aula → foco → evolução consistente no Web e no desktop (#281).
- [x] "Continuar estudando" em destaque na tela inicial e filtros por estado e pasta de origem (#281, #297).
- [x] Anotações com timestamp, busca global em anotações e cartões de memorização (#281, #290, #297).
- [x] Suporte a formatos estendidos de mídia, transcodificação sob demanda no servidor e player do sistema no desktop (#298).
- [ ] Permissões de acesso a cursos para vários usuários locais.

## Em seguida: uma mesa de estudo

- [ ] Acessibilidade contínua: teclado, foco, contraste, movimento reduzido e leitores de tela.
- [ ] Favoritos de cursos e aulas.

## Depois: uma biblioteca mais rica

- [ ] Cache persistente de remux sob demanda para mídias incompatíveis.
- [ ] Busca textual dentro de materiais e legendas.

## Distribuição: portabilidade primeiro

- [ ] Manter o núcleo compilável sem Docker ou Podman.
- [ ] Publicar Flatpak como formato Linux preferencial do desktop. A seleção de diretório usa o
  backend `xdg-portal` e o manifesto não concede filesystem amplo; faltam build e smoke test reais
  no sandbox, incluindo acesso persistente ao diretório escolhido e reprodução de mídia.
- [ ] Publicar imagem OCI e binário nativo para instalação self-hosted.
- [ ] Assinar artefatos desktop quando houver credenciais de plataforma.
- [ ] Manter Docker Compose e Podman rootless/Quadlet como adaptadores opcionais de servidor.

## Mais tarde: recursos opcionais

- [ ] Whisper como capacidade separada e opcional.
- [ ] Indexação de transcrições e legendas pesquisáveis.
- [ ] Backup automatizado com retenção e verificação de restauração.
- [ ] Cliente mobile nativo quando a experiência web estiver estável.

## Regras de entrega

- Cada marco precisa de resultado claro para o usuário, testes, documentação e rollback.
- Alvos de distribuição são aditivos: uma falha de container não bloqueia o caminho nativo.
- Nenhum item modifica ou copia a biblioteca física, que permanece externa e somente leitura.
- Mudanças de escopo passam pelo GitHub e chegam aqui por pull request.

## Decisões em aberto

- [Escopo e prioridades das próximas versões](https://github.com/AndersonTavares0/LearnVault/discussions/32)
- [Camada de análise e dados](https://github.com/AndersonTavares0/LearnVault/discussions/33)
- [Artefatos Windows além do instalador padrão (`.msi`, portátil)](https://github.com/AndersonTavares0/LearnVault/discussions/131)

## Próximos passos

- [[Requirements]] para os requisitos por trás dos itens.
- [[Architecture]] para as decisões técnicas atuais.
- [[Home]] para voltar ao índice da Wiki.
