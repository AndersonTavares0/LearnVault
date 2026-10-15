# Roadmap do LearnVault

Este roadmap é o resumo durável do produto e é orientado a resultados. Detalhes de
implementação pertencem às Issues e pull requests relacionadas.

## Atual — fundação e catálogo

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
- [ ] Manter identidade visual original: azul-petróleo, tinta escura, papel frio e sinal lima

## Depois — experiência de biblioteca ampliada

- [ ] Capas de cursos geradas e manuais
- [ ] Cache de remux sob demanda para mídias incompatíveis
- [ ] Busca no catálogo, materiais e notas
- [ ] Favoritos e notas com timestamp
- [ ] Navegação responsiva e melhorias de acessibilidade

## Distribuição — portabilidade primeiro

- [ ] Manter o núcleo compilável sem Docker ou Podman
- [ ] Publicar artefatos Windows `.exe` versionados
- [ ] Empacotar instaladores Windows `.msi`
- [ ] Publicar pacotes Linux `.deb`
- [ ] Publicar pacotes Linux `.rpm`
- [ ] Publicar bundle e metadados Flatpak
- [ ] Assinar artefatos desktop quando houver credenciais de plataforma
- [ ] Gerar checksums e metadados de proveniência para cada release
- [ ] Manter Docker Compose e Podman rootless/Quadlet como adaptadores opcionais de servidor

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
