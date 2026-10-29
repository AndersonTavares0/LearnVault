# LearnVault — direções visuais

## Escopo e evidência

Os protótipos A/B/C foram iniciados como documentos independentes vazios e comparados em Biblioteca, Curso e Player, desktop e mobile. O Open Design MCP foi usado para criar/recuperar artefatos HTML; a integração observada não expõe canvas vetorial nativo. As capturas abaixo são renders locais desses artefatos em Chromium, com dados sintéticos. Elas comprovam a composição, não playback, persistência ou comportamento do produto.

A pesquisa verificável e o inventário funcional estão em [research.md](research.md). A avaliação ponderada e a decisão estão em [decision.md](decision.md). Os três conceitos iniciais não passaram o limiar; K foi escolhido após exploração posterior e crítica independente documentada.

## A — Biblioteca editorial

- **Navegação:** topbar persistente; Biblioteca, Curso e Player.
- **Composição:** cabeçalho editorial, capas proporcionais, curso tratado como publicação e baixa/média densidade.
- **Identidade:** display serifado, papel claro e tons terrosos.
- **Limite:** navegação de cards e controles do Player eram estáticos; outras telas e estados essenciais não estavam cobertos.
- **Evidência:** [Biblioteca desktop](screenshots/prototypes/concept-a-library-desktop-1440x900.png) · [Curso desktop](screenshots/prototypes/concept-a-course-desktop-1440x900.png) · [Player desktop](screenshots/prototypes/concept-a-player-desktop-1440x900.png) · [Biblioteca mobile](screenshots/prototypes/concept-a-library-mobile-390x844.png) · [Curso mobile](screenshots/prototypes/concept-a-course-mobile-390x844.png) · [Player mobile](screenshots/prototypes/concept-a-player-mobile-390x844.png).

## B — Workbench de estudo

- **Navegação:** rail vertical mais topbar compacta com busca e perfil.
- **Composição:** colunas simultâneas para catálogo, currículo, sessão e mídia; densidade alta, orientada à retomada.
- **Identidade:** sans funcional, paleta fria e acento verde.
- **Limite:** ações do Player eram simulações; no mobile, salvar e estudar ficaram ambíguos.
- **Evidência:** [Biblioteca desktop](screenshots/prototypes/concept-brefined-library-desktop-1440x900.png) · [Curso desktop](screenshots/prototypes/concept-brefined-course-desktop-1440x900.png) · [Player desktop](screenshots/prototypes/concept-brefined-player-desktop-1440x900.png) · [Biblioteca mobile](screenshots/prototypes/concept-brefined-library-mobile-390x844.png) · [Curso mobile](screenshots/prototypes/concept-brefined-course-mobile-390x844.png) · [Player mobile](screenshots/prototypes/concept-brefined-player-mobile-390x844.png).

## C — Atlas de aprendizagem

- **Navegação:** dock inferior; não usa sidebar nem topbar de destinos.
- **Composição:** mapa espacial de cursos e percurso visual de aulas; densidade baixa.
- **Identidade:** display condensado, grade cartográfica, verde profundo e papel marfim.
- **Limite:** a tela chamada Player era uma folha de aula sem timeline, retomada ou controles; a navegação por ponteiro mobile não foi comprovada.
- **Evidência:** [Biblioteca desktop](screenshots/prototypes/concept-c-library-desktop-1440x900.png) · [Curso desktop](screenshots/prototypes/concept-c-course-desktop-1440x900.png) · [Player desktop](screenshots/prototypes/concept-c-player-desktop-1440x900.png) · [Biblioteca mobile](screenshots/prototypes/concept-c-library-mobile-390x844.png) · [Curso mobile](screenshots/prototypes/concept-c-course-mobile-390x844.png) · [Player mobile](screenshots/prototypes/concept-c-player-mobile-390x844.png).

## K — fluxo linear de plataforma de cursos

K aproxima a navegação de padrões familiares de plataformas de aprendizado sem incorporar marketplace ou cloud: topbar compacta, retomada curta, catálogo em prateleiras, curso integral com objetivos e currículo e Player centrado em stage 16:9. A identidade usa azul cobalto funcional, saffron reservado a estado, superfícies navy e wordmark monocromático. A direção não reaproveita o layout dos conceitos anteriores.

- **Seleção:** a crítica independente da candidatura K v5 pontuou 4,05/5, acima do mínimo de 4,00 e sem dimensão abaixo de 3/5. Os pontos e limites estão em [decision.md](decision.md); não são resultado de teste com usuários.
- **Protótipo atual:** [K v7](prototypes/concept-k-course-library-v7.html); [K v6](prototypes/concept-k-course-library-v6.html) permanece como snapshot anterior.
- **Capturas da proposta:** [Biblioteca desktop](screenshots/k-v7-library-1440-final.png) · [Biblioteca mobile](screenshots/k-v7-library-390-final.png) · [Curso desktop](screenshots/k-v7-course-review-1440-dark.png) · [Curso mobile](screenshots/k-v7-course-review-390-dark.png) · [Player desktop](screenshots/k-v7-player-review-1440-dark.png) · [Player mobile](screenshots/k-v7-player-review-390-dark.png).
- **Implementação real:** telas finais, temas, viewports e estados estão em [`screenshots/product-pass/`](screenshots/product-pass/) e [final-review.md](final-review.md).

## Critério de conclusão

A direção está selecionada e implementada. O refinamento final encerrou quando a revisão das capturas não encontrou P0/P1 visual; limites que screenshots não provam — teste com pessoas, acessibilidade certificada, persistência com dados reais e playback real — continuam explicitamente fora das alegações.
