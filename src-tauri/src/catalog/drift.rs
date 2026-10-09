use std::collections::{HashMap, HashSet};

use super::base::Catalog;
use super::interner::Sym;
use super::model::{
    DriftGroup, DriftReport, SchemaDrift, ShapeGroup, ShapeRole, ShapeSummary,
};
use super::shape::{Shape, ShapeId};

/// Quantos schemas de exemplo cada grupo de drift mostra.
const DRIFT_SAMPLE: usize = 5;

/// Semelhança mínima (Jaccard sobre os nomes) para um formato contar como
/// variação do dominante — abaixo disso é outro schema qualquer (`public`, um
/// schema de auditoria…), não um tenant atrasado.
const MIN_SIMILARITY: f64 = 0.5;

/// Variações listadas na árvore agrupada; as demais caem em "outros" (um
/// banco em que cada tenant diverge de um jeito não vira 5.000 grupos).
pub(super) const MAX_VARIANT_GROUPS: usize = 50;

/// Chave do grupo "outros" na árvore agrupada por formato.
pub const OTHER_GROUP: &str = "other";

/// O formato com mais schemas (pelo menos dois) e o conjunto de nomes dele.
pub(super) struct Dominant {
    pub id: ShapeId,
    pub names: HashSet<Sym>,
}

/// Variação parecida o bastante com o dominante: o que falta e o que sobra.
struct Variation {
    missing: Vec<Sym>,
    extra: Vec<Sym>,
}

/// Compara schemas com o molde, lembrando cada formato já comparado — numa
/// janela de 500 schemas há poucos formatos distintos.
pub(super) struct DriftLens<'a> {
    catalog: &'a Catalog,
    dominant: Option<Dominant>,
    memo: HashMap<ShapeId, Option<SchemaDrift>>,
}

impl DriftLens<'_> {
    pub fn drift_of(&mut self, shape: Option<ShapeId>) -> Option<SchemaDrift> {
        let dominant = self.dominant.as_ref()?;
        let shape = shape?;
        if shape == dominant.id {
            return None;
        }
        let catalog = self.catalog;
        *self
            .memo
            .entry(shape)
            .or_insert_with(|| overlap(&dominant.names, &catalog.shapes[shape.index()]))
    }
}

impl Catalog {
    pub(super) fn dominant(&self) -> Option<Dominant> {
        let (id, shape, _) = self
            .live_shapes()
            .filter(|(_, _, members)| *members >= 2)
            .max_by(|a, b| a.2.cmp(&b.2).then(b.0.cmp(&a.0)))?;
        Some(Dominant {
            id,
            names: shape.tables.iter().map(|table| table.name).collect(),
        })
    }

