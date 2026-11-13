use std::collections::HashSet;
use std::mem::size_of;

use super::base::{Catalog, SchemaEntry};
use super::context::{question_terms, score_name, Candidate};
use super::drift::DriftLens;
use super::model::{
    CatalogNode, CatalogPath, CatalogStats, LoadState, NodeKind, Page, RelKind, Resolution,
    SchemaGroup,
};
use super::shape::{Shape, ShapeId, ShapeTable};

/// Quantos schemas de exemplo acompanham um resultado agrupado.
pub(super) const SAMPLE_SIZE: usize = 3;

/// Bônus no ranking do contexto para a relação citada com `@` numa pergunta
/// anterior da conversa: passa à frente do que só casou pelo histórico, mas
/// um nome inteiro na pergunta atual (30) ainda vence.
pub const PREFERRED_SCORE: u32 = 20;

/// Filtro "contém" sem diferenciar maiúsculas (o campo de filtro da árvore).
pub(super) struct Needle {
    lower: String,
    ascii: bool,
}

impl Needle {
    pub fn new(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        (!trimmed.is_empty()).then(|| Self {
            lower: trimmed.to_lowercase(),
            ascii: trimmed.is_ascii(),
        })
    }

    pub fn matches(&self, haystack: &str) -> bool {
        if self.ascii && haystack.is_ascii() {
            let needle = self.lower.as_bytes();
            needle.len() <= haystack.len()
                && haystack
                    .as_bytes()
                    .windows(needle.len())
                    .any(|window| window.eq_ignore_ascii_case(needle))
        } else {
            haystack.to_lowercase().contains(&self.lower)
        }
    }

    pub fn is_prefix_of(&self, haystack: &str) -> bool {
        if self.ascii && haystack.is_ascii() {
            haystack.len() >= self.lower.len()
                && haystack.as_bytes()[..self.lower.len()].eq_ignore_ascii_case(self.lower.as_bytes())
        } else {
            haystack.to_lowercase().starts_with(&self.lower)
        }
    }
}

/// Uma janela de `limit` itens a partir de `offset`, contando o total.
pub(super) fn paginate<I, T>(items: I, offset: usize, limit: usize, mut map: impl FnMut(I::Item) -> T) -> Page<T>
where
    I: Iterator,
{
    let mut total = 0;
    let mut window = Vec::with_capacity(limit.min(256));
    for item in items {
        if total >= offset && window.len() < limit {
            window.push(map(item));
        }
        total += 1;
    }
    Page {
        total,
        offset,
        items: window,
    }
}

