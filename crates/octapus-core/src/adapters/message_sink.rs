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

/// Teto padrão de notices encaminhados por execução (o mesmo do
/// `ChannelSink` do app Tauri).
pub const MAX_STREAMED_NOTICES: usize = 2000;

/// Um [`MessageSink`] com teto de notices por execução, por cima de outro.
///
/// Um `RAISE NOTICE` dentro de um loop de cem mil iterações afogaria
/// qualquer interface. Erros e a linha de conclusão não entram nessa conta —
/// são no máximo um punhado por execução e é justamente o que o usuário mais
/// precisa ver. No teto, sai uma única mensagem avisando que dali em diante
/// corta. Uma instância por execução: o contador nasce com o escopo certo.
pub struct CappedSink<S> {
    inner: S,
    limit: usize,
    notices_sent: std::sync::atomic::AtomicUsize,
}

impl<S: MessageSink> CappedSink<S> {
    pub fn new(inner: S) -> Self {
        Self::with_limit(inner, MAX_STREAMED_NOTICES)
    }

    pub fn with_limit(inner: S, limit: usize) -> Self {
        Self {
            inner,
            limit,
            notices_sent: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

impl<S: MessageSink> MessageSink for CappedSink<S> {
    fn push(&self, message: QueryMessage) {
        use crate::models::QueryMessageKind;
        use std::sync::atomic::Ordering;

        if matches!(message.kind, QueryMessageKind::Error | QueryMessageKind::Status) {
            self.inner.push(message);
            return;
        }

        match self.notices_sent.fetch_add(1, Ordering::Relaxed) {
            sent if sent < self.limit => self.inner.push(message),
            // Exatamente no teto: avisa uma única vez que dali em diante corta.
            sent if sent == self.limit => {
                self.inner.push(QueryMessage::status(format!(
                    "Log truncado em {} mensagens; as demais desta execução foram descartadas",
                    self.limit
                )));
            }
            _ => {}
        }
    }

    fn accepts_more(&self) -> bool {
        self.notices_sent.load(std::sync::atomic::Ordering::Relaxed) <= self.limit && self.inner.accepts_more()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::QueryMessageKind;
    use parking_lot::Mutex;

    #[derive(Default)]
    struct Collect(Mutex<Vec<QueryMessage>>);

    impl MessageSink for &Collect {
        fn push(&self, message: QueryMessage) {
            self.0.lock().push(message);
        }
    }

    fn notice(text: &str) -> QueryMessage {
        QueryMessage {
            kind: QueryMessageKind::Notice,
            severity: "NOTICE".into(),
            ..QueryMessage::status(text)
        }
    }

    #[test]
    fn notices_stop_at_the_limit_with_a_single_warning() {
        let collected = Collect::default();
        let sink = CappedSink::with_limit(&collected, 3);
        for i in 0..10 {
            assert_eq!(sink.accepts_more(), i <= 3, "{i}");
            sink.push(notice(&format!("n{i}")));
        }
        // Erro e conclusão passam mesmo depois do teto
        sink.push(QueryMessage::status("fim"));

        let messages = collected.0.lock();
        let texts: Vec<&str> = messages.iter().map(|m| m.message.as_str()).collect();
        assert_eq!(texts[..3], ["n0", "n1", "n2"]);
        assert!(texts[3].starts_with("Log truncado em 3"), "{}", texts[3]);
        assert_eq!(texts[4], "fim");
        assert_eq!(texts.len(), 5);
    }
}