    pub(super) fn drift_lens(&self) -> DriftLens<'_> {
        DriftLens {
            catalog: self,
            dominant: self.dominant(),
            memo: HashMap::new(),
        }
    }

    /// Os nomes que faltam e que sobram — só para as variações que aparecem
    /// (montar conjuntos para cada formato custava ~10 ms com 1.500 deles).
    fn variation(&self, reference: &HashSet<Sym>, shape: &Shape) -> Variation {
        let names: HashSet<Sym> = shape.tables.iter().map(|table| table.name).collect();
        Variation {
            missing: reference.difference(&names).copied().collect(),
            extra: names.difference(reference).copied().collect(),
        }
    }

    /// Variações do dominante, as com mais schemas primeiro.
    fn variations(&self, dominant: &Dominant) -> Vec<(ShapeId, &Shape, u32)> {
        let mut variations: Vec<_> = self
            .live_shapes()
            .filter(|(id, _, _)| *id != dominant.id)
            .filter(|(_, shape, _)| overlap(&dominant.names, shape).is_some())
            .collect();
        variations.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
        variations
    }

    /// O formato com mais schemas é o "molde"; os formatos parecidos com ele são
    /// tenants que divergem (migração pendente, tabela a mais). Sem um formato
    /// repetido não há molde e o relatório vem vazio.
    pub fn drift(&self) -> DriftReport {
        let Some(dominant) = self.dominant() else {
            return DriftReport {
                dominant: None,
                divergent: Vec::new(),
            };
        };

        let mut divergent: Vec<DriftGroup> = self
            .variations(&dominant)
            .into_iter()
            .map(|(id, shape, _)| {
                let variation = self.variation(&dominant.names, shape);
                DriftGroup {
                    schemas: self.schemas_using(&[id], DRIFT_SAMPLE),
                    missing: self.sorted_names(variation.missing.iter()),
                    extra: self.sorted_names(variation.extra.iter()),
                }
            })
            .collect();

        divergent.sort_by(|a, b| {
            b.schemas
                .total
                .cmp(&a.schemas.total)
                .then_with(|| a.schemas.sample.cmp(&b.schemas.sample))
        });

        DriftReport {
            dominant: Some(ShapeSummary {
                tables: self.shapes[dominant.id.index()].tables.len(),
                schemas: self.schemas_using(&[dominant.id], DRIFT_SAMPLE),
            }),
            divergent,
        }
    }

    /// Grupos da árvore agrupada: o molde, as variações (até
    /// [`MAX_VARIANT_GROUPS`]) e "outros". Vazio quando não há molde — a árvore
    /// fica na lista simples.
    pub fn shape_groups(&self) -> Vec<ShapeGroup> {
        let Some(dominant) = self.dominant() else {
            return Vec::new();
        };
        let mut groups = vec![ShapeGroup {
            key: shape_key(&self.shapes[dominant.id.index()]),
            role: ShapeRole::Dominant,
            tables: Some(self.shapes[dominant.id.index()].tables.len()),
            missing: Vec::new(),
            extra: Vec::new(),
            schemas: self.members[dominant.id.index()] as usize,
        }];
        let mut grouped = self.members[dominant.id.index()] as usize;

        for (_, shape, members) in self.variations(&dominant).into_iter().take(MAX_VARIANT_GROUPS) {
            let variation = self.variation(&dominant.names, shape);
            grouped += members as usize;
            groups.push(ShapeGroup {
                key: shape_key(shape),
                role: ShapeRole::Variant,
                tables: Some(shape.tables.len()),
                missing: self.sorted_names(variation.missing.iter()),
                extra: self.sorted_names(variation.extra.iter()),
                schemas: members as usize,
            });
        }

        let others = self.schemas.len() - grouped;
        if others > 0 {
            groups.push(ShapeGroup {
                key: OTHER_GROUP.to_string(),
                role: ShapeRole::Other,
                tables: None,
                missing: Vec::new(),
                extra: Vec::new(),
                schemas: others,
            });
        }
        groups
    }

    /// Formatos de um grupo da árvore agrupada. `None` = o grupo não existe
    /// mais (o catálogo mudou depois que a árvore pediu os grupos).
    pub(super) fn group_members(&self, key: &str) -> Option<GroupMembers> {
        let dominant = self.dominant()?;
        if key == OTHER_GROUP {
            let mut listed: HashSet<ShapeId> = self
                .variations(&dominant)
                .into_iter()
                .take(MAX_VARIANT_GROUPS)
                .map(|(id, ..)| id)
                .collect();
            listed.insert(dominant.id);
            return Some(GroupMembers::AllBut(listed));
        }
        let shapes: HashSet<ShapeId> = self
            .live_shapes()
            .filter(|(_, shape, _)| shape_key(shape) == key)
            .map(|(id, ..)| id)
            .collect();
        (!shapes.is_empty()).then_some(GroupMembers::Only(shapes))
    }

    fn sorted_names<'a>(&self, syms: impl Iterator<Item = &'a Sym>) -> Vec<String> {
        let mut names: Vec<String> = syms
            .map(|&sym| self.names.resolve(sym).to_string())
            .collect();
        names.sort();
        names
    }
}

/// Que schemas pertencem a um grupo da árvore agrupada.
pub(super) enum GroupMembers {
    Only(HashSet<ShapeId>),
    /// "Outros": quem não está nestes formatos, e quem ainda não tem formato
    AllBut(HashSet<ShapeId>),
}

impl GroupMembers {
    pub fn contains(&self, shape: Option<ShapeId>) -> bool {
        match (self, shape) {
            (Self::Only(shapes), Some(shape)) => shapes.contains(&shape),
            (Self::Only(_), None) => false,
            (Self::AllBut(shapes), Some(shape)) => !shapes.contains(&shape),
            (Self::AllBut(_), None) => true,
        }
    }
}

/// Quanto `shape` difere do molde, ou `None` se não for parecido o bastante
/// (Jaccard < 0,5). Sem alocar: os nomes de um formato não se repetem.
fn overlap(reference: &HashSet<Sym>, shape: &Shape) -> Option<SchemaDrift> {
    let shared = shape
        .tables
        .iter()
        .filter(|table| reference.contains(&table.name))
        .count();
    let union = shape.tables.len() + reference.len() - shared;
    if union == 0 || (shared as f64) / (union as f64) < MIN_SIMILARITY {
        return None;
    }
    Some(SchemaDrift {
        missing: (reference.len() - shared) as u32,
        extra: (shape.tables.len() - shared) as u32,
    })
}

