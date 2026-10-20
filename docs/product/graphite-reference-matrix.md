# Matriz da referência Graphite — 15 pranchas

Esta matriz liga cada prancha aprovada em `learnvault-referencia-15-telas.zip` à
implementação real da SPA. Ela registra o que foi entregue, o que é uma decisão de
produto documentada e o que permanece tecnicamente pendente. Nenhuma linha usa dados
fictícios na aplicação: as fixtures sintéticas existem apenas para screenshots.

| # | Prancha | Implementação | Estado |
| --- | --- | --- | --- |
| 01 | Meus cursos | `#/home`: topbar, busca, Pomodoro, “Continuar estudando”, filtros, grid e ações da biblioteca | Entregue |
| 02 | Curso | `#/course/<id>`: hero petroleum, estatísticas reais, currículo expansível, aula atual e caixa de retomada | Entregue, sem percentual ou duração inventados |
| 03 | Player + currículo | `#/player/...`: topbar imersiva, palco, abas reais Currículo/Anotações/Materiais, recolher/restaurar | Entregue; controles nativos do elemento de mídia |
| 04 | Anotações | `#/notes`: layout de filtros e lista, com indisponibilidade explícita | Estrutura entregue; persistência futura |
| 05 | Preferências | `#/settings`: tema real, paleta Graphite e conta/segurança | Entregue; somente a paleta Graphite |
| 06 | Acesso | `#/login`: composição dividida da prancha, com autenticação real | Comparado diretamente em Chromium 155/1440×1000; evidência e divergências em `docs/screenshots/README.md` |
| 07 | Refinamentos | Decisões de foco, painel, tipografia e estados | Documento de decisão; aplicado onde há contrato |
| 08 | Pomodoro | Chip compacto no topbar e painel sob demanda com transporte persistente | Entregue |
| 09 | Estatísticas | `#/analytics`: faixa de métricas reais e indisponibilidade honesta de séries | Entregue parcialmente; sem histórico diário |
| 10 | Curso · dados | Hero e caixa de progresso do curso com contagens e última atividade reais | Entregue; sem gráfico por curso |
| 11 | Dados · decisões | Regras para não simular histórico, metas ou comparações | Documento de decisão |
| 12 | Descoberta | `ImportPreview`: árvore física, estados e sugestões explicáveis antes da confirmação | Entregue |
| 13 | Materiais | `LooseFiles` e `MaterialPreview` no curso, no player e na aba Materiais | Entregue |
| 14 | Catálogo · decisões | Ordenação manual de módulos/aulas com restauração da ordem automática | Entregue |
| 15 | Importar e revisar | Fluxo de caminho, prévia somente leitura e confirmação explícita | Entregue |

## Limitações declaradas

- Não há persistência de anotações por aula; a tela e a aba do player mostram essa
  indisponibilidade em vez de conteúdo fictício.
- O contrato atual de analytics expõe totais agregados, não séries diárias nem
  progresso por curso; nenhum gráfico é estimado.
- Apenas a paleta Graphite está implementada. Nord e Dracula não são exibidos como
  opções disponíveis.
- O painel do player pode ser recolhido e restaurado, mas a largura não é persistida
  entre sessões.
- O player usa os controles nativos do elemento de mídia; scrubber e transporte
  desenhados na prancha não foram duplicados.
- O catálogo real não fornece duração por aula nem percentual por curso; por isso
  essas informações não aparecem na interface.

## Evidências visuais

As capturas ficam em `docs/screenshots/` e são geradas com biblioteca temporária,
progresso sintético e sessão local. Consulte o `README.md` dessa pasta para a lista
atualizada de viewports e temas.
