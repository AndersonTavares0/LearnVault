//! Domínio do LearnVault.
//!
//! Este crate concentra as regras de domínio compartilhadas pelo servidor (Axum) e pelo
//! aplicativo desktop (Tauri). Ele não conhece HTTP, Tauri, async ou formatos de resposta:
//! expõe operações e erros tipados e recebe, como parâmetros explícitos, a raiz da
//! biblioteca já resolvida pela integração de plataforma, o `library_id` e o `owner_id`.
//!
//! Estado atual: workspace, persistência SQLite com cadeia única de migrações (1.2),
//! scanner da biblioteca e identidade de mídia (1.3) e política de rescan (1.4). As
//! entregas seguintes desta fase adicionam catálogo derivado (1.5), resolução segura de
//! mídia (1.6) e estado de estudo (1.7).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod catalog;
pub mod db;
pub mod error;
pub mod ids;
pub mod media;
pub mod paths;
pub mod rescan;
pub mod scanner;

mod migrations;

pub use catalog::{Availability, Catalog, CatalogFile, CatalogNode, NodeRole, build_catalog};
pub use db::Database;
pub use error::{Error, Result};
pub use media::{OpenMedia, open_media};
pub use paths::KeyMode;
pub use rescan::{RescanSummary, apply_scan, rescan};
pub use scanner::{MediaKind, ScanOutcome, ScannedFile, scan};
