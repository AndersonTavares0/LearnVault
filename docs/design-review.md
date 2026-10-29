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
