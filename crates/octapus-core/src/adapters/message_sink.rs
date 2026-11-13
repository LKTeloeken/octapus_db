use crate::models::QueryMessage;

/// Canal de saída para as mensagens que o banco emite fora do result set.
///
/// Vive aqui, e não em `commands/`, para que a camada de adapters não precise
/// conhecer o Tauri: quem implementa de verdade é o `ChannelSink` de
/// `commands/queries.rs`, por cima de um `tauri::ipc::Channel`.
///
/// As implementações são chamadas de dentro da task da conexão, então `push`
/// não pode bloquear.
pub trait MessageSink: Send + Sync {
    fn push(&self, message: QueryMessage);

    /// Permite ao produtor pular a montagem da mensagem quando o sink já não
    /// vai aproveitá-la (teto de mensagens atingido). Default: sempre aceita.
    fn accepts_more(&self) -> bool {
        true
    }
}
