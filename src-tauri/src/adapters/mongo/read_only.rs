//! Execução só-leitura no Mongo (`ReadOnlyOp::Mongo`), para o assistente.
//!
//! O Mongo não tem transação `READ ONLY`; a barreira daqui vale sozinha, sem
//! depender do guard:
//! - **o comando sai do tipo da operação** (`operation.rs`): só `find`,
//!   `aggregate`, `count` e `distinct` existem aqui, nada de texto no formato
//!   do shell; um pipeline com `$out` ou `$merge` — o único jeito de um
//!   `aggregate` escrever — é recusado antes de ir ao servidor;
//! - **`maxTimeMS`** em todo comando, com um teto também do lado do app, e
//!   **teto de documentos** por página (`ReadOnlyLimits`).
//!
//! O preflight confere que as coleções lidas existem (no Mongo, ler uma
//! coleção que não existe devolve vazio, sem erro) e roda `explain` com
//! `verbosity: "queryPlanner"`, que planeja sem executar. Os erros do
//! servidor saem como `Error::MongoQuery`, com o código para o assistente
//! classificar (`assistant/failure.rs`).

use std::future::Future;
use std::time::{Duration, Instant};

use futures_util::TryStreamExt;
use mongodb::bson::{doc, Bson, Document};
use mongodb::{Cursor, Database};

use crate::adapters::{ReadOnlyLimits, STATEMENT_TIMEOUT_MESSAGE};
use crate::error::{Error, Result};
use crate::models::{QueryColumnInfo, QueryOptions, QueryResult, ReadOnlyPlan};

use super::executor::documents_to_table;
use super::operation::{self, OperationKind, ReadOperation};
use super::types::{bson_to_string, bson_type_name};

/// Folga do teto do app sobre o `maxTimeMS`: o servidor para antes, e o erro
/// dele (com código) é o que vale.
const CLIENT_GRACE: Duration = Duration::from_secs(5);

/// Erro do servidor com o código (`MongoQuery`); o resto segue a conversão
/// de sempre (conexão, autenticação…).
fn mongo_error(error: mongodb::error::Error) -> Error {
    use mongodb::error::ErrorKind;
    match error.kind.as_ref() {
        ErrorKind::Command(command) => Error::MongoQuery {
            code: command.code,
            code_name: command.code_name.clone(),
            message: command.message.clone(),
        },
        _ => error.into(),
    }
}

fn parse(text: &str) -> Result<ReadOperation> {
    let operation = operation::parse(text).map_err(|error| Error::InvalidQuery(error.to_string()))?;
    // A barreira do executor: sem o guard, a escrita para aqui
    if operation::writes(&operation) {
        return Err(Error::GuardRejected(
            "$out and $merge are not allowed in a read-only query".into(),
        ));
    }
    Ok(operation)
}

/// Roda `work` com o teto do app por cima do `maxTimeMS`.
async fn within<T>(limits: &ReadOnlyLimits, work: impl Future<Output = Result<T>>) -> Result<T> {
    match tokio::time::timeout(limits.statement_timeout + CLIENT_GRACE, work).await {
        Ok(result) => result,
        Err(_) => Err(Error::Query(STATEMENT_TIMEOUT_MESSAGE.into())),
    }
}

pub async fn execute(db: &Database, text: &str, page: &QueryOptions, limits: &ReadOnlyLimits) -> Result<QueryResult> {
    let operation = parse(text)?;
    let started = Instant::now();
    let mut result = within(limits, run(db, &operation, page, limits)).await?;
    result.execution_time_ms = started.elapsed().as_millis() as u64;
    Ok(result)
}

