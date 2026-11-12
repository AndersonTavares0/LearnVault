# LearnVault — direções visuais

> **Superseded in part (2026-11-11).** The palette, themes, navigation and gamification described here were replaced by the direction in [`DESIGN.md`](../../DESIGN.md) and [`PRODUCT.md`](../../PRODUCT.md): light and dark themes, brown and amber, Playfair Display and DM Sans, top navigation, no forest. Where this file names a palette, a theme or a layout, those two files win. It is kept as history and for the research and the accessibility and security rules, which still apply.

## Escopo e evidência

Os protótipos A/B/C foram iniciados como documentos independentes vazios e comparados em Biblioteca, Curso e Player, desktop e mobile. O Open Design MCP foi usado para criar/recuperar artefatos HTML; a integração observada não expõe canvas vetorial nativo. As capturas abaixo são renders locais desses artefatos em Chromium, com dados sintéticos. Elas comprovam a composição, não playback, persistência ou comportamento do produto.

A pesquisa verificável e o inventário funcional estão em [research.md](research.md). A avaliação ponderada e a decisão estão em [decision.md](decision.md). Os três conceitos iniciais não passaram o limiar; K foi escolhido após exploração posterior e crítica independente documentada.

## A — Biblioteca editorial

- **Navegação:** topbar persistente; Biblioteca, Curso e Player.
- **Composição:** cabeçalho editorial, capas proporcionais, curso tratado como publicação e baixa/média densidade.
- **Identidade:** display serifado, papel claro e tons terrosos.
- **Limite:** navegação de cards e controles do Player eram estáticos; outras telas e estados essenciais não estavam cobertos.
- **Resultado:** Rejeitado na avaliação comparativa.

## B — Workbench de estudo

- **Navegação:** rail vertical mais topbar compacta com busca e perfil.
- **Composição:** colunas simultâneas para catálogo, currículo, sessão e mídia; densidade alta, orientada à retomada.
- **Identidade:** sans funcional, paleta fria e acento verde.
- **Limite:** ações do Player eram simulações; no mobile, salvar e estudar ficaram ambíguos.
- **Resultado:** Rejeitado na avaliação comparativa.

## C — Atlas de aprendizagem

- **Navegação:** dock inferior; não usa sidebar nem topbar de destinos.
- **Composição:** mapa espacial de cursos e percurso visual de aulas; densidade baixa.
- **Identidade:** display condensado, grade cartográfica, verde profundo e papel marfim.
- **Limite:** a tela chamada Player era uma folha de aula sem timeline, retomada ou controles; a navegação por ponteiro mobile não foi comprovada.
- **Resultado:** Rejeitado na avaliação comparativa.

## K — fluxo linear de plataforma de cursos

K aproxima a navegação de padrões familiares de plataformas de aprendizado sem incorporar marketplace ou cloud: topbar compacta, retomada curta, catálogo em prateleiras, curso integral com objetivos e currículo e Player centrado em stage 16:9. A identidade usa azul cobalto funcional, saffron reservado a estado, superfícies navy e wordmark monocromático. A direção não reaproveita o layout dos conceitos anteriores.

- **Seleção:** a crítica independente da candidatura K v5 pontuou 4,05/5, acima do mínimo de 4,00 e sem dimensão abaixo de 3/5. Os pontos e limites estão em [decision.md](decision.md); não são resultado de teste com usuários.
- **Implementação real:** telas finais, temas, viewports e estados estão formalizados em [DESIGN.md](DESIGN.md) e [final-review.md](final-review.md).

## Critério de conclusão

A direção está selecionada e implementada. O refinamento final encerrou quando a revisão das capturas não encontrou P0/P1 visual; limites que screenshots não provam — teste com pessoas, acessibilidade certificada, persistência com dados reais e playback real — continuam explicitamente fora das alegações.
