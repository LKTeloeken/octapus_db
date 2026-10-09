use serde::{Deserialize, Serialize};

/// Preferências do app, gravadas no SQLite local (`app_settings`).
///
/// Cada campo vira uma linha chave/valor (JSON) na tabela: para uma
/// configuração nova basta um campo aqui (com o padrão no `Default`) e o tipo
/// espelho em `src/api/types/settings.types.ts` — linhas que faltam caem no
/// padrão e chaves que sumiram do struct são ignoradas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    /// Recebe também as pre-releases do GitHub no auto-update (canal beta).
    pub beta_updates: bool,
}

#[allow(clippy::derivable_impls)]
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            beta_updates: false,
        }
    }
}
