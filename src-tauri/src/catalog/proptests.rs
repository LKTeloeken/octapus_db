//! Testes de propriedade: para entradas aleatórias, o catálogo tem que
//! responder igual a uma implementação ingênua (mapas e laços, sem formatos nem
//! índices) — listagem, paginação, filtro, resolução de nomes, busca e a ida e
//! volta pelo arquivo.

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;

use super::persist::tests::observable;
use super::persist::{decode, encode};
use super::model::SchemaGroup;
use super::{
    Catalog, CatalogNode, CatalogPath, LoadState, NodeKind, RelKind, Resolution, SchemaHeader,
    TableRecord,
};
use crate::storage::vault;

/// Nomes poucos e repetidos: força schemas iguais (formatos compartilhados) e
/// o par `orders`/`Orders` exercita a comparação sem caixa.
const POOL: &[&str] = &[
    "orders", "Orders", "customers", "invoices", "order_items", "payments", "users", "tags",
];

type Relation = (String, RelKind, Option<String>);
type Input = BTreeMap<String, Vec<TableRecord>>;

fn kind() -> impl Strategy<Value = RelKind> {
    prop_oneof![
        Just(RelKind::Table),
        Just(RelKind::View),
        Just(RelKind::MaterializedView),
        Just(RelKind::Foreign),
    ]
}

fn schema_tables() -> impl Strategy<Value = Vec<TableRecord>> {
    (
        prop::collection::vec((0..POOL.len(), kind()), 0..8),
        prop::option::of(1usize..4),
        any::<bool>(),
    )
        .prop_map(|(picks, partitions, orphan)| {
            let mut records: Vec<_> = picks
                .into_iter()
                .map(|(index, kind)| TableRecord::new(POOL[index], kind))
                .collect();
            if let Some(count) = partitions {
                records.push(TableRecord::new("measurements", RelKind::Partitioned));
                for p in 0..count {
                    records.push(TableRecord::partition(format!("measurements_p{p}"), "measurements"));
                }
            }
            if orphan {
                records.push(TableRecord::partition("stray_p1", "stray"));
            }
            records
        })
}

fn input() -> impl Strategy<Value = Input> {
    prop::collection::btree_map("[a-c][0-9]{0,2}", schema_tables(), 0..25)
}

/// O que o catálogo deveria guardar: primeira ocorrência de cada nome, ordem de
/// bytes, partição só com pai presente.
fn normalize(records: &[TableRecord]) -> Vec<Relation> {
    let mut seen = BTreeSet::new();
    let kept: Vec<&TableRecord> = records
        .iter()
        .filter(|record| seen.insert(record.name.clone()))
        .collect();
    let mut relations: Vec<Relation> = kept
        .iter()
        .map(|record| {
            let parent = record
                .partition_of
                .clone()
                .filter(|parent| parent != &record.name && seen.contains(parent));
            (record.name.clone(), record.kind, parent)
        })
        .collect();
    relations.sort_by(|a, b| a.0.cmp(&b.0));
    relations
}

fn naive(input: &Input) -> BTreeMap<String, Vec<Relation>> {
    input
        .iter()
        .map(|(schema, records)| (schema.clone(), normalize(records)))
        .collect()
}

fn build(input: &Input) -> Catalog {
    let mut catalog = Catalog::new();
    for (schema, records) in input {
        catalog.set_tables(schema, records.clone(), None);
    }
    catalog
}

fn all(catalog: &Catalog, path: CatalogPath, filter: Option<&str>) -> Vec<CatalogNode> {
    catalog
        .children(&path, filter, 0, usize::MAX)
        .expect("nó existente")
        .items
}

fn naive_resolve(model: &BTreeMap<String, Vec<Relation>>, table: &str, path: &[&str]) -> Resolution {
    let has = |schema: &str, name: &str| {
        model
            .get(schema)
            .and_then(|relations| relations.iter().find(|r| r.0 == name))
            .map(|r| r.1)
    };
    let names: BTreeSet<&str> = model.values().flatten().map(|r| r.0.as_str()).collect();
    let mut candidates: Vec<&str> = names.iter().copied().filter(|n| *n == table).collect();
    candidates.extend(
        names
            .iter()
            .copied()
            .filter(|n| *n != table && n.to_lowercase() == table.to_lowercase()),
    );

    for &schema in path {
        for &candidate in &candidates {
            if let Some(kind) = has(schema, candidate) {
                return Resolution::Found {
                    schema: schema.into(),
                    table: candidate.into(),
                    kind: kind.into(),
                };
            }
        }
    }
    for &candidate in &candidates {
        let schemas: Vec<&String> = model
            .keys()
            .filter(|schema| has(schema, candidate).is_some())
            .collect();
        match schemas.len() {
            0 => continue,
            1 => {
                return Resolution::Found {
                    schema: schemas[0].clone(),
                    table: candidate.into(),
                    kind: has(schemas[0], candidate).unwrap().into(),
                }
            }
            total => {
                return Resolution::Ambiguous {
                    table: candidate.into(),
                    schemas: SchemaGroup {
                        total,
                        sample: schemas.iter().take(3).map(|s| s.to_string()).collect(),
                    },
                }
            }
        }
    }
    Resolution::NotFound
}

