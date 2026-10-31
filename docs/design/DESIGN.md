# LearnVault — design system de produção

A fonte normativa completa está em [`/DESIGN.md`](../../DESIGN.md), escrita no formato DESIGN.md do Impeccable. Este documento fixa a aplicação da direção K escolhida no produto real.

## Direção concluída

**A private course shelf.** O LearnVault usa descoberta de cursos e retomada como eixo da experiência: topbar compacta, busca ampla, linha curta de continuidade, catálogo de cursos e detalhe com currículo. O player dá prioridade ao estágio de mídia e move o currículo para uma coluna contextual no desktop e uma seção posterior no mobile.

## Tokens implementados

| Papel | Claro | Escuro | Uso |
| --- | --- | --- | --- |
| Canvas | `#f4f6f9` | `#0b1220` | Fundo de página |
| Surface | `#ffffff` | `#111b2b` | Campos, painéis, shelves |
| Raised | `#edf1f6` | `#182438` | Seleção, navegação ativa, hover |
| Ink | `#152238` | `#f5f7fb` | Texto principal |
| Muted | `#56657a` | `#a8b5c7` | Metadata e orientação |
| Cobalt | `#1d5fd1` | `#1d5fd1` | Próxima ação e seleção |
| Saffron | `#8a5708` | `#e8b43c` | Posição salva e estado em andamento |
| Line | `#d7dee8` | `#2a3a52` | Divisão estrutural |

A implementação reside em `apps/web/src/redesign.css`; o plugin daisyUI é carregado em `apps/web/src/ui-components.css` com prefixo `lvui-`, evitando colisão com o CSS do produto. Botões, inputs, joins, selects, ranges e checkboxes usam primitives daisyUI; a camada K fornece forma, ritmo e contexto.

## Tipografia e composição

- Headings de entrada e títulos de curso: Georgia/Cambria, peso 400, escala fluida.
- Interface, metadata e controles: Segoe UI Variable/Segoe UI, peso 400–700.
- Medidas, caminhos e estados técnicos: Cascadia Mono/Consolas, somente onde existe informação técnica.
- Desktop: conteúdo central com limite de 1240–1380px e topbar sticky compacta.
- Mobile (`≤820px`): navegação passa para segunda linha, cards viram linhas ou uma coluna, currículo perde sticky e breadcrumbs reduzem apenas contexto redundante.
- Estados vazios, erro, loading e mídia ausente continuam explícitos e acionáveis.

## Componentes e interação

- `lvui-btn lvui-btn-primary`: uma ação dominante por grupo.
- `lvui-input`, `lvui-select`, `lvui-range`, `lvui-checkbox`: controls semânticos com foco visível.
- `.lv-panel`: superfície tonal sem sombra em repouso.
- `.lv-course-card`, `.lv-resume-card`: conteúdo acionável, capas com razão controlada e títulos com clamp seguro.
- `.player-container`: stage 16:9/áudio com um único estado primário de progresso; mídia ausente desativa reprodução e explica recuperação.
- Cobalt é funcional; saffron é estado. Nenhum dos dois é decoração.
- O monograma LV e o wordmark são discretos, monocromáticos e não competem com as capas.

## Estados e qualidade

A validação real usa o servidor Rust com biblioteca temporária, sem modificar dados pessoais: login, biblioteca, retomada, detalhe, player, mídia ausente, capa/thumbnail, estatísticas, Pomodoro, temas claro/escuro, viewport mobile e desktop. Evidências ficam em `docs/design/screenshots/implementation-e2e/`.

Contraste e overflow são verificados pelo E2E existente; foco, navegação sem mouse e estados de erro usam elementos semânticos e `:focus-visible`. O sistema reduz transições sob `prefers-reduced-motion` e não usa gradientes, blur decorativo ou cartões aninhados como estrutura.