impl Catalog {
    /// Filhos diretos de um nó, já filtrados e paginados. `None` = o nó não
    /// existe; um schema ainda não carregado devolve uma página vazia (o estado
    /// dele vem na listagem de schemas).
    pub fn children(
        &self,
        path: &CatalogPath,
        filter: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> Option<Page<CatalogNode>> {
        let needle = filter.and_then(Needle::new);
        let keep = |name: &str| needle.as_ref().is_none_or(|needle| needle.matches(name));
        let empty = || Page {
            total: 0,
            offset,
            items: Vec::new(),
        };

        match path {
            CatalogPath::Schemas => {
                let mut lens = self.drift_lens();
                Some(paginate(
                    self.schemas.iter().filter(|(name, _)| keep(name)),
                    offset,
                    limit,
                    |(name, entry)| self.schema_node(name, entry, &mut lens),
                ))
            }
            CatalogPath::ShapeSchemas(key) => {
                let members = self.group_members(key)?;
                let mut lens = self.drift_lens();
                Some(paginate(
                    self.schemas
                        .iter()
                        .filter(|(name, entry)| members.contains(entry.shape) && keep(name)),
                    offset,
                    limit,
                    |(name, entry)| self.schema_node(name, entry, &mut lens),
                ))
            }
            CatalogPath::Schema(schema) | CatalogPath::Tables(schema) => {
                let entry = self.schemas.get(schema.as_str())?;
                let Some(shape) = entry.shape else {
                    return Some(empty());
                };
                let shape = &self.shapes[shape.index()];
                let loose = matches!(path, CatalogPath::Tables(_));
                let is_view = |kind: RelKind| matches!(kind, RelKind::View | RelKind::MaterializedView);
                Some(paginate(
                    shape.tables.iter().filter(|table| {
                        table.parent.is_none()
                            && !(loose && is_view(table.kind))
                            && keep(self.names.resolve(table.name))
                    }),
                    offset,
                    limit,
                    |table| self.relation_node(shape, table),
                ))
            }
            CatalogPath::Partitions { schema, table } => {
                let shape = self.shape_of(schema)?;
                let parent = shape.find(&self.names, table)?;
                if parent.kind != RelKind::Partitioned {
                    return Some(empty());
                }
                Some(paginate(
                    shape
                        .partitions_of(parent.name)
                        .filter(|partition| keep(self.names.resolve(partition.name))),
                    offset,
                    limit,
                    |partition| self.relation_node(shape, partition),
                ))
            }
        }
    }

    fn schema_node(&self, name: &str, entry: &SchemaEntry, lens: &mut DriftLens<'_>) -> CatalogNode {
        CatalogNode {
            name: name.to_string(),
            kind: NodeKind::Schema,
            child_count: entry.shape.map(|shape| self.shapes[shape.index()].top_level),
            state: Some(entry.state),
            drift: lens.drift_of(entry.shape),
        }
    }

    fn relation_node(&self, shape: &Shape, table: &ShapeTable) -> CatalogNode {
        CatalogNode {
            name: self.names.resolve(table.name).to_string(),
            kind: table.kind.into(),
            child_count: (table.kind == RelKind::Partitioned)
                .then(|| shape.partitions_of(table.name).count() as u32),
            state: None,
            drift: None,
        }
    }

    /// Schemas que começam com `prefix` (sem diferenciar maiúsculas).
    pub fn complete_schemas(&self, prefix: &str, limit: usize) -> Vec<String> {
        let needle = Needle::new(prefix);
        self.schemas
            .keys()
            .filter(|name| needle.as_ref().is_none_or(|needle| needle.is_prefix_of(name)))
            .take(limit)
            .map(|name| name.to_string())
            .collect()
    }

    /// Relações de um schema que começam com `prefix`, partições incluídas
    /// (dá para consultar uma partição direto). `None` = schema desconhecido.
    pub fn complete_tables(&self, schema: &str, prefix: &str, limit: usize) -> Option<Vec<CatalogNode>> {
        self.schemas.get(schema)?;
        let Some(shape) = self.shape_of(schema) else {
            return Some(Vec::new());
        };
        let needle = Needle::new(prefix);
        Some(
            shape
                .tables
                .iter()
                .filter(|table| {
                    needle
                        .as_ref()
                        .is_none_or(|needle| needle.is_prefix_of(self.names.resolve(table.name)))
                })
                .take(limit)
                .map(|table| self.relation_node(shape, table))
                .collect(),
        )
    }

    /// A que relação `table` (escrita sem schema) se refere: o primeiro schema do
    /// `search_path` que a tiver; fora dele, o único schema que a tiver — ou
    /// ambíguo. O nome exato tem prioridade sobre a comparação sem caixa (o
    /// Postgres dobra identificadores sem aspas para minúsculas).
    pub fn resolve(&self, table: &str, search_path: &[&str]) -> Resolution {
        let exact = self.names.get(table);
        let candidates: Vec<_> = exact
            .into_iter()
            .chain(
                self.names
                    .get_folded(table)
                    .iter()
                    .copied()
                    .filter(|&sym| Some(sym) != exact),
            )
            .collect();

        for &schema in search_path {
            let Some(shape) = self.shape_of(schema) else {
                continue;
            };
            for &sym in &candidates {
                if let Some(found) = shape.find(&self.names, self.names.resolve(sym)) {
                    return Resolution::Found {
                        schema: schema.to_string(),
                        table: self.names.resolve(sym).to_string(),
                        kind: found.kind.into(),
                    };
                }
            }
        }

        for &sym in &candidates {
            let shapes = self.by_table.shapes(sym);
            if shapes.is_empty() {
                continue;
            }
            let name = self.names.resolve(sym).to_string();
            let group = self.schemas_using(&shapes, SAMPLE_SIZE);

            match group.total {
                0 => continue,
                1 => {
                    let schema = group.sample[0].clone();
                    let kind = self
                        .shape_of(&schema)
                        .and_then(|shape| shape.find(&self.names, &name))
                        .map_or(NodeKind::Table, |table| table.kind.into());
                    return Resolution::Found {
                        schema,
                        table: name,
                        kind,
                    };
                }
                _ => {
                    return Resolution::Ambiguous {
                        table: name,
                        schemas: group,
                    }
                }
            }
        }

        Resolution::NotFound
    }

    /// O schema `name` como está no catálogo: o nome exato ou, sem ele, o único
    /// que só muda nas maiúsculas. `None` = não existe (ou a lista de schemas
    /// ainda não chegou). Confere as menções com `@` do assistente.
    pub fn find_schema(&self, name: &str) -> Option<String> {
        if self.schemas.contains_key(name) {
            return Some(name.to_string());
        }
        let lower = name.to_lowercase();
        let mut folded = self.schemas.keys().filter(|schema| schema.to_lowercase() == lower);
        match (folded.next(), folded.next()) {
            (Some(schema), None) => Some(schema.to_string()),
            _ => None,
        }
    }

    /// A relação `table` de um schema carregado, partições incluídas, como está
    /// no catálogo: o nome exato ou, sem ele, a grafia que só muda nas
    /// maiúsculas. `None` = não existe ou o schema não foi carregado.
    pub fn find_relation(&self, schema: &str, table: &str) -> Option<(String, RelKind)> {
        let shape = self.shape_of(schema)?;
        let exact = self.names.get(table);
        exact
            .into_iter()
            .chain(self.names.get_folded(table).into_iter().filter(|&sym| Some(sym) != exact))
            .find_map(|sym| {
                let name = self.names.resolve(sym);
                shape.find(&self.names, name).map(|found| (name.to_string(), found.kind))
            })
    }

    /// As relações dos `schemas` como candidatas ao contexto do assistente, da
    /// mais parecida com a pergunta para a menos (empate: schema e nome, o que
    /// deixa o resultado determinístico). Partições ficam de fora — o pai as
    /// representa — e schemas não carregados não entram (quem chama garante a
    /// carga antes). O histórico da conversa pesa metade da pergunta atual, e
    /// as relações citadas com `@` em perguntas anteriores (`preferred`) ganham
    /// [`PREFERRED_SCORE`] a mais.
    pub fn context_candidates(
        &self,
        schemas: &[&str],
        question: &str,
        history: &[&str],
        preferred: &[(String, String)],
    ) -> Vec<Candidate> {
        let current: HashSet<String> = question_terms(question).into_iter().collect();
        let past: HashSet<String> = history
            .iter()
            .flat_map(|text| question_terms(text))
            .filter(|term| !current.contains(term))
            .collect();

        let mut seen = HashSet::new();
        let mut candidates = Vec::new();
        for &schema in schemas {
            if !seen.insert(schema) {
                continue;
            }
            let Some(shape) = self.shape_of(schema) else {
                continue;
            };
            for table in shape.tables.iter().filter(|table| table.parent.is_none()) {
                let name = self.names.resolve(table.name);
                let cited = preferred.iter().any(|(s, t)| s == schema && t == name);
                candidates.push(Candidate {
                    schema: schema.to_string(),
                    table: name.to_string(),
                    kind: table.kind,
                    score: score_name(name, &current) * 2
                        + score_name(name, &past)
                        + if cited { PREFERRED_SCORE } else { 0 },
                });
            }
        }
        candidates.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.schema.cmp(&b.schema))
                .then_with(|| a.table.cmp(&b.table))
        });
        candidates
    }

    /// Quantos schemas usam algum dos formatos, com os primeiros em ordem de nome.
    pub(super) fn schemas_using(&self, shapes: &[ShapeId], sample_size: usize) -> SchemaGroup {
        let total = shapes
            .iter()
            .map(|shape| self.members[shape.index()] as usize)
            .sum();
        let wanted: HashSet<ShapeId> = shapes.iter().copied().collect();
        let sample = self
            .schemas
            .iter()
            .filter(|(_, entry)| entry.shape.is_some_and(|shape| wanted.contains(&shape)))
            .take(sample_size)
            .map(|(name, _)| name.to_string())
            .collect();
        SchemaGroup { total, sample }
    }

    /// Quantos schemas o database tem, se a lista já é conhecida (camada 0).
    pub fn schema_count(&self) -> Option<usize> {
        self.knows_schemas().then_some(self.schemas.len())
    }

    pub fn stats(&self) -> CatalogStats {
        let mut stats = CatalogStats {
            schemas: self.schemas.len(),
            loaded: 0,
            stale: 0,
            unloaded: 0,
            shapes: self.live_shapes().count(),
            distinct_names: self.by_table.len(),
            relations: 0,
            approx_heap_bytes: self.approx_heap_bytes(),
        };
        for entry in self.schemas.values() {
            match entry.state {
                LoadState::Loaded => stats.loaded += 1,
                LoadState::Stale => stats.stale += 1,
                LoadState::Unloaded => stats.unloaded += 1,
            }
            if let Some(shape) = entry.shape {
                stats.relations += self.shapes[shape.index()].tables.len();
            }
        }
        stats
    }

    /// Estimativa do heap ocupado — conferida contra um alocador contador nos
    /// testes de desempenho (`perf::catalog_core`).
    fn approx_heap_bytes(&self) -> usize {
        // Nó do BTreeMap + Box<str> + SchemaEntry, por schema
        let schemas: usize = self
            .schemas
            .keys()
            .map(|name| name.len() + 48 + size_of::<super::base::SchemaEntry>())
            .sum();
        let shapes: usize = self
            .shapes
            .iter()
            .map(|shape| shape.tables.len() * size_of::<ShapeTable>() + size_of::<Shape>() + 4)
            .sum();
        self.names.approx_heap_bytes()
            + schemas
            + shapes
            + self.objects.approx_heap_bytes()
            + self.members.capacity() * 4
            + self.by_table.approx_heap_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::model::{SchemaHeader, TableRecord};

    fn sample_catalog() -> Catalog {
        let mut catalog = Catalog::new();
        catalog.set_schemas(
            ["public", "tenant_001", "tenant_002", "tenant_003", "events"]
                .into_iter()
                .map(|name| SchemaHeader {
                    name: name.into(),
                    fingerprint: None,
                }),
        );
        let tenant = || {
            vec![
                TableRecord::new("orders", RelKind::Table),
                TableRecord::new("customers", RelKind::Table),
                TableRecord::new("order_totals", RelKind::View),
            ]
        };
        catalog.set_tables("tenant_001", tenant(), None);
        catalog.set_tables("tenant_002", tenant(), None);
        catalog.set_tables("public", vec![TableRecord::new("plans", RelKind::Table)], None);
        catalog.set_tables(
            "events",
            vec![
                TableRecord::new("event_log", RelKind::Partitioned),
                TableRecord::partition("event_log_p1", "event_log"),
                TableRecord::partition("event_log_p2", "event_log"),
                TableRecord::new("Orders", RelKind::Table),
            ],
            None,
        );
        catalog
    }

    fn names(page: &Page<CatalogNode>) -> Vec<&str> {
        page.items.iter().map(|node| node.name.as_str()).collect()
    }

    #[test]
    fn lists_schemas_with_state_and_counts() {
        let catalog = sample_catalog();
        let page = catalog.children(&CatalogPath::Schemas, None, 0, 10).unwrap();

        assert_eq!(page.total, 5);
        assert_eq!(names(&page), ["events", "public", "tenant_001", "tenant_002", "tenant_003"]);
        assert_eq!(page.items[2].child_count, Some(3));
        assert_eq!(page.items[4].state, Some(LoadState::Unloaded));
        assert_eq!(page.items[4].child_count, None);
    }

    #[test]
    fn paginates_and_filters_without_case() {
        let catalog = sample_catalog();
        let page = catalog
            .children(&CatalogPath::Schemas, Some(" TENANT_00 "), 1, 1)
            .unwrap();
        assert_eq!(page.total, 3);
        assert_eq!(page.offset, 1);
        assert_eq!(names(&page), ["tenant_002"]);
    }

    #[test]
    fn partitions_live_under_their_parent() {
        let catalog = sample_catalog();
        let schema = catalog
            .children(&CatalogPath::Schema("events".into()), None, 0, 10)
            .unwrap();
        assert_eq!(names(&schema), ["Orders", "event_log"]);
        assert_eq!(schema.items[1].kind, NodeKind::Partitioned);
        assert_eq!(schema.items[1].child_count, Some(2));

        let partitions = catalog
            .children(
                &CatalogPath::Partitions {
                    schema: "events".into(),
                    table: "event_log".into(),
                },
                None,
                0,
                10,
            )
            .unwrap();
        assert_eq!(names(&partitions), ["event_log_p1", "event_log_p2"]);
    }

    #[test]
    fn unknown_nodes_are_none_and_unloaded_schemas_are_empty() {
        let catalog = sample_catalog();
        assert!(catalog
            .children(&CatalogPath::Schema("nope".into()), None, 0, 10)
            .is_none());
        let unloaded = catalog
            .children(&CatalogPath::Schema("tenant_003".into()), None, 0, 10)
            .unwrap();
        assert_eq!(unloaded.total, 0);
    }

    #[test]
    fn completes_by_prefix() {
        let catalog = sample_catalog();
        assert_eq!(catalog.complete_schemas("TEN", 2), ["tenant_001", "tenant_002"]);
        let tables: Vec<_> = catalog
            .complete_tables("tenant_001", "ord", 10)
            .unwrap()
            .into_iter()
            .map(|node| node.name)
            .collect();
        assert_eq!(tables, ["order_totals", "orders"]);
        assert!(catalog.complete_tables("nope", "", 10).is_none());
    }

    #[test]
    fn resolves_through_search_path_then_uniqueness() {
        let catalog = sample_catalog();

        // No search_path
        assert_eq!(
            catalog.resolve("orders", &["tenant_002", "public"]),
            Resolution::Found {
                schema: "tenant_002".into(),
                table: "orders".into(),
                kind: NodeKind::Table,
            }
        );
        // Fora dele e único
        assert_eq!(
            catalog.resolve("plans", &["tenant_001"]),
            Resolution::Found {
                schema: "public".into(),
                table: "plans".into(),
                kind: NodeKind::Table,
            }
        );
        // Fora dele e em vários schemas
        assert_eq!(
            catalog.resolve("customers", &[]),
            Resolution::Ambiguous {
                table: "customers".into(),
                schemas: SchemaGroup {
                    total: 2,
                    sample: vec!["tenant_001".into(), "tenant_002".into()],
                },
            }
        );
        // O nome exato ganha da comparação sem caixa
        assert_eq!(
            catalog.resolve("Orders", &[]),
            Resolution::Found {
                schema: "events".into(),
                table: "Orders".into(),
                kind: NodeKind::Table,
            }
        );
        assert_eq!(
            catalog.resolve("CUSTOMERS", &["tenant_001"]),
            Resolution::Found {
                schema: "tenant_001".into(),
                table: "customers".into(),
                kind: NodeKind::Table,
            }
        );
        assert_eq!(catalog.resolve("missing", &[]), Resolution::NotFound);
    }

    #[test]
    fn stats_count_shapes_not_copies() {
        let stats = sample_catalog().stats();
        assert_eq!(stats.schemas, 5);
        assert_eq!(stats.loaded, 4);
        assert_eq!(stats.unloaded, 1);
        assert_eq!(stats.shapes, 3);
        assert_eq!(stats.relations, 3 + 3 + 1 + 4);
        assert!(stats.approx_heap_bytes > 0);
    }

    #[test]
    fn schema_windows_carry_drift_and_shape_groups_filter_members() {
        let mut catalog = Catalog::new();
        let full = ["customers", "invoices", "orders", "payments"];
        let records = |names: &[&str]| names.iter().map(|n| TableRecord::new(*n, RelKind::Table)).collect::<Vec<_>>();
        for i in 1..=5 {
            catalog.set_tables(&format!("tenant_{i}"), records(&full), None);
        }
        catalog.set_tables("tenant_6", records(&full[..3]), None);
        catalog.set_tables("public", records(&["plans"]), None);

        let drift_of = |page: &Page<CatalogNode>, name: &str| {
            page.items.iter().find(|node| node.name == name).unwrap().drift
        };
        let all = catalog.children(&CatalogPath::Schemas, None, 0, 100).unwrap();
        assert_eq!(drift_of(&all, "tenant_1"), None);
        assert_eq!(drift_of(&all, "public"), None);
        assert_eq!(
            drift_of(&all, "tenant_6"),
            Some(crate::catalog::model::SchemaDrift { missing: 1, extra: 0 })
        );

        let groups = catalog.shape_groups();
        let dominant = catalog
            .children(&CatalogPath::ShapeSchemas(groups[0].key.clone()), Some("tenant_"), 0, 3)
            .unwrap();
        assert_eq!(dominant.total, 5);
        assert_eq!(dominant.items.len(), 3);
        let behind = catalog
            .children(&CatalogPath::ShapeSchemas(groups[1].key.clone()), None, 0, 10)
            .unwrap();
        assert_eq!(behind.items.iter().map(|node| node.name.as_str()).collect::<Vec<_>>(), ["tenant_6"]);
        assert!(behind.items[0].drift.is_some());
        let other = catalog
            .children(&CatalogPath::ShapeSchemas("other".into()), None, 0, 10)
            .unwrap();
        assert_eq!(other.items[0].name, "public");
        // Grupo que sumiu (o catálogo mudou): o nó não existe mais
        assert!(catalog.children(&CatalogPath::ShapeSchemas("ffff".into()), None, 0, 10).is_none());
    }

    #[test]
    fn context_candidates_rank_by_the_question_and_skip_partitions() {
        let catalog = sample_catalog();

        let ranked = catalog.context_candidates(&["tenant_001", "public"], "total dos pedidos (orders)", &[], &[]);
        let names: Vec<_> = ranked.iter().map(|c| (c.schema.as_str(), c.table.as_str())).collect();
        assert_eq!(
            names,
            [
                ("tenant_001", "orders"),
                ("tenant_001", "order_totals"),
                ("public", "plans"),
                ("tenant_001", "customers"),
            ]
        );
        assert!(ranked[0].score > ranked[1].score && ranked[1].score > 0);
        assert_eq!(ranked[2].score, 0);

        // O histórico ajuda, com metade do peso
        let with_history =
            catalog.context_candidates(&["tenant_001"], "e o total?", &["quais os clientes (customers)?"], &[]);
        assert_eq!(with_history[0].table, "customers");

        // Partições ficam de fora; schema não carregado não entra. "eventos"
        // acha `event_log` pelo prefixo comum (pergunta em português, nome em inglês)
        let events = catalog.context_candidates(&["events", "tenant_003", "nope"], "eventos", &[], &[]);
        let names: Vec<_> = events.iter().map(|c| c.table.as_str()).collect();
        assert_eq!(names, ["event_log", "Orders"]);

        // Citada com @ numa pergunta anterior: passa à frente do que só casou
        // pelo histórico, mas não do nome inteiro da pergunta atual
        let preferred = [("tenant_001".to_string(), "order_totals".to_string())];
        let history = ["quais os clientes (customers)?"];
        let plain = catalog.context_candidates(&["tenant_001"], "e os pedidos (orders)?", &history, &[]);
        let names: Vec<_> = plain.iter().map(|c| c.table.as_str()).collect();
        assert_eq!(names, ["orders", "customers", "order_totals"]);
        let cited = catalog.context_candidates(&["tenant_001"], "e os pedidos (orders)?", &history, &preferred);
        let names: Vec<_> = cited.iter().map(|c| c.table.as_str()).collect();
        assert_eq!(names, ["orders", "order_totals", "customers"]);
    }

    #[test]
    fn finds_mentioned_schemas_and_relations_by_exact_or_folded_name() {
        let catalog = sample_catalog();
        assert_eq!(catalog.find_schema("tenant_002").as_deref(), Some("tenant_002"));
        assert_eq!(catalog.find_schema("TENANT_002").as_deref(), Some("tenant_002"));
        assert_eq!(catalog.find_schema("tenant_999"), None);

        assert_eq!(
            catalog.find_relation("tenant_001", "orders"),
            Some(("orders".to_string(), RelKind::Table))
        );
        // O nome exato vence; sem ele, a grafia que só muda nas maiúsculas
        assert_eq!(
            catalog.find_relation("events", "Orders"),
            Some(("Orders".to_string(), RelKind::Table))
        );
        assert_eq!(
            catalog.find_relation("events", "orders"),
            Some(("Orders".to_string(), RelKind::Table))
        );
        assert_eq!(
            catalog.find_relation("tenant_001", "ORDER_TOTALS"),
            Some(("order_totals".to_string(), RelKind::View))
        );
        // Partição dá para citar; schema não carregado e nome inexistente não
        assert!(catalog.find_relation("events", "event_log_p1").is_some());
        assert_eq!(catalog.find_relation("tenant_003", "orders"), None);
        assert_eq!(catalog.find_relation("tenant_001", "nope"), None);
    }
}
