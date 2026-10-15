//! Edição de estrutura (DDL) pela árvore no Postgres. Os statements são
//! montados aqui, puros (o diálogo mostra antes de aplicar), e aplicados numa
//! transação só — o Postgres tem DDL transacional: um passo que falha desfaz
//! os anteriores.

use deadpool_postgres::Pool;
use sqlparser::dialect::PostgreSqlDialect;

use crate::adapters::ddl::{
    column_sql, create_table_body, quote_ident, validate_data_type, validate_default, validate_name,
};
use crate::error::Result;
use crate::models::{RelationKind, StatementResult, StructureChange};

use super::executor;

/// `"schema"."tabela"`; sem schema, o nome sozinho (cai no `search_path`).
fn qualified(schema: Option<&str>, table: &str) -> Result<String> {
    let table = quote_ident(validate_name(table, "Table")?);
    Ok(match schema.map(str::trim).filter(|schema| !schema.is_empty()) {
        Some(schema) => format!("{}.{table}", quote_ident(schema)),
        None => table,
    })
}

/// Mesmo tipo escrito de outro jeito (`INTEGER` e `integer`) não é mudança.
fn same_type(a: &str, b: &str) -> bool {
    let normalize = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    normalize(a) == normalize(b)
}

/// Os statements de uma mudança, na ordem em que rodam.
pub fn statements(change: &StructureChange) -> Result<Vec<String>> {
    let dialect = PostgreSqlDialect {};
    let schema = change.schema();

    Ok(match change {
        StructureChange::CreateTable { table, columns, .. } => vec![format!(
            "CREATE TABLE {} (\n{}\n)",
            qualified(schema, table)?,
            create_table_body(columns, &dialect)?
        )],
        StructureChange::RenameTable { table, new_name, .. } => vec![format!(
            "ALTER TABLE {} RENAME TO {}",
            qualified(schema, table)?,
            quote_ident(validate_name(new_name, "Table")?)
        )],
        StructureChange::DropTable { table, kind, cascade, .. } => {
            let what = match kind {
                RelationKind::Table => "TABLE",
                RelationKind::View => "VIEW",
                RelationKind::MaterializedView => "MATERIALIZED VIEW",
                RelationKind::Foreign => "FOREIGN TABLE",
            };
            vec![format!(
                "DROP {what} {}{}",
                qualified(schema, table)?,
                if *cascade { " CASCADE" } else { "" }
            )]
        }
        StructureChange::AddColumn { table, column, .. } => vec![format!(
            "ALTER TABLE {} ADD COLUMN {}",
            qualified(schema, table)?,
            column_sql(column, &dialect, true)?
        )],
        StructureChange::AlterColumn { table, original, updated, .. } => {
            let target = qualified(schema, table)?;
            let from = validate_name(&original.name, "Column")?;
            let to = validate_name(&updated.name, "Column")?;
            let column = quote_ident(to);
            let mut statements = Vec::new();

            if from != to {
                statements.push(format!(
                    "ALTER TABLE {target} RENAME COLUMN {} TO {column}",
                    quote_ident(from)
                ));
            }
            let data_type = validate_data_type(&updated.data_type)?;
            if !same_type(&original.data_type, data_type) {
                // O USING converte os dados que já estão lá (texto → número, etc.)
                statements.push(format!(
                    "ALTER TABLE {target} ALTER COLUMN {column} TYPE {data_type} USING {column}::{data_type}"
                ));
            }
            let default = validate_default(updated.default_value.as_deref(), &dialect)?;
            let before = original.default_value.as_deref().map(str::trim).filter(|d| !d.is_empty());
            if default != before {
                statements.push(match default {
                    Some(default) => {
                        format!("ALTER TABLE {target} ALTER COLUMN {column} SET DEFAULT {default}")
                    }
                    None => format!("ALTER TABLE {target} ALTER COLUMN {column} DROP DEFAULT"),
                });
            }
            if original.nullable != updated.nullable {
                statements.push(format!(
                    "ALTER TABLE {target} ALTER COLUMN {column} {} NOT NULL",
                    if updated.nullable { "DROP" } else { "SET" }
                ));
            }
            statements
        }
        StructureChange::DropColumn { table, column, cascade, .. } => vec![format!(
            "ALTER TABLE {} DROP COLUMN {}{}",
            qualified(schema, table)?,
            quote_ident(validate_name(column, "Column")?),
            if *cascade { " CASCADE" } else { "" }
        )],
    })
}

