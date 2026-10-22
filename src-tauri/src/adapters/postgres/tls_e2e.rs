//! TLS de ponta a ponta contra um Postgres com certificado de CA própria,
//! emitido para `db.internal` (não para `localhost`). Sobe com
//! `scripts/tls-e2e.sh up`; depois:
//!
//! `OCTAPUS_TLS_CA=/tmp/octapus-tls-e2e/ca.crt cargo test --lib postgres::tls_e2e -- --ignored`

use crate::models::{DatabaseType, Server, SslMode};

use super::pool::connect_dedicated;

fn server(ssl_mode: SslMode, ssl_root_cert: Option<String>) -> Server {
    Server {
        id: Some(1),
        name: "tls-e2e".into(),
        db_type: DatabaseType::Postgres,
        host: "localhost".into(),
        port: 55433,
        username: "postgres".into(),
        password: "tls".into(),
        default_database: None,
        ssl_mode,
        ssl_root_cert,
        connection_uri: None,
        created_at: 0,
        scope_databases: None,
        scope_schemas: None,
        group_id: None,
        assistant_sample_values: false,
    }
}

fn ca() -> String {
    std::env::var("OCTAPUS_TLS_CA").expect("OCTAPUS_TLS_CA (ver scripts/tls-e2e.sh)")
}

/// Conecta e diz se a sessão está cifrada (`pg_stat_ssl`).
async fn uses_ssl(server: &Server) -> crate::error::Result<bool> {
    let (client, _) = connect_dedicated(server, "postgres").await?;
    let row = client
        .query_one("SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()", &[])
        .await?;
    Ok(row.get(0))
}

#[tokio::test]
#[ignore]
async fn modes_without_verification_connect() {
    assert!(!uses_ssl(&server(SslMode::Disable, None)).await.unwrap());
    assert!(uses_ssl(&server(SslMode::Prefer, None)).await.unwrap());
    assert!(uses_ssl(&server(SslMode::Require, None)).await.unwrap());
}

#[tokio::test]
#[ignore]
async fn verify_ca_checks_the_chain_but_not_the_name() {
    assert!(uses_ssl(&server(SslMode::VerifyCa, Some(ca()))).await.unwrap());

    // Sem a CA própria, as do sistema não reconhecem o certificado
    let error = uses_ssl(&server(SslMode::VerifyCa, None)).await.unwrap_err();
    assert!(error.to_string().contains("UnknownIssuer"), "{error}");
}

#[tokio::test]
#[ignore]
async fn verify_full_also_checks_the_name() {
    // O certificado é de db.internal; a conexão vai para localhost
    let error = uses_ssl(&server(SslMode::VerifyFull, Some(ca()))).await.unwrap_err();
    assert!(error.to_string().contains("not valid for name"), "{error}");
}
