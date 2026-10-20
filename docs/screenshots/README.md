# Screenshots Graphite

`reference-*` e `phase7-*` são capturas históricas da implementação anterior; não provam fidelidade da branch atual. Nenhuma dessas imagens foi apagada.

As capturas atuais desta branch estão em `graphite-reference/` e
`graphite-implementation/`. Os dados da implementação usam banco, biblioteca e sessão
HTTP temporários; dados sintéticos existem apenas na fixture e nas capturas.

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
| `graphite-implementation/06-login-browser-1568x795.png` | 1568×795 | Escuro, login full-viewport com painel claro de autenticação |
| `graphite-implementation/06-login-browser-1440x900.png` | 1440×900 | Escuro, desktop refinado |
| `graphite-implementation/06-login-browser-390x844.png` | 390×844 | Escuro, mobile responsivo sem overflow |
| `phase7-login-spa-1440x1000-light.png` | 1440×1000 | SPA, Graphite claro, capturada via REA/Playwright |
| `phase7-login-side-by-side-1440x1000.png` | 2880×1000 | Referência à esquerda; SPA à direita |
| `phase7-board01-reference-1440x1000.png` | 1440×1000 | Captura direta atual de `01-library.html` |
| `phase7-board01-spa-1440x1000-light.png` | 1440×1000 | SPA isolada, Graphite claro, capturada via REA/Playwright |
| `phase7-board01-side-by-side-1440x1000.png` | 2880×1000 | Prancha 01 à esquerda; SPA à direita |

As imagens documentam o shell aprovado do Graphite: barra petroleum com navegação
horizontal, busca real, chip compacto de Pomodoro, biblioteca com "Continuar
estudando", hero de curso, player imersivo, configurações em cards e a rota de
estatísticas com apenas métricas reais. O player usa um arquivo de mídia vazio no
fixture sintético; por isso a captura mostra o estado real de mídia indisponível, sem
inventar um vídeo ou poster. Os temas claro e escuro reutilizam os mesmos tokens e
componentes.

O mapeamento de cada prancha para a implementação, incluindo limitações declaradas,
está em [`docs/product/graphite-reference-matrix.md`](../product/graphite-reference-matrix.md).
## Comparação atual — anotações (prancha 04)

`graphite-reference/board-04-notes-1440x1000.png` e
`graphite-implementation/board-04-notes-1440x1000.png` foram capturadas em Chromium,
viewport CSS 1440×1000, DPR 1.25, tema Graphite claro. A fixture isolada contém três
notas sintéticas de um curso; a prancha demonstra três notas de cursos diferentes e exibe
“8 notas”. Nenhum dado sintético foi inserido na aplicação real.

A comparação local por diferença de pixels reportou 80,80% de pixels alterados. Esse
valor não é critério isolado de fidelidade: o título documental da prancha desloca todo
o quadro; também diferem dados, navegação e timestamps. A captura evidencia divergências,
não aprova a prancha. O rótulo do quadro foi removido da implementação por não fazer parte
da tela do produto. `compare_web_screenshots`/REA não estava disponível nesta sessão; a
comparação foi feita com `graphite-implementation/board-04-notes-diff.png`.

Pendências visuais da prancha 04: alinhar a composição e separar as diferenças de dados
da demonstração original, usando fixture correspondente quando isso não altera
informações reais da aplicação.

## Comparação atual — acesso (prancha 06)

As capturas atuais foram feitas no navegador ativo em viewport CSS equivalente ao
arquivo de cada captura, DPR 1.25 e tema Graphite escuro:

- `06-login-browser-1568x795.png`: validação da resolução usada na regressão visual;
- `06-login-browser-1440x900.png`: desktop solicitado;
- `06-login-browser-390x844.png`: mobile solicitado.

Em todas as capturas, a verificação DOM mediu `.login-board` e `.login` começando em
`x=0`, `y=0`, ocupando a viewport completa. Nos testes 1568×795 e 390×844,
`document.documentElement.scrollWidth`/`scrollHeight` foram respectivamente
`1568×795` e `390×844`; não houve overflow horizontal. O desktop mantém a divisão
institucional escura à esquerda e autenticação clara à direita, com o formulário
centralizado na segunda coluna. O mobile empilha a identidade sobre a autenticação.

`compare_web_screenshots`/REA não estava disponível nesta sessão; a validação foi feita
com viewport equivalente, capturas do navegador e medições de geometria/estilos
computados. As capturas históricas de 1440×1000 abaixo permanecem evidência de
referência, não uma aprovação da implementação atual.

## Evidência histórica da implementação anterior

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

### Comparação histórica — login (prancha 06; não valida a branch atual)

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

### Comparação histórica — biblioteca (prancha 01; não valida a branch atual)

As capturas do login e da biblioteca, seus diffs e IDs de evidência foram produzidos na
implementação anterior. A busca de topbar foi corrigida naquela branch; os dados e as
diferenças registradas ali não descrevem nem validam a implementação atual. As evidências
`ev_bb6e2696598102f2ce2d84bfb59f7211ce6e127ef8254da45b283d8589a985cd`,
`ev_fe8fc703929bcd7c2b231fdbc2b7434870b0aba7d70fa12ac1a2bc2b58b01ff1` e
`ev_f18737fd2fb79a14927533a0e248f7a68ad509724e7322b9767ef06d835916a6` são históricas.