async fn run(db: &Database, operation: &ReadOperation, page: &QueryOptions, limits: &ReadOnlyLimits) -> Result<QueryResult> {
    let (limit, offset) = limits.page(page.limit, page.offset);
    let collection = db.collection::<Document>(&operation.collection);
    let max_time = limits.statement_timeout;

    match operation.kind {
        OperationKind::Find => {
            // A página dentro do `skip`/`limit` que o modelo pediu
            let skip = operation.skip.unwrap_or(0) + offset;
            let remaining = operation.limit.map(|wanted| wanted - offset);
            if remaining.is_some_and(|remaining| remaining <= 0) {
                return Ok(documents_result(Vec::new(), false));
            }
            let fetch = remaining.map_or(limit + 1, |remaining| remaining.min(limit + 1));

            let mut find = collection
                .find(operation.filter.clone())
                .skip(skip as u64)
                .limit(fetch)
                .max_time(max_time);
            if let Some(projection) = &operation.projection {
                find = find.projection(projection.clone());
            }
            if let Some(sort) = &operation.sort {
                find = find.sort(sort.clone());
            }
            let docs = collect(find.await.map_err(mongo_error)?, fetch as usize).await?;
            Ok(page_of(docs, limit))
        }
        OperationKind::Aggregate => {
            let mut pipeline = operation.pipeline.clone();
            if offset > 0 {
                pipeline.push(doc! { "$skip": offset });
            }
            pipeline.push(doc! { "$limit": limit + 1 });
            let cursor = collection
                .aggregate(pipeline)
                .max_time(max_time)
                .await
                .map_err(mongo_error)?;
            let docs = collect(cursor, limit as usize + 1).await?;
            Ok(page_of(docs, limit))
        }
        OperationKind::CountDocuments => {
            if offset > 0 {
                return Ok(single_count(None));
            }
            let count = collection
                .count_documents(operation.filter.clone())
                .max_time(max_time)
                .await
                .map_err(mongo_error)?;
            Ok(single_count(Some(count)))
        }
        OperationKind::Distinct => {
            let field = operation.field.clone().unwrap_or_default();
            let values = collection
                .distinct(&field, operation.filter.clone())
                .max_time(max_time)
                .await
                .map_err(mongo_error)?;
            let has_more = values.len() as i64 > offset + limit;
            let page: Vec<&Bson> = values.iter().skip(offset as usize).take(limit as usize).collect();
            let type_name = page.first().map_or("mixed", |value| bson_type_name(value)).to_string();
            let rows: Vec<Vec<Option<String>>> = page.iter().map(|value| vec![bson_to_string(value)]).collect();
            Ok(QueryResult {
                columns: vec![QueryColumnInfo {
                    name: field,
                    type_name,
                    type_oid: None,
                }],
                row_count: rows.len(),
                rows,
                total_count: None,
                has_more,
                execution_time_ms: 0,
                editable_info: None,
            })
        }
    }
}

async fn collect(mut cursor: Cursor<Document>, cap: usize) -> Result<Vec<Document>> {
    let mut docs = Vec::new();
    while let Some(document) = cursor.try_next().await.map_err(mongo_error)? {
        docs.push(document);
        if docs.len() >= cap {
            break;
        }
    }
    Ok(docs)
}

/// `limit + 1` documentos lidos: o a mais só diz que há outra página.
fn page_of(mut docs: Vec<Document>, limit: i64) -> QueryResult {
    let has_more = docs.len() as i64 > limit;
    docs.truncate(limit as usize);
    documents_result(docs, has_more)
}

/// O resultado é só leitura (RN-16): sem `editable_info`.
fn documents_result(docs: Vec<Document>, has_more: bool) -> QueryResult {
    let (columns, rows) = documents_to_table(&docs);
    QueryResult {
        row_count: rows.len(),
        columns,
        rows,
        total_count: None,
        has_more,
        execution_time_ms: 0,
        editable_info: None,
    }
}

fn single_count(count: Option<u64>) -> QueryResult {
    let rows: Vec<Vec<Option<String>>> = count.map(|count| vec![Some(count.to_string())]).into_iter().collect();
    QueryResult {
        columns: vec![QueryColumnInfo {
            name: "count".into(),
            type_name: "long".into(),
            type_oid: None,
        }],
        row_count: rows.len(),
        rows,
        total_count: None,
        has_more: false,
        execution_time_ms: 0,
        editable_info: None,
    }
}

