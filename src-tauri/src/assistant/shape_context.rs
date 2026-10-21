//! O contexto da estrutura no Mongo (B8): as coleções que interessam à
//! pergunta, cada uma com o formato inferido de uma amostra aleatória
//! (`CatalogSource::infer_shape`), dentro do mesmo orçamento de tokens do SQL.
//!
//! As coleções vêm do ranking do catálogo (`Catalog::context_candidates`,
//! como no SQL), e só as primeiras [`MAX_COLLECTIONS`] são amostradas — nunca
//! o database inteiro. Não há valor nenhum no texto (RN-08): só caminhos,
//! tipos BSON e a % de presença. Os nomes vêm do banco e são dado (RN-22),
//! sempre entre aspas duplas, como no DDL.
//!
//! ```text
//! -- MongoDB
//! collection "orders" (100 sampled documents):
//!   "_id" objectId 100%
//!   "customer.name" string 97%
//!   "items[].qty" int|double 100%
//! -- Other collections (fields not shown): "audit", "carts"
//! ```

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::catalog::{Candidate, CollectionShape, RelKind, FLAT_SCHEMA};
use crate::error::Result;
use crate::services::{CatalogEntry, CatalogService};

use super::schema_context::{budget_tokens, estimate_tokens, others_line, qualified, quote_ident, ContextRequest, SchemaContext};

/// Coleções amostradas por turno, no máximo: cada uma é um `$sample`.
pub const MAX_COLLECTIONS: usize = 12;

const HEADER: &str = "-- MongoDB";

/// Escolhe as coleções pelo ranking, amostra cada uma e monta o texto.
pub async fn build(
    catalogs: &CatalogService,
    entry: &Arc<CatalogEntry>,
    request: ContextRequest<'_>,
) -> Result<SchemaContext> {
    let schemas = [FLAT_SCHEMA.to_string()];
    catalogs.ensure_schemas(entry, &schemas).await?;
    let candidates = {
        let catalog = entry.read();
        let history: Vec<&str> = request.history.iter().map(String::as_str).collect();
        catalog.context_candidates(&[FLAT_SCHEMA], request.question, &history)
    };

    // Com a pergunta casando, as que casaram primeiro; sem casar, por nome
    let chosen: Vec<String> = candidates
        .iter()
        .take(MAX_COLLECTIONS)
        .map(|candidate| candidate.table.clone())
        .collect();
    let shapes = catalogs.infer_shape(entry, &chosen).await?;
    let context = render(&ShapeRender {
        candidates: &candidates,
        chosen: &chosen,
        shapes: &shapes,
        budget_tokens: budget_tokens(request.context_window),
    });
    log::info!(
        "assistente: contexto mongo com {} de {} coleção(ões) ({} amostra(s) sem resposta), ~{} tokens{}",
        context.tables,
        candidates.len(),
        chosen.len() - shapes.len(),
        context.tokens_estimate,
        if context.truncated { ", recortado" } else { "" }
    );
    Ok(context)
}

pub struct ShapeRender<'a> {
    /// Todas as candidatas, na ordem do ranking
    pub candidates: &'a [Candidate],
    /// As amostradas, por prioridade (o orçamento corta do fim)
    pub chosen: &'a [String],
    /// Os formatos que chegaram (a amostra que falhou fica de fora)
    pub shapes: &'a [CollectionShape],
    pub budget_tokens: usize,
}

/// O texto dentro do orçamento, com a mesma regra do DDL: as escolhidas
/// entram por prioridade enquanto couberem (a primeira entra mesmo grande,
/// com os campos cortados), as outras vão só pelo nome, e a saída fica em
/// ordem de nome.
pub fn render(input: &ShapeRender<'_>) -> SchemaContext {
    let budget_chars = input.budget_tokens * 4;
    let reserve = if input.candidates.len() > input.chosen.len().min(1) {
        (budget_chars / 5).min(400)
    } else {
        0
    };
    let collections_chars = budget_chars - reserve;
    let mut used = HEADER.chars().count() + 1;

    let kinds: HashMap<&str, RelKind> = input
        .candidates
        .iter()
        .map(|candidate| (candidate.table.as_str(), candidate.kind))
        .collect();
    let shapes: HashMap<&str, &CollectionShape> =
        input.shapes.iter().map(|shape| (shape.collection.as_str(), shape)).collect();

    let mut included: Vec<(&str, String)> = Vec::new();
    let mut cut = false;
    for name in input.chosen {
        let Some(shape) = shapes.get(name.as_str()) else {
            continue;
        };
        let kind = kinds.get(name.as_str()).copied().unwrap_or(RelKind::Table);
        let full = render_collection(shape, kind, None);
        let cost = full.chars().count() + 1;
        if used + cost <= collections_chars {
            used += cost;
            included.push((name, full));
        } else if included.is_empty() {
            let fitted = fit_fields(shape, kind, collections_chars.saturating_sub(used));
            used += fitted.chars().count() + 1;
            included.push((name, fitted));
            cut = true;
        }
        cut |= shape.truncated && included.last().is_some_and(|(last, _)| *last == name.as_str());
    }

    let with_fields: HashSet<&str> = included.iter().map(|(name, _)| *name).collect();
    let others: Vec<&Candidate> = input
        .candidates
        .iter()
        .filter(|candidate| !with_fields.contains(candidate.table.as_str()))
        .collect();
    let others_line = others_line(&others, budget_chars.saturating_sub(used), ("collections", "fields"));

    included.sort_by(|a, b| a.0.cmp(b.0));
    let mut text = HEADER.to_string();
    for (_, collection) in &included {
        text.push('\n');
        text.push_str(collection);
    }
    if let Some(line) = others_line {
        text.push('\n');
        text.push_str(&line);
    }

    SchemaContext {
        tables: included.len(),
        truncated: cut || !others.is_empty(),
        tokens_estimate: estimate_tokens(&text),
        ddl: text,
    }
}

