use crate::error::{Error, Result};

/// Grava no disco o conteúdo já serializado pelo front (CSV/JSON/SQL), no
/// caminho que o usuário escolheu no diálogo de salvar.
///
/// É um comando próprio em vez do plugin `fs` porque gravar num caminho
/// arbitrário exigiria abrir o escopo do sistema de arquivos inteiro para a
/// janela; aqui a única coisa que a janela ganha é este caminho.
#[tauri::command]
pub async fn write_export_file(path: String, contents: String) -> Result<()> {
    tokio::fs::write(&path, contents)
        .await
        .map_err(|e| Error::External(format!("Não foi possível gravar {path}: {e}")))
}
