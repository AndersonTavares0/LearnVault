# Screenshots Graphite

Capturas sintéticas da interface do LearnVault após o redesign Graphite. Nenhuma
imagem contém cursos reais, credenciais ou dados pessoais. O catálogo e o progresso
usam uma biblioteca temporária de demonstração, criada apenas para validar composição,
estados e responsividade.

| Captura | Viewport | Tema |
| --- | --- | --- |
| `reference-home-light.png` | 1440×1000 | Claro, biblioteca com curso sintético e progresso |
| `reference-home-dark.png` | 1440×1000 | Escuro, mesma biblioteca |
| `reference-home-mobile-dark.png` | 390×844 | Escuro, mobile |
| `reference-course-light.png` | 1440×1000 | Claro, curso com hero, currículo e progresso |
| `reference-player-light.png` | 1440×1000 | Claro, player com painel de currículo |
| `reference-settings-light.png` | 1440×1000 | Claro, configurações em cards |
| `reference-notes-light.png` | 1440×1000 | Claro, anotações com indisponibilidade honesta |
| `reference-analytics-light.png` | 1440×1000 | Claro, métricas reais |
| `reference-analytics-dark.png` | 1440×1000 | Escuro, métricas reais |
| `reference-analytics-mobile-dark.png` | 390×844 | Escuro, métricas reais no mobile |
| `reference-pomodoro-light.png` | 1440×1000 | Claro, painel Pomodoro aberto sobre a biblioteca |
| `reference-login-light.png` | 1440×1000 | Claro, login dividido |
| `reference-login-mobile-dark.png` | 390×844 | Escuro, login mobile |

As imagens documentam o shell aprovado do Graphite: barra petroleum com navegação
horizontal, busca real, chip compacto de Pomodoro, biblioteca com "Continuar
estudando", hero de curso, player imersivo, configurações em cards e a rota de
estatísticas com apenas métricas reais. O player usa um arquivo de mídia vazio no
fixture sintético; por isso a captura mostra o estado real de mídia indisponível, sem
inventar um vídeo ou poster. Os temas claro e escuro reutilizam os mesmos tokens e
componentes.

O mapeamento de cada prancha para a implementação, incluindo limitações declaradas,
está em [`docs/product/graphite-reference-matrix.md`](../product/graphite-reference-matrix.md).

## Comparações com as pranchas aprovadas

As imagens `reference-board-*.png` são capturas diretas das pranchas HTML aprovadas, na
mesma resolução das capturas do aplicativo, para comparação lado a lado:

| Prancha aprovada | Aplicativo |
| --- | --- |
| `reference-board-01-library.png` | `reference-home-light.png` |
| `reference-board-02-course.png` | `reference-course-light.png` |
| `reference-board-03-player.png` | `reference-player-light.png` |
| `reference-board-04-notes.png` | `reference-notes-light.png` |
| `reference-board-05-settings.png` | `reference-settings-light.png` |
| `reference-board-06-login.png` | `reference-login-light.png` |
| `reference-board-09-analytics.png` | `reference-analytics-light.png` |

As demais pranchas são fluxos de importação/materiais já cobertos pelos componentes
reais ou documentos de decisão, conforme a matriz.
