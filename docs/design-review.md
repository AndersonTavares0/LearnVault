# LearnVault — design review

## Resultado

**Direção K implementada; refinamento visual encerrado nesta revisão.** Não há P0/P1 confirmado nas capturas finais. Não serão abertas direções visuais novas sem falha funcional ou bloqueio de acessibilidade reproduzido.

A preferência por uma experiência familiar de plataforma de cursos orientou uma biblioteca course-first, sem marketplace: navegação global compacta, retomada curta, prateleiras de cursos, página de curso completa e Player centrado no conteúdo. A exploração comparou três propostas independentes — editorial, workbench e atlas — antes da escolha. A matriz e a justificativa estão em [`docs/design/decision.md`](design/decision.md); o público, a pesquisa e padrões observados estão em [`docs/design/research.md`](design/research.md).

## Exploração e decisão

- **A — editorial:** topbar, composição de publicação e capas; falhou em fluxos de navegação e Player.
- **B — workbench:** rail + topbar e alta densidade; organizou bem a sessão, mas perdeu clareza em mobile e simulava ações.
- **C — atlas:** dock e mapa de aprendizagem; diferenciou a identidade, mas não sustentou contexto e controles do Player.
- A/B/C receberam médias de **2,90**, **3,65** e **2,90/5**, abaixo do gate de 4,00. K atingiu **4,05/5** na crítica independente, com nenhuma dimensão abaixo de 3/5. A nota não é teste de usuários.
- A direção K v7 foi implementada nas seis superfícies: Biblioteca, Curso, Player, Estatísticas, Ajustes e Login. As decisões estão em [`docs/design/concepts.md`](design/concepts.md); tokens e componentes em [`docs/design/DESIGN.md`](design/DESIGN.md) e [`DESIGN.md`](../DESIGN.md).

## Crítica visual e correções finais

A revisão final serial das capturas reais identificou concorrência entre título/ação no Curso móvel, barra do Player alta, estado de conclusão separado dos controles, meta semanal distante do contexto e busca móvel estreita. Correções aplicadas:

- Reorganização do Curso mobile: título/contexto separados da ação principal, CTA de largura integral.
- Controles do Player agrupados em uma faixa compacta; conclusão e ação relacionadas permanecem próximas.
- Meta semanal movida antes do histórico de streak; métricas mobile reorganizadas para reduzir quebras e altura.
- Busca ampliada; ao pesquisar, a faixa de retomada alheia à consulta desaparece.
- Empty state usa título estável e mensagem explicativa separada; busca vazia preserva “Limpar busca” e não deixa cards/recents contraditórios.
- Aula sem arquivo recebe estado “Mídia ausente”; o Player explica recuperação, não oferece reprodução indisponível e preserva progresso.

O gate visual final não encontrou P0/P1. Partes abaixo da dobra nas telas Course/Estatísticas não foram avaliadas pelas capturas; o cartão seguinte da faixa de retomada aparece parcialmente cortado por rolagem horizontal, mas o gesto não foi testado. Isso é limite de evidência, não alegação de comportamento validado.

## Evidências

As capturas foram feitas no servidor real da aplicação, com biblioteca e mídia sintéticas isoladas; não foi aberto banco ou arquivo real do usuário.

- [`docs/design/screenshots/product-pass/`](design/screenshots/product-pass/): 40 capturas — Biblioteca, Curso, Player, Estatísticas e Ajustes, em 390/1280/1440/1920 px e temas claro/escuro; inclui pares antes/depois para cinco superfícies.
- Estados adicionais: [Login](design/screenshots/product-pass/final-login-390-dark.png), [Player sem mídia](design/screenshots/product-pass/state-player-missing-390-dark.png) e busca vazia em [390×844 escuro](design/screenshots/product-pass/state-library-empty-search-390-dark.png) e [1440×900 claro](design/screenshots/product-pass/state-library-empty-search-1440-light.png).
- Nas 40 capturas de matriz não houve overflow horizontal. Verificação direta do estado vazio: 0 cards, 0 recents não relacionados, ação “Limpar busca” visível, largura excedente 0 px.
- Player de smoke carregou mídia sintética com dimensões 640×360; isso não valida reprodução de conteúdo real.
- As capturas comparativas dos conceitos A/B/C e o baseline estão em [`docs/design/screenshots/`](design/screenshots/); protótipos HTML selecionados estão em [`docs/design/prototypes/`](design/prototypes/).

