# Wiki do LearnVault

LearnVault é uma biblioteca de estudos autocontida formada por um núcleo de domínio em Rust
(`crates/core`), um servidor Axum (`crates/server`) e uma SPA em Svelte 5 + Vite. Ela indexa
uma pasta de cursos existente sem modificar a biblioteca física. O desktop Tauri (Fase 4) já
tem fundação, catálogo, seleção de raiz e rescan, mas mídia completa, autenticação e janelas
reais ainda estão pendentes; a distribuição por plataforma (Fase 5) também. O serviço Go em
`services/go` permanece como implementação de referência e tem paridade de Study com o servidor
Rust e a SPA web.

Use esta Wiki para uma visão navegável do projeto. A documentação do repositório é canônica
e revisada por pull requests.

- [[Getting-Started]]
- [[Architecture]]
- [[Roadmap]]
- [[Development-and-Governance]]
- [[Operations]]
