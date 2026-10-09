use std::cmp::Ordering;
use std::collections::HashMap;
#[cfg(test)]
use std::collections::HashSet;

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use super::base::Catalog;
use super::interner::Sym;
use super::model::{NodeKind, RelKind, SearchHit};
use super::query::SAMPLE_SIZE;
use super::shape::ShapeId;

/// Pontuação fuzzy de um texto contra a busca (o matcher do Helix).
struct Scorer {
    pattern: Pattern,
    matcher: Matcher,
    buf: Vec<char>,
}

impl Scorer {
    /// `None` para busca vazia. Sem a sintaxe do fzf (`^`, `!`, `'`): o usuário
    /// digita nomes, não expressões.
    fn new(query: &str) -> Option<Self> {
        let pattern = Pattern::new(query, CaseMatching::Ignore, Normalization::Smart, AtomKind::Fuzzy);
        if pattern.atoms.is_empty() {
            return None;
        }

        let mut config = Config::DEFAULT;
        // Quem procura uma tabela costuma digitar o começo do nome
        config.prefer_prefix = true;

        Some(Self {
            pattern,
            matcher: Matcher::new(config),
            buf: Vec::new(),
        })
    }

    fn score(&mut self, haystack: &str) -> Option<u32> {
        self.pattern
            .score(Utf32Str::new(haystack, &mut self.buf), &mut self.matcher)
    }
}

/// Maior pontuação primeiro; no empate, o nome mais curto e depois a ordem
/// alfabética — o que deixa o resultado determinístico.
fn rank(a: (u32, &str), b: (u32, &str)) -> Ordering {
    b.0.cmp(&a.0)
        .then(a.1.len().cmp(&b.1.len()))
        .then(a.1.cmp(b.1))
}

/// Os `limit` melhores sem ordenar a lista inteira (a busca no pior caso
/// pontua centenas de milhares de nomes). `cmp` precisa ser total para o
/// resultado não depender da ordem de entrada.
fn top<T>(mut items: Vec<T>, limit: usize, cmp: impl Fn(&T, &T) -> Ordering) -> Vec<T> {
    if items.len() > limit {
        items.select_nth_unstable_by(limit - 1, &cmp);
        items.truncate(limit);
    }
    items.sort_by(&cmp);
    items
}

enum Target<'a> {
    Table(Sym),
    Schema(&'a str),
}

impl<'a> Target<'a> {
    fn label(&self, catalog: &'a Catalog) -> &'a str {
        match self {
            Target::Table(sym) => catalog.names.resolve(*sym),
            Target::Schema(name) => name,
        }
    }
}

impl Catalog {
    /// Busca fuzzy em schemas e relações. Relações com o mesmo nome em vários
    /// schemas viram um resultado só; `schema.tabela` busca os pares (as duas
    /// partes fuzzy, e qualquer uma pode ficar vazia: `tenant_42.` lista as
    /// relações do schema).
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let query = query.trim();
        if query.is_empty() || limit == 0 {
            return Vec::new();
        }

        match query.split_once('.') {
            Some((schema, table)) => self.search_qualified(schema.trim(), table.trim(), limit),
            None => self.search_unqualified(query, limit),
        }
    }

    fn search_unqualified(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let Some(mut scorer) = Scorer::new(query) else {
            return Vec::new();
        };

        let mut candidates: Vec<(u32, Target)> = Vec::new();
        for sym in self.by_table.names() {
            if let Some(score) = scorer.score(self.names.resolve(sym)) {
                candidates.push((score, Target::Table(sym)));
            }
        }
        for name in self.schemas.keys() {
            if let Some(score) = scorer.score(name) {
                candidates.push((score, Target::Schema(name)));
            }
        }

        let best = top(candidates, limit, |(a_score, a), (b_score, b)| {
            rank((*a_score, a.label(self)), (*b_score, b.label(self)))
                // Uma relação e um schema com o mesmo nome: a relação primeiro
                .then_with(|| matches!(b, Target::Table(_)).cmp(&matches!(a, Target::Table(_))))
        });

        best.into_iter()
            .map(|(score, target)| match target {
                Target::Schema(name) => SearchHit {
                    name: name.to_string(),
                    kind: NodeKind::Schema,
                    score,
                    schema: None,
                    schemas: None,
                },
                Target::Table(sym) => {
                    let shapes = self.by_table.shapes(sym);
                    SearchHit {
                        name: self.names.resolve(sym).to_string(),
                        kind: self.dominant_kind(sym, &shapes).into(),
                        score,
                        schema: None,
                        schemas: Some(self.schemas_using(&shapes, SAMPLE_SIZE)),
                    }
                }
            })
            .collect()
    }

