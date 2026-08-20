use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tauri::ipc::{Channel, JavaScriptChannelId};
use tauri::{State, Webview};

use crate::adapters::MessageSink;
use crate::models::{
    EditableInfo, QueryMessage, QueryMessageKind, QueryOptions, QueryResult, RowEdit, RowInsert,
    StatementResult,
};
use crate::state::AppState;

use super::connect_adapter;

/// Teto de notices encaminhados por execução.
///
/// Cada mensagem vira um `eval` enfileirado no event loop da UI, e essa fila
/// não tem backpressure: um `RAISE NOTICE` dentro de um loop de cem mil
/// iterações afogaria a interface. Erros e a linha de conclusão não entram
/// nessa conta — são no máximo um punhado por execução e é justamente o que o
/// usuário mais precisa ver.
const MAX_STREAMED_NOTICES: usize = 2000;

/// Ponte entre o `MessageSink` dos adapters e o IPC do Tauri: cada mensagem
/// atravessa para o front assim que o banco a emite, sem esperar a query
/// terminar. Vive uma instância por execução, então o contador do teto já
/// nasce com o escopo certo.
struct ChannelSink {
    channel: Channel<QueryMessage>,
    notices_sent: AtomicUsize,
}

impl ChannelSink {
    fn new(channel: Channel<QueryMessage>) -> Self {
        Self {
            channel,
            notices_sent: AtomicUsize::new(0),
        }
    }

    fn send(&self, message: QueryMessage) {
        // Canal fechado (aba trocada ou fechada) não é erro: só não há mais
        // ninguém ouvindo do outro lado.
        let _ = self.channel.send(message);
    }
}

impl MessageSink for ChannelSink {
    fn push(&self, message: QueryMessage) {
        if matches!(
            message.kind,
            QueryMessageKind::Error | QueryMessageKind::Status
        ) {
            self.send(message);
            return;
        }

        match self.notices_sent.fetch_add(1, Ordering::Relaxed) {
            sent if sent < MAX_STREAMED_NOTICES => self.send(message),
            // Exatamente no teto: avisa uma única vez que dali em diante corta.
            sent if sent == MAX_STREAMED_NOTICES => {
                self.send(QueryMessage::status(format!(
                    "Log truncado em {MAX_STREAMED_NOTICES} mensagens; as demais desta execução foram descartadas"
                )));
            }
            _ => {}
        }
    }

    fn accepts_more(&self) -> bool {
        self.notices_sent.load(Ordering::Relaxed) <= MAX_STREAMED_NOTICES
    }
}

#[tauri::command]
pub async fn execute_query(
    webview: Webview,
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    query: String,
    options: Option<QueryOptions>,
    // `Channel<T>` não implementa Deserialize, então o argumento opcional
    // trafega como o id do canal e vira Channel aqui.
    messages: Option<JavaScriptChannelId>,
) -> Result<QueryResult, String> {
    let adapter = connect_adapter(&state, server_id, &database).await?;
    let sink = messages.map(|id| {
        Arc::new(ChannelSink::new(id.channel_on(webview))) as Arc<dyn MessageSink>
    });

    state
        .queries
        .execute_query(adapter, &query, options.unwrap_or_default(), sink)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn execute_statement(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    statement: String,
) -> Result<StatementResult, String> {
    let adapter = connect_adapter(&state, server_id, &database).await?;

    state
        .queries
        .execute_statement(adapter, &statement)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn apply_row_edits(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    editable: EditableInfo,
    edits: Vec<RowEdit>,
) -> Result<StatementResult, String> {
    let adapter = connect_adapter(&state, server_id, &database).await?;

    adapter
        .apply_row_edits(&editable, edits)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn insert_rows(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    editable: EditableInfo,
    rows: Vec<RowInsert>,
) -> Result<StatementResult, String> {
    let adapter = connect_adapter(&state, server_id, &database).await?;

    adapter
        .insert_rows(&editable, rows)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_rows(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    editable: EditableInfo,
    pk_values: Vec<Vec<Option<String>>>,
) -> Result<StatementResult, String> {
    let adapter = connect_adapter(&state, server_id, &database).await?;

    adapter
        .delete_rows(&editable, pk_values)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn execute_transaction(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    statements: Vec<String>,
) -> Result<Vec<StatementResult>, String> {
    let adapter = connect_adapter(&state, server_id, &database).await?;

    state
        .queries
        .execute_transaction(adapter, statements)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cancel_query(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    query_id: String,
) -> Result<(), String> {
    // Cancellation only makes sense against the connection running the query.
    // If it isn't open, there is nothing to cancel — don't create a pool.
    let Some(adapter) = state.connections.get_cached(server_id, &database) else {
        return Ok(());
    };

    adapter
        .cancel_query(&query_id)
        .await
        .map_err(|e| e.to_string())
}
