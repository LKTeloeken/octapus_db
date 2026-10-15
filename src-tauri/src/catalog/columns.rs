//! Comparação entre dois schemas: as tabelas que só existem em um deles e,
//! nas que existem nos dois, as colunas que diferem. O catálogo sabe as
//! relações de cada schema; as colunas vêm da fonte, num pedido só para as
//! tabelas dos dois. Aqui fica a parte pura: o que pedir e o relatório.

use std::collections::{BTreeSet, HashMap, HashSet};

use super::base::Catalog;
use super::model::{ColumnChange, ColumnSpec, SchemaComparison, TableColumnsDiff};

/// Colunas de cada `(schema, tabela)` pedida, na ordem da tabela.
pub type RelationColumns = HashMap<(String, String), Vec<(String, ColumnSpec)>>;

impl Catalog {
    /// Relações de um schema carregado, em ordem de nome, sem as partições
    /// (costumam ter nome por período, e cada schema tem as suas). `None` se o
    /// schema não existe ou ainda não carregou.
    pub fn relation_names(&self, schema: &str) -> Option<Vec<String>> {
        let shape = self.shape_of(schema)?;
        let mut names: Vec<String> = shape
            .tables
            .iter()
            .filter(|table| table.parent.is_none())
            .map(|table| self.names.resolve(table.name).to_string())
            .collect();
        names.sort();
        Some(names)
    }
}

/// Os `(schema, tabela)` cujas colunas a comparação precisa: as tabelas que
/// existem nos dois schemas, de cada lado.
pub fn common_relations(
    base: &str,
    base_tables: &[String],
    other: &str,
    other_tables: &[String],
) -> Vec<(String, String)> {
    let other_set: HashSet<&str> = other_tables.iter().map(String::as_str).collect();
    base_tables
        .iter()
        .filter(|table| other_set.contains(table.as_str()))
        .flat_map(|table| {
            [
                (base.to_string(), table.clone()),
                (other.to_string(), table.clone()),
            ]
        })
        .collect()
}

/// O que muda de `reference` para `actual`, coluna a coluna, pelo nome: as que
/// faltam e as diferentes na ordem da referência, depois as a mais.
pub fn column_changes(
    reference: &[(String, ColumnSpec)],
    actual: &[(String, ColumnSpec)],
) -> Vec<ColumnChange> {
    let actual_by_name: HashMap<&str, &ColumnSpec> =
        actual.iter().map(|(name, spec)| (name.as_str(), spec)).collect();
    let reference_names: HashSet<&str> = reference.iter().map(|(name, _)| name.as_str()).collect();

    let mut changes: Vec<ColumnChange> = reference
        .iter()
        .filter_map(|(name, expected)| match actual_by_name.get(name.as_str()) {
            Some(spec) if *spec == expected => None,
            found => Some(ColumnChange {
                column: name.clone(),
                expected: Some(expected.clone()),
                actual: found.map(|spec| (*spec).clone()),
            }),
        })
        .collect();

    changes.extend(
        actual
            .iter()
            .filter(|(name, _)| !reference_names.contains(name.as_str()))
            .map(|(name, spec)| ColumnChange {
                column: name.clone(),
                expected: None,
                actual: Some(spec.clone()),
            }),
    );
    changes
}

/// `other` comparado com `base`, a partir das relações de cada um e das
/// colunas das tabelas em comum.
pub fn compare_schemas(
    base: &str,
    base_tables: &[String],
    other: &str,
    other_tables: &[String],
    columns: &RelationColumns,
) -> SchemaComparison {
    let base_set: BTreeSet<&str> = base_tables.iter().map(String::as_str).collect();
    let other_set: BTreeSet<&str> = other_tables.iter().map(String::as_str).collect();
    let columns_of = |schema: &str, table: &str| -> &[(String, ColumnSpec)] {
        columns
            .get(&(schema.to_string(), table.to_string()))
            .map(Vec::as_slice)
            .unwrap_or_default()
    };

    let common: Vec<&str> = base_set.intersection(&other_set).copied().collect();
    let different = common
        .iter()
        .filter_map(|table| {
            let changes = column_changes(columns_of(base, table), columns_of(other, table));
            (!changes.is_empty()).then(|| TableColumnsDiff {
                table: table.to_string(),
                changes,
            })
        })
        .collect();

    SchemaComparison {
        base: base.to_string(),
        other: other.to_string(),
        only_in_base: base_set.difference(&other_set).map(|t| t.to_string()).collect(),
        only_in_other: other_set.difference(&base_set).map(|t| t.to_string()).collect(),
        common: common.len(),
        different,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::model::{RelKind, TableRecord};

    fn spec(data_type: &str) -> ColumnSpec {
        ColumnSpec {
            data_type: data_type.into(),
            nullable: true,
            default_value: None,
        }
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn relation_names_leave_partitions_out() {
        let mut catalog = Catalog::new();
        let mut partition = TableRecord::new("events_2026", RelKind::Table);
        partition.partition_of = Some("events".into());
        catalog.set_tables(
            "app",
            vec![
                TableRecord::new("orders", RelKind::Table),
                TableRecord::new("events", RelKind::Partitioned),
                partition,
            ],
            None,
        );
        assert_eq!(catalog.relation_names("app").unwrap(), ["events", "orders"]);
        assert!(catalog.relation_names("nope").is_none());
    }

    #[test]
    fn compares_tables_and_columns_of_two_schemas() {
        let base_tables = names(&["customers", "orders", "payments"]);
        let other_tables = names(&["customers", "legacy", "orders"]);

        let wanted = common_relations("t1", &base_tables, "t2", &other_tables);
        assert_eq!(wanted.len(), 4, "customers e orders, dos dois lados");

        let mut columns = RelationColumns::new();
        let same = vec![("id".to_string(), spec("integer")), ("name".to_string(), spec("text"))];
        columns.insert(("t1".into(), "customers".into()), same.clone());
        columns.insert(("t2".into(), "customers".into()), same);
        columns.insert(
            ("t1".into(), "orders".into()),
            vec![("id".into(), spec("integer")), ("status".into(), spec("text"))],
        );
        columns.insert(
            ("t2".into(), "orders".into()),
            vec![("id".into(), spec("bigint")), ("discount".into(), spec("numeric"))],
        );

        let report = compare_schemas("t1", &base_tables, "t2", &other_tables, &columns);
        assert_eq!(report.only_in_base, ["payments"]);
        assert_eq!(report.only_in_other, ["legacy"]);
        assert_eq!(report.common, 2);
        assert_eq!(report.different.len(), 1);
        let changes: Vec<_> = report.different[0]
            .changes
            .iter()
            .map(|change| {
                (
                    change.column.as_str(),
                    change.expected.as_ref().map(|s| s.data_type.as_str()),
                    change.actual.as_ref().map(|s| s.data_type.as_str()),
                )
            })
            .collect();
        assert_eq!(
            changes,
            [
                ("id", Some("integer"), Some("bigint")),
                ("status", Some("text"), None),
                ("discount", None, Some("numeric")),
            ]
        );
    }
}
