---
name: LearnVault
description: Espaço de estudo auto-hospedado com descoberta clara, foco técnico e biblioteca intacta.
colors:
  dark:
    canvas: "#090d13"
    surface: "#111722"
    surface-raised: "#182230"
    ink: "#f1f5f9"
    muted: "#8e9ea9"
    faint: "#5e6f80"
    line: "#1c2736"
    line-strong: "#2a3b4e"
    accent: "#f59e0b"
    accent-soft: "#261e0e"
    focus: "#38bdf8"
    success: "#10b981"
    danger: "#f43f5e"
  light:
    canvas: "#f8fafc"
    surface: "#ffffff"
    surface-raised: "#f1f5f9"
    ink: "#0f172a"
    muted: "#64748b"
    faint: "#94a3b8"
    line: "#e2e8f0"
    line-strong: "#cbd5e1"
    accent: "#d97706"
    accent-soft: "#fef3c7"
    focus: "#0284c7"
    success: "#059669"
    danger: "#e11d48"
typography:
  display:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Inter", system-ui, sans-serif'
    fontWeight: 600
    letterSpacing: "-0.02em"
  body:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Inter", "Helvetica Neue", system-ui, sans-serif'
    fontWeight: 400
    lineHeight: 1.5
  mono:
    fontFamily: '"Cascadia Code", "Cascadia Mono", "JetBrains Mono", Consolas, "SFMono-Regular", monospace'
    fontWeight: 500
    letterSpacing: "0.02em"
rounded:
  sm: "6px"
  md: "8px"
  lg: "10px"
---

# Design System: LearnVault (Atelier Studio)

## Visão Geral

**Norte Criativo: "Atelier Studio — Technical Learning Workspace"**

O LearnVault é um espaço de aprendizado técnico e auto-hospedado projetado para programadores, pesquisadores e estudantes avançados. A experiência combina a clareza de retomada de estudos da Udemy com a densidade informativa e a ergonomia de workstation do Frontend Masters e Linear.

A interface prioriza o foco no conteúdo, a navegação ágil pelo currículo e a preservação estrita dos arquivos locais — que permanecem 100% somente leitura.

### Características Centrais
- **Course-First & Zero Friction:** Retomada imediata da última aula assistida via card de destaque com timestamp exato (`00:14:22`).
- **Paleta Obsidian & Âmbar:** Base escura profunda (`#090d13`), superfícies grafite estratificadas (`#111722` e `#182230`) e acento técnico em âmbar quente (`#f59e0b`), eliminando gradientes chamativos ou cores saturadas desconexas.
- **Tipografia Técnica e Escaneável:** Sistema sans limpo e responsivo para títulos e corpo, com numerais e metadados estruturados em monospace de largura fixa para leitura rápida de tempos e contagens.
- **Micro-interações Precisas:** Elevação sutil em hover (2px), anéis de foco de 2px acessíveis (WCAG 2.2 AA) e alvos de toque com área mínima de 44×44px.
- **Desktop Integrado:** Barra de título frameless customizada para desktop Tauri 2 com drag region nativo e controles integrados ao tema.

---

## Paleta de Cores

A paleta foi desenhada para conforto visual durante longas sessões de estudo noturno ou diurno, sem fadiga ocular.

### Modo Escuro (Padrão)
- **Canvas / Fundo:** `#090d13` (obsidian profundo, reduz emissão de luz azul)
- **Surface / Cartões:** `#111722` (grafite refinado com borda sutil)
- **Surface-2 / Destaque & Hover:** `#182230` (superfície elevada para seleção ativa)
- **Linhas / Divisórias:** `#1c2736` (fronteiras calmas de 1px) e `#2a3b4e` (contornos de inputs e controles)
- **Tipografia Primária (Ink):** `#f1f5f9` (alto contraste, taxa > 14:1)
- **Tipografia Secundária (Muted):** `#8e9ea9` (legibilidade técnica, taxa > 4.5:1)
- **Acento Âmbar:** `#f59e0b` (botões primários, destaque de progresso e XP)
- **Acento Cyan / Foco:** `#38bdf8` (anéis de foco interativo)
- **Status Sucesso:** `#10b981` (aulas concluídas, ofensiva ativa)
- **Status Alerta / Erro:** `#f43f5e` (mídia ausente ou erro de disco)

