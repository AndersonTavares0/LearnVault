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
| `phase7-login-reference-1440x1000.png` | 1440×1000 | Captura atual direta de `06-login.html` |
| `phase7-login-spa-1440x1000-light.png` | 1440×1000 | SPA, Graphite claro, capturada via REA/Playwright |
| `phase7-login-side-by-side-1440x1000.png` | 2880×1000 | Referência à esquerda; SPA à direita |

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

### Comparação direta — login (prancha 06)

Capturas feitas na mesma sessão de validação, Chromium 155, 1440×1000, tema Graphite
claro. Diferenças observadas e tratadas:

- Removidos do login os elementos da versão anterior que não existem na prancha
  (hexágono e marca em caixa alta); markup agora usa a composição `.login`,
  `.login-identity` e `.login-card` da referência.
- Restaurados placeholder e espaçamento do campo de senha conforme a prancha; o reset
  de `.password-field` evita o gap extra herdado da folha legada.
- A referência contém um título documental acima de `.screen`, ausente da SPA por não
  fazer parte do produto. Por isso o quadro começa 33 px mais alto na captura da SPA;
  os elementos internos têm o mesmo posicionamento relativo ao quadro.
- A SPA mantém o botão acessível de mostrar/ocultar senha, necessário ao comportamento
  real de autenticação; esse controle não está desenhado na referência e é a diferença
  visual funcional restante. Erros e estado ocupado também permanecem funcionais.

Evidência REA/Playwright da captura clara: `ev_bb6e2696598102f2ce2d84bfb59f7211ce6e127ef8254da45b283d8589a985cd`.
