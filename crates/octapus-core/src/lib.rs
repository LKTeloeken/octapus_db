//! Núcleo do Octapus, sem Tauri: adapters dos bancos, catálogo de metadados,
//! storage local (SQLite + vault), serviços e o assistente. O app desktop
//! (`src-tauri`) e o binário de terminal (`crates/octapus-cli`) rodam por cima
//! daqui — ver BACKEND.md.

pub mod adapters;
pub mod assistant;
pub mod catalog;
pub mod error;
pub mod logging;
pub mod models;
pub mod paths;
pub mod services;
pub mod state;
pub mod storage;

#[cfg(test)]
mod perf;

pub use error::{Error, Result};
