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
- **Player** com velocidade independente para áudio e vídeo, seleção de mídia e legenda por aula,
  atalhos limitados ao foco e avanço automático opcional.
- **Capas** geradas a partir do título e capas manuais por upload do admin ou seletor nativo no
  desktop, com validação PNG/JPEG/WebP, limite de 4 MiB e gravação atômica fora das pastas de curso.
- **Distribuição** com instalador Windows NSIS, tarballs Linux do servidor e `SHA256SUMS` em cada
  release.

## Agora: uma rotina de estudo completa

- [ ] Página de detalhes do curso. A navegação anterior/próxima e o índice de aulas no player já
  existem.
- [ ] Player direto com links para legendas e materiais.
- [ ] Permissões de acesso a cursos para vários usuários locais.

## Em seguida: uma mesa de estudo

- [ ] Sidebar persistente no desktop e navegação mobile.
- [ ] "Continuar estudando" em destaque na tela inicial.
- [ ] Player como área principal de estudo, com índice de aulas.
- [ ] Acessibilidade: teclado, foco, contraste, movimento reduzido e leitores de tela.
- [ ] Manter a identidade visual original do projeto.

## Depois: uma biblioteca mais rica

- [ ] Capas por imagem dentro da pasta do curso.
- [ ] Cache de remux sob demanda para mídias incompatíveis.
- [ ] Busca no catálogo, nos materiais e nas notas.
- [ ] Favoritos e notas com timestamp.
- [ ] Navegação responsiva.

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
