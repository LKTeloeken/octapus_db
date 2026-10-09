use std::sync::Arc;
use std::time::Duration;

use deadpool_postgres::{
    Config, Hook, HookError, Manager, ManagerConfig, Pool, RecyclingMethod, Runtime,
};
use postgres_native_tls::MakeTlsConnector;
use tokio_postgres::tls::{MakeTlsConnect, TlsConnect};
use tokio_postgres::{CancelToken, Client, NoTls, Socket};

use crate::error::{Error, Result};
use crate::models::Server;

use super::notices::{NoticeConnect, NoticeHub};

const POOL_MAX_SIZE: usize = 16;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Configuração de conexão comum ao pool e à conexão dedicada do catálogo.
fn base_config(server: &Server, database: &str) -> Config {
    let mut cfg = Config::new();

    if let Some(uri) = server.connection_uri.as_deref() {
        // The URI provides host/port/user/password; the selected database
        // still takes precedence so one server can open several databases.
        cfg.url = Some(uri.to_string());
    } else {
        cfg.host = Some(server.host.clone());
        cfg.port = Some(server.port);
        cfg.user = Some(server.username.clone());
        cfg.password = Some(server.password.clone());
    }
    cfg.dbname = Some(database.to_string());
    cfg.connect_timeout = Some(CONNECT_TIMEOUT);

    // Garante a entrega dos RAISE NOTICE mesmo em servidores cujo default de
    // client_min_messages seja mais restritivo que o do Postgres.
    cfg.options = Some("-c client_min_messages=notice".to_string());

    // Keepalive TCP: evita que firewalls/NAT derrubem conexões ociosas em
    // silêncio enquanto o app fica aberto sem uso.
    cfg.keepalives = Some(true);
    cfg.keepalives_idle = Some(Duration::from_secs(60));
    cfg
}

/// sslmode=require semantics: encrypt the connection without verifying the
/// certificate (common for self-signed DB servers)
fn tls_connector() -> Result<MakeTlsConnector> {
    let connector = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()
        .map_err(|e| Error::Connection(format!("TLS setup failed: {e}")))?;
    Ok(MakeTlsConnector::new(connector))
}

pub fn create_pool(server: &Server, database: &str, hub: &Arc<NoticeHub>) -> Result<Pool> {
    let mut cfg = base_config(server, database);

    cfg.manager = Some(ManagerConfig {
        // Fast: só confere se o socket fechou. Quem testa de verdade é o hook
        // `verify_if_idle`, e só as conexões paradas — o Verified testava
        // todas, a uma ida e volta por comando.
        recycling_method: RecyclingMethod::Fast,
    });

    cfg.pool = Some(deadpool_postgres::PoolConfig {
        max_size: POOL_MAX_SIZE,
        timeouts: deadpool_postgres::Timeouts {
            wait: Some(Duration::from_secs(30)),
            create: Some(CONNECT_TIMEOUT),
            recycle: Some(Duration::from_secs(5)),
        },
        ..Default::default()
    });

    if server.ssl_enabled {
        build_pool(&cfg, tls_connector()?, hub)
    } else {
        build_pool(&cfg, NoTls, hub)
    }
}

/// Cancela a query em andamento numa conexão dedicada (manda o cancel request
/// do protocolo por uma conexão nova, com o mesmo TLS da original).
#[derive(Clone)]
pub enum CancelHandle {
    Plain(CancelToken),
    Tls(CancelToken, MakeTlsConnector),
}

impl CancelHandle {
    pub async fn cancel(&self) -> Result<()> {
        let result = match self {
            Self::Plain(token) => token.cancel_query(NoTls).await,
            Self::Tls(token, tls) => token.cancel_query(tls.clone()).await,
        };
        result.map_err(|e| Error::Connection(format!("Failed to cancel query: {e}")))
    }
}

/// Uma conexão fora do pool, só do catálogo: a introspecção não disputa as 16
/// conexões com as queries do usuário, e aparece como `octapus_db catalog` no
/// `pg_stat_activity` (o DBA sabe de onde vem).
pub async fn connect_dedicated(server: &Server, database: &str) -> Result<(Client, CancelHandle)> {
    let mut cfg = base_config(server, database);
    cfg.application_name = Some("octapus_db catalog".to_string());
    let pg_config = cfg
        .get_pg_config()
        .map_err(|e| Error::Connection(e.to_string()))?;

    let connect_error = |e: tokio_postgres::Error| Error::Connection(e.to_string());
    if server.ssl_enabled {
        let tls = tls_connector()?;
        let (client, connection) = pg_config.connect(tls.clone()).await.map_err(connect_error)?;
        tokio::spawn(connection);
        let cancel = CancelHandle::Tls(client.cancel_token(), tls);
        Ok((client, cancel))
    } else {
        let (client, connection) = pg_config.connect(NoTls).await.map_err(connect_error)?;
        tokio::spawn(connection);
        let cancel = CancelHandle::Plain(client.cancel_token());
        Ok((client, cancel))
    }
}

/// Uma conexão do pool parada há mais que isto é testada antes de voltar ao uso.
const VERIFY_IDLE_AFTER: Duration = Duration::from_secs(30);

/// Antes de devolver uma conexão do pool: se ficou parada mais que
/// [`VERIFY_IDLE_AFTER`], um `simple_query("")` confirma que está viva
/// (firewall/NAT derrubam conexões ociosas em silêncio, e uma conexão morta
/// faria o comando falhar na mão do usuário). Se falhar ou demorar, o deadpool
/// descarta a conexão e entrega outra (ou cria uma). As usadas há pouco voltam
/// direto, sem ida e volta.
fn verify_if_idle() -> Hook {
    Hook::async_fn(|client, metrics| {
        Box::pin(async move {
            if metrics.last_used() < VERIFY_IDLE_AFTER {
                return Ok(());
            }
            match tokio::time::timeout(CONNECT_TIMEOUT, client.simple_query("")).await {
                Ok(Ok(_)) => Ok(()),
                _ => Err(HookError::Message("idle connection failed verification".into())),
            }
        })
    })
}

/// Monta o pool com o `Connect` próprio em vez de `Config::create_pool`: é o
/// único jeito de ficar com os notices, que o connect padrão do deadpool joga
/// fora (ver `notices.rs`).
fn build_pool<T>(cfg: &Config, tls: T, hub: &Arc<NoticeHub>) -> Result<Pool>
where
    T: MakeTlsConnect<Socket> + Clone + Sync + Send + 'static,
    T::Stream: Sync + Send,
    T::TlsConnect: Sync + Send,
    <T::TlsConnect as TlsConnect<Socket>>::Future: Send,
{
    let pg_config = cfg
        .get_pg_config()
        .map_err(|e| Error::Connection(e.to_string()))?;

    let manager = Manager::from_connect(
        pg_config,
        NoticeConnect {
            tls,
            hub: Arc::clone(hub),
        },
        cfg.get_manager_config(),
    );

    Pool::builder(manager)
        .config(cfg.get_pool_config())
        .runtime(Runtime::Tokio1)
        .pre_recycle(verify_if_idle())
        .build()
        .map_err(|e| Error::Connection(e.to_string()))
}