## Verificações

- `pnpm --filter learnvault-web check`: passou, 0 erros e 0 warnings.
- `pnpm --filter learnvault-web test`: passou, 22 arquivos / 313 testes.
- `pnpm exec playwright test tests/e2e/server.spec.ts --retries=0`: passou, 4/4 cenários.
- `pnpm --filter learnvault-web build` e `pnpm --filter learnvault-web build:desktop`: passaram.
- `git diff --check`: passou; Git apenas avisou conversão LF→CRLF em arquivos do checkout Windows.
- Detector visual final retornou `[]` para os alvos de UI.
- O check remoto do PR falhou somente no `pnpm audit --audit-level=low` por advisory baixo transitivo em `deepmerge` (`GHSA-gcqx-w48m-w2h4`), via `@sveltejs/vite-plugin-svelte`; o audit reporta sem versão corrigida. Não foi suprimido nem alterado como parte da mudança visual.

## Limites

Sem teste com usuários, certificação WCAG 2.2 AA, validação de mídia real, verificação de persistência em dados reais ou execução interativa nativa do binário desktop. A revisão especializada de screenshots não substitui esses testes.

## Refinamento posterior — 2026-10-29

### Crítica e decisão

Uma revisão visual independente da captura de 390 px apontou catálogo abaixo da primeira dobra útil e a faixa horizontal de retomada parcialmente cortada. A decisão foi preservar a ordem de retomada → catálogo e as capas compactadas já existentes no mobile; tornar Compacto o padrão esconderia a identidade das capas sem aproximar o início do catálogo. O refinamento reduziu apenas o espaçamento vertical da introdução, busca e seções em até 620 px.

A revisão de implementação encontrou uma regressão funcional confirmada: a regra `hover: none` que exibia as ações de capa era sobrescrita mais abaixo pela regra compartilhada de opacidade zero. O problema foi reproduzido no Playwright antes da correção (`opacity: 0`). A regra touch final agora mantém o controle visível e com alvo de 44×44 px; mouse e foco por teclado continuam com seus estados existentes.

### Evidência visual e interação

- Capturas reais do fluxo isolado em viewport de 390×844: [Biblioteca touch](design/screenshots/mobile-refinement/mobile-library-cover-actions-390-light.png) e [menu de capa aberto](design/screenshots/mobile-refinement/mobile-library-cover-actions-open-390-light.png).
- O fixture deste smoke contém um curso e não tem retomadas; portanto, as capturas novas **não** são uma comparação pixel a pixel com a Biblioteca anterior de 12 cursos. Elas evidenciam o espaçamento reduzido e o controle touch visível/acionável. O baseline completo permanece em [`product-pass/`](design/screenshots/product-pass/).
- O teste touch confirma `hover: none`, opacidade 1, alvo de 44 px e abertura do menu por toque.

### Verificação

- Falha reproduzida antes: Playwright esperava o controle touch visível e recebeu opacidade 0.
- Após a correção: `pnpm exec playwright test tests/e2e/server.spec.ts --retries=0` passou, 5/5 cenários.
- `pnpm --filter learnvault-web test`: 22 arquivos / 313 testes; `check`: 0 erros / 0 warnings; build web e build desktop passaram.
- Impeccable detector executado em `src/redesign.css`. O relatório contém avisos legados de tokens/radii/tipografia em outras partes da folha; nenhum aviso aponta para as novas regras no final do arquivo. O CSS anterior não foi reformatado nem alterado fora do escopo.

