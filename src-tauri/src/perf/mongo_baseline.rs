//! Baseline do Mongo: database com 5 mil coleções (`tenants`) e um database
//! por cliente (`tenantdb_NNN`), da fixture `perf/catalog/seed/mongo.js`.
//!
//! Desde a Fase 5 o adapter não faz mais a listagem antiga (tamanho de todos
//! os databases, `$collStats` de toda coleção): ela está copiada aqui, com o
//! cliente cru, para a baseline continuar comparável.

use std::time::Duration;

use mongodb::bson::{doc, Bson, Document};
use mongodb::options::ClientOptions;
use mongodb::{Client, Database};
use serde_json::json;
use tokio::time::timeout;

use super::fixture::{env_u64, timed, Net, Report, MONGO_PORT};

pub(super) const TENANTS_DB: &str = "tenants";

pub(super) async fn raw_client(port: u16) -> Client {
    let options = ClientOptions::parse(format!("mongodb://127.0.0.1:{port}/?directConnection=true"))
        .await
        .unwrap();
    Client::with_options(options).unwrap()
}

/// `$collStats` de uma coleção, como o adapter fazia para todas.
async fn legacy_collection_size(db: &Database, collection: &str) -> Option<i64> {
    let mut cursor = db
        .collection::<Document>(collection)
        .aggregate(vec![doc! { "$collStats": { "storageStats": {} } }])
        .await
        .ok()?;
    let mut total = None;
    while cursor.advance().await.ok()? {
        let stats = cursor.deserialize_current().ok()?;
        let size = match stats.get_document("storageStats").ok()?.get("totalSize") {
            Some(Bson::Int32(n)) => *n as i64,
            Some(Bson::Int64(n)) => *n,
            Some(Bson::Double(n)) => *n as i64,
            _ => continue,
        };
        total = Some(total.unwrap_or(0) + size);
    }
    total
}

/// A listagem que a árvore fazia até a Fase 5: nomes e, em paralelo, o
/// `$collStats` de cada coleção. Devolve (coleções, bytes do JSON).
async fn legacy_structure(db: &Database) -> mongodb::error::Result<(usize, usize)> {
    let mut names = db.list_collection_names().await?;
    names.sort();
    let mut tasks = tokio::task::JoinSet::new();
    for name in names.clone() {
        let db = db.clone();
        tasks.spawn(async move { (name.clone(), legacy_collection_size(&db, &name).await) });
    }
    let mut tables = Vec::new();
    while let Some(Ok((name, size))) = tasks.join_next().await {
        tables.push(json!({ "name": name, "tableType": "table", "sizeBytes": size }));
    }
    let payload = serde_json::to_vec(&json!({ "schemas": [{ "name": db.name(), "tables": tables }] })).unwrap();
    Ok((names.len(), payload.len()))
}

#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh up + mongo)"]
async fn perf_mongo_server_level() {
    let net = Net::from_env(MONGO_PORT).await;
    let report = Report::new("mongo", &net);
    let client = raw_client(net.port(MONGO_PORT)).await;
    client.list_database_names().await.unwrap();

    let (databases, elapsed) = timed(async { client.list_databases().await }).await;
    report.record(
        "list_databases (atual, com sizeOnDisk)",
        Some(elapsed),
        json!({ "databases": databases.unwrap().len() }),
    );

    let (names, elapsed) = timed(async { client.list_database_names().await }).await;
    report.record(
        "list_databases (nameOnly)",
        Some(elapsed),
        json!({ "databases": names.unwrap().len() }),
    );
}

#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh up + mongo)"]
async fn perf_mongo_tree_current() {
    let net = Net::from_env(MONGO_PORT).await;
    let report = Report::new("mongo", &net);
    let client = raw_client(net.port(MONGO_PORT)).await;
    let db = client.database(TENANTS_DB);
    client.list_database_names().await.unwrap();

    let (names, elapsed) = timed(async { db.list_collection_names().await }).await;
    report.record(
        "list_tables (atual, nomes)",
        Some(elapsed),
        json!({ "collections": names.unwrap().len() }),
    );

    let limit = Duration::from_secs(env_u64("OCTAPUS_PERF_FULL_TIMEOUT_S", 900));
    let (outcome, elapsed) = timed(timeout(limit, legacy_structure(&db))).await;
    let extra = match outcome {
        Ok(Ok((collections, json_bytes))) => json!({ "collections": collections, "json_bytes": json_bytes }),
        Ok(Err(error)) => json!({ "error": error.to_string() }),
        Err(_) => json!({ "timeout_s": limit.as_secs() }),
    };
    report.record(
        "list_schemas_with_tables (atual, $collStats por coleção)",
        Some(elapsed),
        extra,
    );
}
