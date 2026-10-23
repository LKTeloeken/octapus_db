use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Preferências do app, gravadas no SQLite local (`app_settings`).
///
/// Cada campo vira uma linha chave/valor (JSON) na tabela: para uma
/// configuração nova basta um campo aqui (com o padrão no `Default`) e o tipo
/// espelho em `src/api/types/settings.types.ts` — linhas que faltam caem no
/// padrão e chaves que sumiram do struct são ignoradas.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    /// Recebe também as pre-releases do GitHub no auto-update (canal beta).
    pub beta_updates: bool,
    /// Pasta do `pg_dump`/`pg_restore`/`psql` escolhida pelo usuário; `None`
    /// = detectar sozinho (ver `services/backup/tools.rs`).
    pub pg_tools_dir: Option<String>,
    /// O assistente executa sozinho a consulta de leitura que gerou. Desligado,
    /// ela fica para revisão com o botão Executar (RN-13).
    pub assistant_auto_execute: bool,
    /// Atalhos que o usuário trocou: id da ação → teclas (`"Mod+K"`,
    /// `"g g"`). Só o que difere do padrão do front (`src/lib/keybindings`);
    /// lista vazia = ação sem atalho.
    pub keybindings: BTreeMap<String, Vec<String>>,
}

#[allow(clippy::derivable_impls)]
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            beta_updates: false,
            pg_tools_dir: None,
            assistant_auto_execute: false,
            keybindings: BTreeMap::new(),
        }
    }
}