/// `collection "orders" (100 sampled documents):` e um campo por linha.
/// `max_fields` corta os campos (com uma linha dizendo quantos ficaram de
/// fora).
fn render_collection(shape: &CollectionShape, kind: RelKind, max_fields: Option<usize>) -> String {
    let keyword = match kind {
        RelKind::View | RelKind::MaterializedView => "view",
        _ => "collection",
    };
    let name = qualified(FLAT_SCHEMA, &shape.collection);
    if shape.sampled == 0 {
        return format!("{keyword} {name}: no documents in the sample");
    }
    let plural = if shape.sampled == 1 { "" } else { "s" };
    let mut text = format!("{keyword} {name} ({} sampled document{plural}):", shape.sampled);

    let shown = max_fields.unwrap_or(shape.fields.len()).min(shape.fields.len());
    for field in &shape.fields[..shown] {
        text.push_str(&format!(
            "\n  {} {} {}%",
            quote_ident(&field.path),
            field.types.join("|"),
            field.presence
        ));
    }
    if shown < shape.fields.len() {
        text.push_str(&format!("\n  -- … {} more field(s)", shape.fields.len() - shown));
    } else if shape.truncated {
        text.push_str("\n  -- … more fields exist, only the most frequent are shown");
    }
    text
}

/// A coleção com o máximo de campos que cabe em `room` caracteres (pelo
/// menos um).
fn fit_fields(shape: &CollectionShape, kind: RelKind, room: usize) -> String {
    let mut shown = shape.fields.len();
    loop {
        let text = render_collection(shape, kind, Some(shown));
        if shown <= 1 || text.chars().count() < room {
            return text;
        }
        let ratio = room as f64 / text.chars().count() as f64;
        shown = ((shown as f64 * ratio) as usize).min(shown - 1).max(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::FieldShape;

    fn candidate(table: &str, kind: RelKind, score: u32) -> Candidate {
        Candidate {
            schema: FLAT_SCHEMA.into(),
            table: table.into(),
            kind,
            score,
        }
    }

    fn field(path: &str, types: &[&'static str], presence: u8) -> FieldShape {
        FieldShape {
            path: path.into(),
            types: types.to_vec(),
            presence,
        }
    }

    fn shop() -> (Vec<Candidate>, Vec<CollectionShape>) {
        let candidates = vec![
            candidate("orders", RelKind::Table, 25),
            candidate("active_customers", RelKind::View, 0),
            candidate("audit", RelKind::Table, 0),
        ];
        let shapes = vec![
            CollectionShape {
                collection: "orders".into(),
                sampled: 100,
                fields: vec![
                    field("_id", &["objectId"], 100),
                    field("customer.name", &["string"], 97),
                    field("items[].qty", &["int", "double"], 100),
                    field("na\"me", &["string", "null"], 4),
                ],
                truncated: false,
            },
            CollectionShape {
                collection: "active_customers".into(),
                sampled: 1,
                fields: vec![field("_id", &["objectId"], 100)],
                truncated: false,
            },
            CollectionShape {
                collection: "audit".into(),
                sampled: 0,
                fields: vec![],
                truncated: false,
            },
        ];
        (candidates, shapes)
    }

    fn render_shop(budget_tokens: usize) -> SchemaContext {
        let (candidates, shapes) = shop();
        let chosen: Vec<String> = candidates.iter().map(|c| c.table.clone()).collect();
        render(&ShapeRender {
            candidates: &candidates,
            chosen: &chosen,
            shapes: &shapes,
            budget_tokens,
        })
    }

    #[test]
    fn renders_shapes_in_stable_order() {
        let context = render_shop(4_000);
        assert_eq!(
            context.ddl,
            "-- MongoDB\n\
             view \"active_customers\" (1 sampled document):\n  \"_id\" objectId 100%\n\
             collection \"audit\": no documents in the sample\n\
             collection \"orders\" (100 sampled documents):\n  \"_id\" objectId 100%\n  \"customer.name\" string 97%\n  \"items[].qty\" int|double 100%\n  \"na\"\"me\" string|null 4%"
        );
        assert_eq!((context.tables, context.truncated), (3, false));
        assert_eq!(context.tokens_estimate, estimate_tokens(&context.ddl));
    }

    #[test]
    fn small_budget_keeps_the_best_and_names_the_rest() {
        // Cabe só a primeira do ranking, inteira; as outras ficam pelo nome
        let context = render_shop(60);
        assert_eq!((context.tables, context.truncated), (1, true));
        assert!(context.ddl.contains("  \"items[].qty\" int|double 100%"), "{}", context.ddl);
        assert!(context.ddl.to_lowercase().contains("other collection"), "{}", context.ddl);

        // Nem a primeira cabe: entra com os campos cortados
        let context = render_shop(25);
        assert_eq!((context.tables, context.truncated), (1, true));
        assert!(context.ddl.contains("collection \"orders\""), "{}", context.ddl);
        assert!(context.ddl.contains("  -- … 3 more field(s)"), "{}", context.ddl);
    }

    #[test]
    fn failed_samples_and_truncated_shapes_are_marked() {
        let (candidates, mut shapes) = shop();
        shapes.retain(|shape| shape.collection != "audit");
        shapes[0].truncated = true;
        let chosen: Vec<String> = candidates.iter().map(|c| c.table.clone()).collect();
        let context = render(&ShapeRender {
            candidates: &candidates,
            chosen: &chosen,
            shapes: &shapes,
            budget_tokens: 4_000,
        });
        assert!(context.truncated);
        assert!(context.ddl.contains("more fields exist"), "{}", context.ddl);
        // A amostra que falhou: a coleção existe, sem campos
        assert!(context.ddl.ends_with("-- Other collections (fields not shown): \"audit\""), "{}", context.ddl);
    }
}

/// O caminho inteiro (catálogo → ranking → `$sample` → texto) contra o Mongo
/// dos e2e, com canários: nenhum valor dos documentos chega ao contexto.
#[cfg(test)]
mod e2e {
    use mongodb::bson::{doc, Document};

    use super::*;
    use crate::models::{DatabaseType, Server, SslMode};
    use crate::services::SourceOpener;
    use crate::storage::vault;

    fn server() -> Server {
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

    /// `cargo test --lib assistant::shape_context -- --ignored`
    #[tokio::test]
    #[ignore = "requires local mongodb on localhost:27017 (no auth)"]
    async fn e2e_mongo_context_without_values() {
        const DATABASE: &str = "octapus_e2e_assistant_shapes";
        let client = mongodb::Client::with_uri_str("mongodb://localhost:27017").await.unwrap();
        let db = client.database(DATABASE);
        db.drop().await.unwrap();
        let orders: Vec<Document> = (0..300)
            .map(|i| {
                doc! {
                    "status": if i % 3 == 0 { "CANARY_status_paid" } else { "CANARY_status_open" },
                    "total": i as f64 * 1.5,
                    "customer": { "name": format!("CANARY_name_{i}"), "email": format!("canary{i}@exemplo.com") },
                    "items": [{ "sku": "CANARY_sku", "qty": i % 4 }],
                    "scores": { format!("CANARY_user_{i}"): i },
                }
            })
            .collect();
        db.collection::<Document>("orders").insert_many(orders).await.unwrap();
        for i in 0..20 {
            db.collection::<Document>(&format!("noise_{i:02}")).insert_one(doc! { "v": "CANARY_noise" }).await.unwrap();
        }

        vault::init_for_tests();
        let dir = std::env::temp_dir().join(format!("octapus-shape-context-{}", std::process::id()));
        let service = CatalogService::default();
        service.init(dir.clone(), Arc::new(|_| {}));
        let opener: SourceOpener = Arc::new(|| {
            Box::pin(async { crate::adapters::create_catalog_source(&server(), DATABASE).await })
        });
        let entry = service.open(1, DATABASE, "mongo-shapes".into(), opener).await;

        let schemas = [FLAT_SCHEMA.to_string()];
        let context = super::super::schema_context::build(
            &service,
            &entry,
            DatabaseType::Mongodb,
            ContextRequest {
                schemas: &schemas,
                question: "total dos orders por status",
                history: &[],
                context_window: 200_000,
            },
        )
        .await
        .unwrap();

        assert!(context.ddl.starts_with("-- MongoDB\n"), "{}", context.ddl);
        assert!(context.ddl.contains("collection \"orders\" (100 sampled documents):"), "{}", context.ddl);
        assert!(context.ddl.contains("  \"customer.email\" string 100%"), "{}", context.ddl);
        assert!(context.ddl.contains("  \"items[].qty\" int 100%"), "{}", context.ddl);
        assert!(context.ddl.contains("  \"scores.{key}\" int"), "{}", context.ddl);
        // 21 coleções, 12 amostradas: as outras vão pelo nome
        assert_eq!(context.tables, MAX_COLLECTIONS);
        assert!(context.truncated);
        for canary in ["CANARY", "canary", "exemplo.com"] {
            assert!(!context.ddl.contains(canary), "{canary} vazou: {}", context.ddl);
        }

        drop(entry);
        let _ = std::fs::remove_dir_all(dir);
        db.drop().await.unwrap();
    }
}