/// `needle` aparece em `haystack` como subsequência (sem caixa) — o mínimo que
/// um resultado fuzzy precisa cumprir.
fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut chars = haystack.chars().flat_map(char::to_lowercase);
    needle
        .chars()
        .flat_map(char::to_lowercase)
        .all(|wanted| chars.any(|c| c == wanted))
}

proptest! {
    #[test]
    fn listing_matches_the_naive_model(input in input()) {
        let catalog = build(&input);
        let model = naive(&input);

        let schemas = all(&catalog, CatalogPath::Schemas, None);
        prop_assert_eq!(
            schemas.iter().map(|n| n.name.clone()).collect::<Vec<_>>(),
            model.keys().cloned().collect::<Vec<_>>()
        );

        for (node, (schema, relations)) in schemas.iter().zip(&model) {
            let top: Vec<&Relation> = relations.iter().filter(|r| r.2.is_none()).collect();
            prop_assert_eq!(node.child_count, Some(top.len() as u32));

            let listed = all(&catalog, CatalogPath::Schema(schema.clone()), None);
            prop_assert_eq!(
                listed.iter().map(|n| (n.name.clone(), n.kind)).collect::<Vec<_>>(),
                top.iter().map(|r| (r.0.clone(), NodeKind::from(r.1))).collect::<Vec<_>>()
            );

            for parent in top.iter().filter(|r| r.1 == RelKind::Partitioned) {
                let partitions = all(
                    &catalog,
                    CatalogPath::Partitions { schema: schema.clone(), table: parent.0.clone() },
                    None,
                );
                let expected: Vec<String> = relations
                    .iter()
                    .filter(|r| r.2.as_deref() == Some(parent.0.as_str()))
                    .map(|r| r.0.clone())
                    .collect();
                prop_assert_eq!(partitions.iter().map(|n| n.name.clone()).collect::<Vec<_>>(), expected);
            }
        }
    }

    #[test]
    fn pages_concatenate_to_the_full_list(input in input(), size in 1usize..7) {
        let catalog = build(&input);
        let full = all(&catalog, CatalogPath::Schemas, None);

        let mut paged = Vec::new();
        let mut offset = 0;
        loop {
            let page = catalog.children(&CatalogPath::Schemas, None, offset, size).unwrap();
            prop_assert_eq!(page.total, full.len());
            if page.items.is_empty() {
                break;
            }
            offset += page.items.len();
            paged.extend(page.items);
        }
        prop_assert_eq!(paged, full);
    }

    #[test]
    fn filter_is_case_insensitive_contains(input in input(), filter in "[a-cA-C0-9_]{1,2}") {
        let catalog = build(&input);
        let model = naive(&input);
        let lower = filter.to_lowercase();

        let schemas = all(&catalog, CatalogPath::Schemas, Some(&filter));
        let expected: Vec<&String> = model.keys().filter(|s| s.to_lowercase().contains(&lower)).collect();
        prop_assert_eq!(schemas.iter().map(|n| &n.name).collect::<Vec<_>>(), expected);

        for (schema, relations) in &model {
            let listed = all(&catalog, CatalogPath::Schema(schema.clone()), Some(&filter));
            let expected: Vec<&String> = relations
                .iter()
                .filter(|r| r.2.is_none() && r.0.to_lowercase().contains(&lower))
                .map(|r| &r.0)
                .collect();
            prop_assert_eq!(listed.iter().map(|n| &n.name).collect::<Vec<_>>(), expected);
        }
    }

    #[test]
    fn identical_schemas_share_one_shape(input in input()) {
        let catalog = build(&input);
        let distinct: BTreeSet<Vec<Relation>> = naive(&input).into_values().collect();
        let stats = catalog.stats();

        prop_assert_eq!(stats.shapes, distinct.len());
        prop_assert_eq!(stats.relations, naive(&input).values().map(Vec::len).sum::<usize>());
        prop_assert_eq!(catalog.members.iter().sum::<u32>() as usize, input.len());
    }

    #[test]
    fn resolve_matches_the_naive_model(
        input in input(),
        table in prop::sample::select(vec![
            "orders", "Orders", "customers", "CUSTOMERS", "Invoices", "measurements", "missing",
        ]),
        path_picks in prop::collection::vec(any::<prop::sample::Index>(), 0..3),
    ) {
        let catalog = build(&input);
        let model = naive(&input);
        let schemas: Vec<&str> = model.keys().map(String::as_str).collect();
        let path: Vec<&str> = if schemas.is_empty() {
            Vec::new()
        } else {
            path_picks.iter().map(|pick| *pick.get(&schemas)).collect()
        };

        prop_assert_eq!(catalog.resolve(table, &path), naive_resolve(&model, table, &path));
    }

    #[test]
    fn search_finds_every_name_and_only_matches(input in input(), query in "[a-z]{1,3}") {
        let catalog = build(&input);
        let model = naive(&input);

        for name in catalog.distinct_relation_names() {
            let holders = model
                .values()
                .filter(|relations| relations.iter().any(|r| r.0 == name))
                .count();
            let hits = catalog.search(&name, 1_000);
            let hit = hits.iter().find(|hit| hit.name == name && hit.kind != NodeKind::Schema);
            prop_assert!(hit.is_some(), "{} não apareceu na busca", name);
            prop_assert_eq!(hit.unwrap().schemas.as_ref().unwrap().total, holders);
        }

        for hit in catalog.search(&query, 50) {
            prop_assert!(is_subsequence(&query, &hit.name), "{} não casa com {}", hit.name, query);
        }
    }

    #[test]
    fn file_round_trip_preserves_everything(input in input()) {
        vault::init_for_tests();
        let catalog = build(&input);
        let bytes = encode(&catalog, "id").unwrap();
        let loaded = decode(&bytes, "id").unwrap();
        prop_assert_eq!(observable(&loaded), observable(&catalog));
    }

    #[test]
    fn schema_diff_tracks_lifecycle_without_leaking_shapes(
        input in input(),
        fingerprints in prop::collection::vec(prop::option::of(0i64..3), 25),
        keep in prop::collection::vec(any::<bool>(), 25),
        bumps in prop::collection::vec(any::<bool>(), 25),
        added in prop::collection::btree_set("[x-z][0-9]", 0..4),
    ) {
        let mut catalog = Catalog::new();
        for (i, (schema, records)) in input.iter().enumerate() {
            catalog.set_tables(schema, records.clone(), fingerprints[i]);
        }

        let mut headers = Vec::new();
        let mut expected_changed = Vec::new();
        let mut expected_removed = Vec::new();
        for (i, schema) in input.keys().enumerate() {
            if !keep[i] {
                expected_removed.push(schema.clone());
                continue;
            }
            let fingerprint = if bumps[i] {
                Some(fingerprints[i].map_or(100, |f| f + 10))
            } else {
                fingerprints[i]
            };
            if fingerprint.is_some() && fingerprint != fingerprints[i] {
                expected_changed.push(schema.clone());
            }
            headers.push(SchemaHeader { name: schema.clone(), fingerprint });
        }
        headers.extend(added.iter().map(|name| SchemaHeader { name: name.clone(), fingerprint: None }));

        let diff = catalog.set_schemas(headers);
        prop_assert_eq!(&diff.removed, &expected_removed);
        prop_assert_eq!(&diff.changed, &expected_changed);
        prop_assert_eq!(diff.added, added.iter().cloned().collect::<Vec<_>>());

        for schema in &expected_changed {
            prop_assert_eq!(catalog.schema_state(schema), Some(LoadState::Stale));
        }
        for schema in &added {
            prop_assert_eq!(catalog.schema_state(schema), Some(LoadState::Unloaded));
        }

        // Formatos vivos = conjuntos distintos entre os schemas que sobraram
        let model = naive(&input);
        let remaining: BTreeSet<Vec<Relation>> = model
            .iter()
            .filter(|(schema, _)| !expected_removed.contains(schema))
            .map(|(_, relations)| relations.clone())
            .collect();
        prop_assert_eq!(catalog.stats().shapes, remaining.len());
        prop_assert_eq!(
            catalog.members.iter().sum::<u32>() as usize,
            input.len() - expected_removed.len()
        );
    }
}
