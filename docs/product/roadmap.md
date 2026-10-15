# Roadmap do LearnVault

Este roadmap é o resumo durável do produto e é orientado a resultados. Detalhes de
implementação pertencem às Issues e pull requests relacionadas.

## Corrente — reescrita em Rust

A arquitetura aprovada está no [ADR 0001](../architecture/adr/0001-reescrita-em-rust.md).
A execução avança por fases, com um pull request por fase:

- [x] Fase 0 — ADR, arquitetura e roadmap atualizados
- [x] Fase 1 — `crates/core` com domínio, SQLite, catálogo, identidade e política de rescan
- [x] Fase 1 (complemento) — remoção explícita de mídia no núcleo
- [x] Fase 2 — `crates/server` com Axum, autenticação, sessões e mídia com Range
- [x] Fase 3 — SPA Svelte 5 + TypeScript + Vite com dois builds
- [x] Fase 3a — recusa segura de bancos Go legados; conversão automática permanece fora do escopo
- [ ] Fase 4 — `crates/desktop` com Tauri 2, offline e sem portas de rede
- [ ] Fase 5 — distribuição por plataforma e remoção do serviço Go

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

## Próximo — ciclo de estudo utilizável

- [ ] Detalhes do curso e navegação de aulas
- [ ] Player direto com links de legendas e materiais
- [ ] Progresso individual e posição de retomada
- [ ] Marcar aula como concluída e painel de histórico recente
- [ ] Permissões de acesso a cursos para múltiplos usuários locais

## Interface — mesa de estudo

- [ ] Estabelecer sidebar persistente no desktop e navegação mobile
- [ ] Priorizar “Continuar estudando” na tela inicial
- [ ] Construir o player como canvas principal de estudo com índice de aulas
- [ ] Adicionar capas, estados vazios, carregamento e recuperação
- [ ] Validar navegação por teclado, foco, contraste, movimento reduzido e labels para leitores de tela
- [ ] Manter a identidade visual original do projeto

## Depois — experiência de biblioteca ampliada

- [ ] Capas de cursos geradas e manuais
- [ ] Cache de remux sob demanda para mídias incompatíveis
- [ ] Busca no catálogo, materiais e notas
- [ ] Favoritos e notas com timestamp
- [ ] Navegação responsiva e melhorias de acessibilidade

## Distribuição — portabilidade primeiro

A matriz aprovada está no [ADR 0001](../architecture/adr/0001-reescrita-em-rust.md).

- [ ] Manter o núcleo compilável sem Docker ou Podman
- [ ] Publicar instalador Windows `.exe` via NSIS
- [ ] Publicar Flatpak como formato Linux preferencial do desktop
- [ ] Publicar imagem OCI e binário nativo para instalação self-hosted
- [ ] Assinar artefatos desktop quando houver credenciais de plataforma
- [ ] Gerar checksums e metadados de proveniência para cada release
- [ ] Manter Docker Compose e Podman rootless/Quadlet como adaptadores opcionais de servidor

O Flatpak permanece sujeito à validação técnica de permissões, acesso persistente às bibliotecas
e reprodução de mídia antes de ser tratado como formato definitivo.

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