/// Aplica a mudança numa transação. Sem nada a mudar, não abre nem a conexão.
pub async fn apply(pool: &Pool, change: &StructureChange) -> Result<StatementResult> {
    let statements = statements(change)?;
    if statements.is_empty() {
        return Ok(StatementResult {
            affected_rows: 0,
            execution_time_ms: 0,
        });
    }
    let results = executor::execute_transaction(pool, statements).await?;
    Ok(StatementResult {
        affected_rows: 0,
        execution_time_ms: results.iter().map(|result| result.execution_time_ms).sum(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ColumnDefinition;

    fn column(name: &str, data_type: &str, nullable: bool, default: Option<&str>) -> ColumnDefinition {
        ColumnDefinition {
            name: name.into(),
            data_type: data_type.into(),
            nullable,
            default_value: default.map(Into::into),
            primary_key: false,
        }
    }

    #[test]
    fn creates_renames_and_drops_tables() {
        let mut id = column("id", "bigint", false, None);
        id.primary_key = true;
        let create = StructureChange::CreateTable {
            schema: Some("tenant_1".into()),
            table: "orders".into(),
            columns: vec![id, column("total", "numeric(10, 2)", true, Some("0"))],
        };
        assert_eq!(
            statements(&create).unwrap(),
            ["CREATE TABLE \"tenant_1\".\"orders\" (\n  \"id\" bigint NOT NULL,\n  \"total\" numeric(10, 2) DEFAULT 0,\n  PRIMARY KEY (\"id\")\n)"]
        );

        let rename = StructureChange::RenameTable {
            schema: Some("public".into()),
            table: "orders".into(),
            new_name: "pedidos".into(),
        };
        assert_eq!(
            statements(&rename).unwrap(),
            ["ALTER TABLE \"public\".\"orders\" RENAME TO \"pedidos\""]
        );

        let drop = StructureChange::DropTable {
            schema: Some("public".into()),
            table: "resumo".into(),
            kind: RelationKind::MaterializedView,
            cascade: true,
        };
        assert_eq!(
            statements(&drop).unwrap(),
            ["DROP MATERIALIZED VIEW \"public\".\"resumo\" CASCADE"]
        );
    }

    #[test]
    fn alter_column_only_touches_what_changed() {
        let original = column("status", "text", true, Some("'novo'"));

        // Nada mudou (o tipo só mudou de caixa)
        let same = StructureChange::AlterColumn {
            schema: Some("public".into()),
            table: "orders".into(),
            original: original.clone(),
            updated: column("status", "TEXT", true, Some(" 'novo' ")),
        };
        assert!(statements(&same).unwrap().is_empty());

        let everything = StructureChange::AlterColumn {
            schema: Some("public".into()),
            table: "orders".into(),
            original,
            updated: column("situacao", "varchar(20)", false, None),
        };
        assert_eq!(
            statements(&everything).unwrap(),
            [
                "ALTER TABLE \"public\".\"orders\" RENAME COLUMN \"status\" TO \"situacao\"",
                "ALTER TABLE \"public\".\"orders\" ALTER COLUMN \"situacao\" TYPE varchar(20) USING \"situacao\"::varchar(20)",
                "ALTER TABLE \"public\".\"orders\" ALTER COLUMN \"situacao\" DROP DEFAULT",
                "ALTER TABLE \"public\".\"orders\" ALTER COLUMN \"situacao\" SET NOT NULL",
            ]
        );
    }

    #[test]
    fn adds_and_drops_columns() {
        let mut uuid = column("ref", "uuid", false, Some("gen_random_uuid()"));
        uuid.primary_key = true;
        let add = StructureChange::AddColumn {
            schema: Some("public".into()),
            table: "t".into(),
            column: uuid,
        };
        assert_eq!(
            statements(&add).unwrap(),
            ["ALTER TABLE \"public\".\"t\" ADD COLUMN \"ref\" uuid NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY"]
        );

        let drop = StructureChange::DropColumn {
            schema: Some("public".into()),
            table: "t".into(),
            column: "ref".into(),
            cascade: false,
        };
        assert_eq!(
            statements(&drop).unwrap(),
            ["ALTER TABLE \"public\".\"t\" DROP COLUMN \"ref\""]
        );

        // Um default que emenda outro statement é recusado antes de ir ao banco
        let sneaky = StructureChange::AddColumn {
            schema: None,
            table: "t".into(),
            column: column("x", "int", true, Some("1; DROP TABLE t")),
        };
        assert!(statements(&sneaky).is_err());
    }

    // ── End-to-end (requires a local Postgres; run with `cargo test -- --ignored`) ──

    #[tokio::test]
    #[ignore = "requires local postgres on localhost:5432 (postgres/1234)"]
    async fn e2e_structure_changes_and_schema_comparison() {
        use parking_lot::RwLock;

        use crate::adapters::postgres::introspect::{IntrospectOptions, Introspector};
        use crate::adapters::postgres::PostgresAdapter;
        use crate::adapters::DatabaseAdapter;
        use crate::catalog::{common_relations, compare_schemas, Catalog, CatalogSource};
        use crate::models::{DatabaseType, Server};

        let server = Server {
            id: Some(1),
            name: "e2e".into(),
            db_type: DatabaseType::Postgres,
            host: "localhost".into(),
            port: 5432,
            username: "postgres".into(),
            password: "1234".into(),
            default_database: None,
            ssl_enabled: false,
            connection_uri: None,
            scope_databases: None,
            scope_schemas: None,
            group_id: None,
            created_at: 0,
        };
        let admin = PostgresAdapter::new(&server, "postgres").unwrap();
        admin
            .execute_statement("DROP DATABASE IF EXISTS octapus_db_e2e_ddl WITH (FORCE)")
            .await
            .unwrap();
        admin.execute_statement("CREATE DATABASE octapus_db_e2e_ddl").await.unwrap();
        let adapter = PostgresAdapter::new(&server, "octapus_db_e2e_ddl").unwrap();

        // ── DDL pela árvore ──────────────────────────────────────────────
        adapter.execute_statement("CREATE SCHEMA app").await.unwrap();
        let mut id = column("id", "serial", false, None);
        id.primary_key = true;
        adapter
            .apply_structure_change(StructureChange::CreateTable {
                schema: Some("app".into()),
                table: "notas".into(),
                columns: vec![id, column("texto", "text", true, None)],
            })
            .await
            .unwrap();
        adapter
            .execute_statement("INSERT INTO app.notas (texto) VALUES ('42'), ('7')")
            .await
            .unwrap();
        adapter
            .apply_structure_change(StructureChange::AddColumn {
                schema: Some("app".into()),
                table: "notas".into(),
                column: column("criada", "timestamptz", false, Some("now()")),
            })
            .await
            .unwrap();
        // Texto → número pelo USING, renomeando junto
        adapter
            .apply_structure_change(StructureChange::AlterColumn {
                schema: Some("app".into()),
                table: "notas".into(),
                original: column("texto", "text", true, None),
                updated: column("valor", "integer", false, Some("0")),
            })
            .await
            .unwrap();
        let columns = adapter.list_columns("app", "notas").await.unwrap();
        let valor = columns.iter().find(|c| c.name == "valor").unwrap();
        assert_eq!(valor.data_type, "integer");
        assert!(!valor.is_nullable);
        assert_eq!(valor.default_value.as_deref(), Some("0"));

        // Um passo que falha desfaz os anteriores (rename + NOT NULL com nulo)
        adapter
            .execute_statement("INSERT INTO app.notas (valor) VALUES (NULL)")
            .await
            .unwrap_err();
        adapter
            .execute_statement("ALTER TABLE app.notas ALTER COLUMN valor DROP NOT NULL")
            .await
            .unwrap();
        adapter
            .execute_statement("INSERT INTO app.notas (valor) VALUES (NULL)")
            .await
            .unwrap();
        let err = adapter
            .apply_structure_change(StructureChange::AlterColumn {
                schema: Some("app".into()),
                table: "notas".into(),
                original: column("valor", "integer", true, Some("0")),
                updated: column("v", "integer", false, Some("0")),
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("null"), "erro inesperado: {err}");
        assert!(adapter.list_columns("app", "notas").await.unwrap().iter().any(|c| c.name == "valor"));

        adapter
            .apply_structure_change(StructureChange::DropColumn {
                schema: Some("app".into()),
                table: "notas".into(),
                column: "criada".into(),
                cascade: false,
            })
            .await
            .unwrap();
        adapter
            .apply_structure_change(StructureChange::RenameTable {
                schema: Some("app".into()),
                table: "notas".into(),
                new_name: "lembretes".into(),
            })
            .await
            .unwrap();
        adapter
            .apply_structure_change(StructureChange::DropTable {
                schema: Some("app".into()),
                table: "lembretes".into(),
                kind: RelationKind::Table,
                cascade: false,
            })
            .await
            .unwrap();
        assert!(adapter.list_tables("app").await.unwrap().is_empty());

        // ── Comparação entre schemas ─────────────────────────────────────
        for tenant in 1..=2 {
            for sql in [
                format!("CREATE SCHEMA t{tenant}"),
                format!("CREATE TABLE t{tenant}.orders (id serial PRIMARY KEY, status text DEFAULT 'novo', total numeric(10,2))"),
                format!("CREATE TABLE t{tenant}.customers (id serial PRIMARY KEY, name text NOT NULL)"),
            ] {
                adapter.execute_statement(&sql).await.unwrap();
            }
        }
        // t2: coluna a mais, tipo diferente e uma tabela só dele
        for sql in [
            "ALTER TABLE t2.orders ADD COLUMN discount numeric",
            "ALTER TABLE t2.orders ALTER COLUMN status TYPE varchar(20)",
            "CREATE TABLE t2.legacy (id int)",
        ] {
            adapter.execute_statement(sql).await.unwrap();
        }

        let mut introspector = Introspector::connect(&server, "octapus_db_e2e_ddl", IntrospectOptions::default())
            .await
            .unwrap();
        let catalog = RwLock::new(Catalog::new());
        introspector.sync(&catalog, &mut |_| {}).await.unwrap();
        let base_tables = catalog.read().relation_names("t1").unwrap();
        let other_tables = catalog.read().relation_names("t2").unwrap();

        let wanted = common_relations("t1", &base_tables, "t2", &other_tables);
        let columns = CatalogSource::relation_columns(&mut introspector, &wanted).await.unwrap();
        let report = compare_schemas("t1", &base_tables, "t2", &other_tables, &columns);

        assert_eq!(report.only_in_other, ["legacy"]);
        assert!(report.only_in_base.is_empty());
        assert_eq!(report.common, 2);
        // `customers` é igual: as sequências de cada schema não contam
        assert_eq!(report.different.len(), 1, "{report:?}");
        let mut changes: Vec<String> = report.different[0]
            .changes
            .iter()
            .map(|change| {
                format!(
                    "{}:{}->{}",
                    change.column,
                    change.expected.as_ref().map_or("-", |s| s.data_type.as_str()),
                    change.actual.as_ref().map_or("-", |s| s.data_type.as_str())
                )
            })
            .collect();
        changes.sort();
        assert_eq!(
            changes,
            ["discount:-->numeric", "status:text->character varying(20)"]
        );

        drop(introspector);
        drop(adapter);
        admin
            .execute_statement("DROP DATABASE octapus_db_e2e_ddl WITH (FORCE)")
            .await
            .unwrap();
    }
}