### Limitação de integração

O Open Design MCP não está exposto como ferramenta utilizável neste runtime; não houve chamada nem avaliação atribuída a ele nesta iteração. A crítica independente veio de dois agentes separados e a validação visual/interativa veio do Playwright. Nenhuma avaliação de Open Design foi inventada.

## Diagnóstico revisado do Open Design MCP — 2026-10-29

A nota anterior descrevia corretamente que o runtime desta conversa não forneceu uma ferramenta MCP de topo, mas concluía incorretamente que a integração não estava utilizável. A configuração do usuário está ativa e o bridge STDIO real respondeu; nenhuma configuração ou credencial foi alterada.

- `~/.omp/agent/mcp.json` contém `open-design` habilitado, transporte STDIO e timeout de 120 s. O processo do Open Design estava ativo e seu daemon local escutava na porta configurada.
- O handshake MCP `2025-11-25` passou pelo comando configurado. `tools/list` retornou 22 ferramentas; `list_projects` e `get_project` responderam para `learnvault-concept-k-course-library-fresh-20261029`.
- `get_active_context` informou que não havia projeto ativo naquele momento. O projeto pode ser selecionado explicitamente no argumento `project`, sem ativar ou alterar o canvas.
- `mcp://opendesign` consulta recursos MCP, não a lista de ferramentas; a ausência de recursos é normal neste servidor e não indica falha de conexão.
- Esta investigação não criou nem editou projetos ou artefatos.

## Refinamento da Biblioteca mobile — 2026-10-29

A revisão independente das capturas reais (390×844 claro/escuro e 1440×900 claro; Open Design run `0a06d008-94e5-4c67-9e39-871a311c7798`) não encontrou P0/P1. Registrou três P2 observáveis: metadados da capa mobile visualmente recuados, 8 px entre seletor e grade, e CTAs ocupando 310 px de uma coluna útil de 366 px.

Correções aplicadas:

- Capas mobile agora esticam o conteúdo pela largura do cartão; título e metadados alinham com a capa e ganham espaço para nomes longos.
- O intervalo entre seletor e grade sobe para 14 px.
- Importar e Atualizar formam duas colunas iguais na largura disponível; abaixo de 340 px, padding e fonte diminuem, com os dois rótulos verificados em viewport de 320 px.
- Importar mantém hierarquia secundária quando há retomada e volta a ser primário sem retomada ou durante uma busca.

O E2E verifica o alinhamento do título (≤16 px), o intervalo (≥12 px), igualdade das colunas e texto sem overflow em viewports de 320/390 px; também aguarda o fim das transições CSS antes de medir contraste para não ler valores intermediários. Capturas após a correção: [Biblioteca mobile clara](design/screenshots/mobile-density-iteration/mobile-library-refinement-390-light.png), [escura](design/screenshots/mobile-density-iteration/mobile-library-refinement-390-dark.png) e [desktop](design/screenshots/mobile-density-iteration/redesign-biblioteca-compacta-desktop.png). São fixtures sintéticas de um curso; a captura mobile é full-page, viewport de 390×844.

### Segunda crítica visual

Reavaliação serial após as correções no Open Design run `3b1428ea-0000-48c3-8fb7-8d715b79987d`: aprovada, sem P0/P1. A segunda rodada confirmou CTAs em duas colunas de 179 px com 8 px de intervalo (antes 148/154 px), 14 px entre seletor e grade (antes 8 px) e conteúdo da capa alinhado com recuo interno de 11 px. A ressalva P2 sobre títulos longos foi tratada como hipótese a verificar, não como defeito confirmado.

Um E2E com título português longo confirmou o tratamento atual: texto serifado de 15 px, limite de duas linhas com reticências e nenhuma largura excedente no cartão; ao abrir o curso, o título integral aparece no cabeçalho. A captura real de 390×844 está em [Biblioteca mobile com título longo](design/screenshots/mobile-density-iteration/mobile-library-long-title-390-light.png). A abreviação na capa é deliberada para manter a grade compacta; o conteúdo completo permanece acessível no fluxo de Curso.

