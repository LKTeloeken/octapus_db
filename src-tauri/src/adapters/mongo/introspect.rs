//! Catálogo do Mongo (refactor do catálogo, Fase 5).
//!
//! Um database Mongo não tem nível de schema: as coleções vão para o
//! [`FLAT_SCHEMA`] e uma sincronização é um `listCollections` só, com
//! `nameOnly` (sem as opções de cada coleção e sem lock de coleção) e
//! `authorizedCollections` (quem não tem `listCollections` no database vê as
//! coleções em que tem permissão). O tamanho fica para a coleção aberta
//! (`$collStats`); em massa ele custava 49 s para 5.000 coleções em rede
//! remota (`perf/catalog/BASELINE.md`, H8).
//!
//! O formato das coleções para o assistente (`infer_shape`) também vem daqui,
//! na mesma conexão dedicada: uma amostra `$sample` por coleção escolhida,
//! com `maxTimeMS`, e os documentos descartados assim que o formato sai
//! (`shape.rs`, RN-08).

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures_util::{StreamExt, TryStreamExt};
use mongodb::bson::{doc, Bson, Document};
use mongodb::{Client, Database};
use parking_lot::RwLock;
use tokio::sync::Notify;

use crate::catalog::{
    stable_hash, sync_flat, Catalog, CatalogSource, CollectionShape, IndexObject, ObjectKind, ObjectRef, RelKind,
    SourceCanceller, SyncEvent, SyncReport, TableRecord, FLAT_SCHEMA,
};
use crate::error::{Error, Result};
use crate::models::Server;

use super::metadata::{collection_size, list_indexes};
use super::shape::{self, SAMPLE_SIZE};

/// Conexões do cliente dedicado: uma faixa faz uma operação por vez
const POOL_SIZE: u32 = 2;
/// Tempo de servidor de uma amostra (`maxTimeMS`): numa view cara, a coleção
/// fica de fora do contexto em vez de segurar o turno.
const SAMPLE_TIME: Duration = Duration::from_secs(3);

pub struct MongoSource {
    client: Client,
    database: String,
    version: String,
    /// Acorda a operação em andamento para ela desistir
    cancel: Arc<Notify>,
}

impl MongoSource {
    /// Cliente próprio, separado do pool do editor (o catálogo não disputa
    /// conexão com uma consulta longa), e já valida a conexão com o `buildInfo`.
    pub async fn connect(server: &Server, database: &str) -> Result<Self> {
        let mut options = super::client_options(server).await?;
        options.app_name = Some("octapus_db catalog".to_string());
        options.max_pool_size = Some(POOL_SIZE);
        options.min_pool_size = Some(0);

        let client = Client::with_options(options).map_err(|e| Error::Connection(e.to_string()))?;
        let info = client
            .database(database)
            .run_command(doc! { "buildInfo": 1 })
            .await?;
        let version = match info.get_str("version") {
            Ok(version) => format!("MongoDB {version}"),
            Err(_) => "MongoDB".to_string(),
        };

        Ok(Self {
            client,
            database: database.to_string(),
            version,
            cancel: Arc::default(),
        })
    }

    fn db(&self) -> Database {
        self.client.database(&self.database)
    }

    /// Roda `work` até o fim ou até um cancelamento. O driver não tem cancel
    /// request: largar o future fecha o cursor (e a conexão, se preciso).
    async fn cancellable<T>(&self, work: impl Future<Output = Result<T>>) -> Result<T> {
        tokio::select! {
            result = work => result,
            _ = self.cancel.notified() => Err(Error::Cancelled),
        }
    }

    /// As coleções do database, em ordem de bytes do nome.
    async fn list(&self) -> Result<Vec<TableRecord>> {
        let mut cursor = self
            .db()
            .run_cursor_command(doc! {
                "listCollections": 1,
                "nameOnly": true,
                "authorizedCollections": true,
            })
            .await?;

        let mut tables = Vec::new();
        while let Some(spec) = cursor.try_next().await? {
            if let Some(table) = collection_record(&spec) {
                tables.push(table);
            }
        }
        tables.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(tables)
    }
}

/// JSON estendido relaxado (o que o editor e o mongosh leem).
fn to_json(value: Bson, pretty: bool) -> String {
    let json = value.into_relaxed_extjson();
    let text = if pretty {
        serde_json::to_string_pretty(&json)
    } else {
        serde_json::to_string(&json)
    };
    text.unwrap_or_default()
}

