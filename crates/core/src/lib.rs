//! Domínio do LearnVault.
//!
//! Este crate concentra as regras de domínio compartilhadas pelo servidor (Axum) e pelo
//! aplicativo desktop (Tauri). Ele não conhece HTTP, Tauri, async ou formatos de resposta:
//! expõe operações e erros tipados e recebe, como parâmetros explícitos, a raiz da
//! biblioteca já resolvida pela integração de plataforma, o `library_id` e o `owner_id`.
//!
//! Estado atual: workspace e persistência SQLite com cadeia única de migrações (1.2).
//! As entregas seguintes desta fase adicionam scanner e identidade de mídia (1.3),
//! política de rescan (1.4), catálogo derivado (1.5), resolução segura de mídia (1.6)
//! e estado de estudo (1.7).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod db;
pub mod error;

mod migrations;

pub use db::Database;
pub use error::{Error, Result};