    fn search_qualified(&self, schema_query: &str, table_query: &str, limit: usize) -> Vec<SearchHit> {
        let mut schema_scorer = Scorer::new(schema_query);
        let mut table_scorer = Scorer::new(table_query);
        if schema_scorer.is_none() && table_scorer.is_none() {
            return Vec::new();
        }

        // As relações que casam com a parte da tabela, uma vez por formato
        let mut per_shape: HashMap<ShapeId, Vec<(Sym, RelKind, u32)>> = HashMap::new();
        let mut pairs: Vec<(u32, &str, Sym, RelKind)> = Vec::new();

        for (schema, entry) in &self.schemas {
            let Some(shape_id) = entry.shape else {
                continue;
            };
            let schema_score = match schema_scorer.as_mut() {
                Some(scorer) => match scorer.score(schema) {
                    Some(score) => score,
                    None => continue,
                },
                None => 0,
            };

            let matches = per_shape.entry(shape_id).or_insert_with(|| {
                self.shapes[shape_id.index()]
                    .tables
                    .iter()
                    .filter_map(|table| {
                        let score = match table_scorer.as_mut() {
                            Some(scorer) => scorer.score(self.names.resolve(table.name))?,
                            None => 0,
                        };
                        Some((table.name, table.kind, score))
                    })
                    .collect()
            });

            for &(sym, kind, table_score) in matches.iter() {
                pairs.push((schema_score + table_score, schema, sym, kind));
            }
        }

        // Empate entre pares: schema e depois relação, em ordem alfabética
        let best = top(pairs, limit, |a, b| {
            b.0.cmp(&a.0)
                .then(a.1.cmp(b.1))
                .then_with(|| self.names.resolve(a.2).cmp(self.names.resolve(b.2)))
        });

        best.into_iter()
            .map(|(score, schema, sym, kind)| SearchHit {
                name: self.names.resolve(sym).to_string(),
                kind: kind.into(),
                score,
                schema: Some(schema.to_string()),
                schemas: None,
            })
            .collect()
    }

    /// Um mesmo nome pode ser tabela num formato e view em outro: vale o tipo do
    /// formato com mais schemas.
    fn dominant_kind(&self, sym: Sym, shapes: &[ShapeId]) -> RelKind {
        shapes
            .iter()
            .max_by_key(|shape| self.members[shape.index()])
            .and_then(|shape| self.shapes[shape.index()].find(&self.names, self.names.resolve(sym)))
            .map_or(RelKind::Table, |table| table.kind)
    }

    /// Nomes distintos de relações vivas — usado pelos testes de propriedade.
    #[cfg(test)]
    pub(super) fn distinct_relation_names(&self) -> HashSet<String> {
        self.by_table
            .names()
            .map(|sym| self.names.resolve(sym).to_string())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::model::{SchemaHeader, TableRecord};

    fn catalog() -> Catalog {
        let mut catalog = Catalog::new();
        let tenant = || {
            vec![
                TableRecord::new("orders", RelKind::Table),
                TableRecord::new("order_items", RelKind::Table),
                TableRecord::new("customers", RelKind::Table),
            ]
        };
        for i in 1..=40 {
            catalog.set_tables(&format!("tenant_{i:03}"), tenant(), None);
        }
        catalog.set_tables("public", vec![TableRecord::new("orders_archive", RelKind::View)], None);
        catalog.set_schemas(
            catalog
                .schemas
                .keys()
                .map(|name| SchemaHeader {
                    name: name.to_string(),
                    fingerprint: None,
                })
                .chain([SchemaHeader {
                    name: "ordering".into(),
                    fingerprint: None,
                }])
                .collect::<Vec<_>>(),
        );
        catalog
    }

    #[test]
    fn groups_the_same_table_across_tenants() {
        let hits = catalog().search("orders", 5);

        assert_eq!(hits[0].name, "orders");
        assert_eq!(hits[0].kind, NodeKind::Table);
        let group = hits[0].schemas.as_ref().unwrap();
        assert_eq!(group.total, 40);
        assert_eq!(group.sample, ["tenant_001", "tenant_002", "tenant_003"]);
        // Um resultado por nome, não um por tenant
        assert_eq!(hits.iter().filter(|hit| hit.name == "orders").count(), 1);
    }

    #[test]
    fn mixes_schemas_and_relations_by_score() {
        let hits = catalog().search("ord", 10);
        let names: Vec<_> = hits.iter().map(|hit| hit.name.as_str()).collect();

        assert!(names.contains(&"ordering"));
        assert!(names.contains(&"orders_archive"));
        assert!(!names.contains(&"customers"));
        let schema_hit = hits.iter().find(|hit| hit.name == "ordering").unwrap();
        assert_eq!(schema_hit.kind, NodeKind::Schema);
        assert!(schema_hit.schemas.is_none());
    }

    #[test]
    fn qualified_search_returns_pairs() {
        let hits = catalog().search("tenant_007.ord", 50);

        assert!(!hits.is_empty());
        assert_eq!(hits[0].schema.as_deref(), Some("tenant_007"));
        assert!(hits.iter().all(|hit| hit.name.contains("ord") || hit.name.starts_with("ord")));

        // Só o schema: lista as relações dele
        let listed = catalog().search("tenant_007.", 10);
        assert!(listed.len() >= 3);
        assert_eq!(listed[0].schema.as_deref(), Some("tenant_007"));
    }

    #[test]
    fn respects_limit_and_ignores_empty_queries() {
        assert_eq!(catalog().search("t", 2).len(), 2);
        assert!(catalog().search("   ", 10).is_empty());
        assert!(catalog().search(".", 10).is_empty());
        assert!(catalog().search("zzzz", 10).is_empty());
    }
}
