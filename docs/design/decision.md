# Decisão de direção visual

## Critério

Conceitos foram avaliados em screenshots, não em testes com usuários. Aprovação exige média ponderada ≥4,00/5 e nenhuma dimensão abaixo de 3/5:

| Dimensão | Peso |
|---|---:|
| Usabilidade e clareza | 30% |
| Hierarquia e arquitetura visual | 25% |
| Identidade e qualidade estética | 20% |
| Consistência e acessibilidade | 15% |
| Viabilidade técnica | 10% |

## A/B/C — três propostas independentes

| Conceito | Usabilidade | Hierarquia | Identidade | Consistência | Viabilidade | Média | Decisão |
|---|---:|---:|---:|---:|---:|---:|---|
| A — Biblioteca editorial | 2 | 3 | 4 | 3 | 3 | **2,90/5** | Rejeitado |
| B — Workbench de estudo | 4 | 4 | 3 | 3 | 4 | **3,65/5** | Rejeitado |
| C — Atlas de aprendizagem | 2,5 | 3 | 4 | 2 | 3 | **2,90/5** | Rejeitado |

B organizava melhor currículo, aula atual e sessão, mas o Player simulava ações, não persistia progresso e tornava “Salvar” ambíguo em mobile. A tinha identidade editorial, mas navegação/player incompletos e ausência de fluxos essenciais. C era visualmente distintivo, porém a metáfora espacial enfraquecia retomada, controles e navegação por ponteiro. Nenhum atendia o limiar; as propostas foram descartadas, não refinadas como solução final. Ver capturas A/B/C em [concepts.md](concepts.md).

## K — proposta escolhida

A direção K usa padrões reconhecíveis de plataforma de cursos com arquitetura local-first: Biblioteca em prateleiras com retomada curta, Curso como página integral com objetivos/progresso/currículo e Player centrado na mídia com currículo contextual. Evita marketplace, avaliações, matrícula, métricas decorativas e dependência de conta remota. Topbar compacta em vez de sidebar persistente; os destinos globais cabem em telas largas e reorganizam em mobile.

A crítica independente da candidatura K v5 pontuou:

| Dimensão | Peso | Nota | Parcela |
|---|---:|---:|---:|
| Usabilidade e clareza | 30% | 4,0 | 1,20 |
| Hierarquia e arquitetura visual | 25% | 4,2 | 1,05 |
| Identidade e qualidade estética | 20% | 4,1 | 0,82 |
| Consistência e acessibilidade | 15% | 3,5 | 0,525 |
| Viabilidade técnica | 10% | 4,5 | 0,45 |
| **Total** | **100%** |  | **4,045 ≈ 4,05/5** |

**Gate aprovado.** A avaliação foi especializada e cega a justificativas/código; não equivale a teste de usuários. A crítica da implementação real é uma etapa distinta: o ciclo final de screenshots não encontrou P0/P1, com limitações de conteúdo abaixo da dobra e gesture de carrossel explicitadas no [review final](final-review.md).

## Motivos da seleção

- Equilibra familiaridade com plataforma de cursos e identidade própria, sem importar affordances de marketplace que não pertencem à biblioteca local.
- Coloca próximo passo e retomada antes de métricas, preserva o contexto curso → aula → progresso e mantém o stage 16:9 como área principal do Player.
- Cobalto indica ação; saffron indica posição/estado. Wordmark LV é monocromático e discreto.
- Interface em pt-BR; títulos e descrições fornecidos pelos arquivos mantêm idioma original e recebem rótulo contextual.
- A implementação K v7 e o sistema de tokens estão em [DESIGN.md](DESIGN.md); evidências reais desktop/mobile, claro/escuro, vazios e falha de mídia estão em [`screenshots/product-pass/`](screenshots/product-pass/).

## Riscos e limites aceitos

- A nota de K veio da candidatura v5; versões posteriores ajustaram composição e conteúdo. A implementação final recebeu avaliação visual separada, sem novos P0/P1 aparentes.
- Screenshots não demonstram navegação completa com teclado/leitor de tela, playback real, persistência fora da fixture nem qualidade em todos os conteúdos do usuário.
- Não houve pesquisa com participantes nem certificação WCAG 2.2 AA. A cobertura automatizada e suas limitações constam em [final-review.md](final-review.md).

**Decisão:** manter K como direção final e não abrir outra exploração visual sem falha funcional ou bloqueio de acessibilidade confirmado.
