use serde::Serialize;

/// O `octapus` de terminal nesta máquina, para a tela de Preferências.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliStatus {
    /// Há um binário publicado para este sistema e arquitetura
    pub supported: bool,
    /// O arquivo já está em `path`
    pub installed: bool,
    /// Onde o `octapus` fica (ou ficaria) instalado
    pub path: String,
    /// A pasta de `path` está no `PATH`: dá para chamar só `octapus`
    pub on_path: bool,
}