### Modo Claro
- **Canvas:** `#f8fafc` (slate muito suave, sem ofuscamento)
- **Surface:** `#ffffff` (branco puro estruturado por linhas de 1px)
- **Surface-2:** `#f1f5f9` (superfície de hover e estados selecionados)
- **Linhas:** `#e2e8f0` e `#cbd5e1`
- **Tipografia Primária:** `#0f172a` (slate escuro de alto contraste)
- **Tipografia Secundária:** `#64748b`
- **Acento Âmbar/Ocre:** `#d97706`
- **Acento Cyan / Foco:** `#0284c7`
- **Status Sucesso:** `#059669`
- **Status Alerta / Erro:** `#e11d48`

---

## Tipografia

| Papel | Família | Peso | Aplicação |
|---|---|---|---|
| **Display / Títulos** | Sistema Sans (`-apple-system, BlinkMacSystemFont, Segoe UI, Roboto, Inter`) | 600 / 700 | Título da biblioteca, cabeçalhos de curso, títulos de cards (`line-height: 1.25`) |
| **Corpo / Leitura** | Sistema Sans | 400 | Descrições, notas, mensagens de sistema (`line-height: 1.5`) |
| **Técnico / Mono** | Cascadia Code, JetBrains Mono, Consolas, monospace | 500 / 600 | Duração de vídeos, posições salvas, contadores de aula, atalhos de teclado |

---

## Componentes Chave

### 1. Brand Mark (Identidade)
- Hexágono de bordas suaves em 120° com monograma vazado "L" e "V" em precisão técnica.
- Inserido no topo esquerdo da barra de navegação com espaçamento proporcional (`gap: 10px`), sem parecer desconectado do nome LearnVault.

### 2. Biblioteca e Modos de Visualização
- **Continue Estudando:** Carrossel/linha horizontal de retomada imediata com miniatura 16:9, título da aula atual e botão de play rápido.
- **Seletor de Visualização:** Quatro modos ergonômicos persistidos em `localStorage`:
  - *Capas (Grade):* Miniaturas 16:9 com padrão generativo ou miniatura de vídeo extraída, indicador de "Em andamento" e badge de contagem.
  - *Lista:* Linhas com título expandido, duração estimada e detalhes de aulas.
  - *Compacto:* Máxima densidade para acervos com dezenas de cursos.
  - *Estante:* Visualização visual inspirada em lombadas de biblioteca técnica.
- **Busca Global:** Campo de busca com atalho `/` que filtra em tempo real por título do curso e nome das aulas.

### 3. Player e Workstation de Estudo
- **Split View:** Vídeo à esquerda/topo com proporção fixa 16:9; currículo navegável à direita com scroll independente.
- **Barra de Aula:**
  - Toggle de "Avançar automaticamente" para maratona de estudo.
  - Botão de alternância de conclusão ("Marcar como concluído" / "Concluído").
  - Botão "Retomar de MM:SS" quando houver progresso parcial registrado.
- **Atalhos de Teclado:** `Espaço` para play/pause, setas para seek ±10 segundos, indicados discretamente na interface.

### 4. Estatísticas e Evolução
- **Painel de Aprendizado:** Métricas de aulas concluídas, sessões de foco acumuladas e dias ativos na semana.
- **Evolução & Gamificação:** Nível do aluno, barra de progresso de XP, seletor de meta semanal configurável (1 a 7 dias) e ofensiva (streak) com histórico visual diário.
- **Sessões Pomodoro:** Relatório da semana com minutos de foco e taxa de conclusão.

---

## Regras de Integridade e Acessibilidade

1. **Biblioteca Somente Leitura:** A interface não oferece ações de mover, deletar ou alterar pastas físicas de cursos.
2. **WCAG 2.2 AA:** Todos os textos respeitam contraste mínimo de 4.5:1; controles interativos possuem área de clique de no mínimo 44×44px.
3. **Sem AI-Slop:** Ausência de gradientes multicoloridos artificiais, bordas duplas em cascata ou cartões aninhados desnecessários.
4. **Resiliência Offline:** O player lida graciosamente com mídias ausentes ou discos desconectados, explicando como restabelecer a conexão sem perder o histórico do aluno.