/// O plano, sem executar: as coleções existem? Depois `explain` com
/// `queryPlanner`. O resumo do plano fica na máquina (não vai à API).
pub async fn explain(db: &Database, text: &str, limits: &ReadOnlyLimits) -> Result<ReadOnlyPlan> {
    let operation = parse(text)?;
    within(limits, async {
        check_collections(db, &operation.collections()).await?;
        let explained = db
            .run_command(doc! { "explain": explain_target(&operation), "verbosity": "queryPlanner" })
            .await
            .map_err(mongo_error)?;
        Ok(ReadOnlyPlan {
            estimated_rows: None,
            estimated_cost: None,
            plan: plan_summary(&explained),
        })
    })
    .await
}

fn explain_target(operation: &ReadOperation) -> Document {
    let collection = operation.collection.as_str();
    match operation.kind {
        OperationKind::Find => {
            let mut find = doc! { "find": collection, "filter": operation.filter.clone() };
            if let Some(projection) = &operation.projection {
                find.insert("projection", projection.clone());
            }
            if let Some(sort) = &operation.sort {
                find.insert("sort", sort.clone());
            }
            if let Some(skip) = operation.skip {
                find.insert("skip", skip);
            }
            if let Some(limit) = operation.limit {
                find.insert("limit", limit);
            }
            find
        }
        OperationKind::Aggregate => doc! {
            "aggregate": collection,
            "pipeline": operation.pipeline.clone(),
            "cursor": {},
        },
        OperationKind::CountDocuments => doc! { "count": collection, "query": operation.filter.clone() },
        OperationKind::Distinct => doc! {
            "distinct": collection,
            "key": operation.field.clone().unwrap_or_default(),
            "query": operation.filter.clone(),
        },
    }
}

/// As coleções que a operação lê existem? Sem permissão de
/// `listCollections`, a conferência fica de fora (o `explain` segue).
async fn check_collections(db: &Database, collections: &[String]) -> Result<()> {
    if collections.is_empty() {
        return Ok(());
    }
    let listed = async {
        let mut cursor = db
            .run_cursor_command(doc! {
                "listCollections": 1,
                "nameOnly": true,
                "authorizedCollections": true,
                "filter": { "name": { "$in": collections } },
            })
            .await?;
        let mut names = Vec::new();
        while let Some(spec) = cursor.try_next().await? {
            if let Ok(name) = spec.get_str("name") {
                names.push(name.to_string());
            }
        }
        Ok::<_, mongodb::error::Error>(names)
    };
    let names = match listed.await {
        Ok(names) => names,
        Err(error) => {
            log::info!("assistente: listCollections indisponível no preflight ({})", Error::from(error).code());
            return Ok(());
        }
    };
    match collections.iter().find(|name| !names.contains(name)) {
        // O nome é o que o modelo escreveu: o sanitizador o mantém
        Some(missing) => Err(Error::MongoQuery {
            code: 26,
            code_name: "NamespaceNotFound".into(),
            message: format!("collection \"{missing}\" does not exist"),
        }),
        None => Ok(()),
    }
}