/// `db.createView(...)` de uma view, do `listCollections` filtrado por ela.
async fn view_definition(db: &Database, name: &str) -> Result<Option<String>> {
    let mut cursor = db
        .run_cursor_command(doc! {
            "listCollections": 1,
            "filter": { "name": name, "type": "view" },
            "authorizedCollections": true,
        })
        .await?;
    let Some(spec) = cursor.try_next().await? else {
        return Ok(None);
    };
    let options = spec.get_document("options").cloned().unwrap_or_default();
    let view_on = options.get_str("viewOn").unwrap_or_default().to_string();
    let pipeline = options.get("pipeline").cloned().unwrap_or(Bson::Array(Vec::new()));
    Ok(Some(format!(
        "db.createView({}, {}, {})\n",
        to_json(Bson::String(name.to_string()), false),
        to_json(Bson::String(view_on), false),
        to_json(pipeline, true)
    )))
}

/// `db.getCollection(...).createIndex(chaves, opções)` de um índice.
async fn index_definition(db: &Database, collection: &str, name: &str) -> Result<Option<String>> {
    let mut cursor = db.collection::<Document>(collection).list_indexes().await?;
    while let Some(model) = cursor.try_next().await? {
        let options = model.options.as_ref();
        if options.and_then(|o| o.name.as_deref()) != Some(name) {
            continue;
        }
        let mut options = options
            .and_then(|o| mongodb::bson::to_document(o).ok())
            .unwrap_or_default();
        // Versão do formato do índice: o servidor escolhe
        options.remove("v");
        return Ok(Some(format!(
            "db.getCollection({}).createIndex({}, {})\n",
            to_json(Bson::String(collection.to_string()), false),
            to_json(Bson::Document(model.keys), false),
            to_json(Bson::Document(options), false)
        )));
    }
    Ok(None)
}

/// Amostra aleatória da coleção → formato (com os valores de exemplo, se
/// `values`). `None` quando a amostra falha (tempo, permissão): o erro vai
/// para o log só pelo código.
async fn sample_shape(db: &Database, collection: &str, values: bool) -> Option<CollectionShape> {
    let sample = async {
        let mut cursor = db
            .collection::<Document>(collection)
            .aggregate(vec![doc! { "$sample": { "size": SAMPLE_SIZE } }])
            .max_time(SAMPLE_TIME)
            .batch_size(SAMPLE_SIZE as u32)
            .await?;
        let mut docs = Vec::new();
        while let Some(document) = cursor.try_next().await? {
            docs.push(document);
            if docs.len() as i64 >= SAMPLE_SIZE {
                break;
            }
        }
        Ok::<_, mongodb::error::Error>(docs)
    };
    match sample.await {
        // Os documentos morrem aqui: só o formato segue
        Ok(docs) => Some(shape::infer(collection, &docs, values)),
        Err(error) => {
            log::info!("catálogo mongo: amostra de coleção falhou ({})", Error::from(error).code());
            None
        }
    }
}

/// As `description` do validador `$jsonSchema` de uma coleção: a dela e as
/// dos campos, pelo mesmo caminho do formato (`items[].sku`).
#[derive(Default)]
struct Descriptions {
    collection: Option<String>,
    fields: HashMap<String, String>,
}

impl Descriptions {
    fn apply(mut self, shape: &mut CollectionShape) {
        shape.description = self.collection.take();
        for field in &mut shape.fields {
            field.description = self.fields.remove(&field.path);
        }
    }
}

/// Níveis do `$jsonSchema` lidos, no máximo.
const SCHEMA_MAX_DEPTH: usize = 8;

/// As descrições dos validadores das coleções pedidas, num `listCollections`
/// com filtro pelos nomes (é o que traz as opções; o da sincronização é
/// `nameOnly`). Sem permissão ou sem validador, nada: o formato segue sem.
async fn validator_descriptions(db: &Database, collections: &[String]) -> HashMap<String, Descriptions> {
    let listed = async {
        let specs: Vec<Document> = db
            .list_collections()
            .filter(doc! { "name": { "$in": collections } })
            .authorized_collections(true)
            .await?
            .with_type::<Document>()
            .try_collect()
            .await?;
        Ok::<_, mongodb::error::Error>(specs)
    };
    let specs = match listed.await {
        Ok(specs) => specs,
        Err(error) => {
            log::info!("catálogo mongo: validadores não lidos ({})", Error::from(error).code());
            return HashMap::new();
        }
    };
    specs
        .iter()
        .filter_map(|spec| {
            let name = spec.get_str("name").ok()?;
            let schema = spec.get_document("options").ok()?.get_document("validator").ok()?.get_document("$jsonSchema").ok()?;
            let mut descriptions = Descriptions {
                collection: description_of(schema),
                ..Descriptions::default()
            };
            walk_schema(schema, "", 0, &mut descriptions.fields);
            Some((name.to_string(), descriptions))
        })
        .collect()
}

