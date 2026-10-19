# Wiki do LearnVault

LearnVault usa um núcleo Rust (`crates/core`), um servidor Axum (`crates/server`), um desktop Tauri
offline (`crates/desktop`) e uma SPA Svelte 5 + Vite. Pastas de cursos são registradas
individualmente e permanecem somente leitura.

A migração do stack Rust foi concluída e o serviço Go removido. O importador de bancos Go legados
permanece no servidor Rust. O desktop tem janela real, IPC e reprodução local, sem autenticação nem
portas de rede. São publicados o instalador Windows NSIS e tarballs Linux do servidor. Flatpak/Linux
desktop e publicação OCI self-hosted continuam pendentes; veja [[Roadmap]] e [[Operations]].

Use esta Wiki para uma visão navegável do projeto. A documentação do repositório é canônica
e revisada por pull requests.

- [[Getting-Started]]
- [[Architecture]]
- [[Roadmap]]
- [[Requirements]]
- [[Development-and-Governance]]
- [[Operations]]
