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
- A implementação K v7 e o sistema de tokens estão formalizados em [DESIGN.md](DESIGN.md); as evidências do review final estão registradas em [final-review.md](final-review.md).

## Riscos e limites aceitos

- A nota de K veio da candidatura v5; versões posteriores ajustaram composição e conteúdo. A implementação final recebeu avaliação visual separada, sem novos P0/P1 aparentes.
- Screenshots não demonstram navegação completa com teclado/leitor de tela, playback real, persistência fora da fixture nem qualidade em todos os conteúdos do usuário.
- Não houve pesquisa com participantes nem certificação WCAG 2.2 AA. A cobertura automatizada e suas limitações constam em [final-review.md](final-review.md).

**Decisão:** manter K como direção final e não abrir outra exploração visual sem falha funcional ou bloqueio de acessibilidade confirmado.

## Post-K review (reopening, see #330)

Three maintainer-confirmed functional/accessibility failures satisfy the reopening clause above: #317 (layout overflow at ~1270px), #319 (language toggle unreachable at desktop widths), and #324 (27 interactive elements below the WCAG 44×44px target). This authorizes a composition revision within K — library, course page, and Player — using course platforms as a structural reference only, never as a source of code, assets, or pixel-level layout (clean-room policy, [`.github/CONTRIBUTING.md`](../../.github/CONTRIBUTING.md)). The original A/B/C/K evaluation above remains a historical record and is not redone.

## Direction L: course-platform redesign on React (reopened, see #347)

On 2026-11-09 the maintainer reopened the visual direction as part of the React migration
([ADR 0003](../architecture/adr/0003-react-frontend.md)). The reasons were recurring layout and target-size failures under K (#317, #319, #324, #321), CSS debt that blocks further fixes, and a
documented palette that no longer matches the shipped one. Direction L replaces K. The A/B/C/K
records above stay as history.

L keeps K's product stance (local-first, no marketplace, ratings, or enrollment). It changes:

- **Navigation.** A topbar holds the four global destinations: My learning, Progress, Notes, and the account menu. A curriculum sidebar appears only inside the course page and the player. Mobile uses a bottom tab bar. Research behind this choice: course platforms keep curriculum and completion next to the lesson, and Open edX is restoring the course sidebar it removed in 2017.
- **Palette.** We use ready-made shadcn/ui themes built on the Tailwind palette instead of custom tokens. The default is **Stone + Emerald**; users can switch to **Zinc + Teal** or **Zinc + Indigo** under Settings → Appearance. Pure blue and purple accents are avoided because they read as other platforms' brands.
- **Gamification.** Habit and visible progress come first: a forgiving streak, the weekly goal, per-course progress, and the next lesson one click away. XP, levels, and achievements stay on the Progress screen. The forest, island, and nursery are removed.
- **Accessibility and security** are written requirements. See [accessibility.md](accessibility.md) and [security.md](security.md).

The screen-level spec lives in [redesign.md](redesign.md). It becomes `DESIGN.md` at cutover.

**Gate.** L is scored with the same rubric and threshold (weighted ≥ 4.00, no dimension below 3). The scoring is done by a blind critique of the implemented React screens before cutover (#340), not of mockups. Until then L is the approved working direction, and `DESIGN.md` describes what currently ships.
