# Wiki do LearnVault

LearnVault é uma biblioteca de estudos autocontida formada por um núcleo de domínio em Rust
(`crates/core`), um servidor Axum (`crates/server`) e uma SPA em Svelte 5 + Vite. Ela indexa
pastas de curso existentes, registradas individualmente, sem modificar a biblioteca física. O
desktop Tauri (Fase 4) já
tem fundação, IPC de cursos/catálogo, importação e remoção de cursos, rescan explícito de todos e
entrega de
 materiais, vídeo, áudio e arquivos de legenda pelo protocolo de mídia com faixas limitadas. O player
 desktop renderiza VTT; SRT, ASS e SSA são baixáveis, e o build HTTP pode converter SRT sob demanda.
 Autenticação, janelas reais e a distribuição por plataforma (Fase 5) foram consolidadas com Rust
 como backend único.

Use esta Wiki para uma visão navegável do projeto. A documentação do repositório é canônica
e revisada por pull requests.

- [[Getting-Started]]
- [[Architecture]]
- [[Roadmap]]
- [[Requirements]]
- [[Development-and-Governance]]
- [[Operations]]