### Verificação final

- `pnpm --filter learnvault-web check`: 0 erros, 0 warnings.
- `pnpm --filter learnvault-web test`: 22 arquivos, 313 testes.
- `pnpm --filter learnvault-web build` e `pnpm --filter learnvault-web build:desktop`: passaram.
- `pnpm exec playwright test tests/e2e/server.spec.ts --retries=0`: 6/6 cenários; inclui CTA sem overflow em 320/390 px, controles touch, título longo sem overflow e acesso ao título integral na tela de Curso.
- As duas rodadas de crítica independente não encontraram P0/P1; os pontos P2 mensuráveis foram corrigidos e a hipótese sobre nomes longos foi reproduzida, mitigada e verificada.
- A validação não cobre dados pessoais reais nem equivale a teste com usuários.

## Terceira rodada de crítica e faixa de retomada — 2026-10-29

Uma terceira leitura independente das mesmas capturas de 390×844 propôs catorze achados. Sete não se sustentaram na verificação do código:

- **Contraste do seletor e das bordas no tema escuro:** os tokens `--blue-soft`, `--blue`, `--ink` e `--line` já entregam cerca de 13:1 para o texto selecionado e 7:1 para a borda; substituí-los por hex fixos teria saído da paleta. Nenhuma alteração aplicada.
- **Escala tipográfica achatada:** o `h1` da página é 32 px e os `h2` de seção são 24 px no mobile; existe degrau. Nenhuma alteração aplicada.
- **Título truncado em duas linhas:** é decisão registrada, com o título integral no cabeçalho do Curso e cobertura E2E. Mantido.
- **Paridade de CTAs:** a alternância primário/secundário é intencional quando há retomada e é verificada por E2E. Mantida.
- **Badge de status:** “Em andamento” e “Catálogo atualizado.” têm semânticas diferentes (estado persistente e aviso transitório); unificá-las seria abstração sem demanda.
- **Busca fixa ao rolar:** o topbar quebra em duas linhas a ≤820 px com alturas mínimas diferentes, então qualquer `top` fixo esconde o campo atrás dele. Sem JS ou reorganização de layout, a alternativa seria uma sobreposição incorreta. Descartada.
- **Esqueletos de carregamento:** o catálogo é carregado uma vez; não há caminho assíncrono que justifique shimmer.

O achado que correspondia a uma lacuna real já documentada era a faixa de retomada no mobile: a rolagem horizontal nunca havia sido exercida. A correção foi `scroll-snap-type: x mandatory` (antes `proximity`), escolhida por comportamento determinístico entre Chromium e WebKit, e não por ganho medido no Chromium.

O E2E novo usa fixture com duas aulas e quatro retomadas, faz um gesto real de roda horizontal sobre a faixa, confirma `scrollLeft > 0`, verifica que o repouso casa com o alinhamento de referência do primeiro cartão (≤1 px) e que cada cartão, ao ser alcançado, permanece dentro das bordas da faixa. Para provar que o teste não é vazio, a faixa foi temporariamente trocada para `overflow-x: visible`: o cenário falhou com `scrollLeft` em 0, como esperado.

Limites desta verificação: o gesto é roda de mouse, não toque real; `proximity` também passa no mesmo desenho de teste, portanto a escolha por `mandatory` não tem evidência de superioridade no Chromium, apenas de determinismo entre engines. Captura: [faixa de retomada em 390×844](design/screenshots/mobile-density-iteration/mobile-library-resume-carousel-390-light.png).

### Verificação

- `pnpm --filter learnvault-web check`: 0 erros, 0 warnings.
- `pnpm --filter learnvault-web test`: 22 arquivos, 313 testes.
- `pnpm --filter learnvault-web build` e `build:desktop`: passaram.
- `pnpm exec playwright test tests/e2e/server.spec.ts --retries=0`: 7/7 cenários.
