/// Grava no disco o conteúdo já serializado pelo front (CSV/JSON/SQL), no
/// caminho que o usuário escolheu no diálogo de salvar.
///
/// É um comando próprio em vez do plugin `fs` porque gravar num caminho
/// arbitrário exigiria abrir o escopo do sistema de arquivos inteiro para a
/// janela; aqui a única coisa que a janela ganha é este caminho.
#[tauri::command]
pub async fn write_export_file(path: String, contents: String) -> Result<(), String> {
    tokio::fs::write(&path, contents)
        .await
        .map_err(|e| format!("Não foi possível gravar {path}: {e}"))
}
