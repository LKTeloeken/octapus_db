//! Captura dos `RAISE NOTICE/WARNING/INFO` do Postgres.
//!
//! Notices não vêm no result set: o servidor os manda como `AsyncMessage` no
//! meio do stream da conexão, enquanto a query ainda roda. O `deadpool` padrão
//! dá `connection.await`, e o `Future` do `Connection` **descarta** todo
//! `AsyncMessage` — por isso aqui trocamos esse task por um que drena a conexão
//! com `poll_message` e roteia cada notice para quem estiver executando.
//!
//! O roteamento é por **backend PID**: cada conexão física descobre o seu ao
//! ser criada, e o executor inscreve o PID da conexão que pegou do pool
//! enquanto a query dura. Notices de um PID sem inscrição (o COUNT paralelo, o
//! recycle do pool, metadados) são descartados.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use deadpool_postgres::Connect;
use tokio::task::JoinHandle;
use tokio_postgres::error::{DbError, ErrorPosition};
use tokio_postgres::tls::{MakeTlsConnect, TlsConnect};
use tokio_postgres::{AsyncMessage, Client, Config as PgConfig, Socket};

use crate::adapters::MessageSink;
use crate::models::{now_ms, QueryMessage, QueryMessageKind};

/// Converte um `DbError` (notices e erros usam o mesmo formato no protocolo).
/// `force_error` marca a mensagem como erro mesmo que a severidade venha
/// localizada e não bata com nenhum rótulo conhecido.
pub fn message_from_db_error(err: &DbError, force_error: bool) -> QueryMessage {
    let severity = err.severity().to_string();
    let kind = if force_error {
        QueryMessageKind::Error
    } else {
        match severity.to_ascii_uppercase().as_str() {
            "WARNING" => QueryMessageKind::Warning,
            "ERROR" | "FATAL" | "PANIC" | "EXCEPTION" => QueryMessageKind::Error,
            "NOTICE" => QueryMessageKind::Notice,
            // INFO, LOG, DEBUG e severidades localizadas
            _ => QueryMessageKind::Info,
        }
    };

    QueryMessage {
        kind,
        severity,
        message: err.message().to_string(),
        detail: err.detail().map(str::to_string),
        hint: err.hint().map(str::to_string),
        context: err.where_().map(str::to_string),
        sql_state: Some(err.code().code().to_string()),
        position: err.position().map(|p| match p {
            ErrorPosition::Original(pos) => *pos,
            ErrorPosition::Internal { position, .. } => *position,
        }),
        timestamp_ms: now_ms(),
    }
}

/// Roteador de notices, compartilhado por todas as conexões de um adapter.
#[derive(Default)]
pub struct NoticeHub {
    sinks: Mutex<HashMap<i32, Arc<dyn MessageSink>>>,
}

impl NoticeHub {
    /// Passa a entregar os notices da conexão `pid` para `sink`. A inscrição
    /// dura enquanto o guard viver.
    pub fn subscribe(self: &Arc<Self>, pid: i32, sink: Arc<dyn MessageSink>) -> NoticeSub {
        self.sinks.lock().unwrap().insert(pid, sink);
        NoticeSub {
            hub: Arc::clone(self),
            pid,
        }
    }

    fn dispatch(&self, pid: i32, notice: &DbError) {
        // Solta o lock antes de entregar: o push atravessa o IPC do Tauri e a
        // task da conexão não pode ficar presa nele.
        let sink = self.sinks.lock().unwrap().get(&pid).cloned();

        // Checa antes de montar: numa função com RAISE em loop, construir a
        // mensagem só para descartá-la seria uma rajada de alocações à toa na
        // task da conexão.
        if let Some(sink) = sink {
            if sink.accepts_more() {
                sink.push(message_from_db_error(notice, false));
            }
        }
    }
}

/// Desinscreve o PID no `Drop`, cobrindo também erro e cancelamento.
pub struct NoticeSub {
    hub: Arc<NoticeHub>,
    pid: i32,
}

impl Drop for NoticeSub {
    fn drop(&mut self) {
        self.hub.sinks.lock().unwrap().remove(&self.pid);
    }
}

/// `Connect` do deadpool que, em vez de descartar os `AsyncMessage`, encaminha
/// os notices para o [`NoticeHub`].
pub struct NoticeConnect<T> {
    pub tls: T,
    pub hub: Arc<NoticeHub>,
}

impl<T> Connect for NoticeConnect<T>
where
    T: MakeTlsConnect<Socket> + Clone + Sync + Send + 'static,
    T::Stream: Sync + Send,
    T::TlsConnect: Sync + Send,
    <T::TlsConnect as TlsConnect<Socket>>::Future: Send,
{
    fn connect(
        &self,
        pg_config: &PgConfig,
    ) -> Pin<
        Box<dyn Future<Output = Result<(Client, JoinHandle<()>), tokio_postgres::Error>> + Send + '_>,
    > {
        let tls = self.tls.clone();
        let hub = Arc::clone(&self.hub);
        let pg_config = pg_config.clone();

        Box::pin(async move {
            let (client, mut connection) = pg_config.connect(tls).await?;

            // O PID só dá para descobrir com uma query, e a query só anda
            // depois que a task abaixo estiver drenando a conexão. Por isso o
            // valor é preenchido logo em seguida, e a task o relê a cada notice.
            let pid = Arc::new(AtomicI32::new(0));
            let task_pid = Arc::clone(&pid);

            let conn_task = tokio::spawn(async move {
                while let Some(message) =
                    std::future::poll_fn(|cx| connection.poll_message(cx)).await
                {
                    match message {
                        Ok(AsyncMessage::Notice(notice)) => {
                            dispatch_notice(&hub, &task_pid, &notice);
                        }
                        // LISTEN/NOTIFY: sem uso no app hoje.
                        Ok(_) => {}
                        // Conexão morreu; o pool a descarta no próximo checkout.
                        Err(_) => break,
                    }
                }
            });

            if let Ok(row) = client.query_one("SELECT pg_backend_pid()", &[]).await {
                pid.store(row.get::<_, i32>(0), Ordering::Relaxed);
            }

            Ok((client, conn_task))
        })
    }
}

fn dispatch_notice(hub: &NoticeHub, pid: &AtomicI32, notice: &DbError) {
    let pid = pid.load(Ordering::Relaxed);

    // pid 0 = notice chegou antes de sabermos quem somos (só na criação da
    // conexão, onde nenhuma query do usuário roda ainda).
    if pid != 0 {
        hub.dispatch(pid, notice);
    }
}