/// O caminho do plano vencedor, do primeiro estágio ao último
/// (`IXSCAN status_1 → FETCH → LIMIT`). Sem `winningPlan`, o explain deu
/// certo mas não diz o plano (views, alguns pipelines).
fn plan_summary(explained: &Document) -> String {
    fn find_winning(value: &Bson) -> Option<&Document> {
        match value {
            Bson::Document(document) => {
                if let Ok(plan) = document.get_document("winningPlan") {
                    // Motor SBE: o plano clássico mora em `queryPlan`
                    return Some(plan.get_document("queryPlan").unwrap_or(plan));
                }
                document.values().find_map(find_winning)
            }
            Bson::Array(items) => items.iter().find_map(find_winning),
            _ => None,
        }
    }
    fn chain(plan: &Document, steps: &mut Vec<String>) {
        if let Ok(input) = plan.get_document("inputStage") {
            chain(input, steps);
        } else if let Ok(inputs) = plan.get_array("inputStages") {
            for input in inputs.iter().filter_map(Bson::as_document) {
                chain(input, steps);
            }
        }
        if let Ok(stage) = plan.get_str("stage") {
            match plan.get_str("indexName") {
                Ok(index) => steps.push(format!("{stage} {index}")),
                Err(_) => steps.push(stage.to_string()),
            }
        }
    }

    match find_winning(&Bson::Document(explained.clone())) {
        Some(plan) => {
            let mut steps = Vec::new();
            chain(plan, &mut steps);
            if steps.is_empty() {
                "planned".into()
            } else {
                steps.join(" → ")
            }
        }
        None => "planned".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executor_refuses_write_stages_by_itself() {
        let error = parse(r#"{"collection":"c","operation":"aggregate","pipeline":[{"$out":"x"}]}"#).unwrap_err();
        assert_eq!(error.code(), "GUARD_REJECTED");
        let error = parse(r#"{"collection":"c","operation":"deleteMany"}"#).unwrap_err();
        assert_eq!(error.code(), "INVALID_QUERY");
    }

    #[test]
    fn summarizes_the_winning_plan() {
        let explained = doc! {
            "queryPlanner": {
                "winningPlan": {
                    "stage": "LIMIT",
                    "inputStage": { "stage": "FETCH", "inputStage": { "stage": "IXSCAN", "indexName": "status_1" } },
                },
            },
        };
        assert_eq!(plan_summary(&explained), "IXSCAN status_1 → FETCH → LIMIT");
        let sbe = doc! { "stages": [{ "$cursor": { "queryPlanner": { "winningPlan": { "queryPlan": { "stage": "COLLSCAN" } } } } }] };
        assert_eq!(plan_summary(&sbe), "COLLSCAN");
        assert_eq!(plan_summary(&doc! { "ok": 1 }), "planned");
    }

    #[test]
    fn explain_targets_follow_the_operation() {
        let find = operation::parse(r#"{"collection":"c","operation":"find","filter":{"a":1},"sort":{"a":-1},"limit":3}"#).unwrap();
        assert_eq!(
            explain_target(&find),
            doc! { "find": "c", "filter": { "a": 1 }, "sort": { "a": -1 }, "limit": 3_i64 }
        );
        let distinct = operation::parse(r#"{"collection":"c","operation":"distinct","field":"a"}"#).unwrap();
        assert_eq!(explain_target(&distinct), doc! { "distinct": "c", "key": "a", "query": {} });
    }
}

/// E2E contra o Mongo dos testes (localhost:27017, sem auth — ver a memória
/// do projeto). `cargo test --lib mongo::read_only -- --ignored`. Aqui o
/// guard fica **desligado**: o executor é chamado direto.
#[cfg(test)]
mod e2e {
    use super::*;
    use crate::adapters::mongo::MongoAdapter;
    use crate::adapters::{DatabaseAdapter, ReadOnlyOp};
    use crate::models::{DatabaseType, Server, SslMode};

    pub(crate) fn server() -> Server {
        Server {
            id: Some(1),
            name: "e2e".into(),
            db_type: DatabaseType::Mongodb,
            host: "localhost".into(),
            port: 27017,
            username: String::new(),
            password: String::new(),
            default_database: None,
            ssl_mode: SslMode::Disable,
            ssl_root_cert: None,
            connection_uri: None,
            scope_databases: None,
            scope_schemas: None,
            group_id: None,
            created_at: 0,
        }
    }

    fn op(text: &str) -> ReadOnlyOp {
        ReadOnlyOp::Mongo(text.into())
    }

    #[tokio::test]
    #[ignore = "requires local mongodb on localhost:27017 (no auth)"]
    async fn e2e_mongo_read_only_executor() {
        const DATABASE: &str = "octapus_e2e_assistant_read_only";
        let adapter = MongoAdapter::new(&server(), DATABASE).await.unwrap();
        adapter.db().drop().await.unwrap();
        let docs: Vec<Document> = (1..=25)
            .map(|i| doc! { "idx": i, "status": if i % 2 == 0 { "paid" } else { "open" }, "label": format!("CANARY_{i}") })
            .collect();
        adapter.db().collection::<Document>("orders").insert_many(docs).await.unwrap();
        let count = || async { adapter.db().collection::<Document>("orders").count_documents(doc! {}).await.unwrap() };
        let page = |limit: i64, offset: i64| QueryOptions { limit, offset, ..QueryOptions::default() };

        // find com sort, skip e limit do modelo, paginado por cima
        let text = r#"{"collection":"orders","operation":"find","filter":{"status":"paid"},"sort":{"idx":1},"skip":1,"limit":8,"projection":{"_id":0,"idx":1}}"#;
        let first = adapter.execute_read_only(&op(text), page(5, 0)).await.unwrap();
        assert_eq!(first.rows.iter().map(|row| row[0].clone().unwrap()).collect::<Vec<_>>(), ["4", "6", "8", "10", "12"]);
        assert!(first.has_more && first.editable_info.is_none());
        let second = adapter.execute_read_only(&op(text), page(5, 5)).await.unwrap();
        assert_eq!(second.rows.iter().map(|row| row[0].clone().unwrap()).collect::<Vec<_>>(), ["14", "16", "18"]);
        assert!(!second.has_more);

        // aggregate, count e distinct
        let grouped = adapter
            .execute_read_only(
                &op(r#"{"collection":"orders","operation":"aggregate","pipeline":[{"$group":{"_id":"$status","n":{"$sum":1}}},{"$sort":{"_id":1}}]}"#),
                page(10, 0),
            )
            .await
            .unwrap();
        assert_eq!(grouped.rows, vec![vec![Some("open".into()), Some("13".into())], vec![Some("paid".into()), Some("12".into())]]);
        let counted = adapter
            .execute_read_only(&op(r#"{"collection":"orders","operation":"countDocuments","filter":{"idx":{"$gt":20}}}"#), page(10, 0))
            .await
            .unwrap();
        assert_eq!(counted.rows[0][0].as_deref(), Some("5"));
        let distinct = adapter
            .execute_read_only(&op(r#"{"collection":"orders","operation":"distinct","field":"status"}"#), page(1, 0))
            .await
            .unwrap();
        assert_eq!((distinct.row_count, distinct.has_more), (1, true));

        // Escrita sem o guard: o executor recusa e nada muda
        for text in [
            r#"{"collection":"orders","operation":"aggregate","pipeline":[{"$out":"copied"}]}"#,
            r#"{"collection":"orders","operation":"aggregate","pipeline":[{"$merge":{"into":"orders"}}]}"#,
            r#"{"collection":"orders","operation":"deleteMany","filter":{}}"#,
        ] {
            assert!(adapter.execute_read_only(&op(text), page(10, 0)).await.is_err(), "{text}");
        }
        assert_eq!(count().await, 25);
        let names = adapter.db().list_collection_names().await.unwrap();
        assert_eq!(names, ["orders"]);

        // Preflight: planeja sem executar; coleção inexistente e operador
        // desconhecido aparecem sem rodar nada
        let plan = adapter
            .explain_read_only(&op(r#"{"collection":"orders","operation":"find","filter":{"status":"paid"}}"#), page(10, 0))
            .await
            .unwrap();
        assert!(plan.plan.contains("COLLSCAN"), "{plan:?}");
        let missing = adapter
            .explain_read_only(&op(r#"{"collection":"pedidos","operation":"find"}"#), page(10, 0))
            .await
            .unwrap_err();
        assert!(matches!(&missing, Error::MongoQuery { code: 26, .. }), "{missing:?}");
        let unknown = adapter
            .explain_read_only(&op(r#"{"collection":"orders","operation":"find","filter":{"idx":{"$bogus":1}}}"#), page(10, 0))
            .await
            .unwrap_err();
        assert!(matches!(&unknown, Error::MongoQuery { code: 2, .. }), "{unknown:?}");

        // Teto de tempo: o servidor para no maxTimeMS
        let limits = ReadOnlyLimits {
            statement_timeout: Duration::from_millis(50),
            ..ReadOnlyLimits::default()
        };
        let slow = operation::parse(
            r#"{"collection":"orders","operation":"find","filter":{"$where":"sleep(100) || true"}}"#,
        )
        .unwrap();
        let error = within(&limits, run(&adapter.db(), &slow, &page(10, 0), &limits)).await.unwrap_err();
        assert!(matches!(&error, Error::MongoQuery { code: 50, .. }), "{error:?}");

        adapter.db().drop().await.unwrap();
    }
}