fn shape_key(shape: &Shape) -> String {
    format!("{:016x}", shape.hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::model::{RelKind, TableRecord};

    fn tables(names: &[&str]) -> Vec<TableRecord> {
        names
            .iter()
            .map(|name| TableRecord::new(*name, RelKind::Table))
            .collect()
    }

    #[test]
    fn finds_tenants_behind_the_dominant_shape() {
        let mut catalog = Catalog::new();
        let full = ["customers", "invoices", "orders", "payments"];
        for i in 1..=10 {
            catalog.set_tables(&format!("tenant_{i:02}"), tables(&full), None);
        }
        // Migração pendente: sem `payments`
        catalog.set_tables("tenant_11", tables(&["customers", "invoices", "orders"]), None);
        catalog.set_tables("tenant_12", tables(&["customers", "invoices", "orders"]), None);
        // Tabela a mais
        catalog.set_tables("tenant_13", tables(&["customers", "invoices", "orders", "payments", "legacy"]), None);
        // Nada a ver com os tenants
        catalog.set_tables("public", tables(&["plans", "schema_migrations"]), None);

        let report = catalog.drift();
        let dominant = report.dominant.unwrap();
        assert_eq!(dominant.tables, 4);
        assert_eq!(dominant.schemas.total, 10);

        assert_eq!(report.divergent.len(), 2);
        assert_eq!(report.divergent[0].schemas.total, 2);
        assert_eq!(report.divergent[0].schemas.sample, ["tenant_11", "tenant_12"]);
        assert_eq!(report.divergent[0].missing, ["payments"]);
        assert!(report.divergent[0].extra.is_empty());
        assert_eq!(report.divergent[1].extra, ["legacy"]);
    }

    fn tenants_catalog() -> Catalog {
        let mut catalog = Catalog::new();
        let full = ["customers", "invoices", "orders", "payments"];
        for i in 1..=10 {
            catalog.set_tables(&format!("tenant_{i:02}"), tables(&full), None);
        }
        catalog.set_tables("tenant_11", tables(&["customers", "invoices", "orders"]), None);
        catalog.set_tables("tenant_12", tables(&["customers", "invoices", "orders"]), None);
        catalog.set_tables("tenant_13", tables(&["customers", "invoices", "orders", "payments", "legacy"]), None);
        catalog.set_tables("public", tables(&["plans", "schema_migrations"]), None);
        catalog.insert_unloaded("tenant_99");
        catalog
    }

    #[test]
    fn groups_by_shape_with_the_dominant_first_and_the_rest_in_other() {
        let catalog = tenants_catalog();
        let groups = catalog.shape_groups();
        let summary: Vec<_> = groups
            .iter()
            .map(|group| (group.role, group.schemas, group.missing.clone(), group.extra.clone()))
            .collect();
        assert_eq!(
            summary,
            vec![
                (ShapeRole::Dominant, 10, vec![], vec![]),
                (ShapeRole::Variant, 2, vec!["payments".to_string()], vec![]),
                (ShapeRole::Variant, 1, vec![], vec!["legacy".to_string()]),
                // public + o ainda não carregado
                (ShapeRole::Other, 2, vec![], vec![]),
            ]
        );
        assert_eq!(groups[0].tables, Some(4));
        assert_eq!(groups[3].key, OTHER_GROUP);
        // A chave é o conteúdo: estável entre catálogos com os mesmos formatos
        assert_eq!(tenants_catalog().shape_groups()[1].key, groups[1].key);

        let behind = catalog.group_members(&groups[1].key).unwrap();
        let other = catalog.group_members(OTHER_GROUP).unwrap();
        let members = |group: &GroupMembers| -> Vec<String> {
            catalog
                .schemas
                .iter()
                .filter(|(_, entry)| group.contains(entry.shape))
                .map(|(name, _)| name.to_string())
                .collect()
        };
        assert_eq!(members(&behind), ["tenant_11", "tenant_12"]);
        assert_eq!(members(&other), ["public", "tenant_99"]);
        assert!(catalog.group_members("0000000000000000").is_none());
    }

    #[test]
    fn lens_measures_each_schema_against_the_dominant() {
        let catalog = tenants_catalog();
        let mut lens = catalog.drift_lens();
        let shape_of = |name: &str| catalog.schemas[name].shape;
        assert_eq!(lens.drift_of(shape_of("tenant_01")), None);
        assert_eq!(lens.drift_of(shape_of("tenant_11")), Some(SchemaDrift { missing: 1, extra: 0 }));
        assert_eq!(lens.drift_of(shape_of("tenant_13")), Some(SchemaDrift { missing: 0, extra: 1 }));
        // Nada a ver com o molde, ou sem formato ainda
        assert_eq!(lens.drift_of(shape_of("public")), None);
        assert_eq!(lens.drift_of(shape_of("tenant_99")), None);
    }

    #[test]
    fn no_repeated_shape_means_no_report() {
        let mut catalog = Catalog::new();
        catalog.set_tables("a", tables(&["x"]), None);
        catalog.set_tables("b", tables(&["y"]), None);
        let report = catalog.drift();
        assert!(report.dominant.is_none());
        assert!(report.divergent.is_empty());
        assert!(catalog.shape_groups().is_empty());
        assert!(catalog.group_members(OTHER_GROUP).is_none());
    }
}