fn description_of(schema: &Document) -> Option<String> {
    schema
        .get_str("description")
        .ok()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

/// `properties` (campo), `items` (elemento de array, `[]`) e as alternativas
/// (`anyOf`, `oneOf`, `allOf`, no mesmo caminho); a primeira descrição vale.
fn walk_schema(schema: &Document, path: &str, depth: usize, out: &mut HashMap<String, String>) {
    if depth >= SCHEMA_MAX_DEPTH {
        return;
    }
    if !path.is_empty() {
        if let Some(text) = description_of(schema) {
            out.entry(path.to_string()).or_insert(text);
        }
    }
    if let Ok(properties) = schema.get_document("properties") {
        for (key, value) in properties {
            if let Bson::Document(child) = value {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                walk_schema(child, &child_path, depth + 1, out);
            }
        }
    }
    let element = format!("{path}[]");
    match schema.get("items") {
        Some(Bson::Document(items)) => walk_schema(items, &element, depth + 1, out),
        Some(Bson::Array(items)) => {
            for item in items {
                if let Bson::Document(item) = item {
                    walk_schema(item, &element, depth + 1, out);
                }
            }
        }
        _ => {}
    }
    for key in ["anyOf", "oneOf", "allOf"] {
        if let Ok(alternatives) = schema.get_array(key) {
            for alternative in alternatives {
                if let Bson::Document(alternative) = alternative {
                    walk_schema(alternative, path, depth + 1, out);
                }
            }
        }
    }
}

/// `{ name, type }` do `listCollections` → relação. Views viram view; coleção
/// comum e timeseries, tabela. A `system.views` (onde o servidor guarda as
/// definições das views) fica de fora: o que ela tem aparece na pasta Views.
fn collection_record(spec: &Document) -> Option<TableRecord> {
    let name = spec.get_str("name").ok()?;
    if name == "system.views" {
        return None;
    }
    let kind = match spec.get_str("type") {
        Ok("view") => RelKind::View,
        _ => RelKind::Table,
    };
    Some(TableRecord {
        name: name.to_string(),
        kind,
        partition_of: None,
    })
}

/// Muda quando entra, sai ou troca de tipo alguma coleção.
fn fingerprint(tables: &[TableRecord]) -> i64 {
    let mut parts: Vec<&[u8]> = Vec::with_capacity(tables.len() * 2);
    let kinds: Vec<[u8; 1]> = tables.iter().map(|table| [table.kind as u8]).collect();
    for (table, kind) in tables.iter().zip(&kinds) {
        parts.push(table.name.as_bytes());
        parts.push(kind);
    }
    stable_hash(&parts) as i64
}

struct MongoCanceller(Arc<Notify>);

#[async_trait]
impl SourceCanceller for MongoCanceller {
    async fn cancel(&self) -> Result<()> {
        // Sem permissão guardada: só a operação em andamento desiste
        self.0.notify_waiters();
        Ok(())
    }
}

#[async_trait]
impl CatalogSource for MongoSource {
    async fn sync(
        &mut self,
        catalog: &RwLock<Catalog>,
        on_event: &mut (dyn FnMut(SyncEvent) + Send),
    ) -> Result<SyncReport> {
        let started = Instant::now();
        let tables = self.cancellable(self.list()).await?;
        let fingerprint = fingerprint(&tables);
        Ok(sync_flat(catalog, tables, fingerprint, started, on_event))
    }

    async fn load_schemas(&mut self, catalog: &RwLock<Catalog>, schemas: &[String]) -> Result<usize> {
        // Só existe o schema sem nome
        if !schemas.iter().any(|schema| schema == FLAT_SCHEMA) {
            return Ok(0);
        }
        let tables = self.cancellable(self.list()).await?;
        let fingerprint = fingerprint(&tables);
        catalog.write().set_tables(FLAT_SCHEMA, tables, Some(fingerprint));
        Ok(1)
    }

    async fn relation_size(&mut self, _schema: &str, table: &str) -> Result<Option<i64>> {
        Ok(collection_size(&self.db(), table).await)
    }

    async fn infer_shape(&mut self, collections: &[String], values: bool) -> Result<Vec<CollectionShape>> {
        let db = self.db();
        let work = async {
            // Duas amostras por vez (as conexões do cliente), na ordem pedida
            let samples: Vec<_> = collections
                .iter()
                .map(|collection| sample_shape(&db, collection, values))
                .collect();
            let shapes: Vec<Option<CollectionShape>> = futures_util::stream::iter(samples)
                .buffered(POOL_SIZE as usize)
                .collect()
                .await;
            let mut shapes: Vec<CollectionShape> = shapes.into_iter().flatten().collect();
            let mut described = validator_descriptions(&db, collections).await;
            for shape in &mut shapes {
                if let Some(descriptions) = described.remove(&shape.collection) {
                    descriptions.apply(shape);
                }
            }
            Ok(shapes)
        };
        self.cancellable(work).await
    }

    async fn list_indexes(&mut self, _schema: &str, table: Option<&str>) -> Result<Vec<IndexObject>> {
        let Some(collection) = table else {
            return Err(Error::UnsupportedType("MongoDB lists indexes per collection".into()));
        };
        let db = self.db();
        let indexes = self.cancellable(list_indexes(&db, collection)).await?;
        Ok(indexes
            .into_iter()
            .map(|index| IndexObject {
                name: index.name,
                table: collection.to_string(),
                method: index.index_type,
                unique: index.is_unique,
                primary: index.is_primary,
                columns: index.columns,
                predicate: None,
            })
            .collect())
    }

    async fn definition(&mut self, target: &ObjectRef) -> Result<Option<String>> {
        let db = self.db();
        match (target.kind, target.table.as_deref()) {
            (ObjectKind::View, _) => self.cancellable(view_definition(&db, &target.name)).await,
            (ObjectKind::Index, Some(collection)) => {
                self.cancellable(index_definition(&db, collection, &target.name)).await
            }
            _ => Ok(None),
        }
    }

    fn canceller(&self) -> Box<dyn SourceCanceller> {
        Box::new(MongoCanceller(Arc::clone(&self.cancel)))
    }

    fn is_closed(&self) -> bool {
        // O driver reconecta sozinho
        false
    }

    fn server_version(&self) -> String {
        self.version.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_list_collections_entries() {
        let view = collection_record(&doc! { "name": "active_users", "type": "view" }).unwrap();
        assert_eq!(view.kind, RelKind::View);
        let series = collection_record(&doc! { "name": "metrics", "type": "timeseries" }).unwrap();
        assert_eq!(series.kind, RelKind::Table);
        let plain = collection_record(&doc! { "name": "orders", "type": "collection" }).unwrap();
        assert_eq!((plain.name.as_str(), plain.kind), ("orders", RelKind::Table));
        assert!(collection_record(&doc! { "type": "collection" }).is_none());
        assert!(collection_record(&doc! { "name": "system.views", "type": "collection" }).is_none());
    }

    #[test]
    fn fingerprint_follows_names_and_kinds() {
        let tables = |entries: &[(&str, RelKind)]| -> Vec<TableRecord> {
            entries
                .iter()
                .map(|(name, kind)| TableRecord {
                    name: name.to_string(),
                    kind: *kind,
                    partition_of: None,
                })
                .collect()
        };
        let base = fingerprint(&tables(&[("orders", RelKind::Table), ("users", RelKind::Table)]));
        assert_eq!(base, fingerprint(&tables(&[("orders", RelKind::Table), ("users", RelKind::Table)])));
        assert_ne!(base, fingerprint(&tables(&[("orders", RelKind::Table)])));
        assert_ne!(base, fingerprint(&tables(&[("orders", RelKind::View), ("users", RelKind::Table)])));
        // A fronteira entre nomes conta
        assert_ne!(
            fingerprint(&tables(&[("ab", RelKind::Table), ("c", RelKind::Table)])),
            fingerprint(&tables(&[("a", RelKind::Table), ("bc", RelKind::Table)]))
        );
    }
}

/// Pastas da árvore contra um Mongo sem auth (localhost:27017, ou a porta de
/// `OCTAPUS_E2E_MONGO_PORT`), num database scratch próprio.
/// `cargo test --lib mongo::introspect::e2e -- --ignored`
#[cfg(test)]
mod e2e {
    use super::*;
    use crate::models::{DatabaseType, SslMode};

    const DATABASE: &str = "octapus_e2e_folders";

    fn port() -> u16 {
        std::env::var("OCTAPUS_E2E_MONGO_PORT")
            .ok()
            .and_then(|port| port.parse().ok())
            .unwrap_or(27017)
    }

    fn server() -> Server {
        Server {
            id: Some(1),
            name: "e2e".into(),
            db_type: DatabaseType::Mongodb,
            host: "localhost".into(),
            port: port(),
            username: String::new(),
            password: String::new(),
            default_database: None,
            ssl_mode: SslMode::Disable,
            ssl_root_cert: None,
            connection_uri: None,
            scope_databases: None,
            scope_schemas: None,
            group_id: None,
            assistant_sample_values: false,
            created_at: 0,
        }
    }

    #[tokio::test]
    #[ignore = "requires local mongodb without auth (OCTAPUS_E2E_MONGO_PORT, default 27017)"]
    async fn e2e_views_and_indexes_per_collection() {
        let client = Client::with_uri_str(format!("mongodb://localhost:{}", port()))
            .await
            .unwrap();
        let db = client.database(DATABASE);
        db.drop().await.unwrap();
        let orders = db.collection::<Document>("orders");
        orders.insert_one(doc! { "status": "paid", "total": 10 }).await.unwrap();
        orders
            .create_index(
                mongodb::IndexModel::builder()
                    .keys(doc! { "status": 1, "total": -1 })
                    .options(
                        mongodb::options::IndexOptions::builder()
                            .name("status_total".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
            )
            .await
            .unwrap();
        db.create_collection("paid_orders")
            .view_on("orders".to_string())
            .pipeline(vec![doc! { "$match": { "status": "paid" } }])
            .await
            .unwrap();

        let mut source = MongoSource::connect(&server(), DATABASE).await.unwrap();
        let catalog = RwLock::new(Catalog::new());
        source.sync(&catalog, &mut |_| {}).await.unwrap();
        let loose: Vec<String> = catalog
            .read()
            .children(&crate::catalog::CatalogPath::Tables(FLAT_SCHEMA.into()), None, 0, 10)
            .unwrap()
            .items
            .into_iter()
            .map(|node| node.name)
            .collect();
        assert_eq!(loose, ["orders"], "a view mora na pasta");
        let views = catalog
            .read()
            .objects(ObjectKind::View, FLAT_SCHEMA, None, None, 0, 10)
            .unwrap();
        assert_eq!(views.total, 1);

        // O banco inteiro não lista índices; por coleção, sim
        assert!(source.list_indexes(FLAT_SCHEMA, None).await.is_err());
        let indexes = source.list_indexes(FLAT_SCHEMA, Some("orders")).await.unwrap();
        let names: Vec<&str> = indexes.iter().map(|index| index.name.as_str()).collect();
        assert_eq!(names, ["_id_", "status_total"]);
        assert!(indexes[0].primary);
        assert_eq!(
            (indexes[1].unique, indexes[1].columns.clone()),
            (true, vec!["status".into(), "total".into()])
        );
        assert_eq!(indexes[1].table, "orders");

        let target = |kind, name: &str, table: Option<&str>| ObjectRef {
            kind,
            schema: FLAT_SCHEMA.into(),
            name: name.into(),
            table: table.map(str::to_string),
            signature: None,
        };
        let view = source
            .definition(&target(ObjectKind::View, "paid_orders", None))
            .await
            .unwrap()
            .unwrap();
        assert!(
            view.starts_with("db.createView(\"paid_orders\", \"orders\", ["),
            "{view}"
        );
        assert!(view.contains("\"$match\""), "{view}");
        let index = source
            .definition(&target(ObjectKind::Index, "status_total", Some("orders")))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            index,
            "db.getCollection(\"orders\").createIndex({\"status\":1,\"total\":-1}, {\"name\":\"status_total\",\"unique\":true})\n"
        );
        assert!(source
            .definition(&target(ObjectKind::Index, "nope", Some("orders")))
            .await
            .unwrap()
            .is_none());

        db.drop().await.unwrap();
    }
}
