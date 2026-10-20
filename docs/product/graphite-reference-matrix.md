# Matriz Graphite — implementação e validação

A origem visual e o detalhamento por prancha estão em [`docs/design/graphite/README.md`](../design/graphite/README.md). Essa matriz mantém a correspondência entre boards, superfície Svelte, dados disponíveis e validação visual. Os HTML/CSS originais em `docs/design/graphite/` são fonte de verdade; a implementação do PR #274 e suas capturas são apenas evidência histórica.

| # | Prancha | Destino e componentes Svelte | Dados/decisão aplicada | Validação visual |
| --- | --- | --- | --- | --- |
| 01 | Meus cursos | `#/home` — `App.svelte`, `CourseCard.svelte`, `Pomodoro.svelte` | Catálogo, progresso recente e capas reais; sem duração ou percentual inferido. | Comparação desta branch pendente. |
| 02 | Curso | `#/course/:courseId` — `App.svelte`, `CatalogTree.svelte`, `LooseFiles.svelte` | Árvore/contagens e retomada reais; sem progresso percentual não fornecido. | Comparação desta branch pendente. |
| 03 | Player + currículo | `#/player/:courseId/:lessonPath` — `App.svelte`, `Player.svelte`, `CatalogTree.svelte`, `StudyNotes.svelte` | Streaming, posição, conclusão, preferências, materiais e notas por dono. | Comparação desta branch pendente. |
| 04 | Anotações | `#/notes` — `NotesIndex.svelte`, `notes-filter.ts` | Persistência de notas e filtros reais; a busca do topbar e do filtro lateral compartilham o texto. Dados demonstrativos da prancha permanecem distintos da fixture. | Comparação registrada nas capturas `docs/screenshots/graphite-reference/` e `graphite-implementation/`; divergência relevante: fixture com três notas de um curso, referência com três notas de cursos distintos e “8 notas”. |
| 05 | Preferências | `#/settings` — `App.svelte`, `PasswordField.svelte` | Tema, credenciais e atualização conforme transporte; Graphite é a única paleta ativa. | Comparação desta branch pendente. |
| 06 | Acesso | `#/login` HTTP — `App.svelte`, `PasswordField.svelte` | Autenticação real; desktop permanece sem login. | Captura final `docs/screenshots/graphite-implementation/06-login-browser-1568x795.png`: viewport cheia 1568×795, duas colunas preservadas e formulário centralizado na coluna direita. |
| 07 | Refinamentos | Decisões aplicadas a biblioteca/player; sem rota própria. | Continuidade, hierarquia e estados; conteúdo original está marcado como sintético. | Não é tela de produto; validar nos boards 01–06. |
| 08 | Pomodoro | Painel global — `Pomodoro.svelte` | Timer persistido do usuário/local; valores da prancha não são estado inicial. | Comparação desta branch pendente. |
| 09 | Estatísticas | `#/analytics` — `App.svelte` | Totais agregados reais; séries, heatmap e comparação sem fonte não são simulados. | Comparação desta branch pendente. |
| 10 | Dados do curso | Seção em `#/course/:courseId` — `App.svelte` | Contagens/retomada reais; anel, metas e métricas sem endpoint não são exibidos. | Limitação de dados documentada; sem comparação independente. |
| 11 | Decisões de analytics | Regras aplicadas em `#/analytics`; sem rota própria. | Não inferir atenção, série temporal, metas ou comparações. | Não é tela de produto; validar no board 09. |
| 12 | Descoberta | Prévia de importação — `ImportPreview.svelte`, `App.svelte` | Servidor deriva prévia somente leitura do caminho escolhido; recomendações são explicáveis. Desktop mantém seletor nativo. | Comparação desta branch pendente. |
| 13 | Materiais | Curso/player — `LooseFiles.svelte`, `MaterialPreview.svelte` | Apenas arquivos autorizados e tipos suportados para prévia/download. | Comparação desta branch pendente. |
| 14 | Decisões de catálogo | Modo admin no currículo — `CatalogTree.svelte`, `CatalogOrderControls.svelte`, `catalog-order.ts` | Reordenação e restauração da ordem natural usam APIs canônicas e não alteram arquivos-fonte. | Comparação desta branch pendente. |
| 15 | Importar e revisar | Fluxo admin — `ImportPreview.svelte`, `App.svelte`; desktop usa seletor nativo | Preview servidor sem gravação até confirmação; importação e rescan permanecem explícitos. | Comparação desta branch pendente. |

## Limites verificados no contrato

- A API de estudo não expõe duração de aula, série histórica diária nem agregados por curso; a interface não fabrica esses valores.
- A prancha 10 apresenta métricas demonstrativas que não podem ser calculadas pelo contrato atual. A tela de curso preserva contagens e última atividade disponíveis.
- A prévia de descoberta existe para o transporte HTTP; o desktop mantém o seletor nativo já aprovado e não apresenta uma prévia fictícia.
- Os estados, notas, cursos, arquivos e métricas das referências são exemplos. Fixtures de screenshot são temporárias, determinísticas e claramente separadas da aplicação.
- Capturas em Chromium, viewport CSS 1440×1000, DPR 1.25, tema Graphite claro; fixture HTTP com banco/biblioteca temporários. Diff local: 80,80% de pixels alterados; prancha 04 ainda não aprovada visualmente. O título documental da referência desloca o quadro inteiro; dados, navegação e timestamps também diferem. `compare_web_screenshots`/REA não estava disponível nesta sessão; diff em `docs/screenshots/graphite-implementation/board-04-notes-diff.png`.
