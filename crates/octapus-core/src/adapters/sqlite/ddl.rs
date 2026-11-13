//! Edição de estrutura (DDL) pela árvore no SQLite.
//!
//! O `ALTER TABLE` do SQLite cria, renomeia e remove colunas, mas não muda o
//! tipo, o `NOT NULL` nem o default de uma. Para isso a tabela é recriada,
//! seguindo o procedimento da documentação (lang_altertable, §7): tabela nova
//! com a definição alterada, cópia dos dados, troca de nome e de volta os
//! índices e triggers — tudo numa transação, com as foreign keys desligadas e
//! conferidas antes do commit.
//!
//! A definição nova sai da `CREATE TABLE` original, lida do `sqlite_schema` e
//! alterada só na coluna editada — constraints, `AUTOINCREMENT`, `STRICT` e
//! `WITHOUT ROWID` continuam como estavam.

use std::time::Instant;

use rusqlite::{Connection, OptionalExtension};
use sqlparser::ast::{ColumnDef, ColumnOption, Ident, ObjectName, Statement};
use sqlparser::dialect::SQLiteDialect;
use sqlparser::parser::Parser;

use crate::adapters::ddl::{
    column_sql, create_table_body, quote_ident, validate_data_type, validate_default, validate_name,
};
use crate::error::{Error, Result};
use crate::models::{ColumnDefinition, RelationKind, StatementResult, StructureChange};

use super::util::db_err;

/// Prefixo da tabela temporária da recriação.
const REBUILD_PREFIX: &str = "_octapus_new_";

/// Mesmo tipo escrito de outro jeito (`INTEGER` e `integer`) não é mudança.
fn same_type(a: &str, b: &str) -> bool {
    let normalize = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    normalize(a) == normalize(b)
}

fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// O que o `ALTER TABLE` do SQLite não faz: aí a tabela é recriada.
fn needs_rebuild(original: &ColumnDefinition, updated: &ColumnDefinition) -> bool {
    !same_type(&original.data_type, &updated.data_type)
        || original.nullable != updated.nullable
        || trimmed(original.default_value.as_deref()) != trimmed(updated.default_value.as_deref())
}

fn is_rebuild(change: &StructureChange) -> bool {
    matches!(change, StructureChange::AlterColumn { original, updated, .. } if needs_rebuild(original, updated))
}

/// A coluna como o diálogo descreve, já como nó da AST do SQLite.
fn parse_column(column: &ColumnDefinition) -> Result<ColumnDef> {
    let sql = format!("CREATE TABLE t ({})", column_sql(column, &SQLiteDialect {}, false)?);
    match Parser::parse_sql(&SQLiteDialect {}, &sql).ok().and_then(|mut parsed| parsed.pop()) {
        Some(Statement::CreateTable(mut table)) if table.columns.len() == 1 => {
            Ok(table.columns.remove(0))
        }
        _ => Err(Error::InvalidQuery(format!(
            "Invalid column definition: {}",
            column.name.trim()
        ))),
    }
}

/// A recriação da tabela com `column` (já com o nome final) redefinida.
fn rebuild_statements(conn: &Connection, table: &str, column: &ColumnDefinition) -> Result<Vec<String>> {
    let create: String = conn
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_err)?
        .ok_or_else(|| Error::NotFound(format!("Table {table} not found")))?;

    let unreadable = || Error::InvalidQuery(format!("Could not read the definition of {table}"));
    let Some(Statement::CreateTable(mut definition)) = Parser::parse_sql(&SQLiteDialect {}, &create)
        .map_err(|_| unreadable())?
        .pop()
    else {
        return Err(unreadable());
    };

    let name = column.name.trim();
    let target = definition
        .columns
        .iter_mut()
        .find(|existing| existing.name.value.eq_ignore_ascii_case(name))
        .ok_or_else(|| Error::NotFound(format!("Column {name} not found in {table}")))?;
    let replacement = parse_column(column)?;
    target.data_type = replacement.data_type;
    // O resto (PRIMARY KEY, UNIQUE, CHECK, REFERENCES, COLLATE…) fica
    target.options.retain(|option| {
        !matches!(
            option.option,
            ColumnOption::Null | ColumnOption::NotNull | ColumnOption::Default(_)
        )
    });
    target.options.extend(replacement.options);

    let temp = format!("{REBUILD_PREFIX}{table}");
    definition.name = ObjectName::from(vec![Ident::with_quote('"', temp.clone())]);

    // Coluna gerada não recebe valor: é recalculada na tabela nova
    let copied = definition
        .columns
        .iter()
        .filter(|column| {
            !column
                .options
                .iter()
                .any(|option| matches!(option.option, ColumnOption::Generated { .. }))
        })
        .map(|column| quote_ident(&column.name.value))
        .collect::<Vec<_>>()
        .join(", ");

    let mut statements = vec![
        // Views que citam a tabela continuam válidas durante a troca de nome
        "PRAGMA legacy_alter_table = ON".to_string(),
        Statement::CreateTable(definition).to_string(),
        format!(
            "INSERT INTO {} ({copied}) SELECT {copied} FROM {}",
            quote_ident(&temp),
            quote_ident(table)
        ),
        format!("DROP TABLE {}", quote_ident(table)),
        format!("ALTER TABLE {} RENAME TO {}", quote_ident(&temp), quote_ident(table)),
    ];

    // Índices e triggers somem com a tabela antiga: voltam como eram
    let mut stmt = conn
        .prepare(
            "SELECT sql FROM sqlite_schema \
             WHERE tbl_name = ?1 AND type IN ('index', 'trigger') AND sql IS NOT NULL \
             ORDER BY type, name",
        )
        .map_err(db_err)?;
    let dependents = stmt
        .query_map([table], |row| row.get::<_, String>(0))
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_err)?;
    statements.extend(dependents);
    Ok(statements)
}

/// Os statements de uma mudança, rodando cada um quando `execute`. Sem
/// `execute` (a prévia) só lista — menos o rename de coluna que antecede uma
/// recriação, que roda sempre: a recriação parte do schema já renomeado (os
/// índices e triggers citam o nome novo). A prévia desfaz no rollback.
fn run(conn: &Connection, change: &StructureChange, execute: bool) -> Result<Vec<String>> {
    let dialect = SQLiteDialect {};
    let mut done = Vec::new();
    let mut step = |sql: String, force: bool| -> Result<()> {
        if execute || force {
            conn.execute_batch(&sql).map_err(db_err)?;
        }
        done.push(sql);
        Ok(())
    };

    match change {
        StructureChange::CreateTable { table, columns, .. } => step(
            format!(
                "CREATE TABLE {} (\n{}\n)",
                quote_ident(validate_name(table, "Table")?),
                create_table_body(columns, &dialect)?
            ),
            false,
        )?,
        StructureChange::RenameTable { table, new_name, .. } => step(
            format!(
                "ALTER TABLE {} RENAME TO {}",
                quote_ident(validate_name(table, "Table")?),
                quote_ident(validate_name(new_name, "Table")?)
            ),
            false,
        )?,
        StructureChange::DropTable { table, kind, .. } => step(
            format!(
                "DROP {} {}",
                if *kind == RelationKind::View { "VIEW" } else { "TABLE" },
                quote_ident(validate_name(table, "Table")?)
            ),
            false,
        )?,
        StructureChange::AddColumn { table, column, .. } => step(
            format!(
                "ALTER TABLE {} ADD COLUMN {}",
                quote_ident(validate_name(table, "Table")?),
                column_sql(column, &dialect, true)?
            ),
            false,
        )?,
        StructureChange::DropColumn { table, column, .. } => step(
            format!(
                "ALTER TABLE {} DROP COLUMN {}",
                quote_ident(validate_name(table, "Table")?),
                quote_ident(validate_name(column, "Column")?)
            ),
            false,
        )?,
        StructureChange::AlterColumn { table, original, updated, .. } => {
            let table = validate_name(table, "Table")?;
            let from = validate_name(&original.name, "Column")?;
            let to = validate_name(&updated.name, "Column")?;
            let rebuild = needs_rebuild(original, updated);
            if rebuild {
                // Valida antes de qualquer passo rodar
                validate_data_type(&updated.data_type)?;
                validate_default(updated.default_value.as_deref(), &dialect)?;
            }
            if from != to {
                step(
                    format!(
                        "ALTER TABLE {} RENAME COLUMN {} TO {}",
                        quote_ident(table),
                        quote_ident(from),
                        quote_ident(to)
                    ),
                    rebuild,
                )?;
            }
            if rebuild {
                for sql in rebuild_statements(conn, table, updated)? {
                    step(sql, false)?;
                }
            }
        }
    }
    Ok(done)
}

/// Os statements que [`apply`] vai rodar, sem mudar nada.
pub fn preview(conn: &mut Connection, change: &StructureChange) -> Result<Vec<String>> {
    // Sem commit: o drop da transação desfaz o rename feito para a prévia
    let tx = conn.transaction().map_err(db_err)?;
    run(&tx, change, false)
}

/// Aplica a mudança numa transação. A recriação de tabela desliga as foreign
/// keys (dentro de uma transação o pragma não tem efeito) e confere tudo antes
/// do commit; o estado anterior volta mesmo se falhar.
pub fn apply(conn: &mut Connection, change: &StructureChange) -> Result<StatementResult> {
    let start = Instant::now();
    let rebuild = is_rebuild(change);
    let foreign_keys = rebuild
        && conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))
            .map_err(db_err)?
            != 0;
    if rebuild {
        conn.execute_batch("PRAGMA foreign_keys = OFF").map_err(db_err)?;
    }

    let result = (|| -> Result<()> {
        let tx = conn.transaction().map_err(db_err)?;
        run(&tx, change, true)?;
        if foreign_keys {
            let broken: Option<String> = tx
                .query_row("SELECT \"table\" FROM pragma_foreign_key_check LIMIT 1", [], |row| {
                    row.get(0)
                })
                .optional()
                .map_err(db_err)?;
            if let Some(table) = broken {
                return Err(Error::Query(format!(
                    "The change breaks a foreign key in {table}"
                )));
            }
        }
        tx.commit().map_err(db_err)
    })();

    if rebuild {
        let restore = format!(
            "PRAGMA legacy_alter_table = OFF; PRAGMA foreign_keys = {};",
            if foreign_keys { "ON" } else { "OFF" }
        );
        conn.execute_batch(&restore).map_err(db_err)?;
    }
    result?;

    Ok(StatementResult {
        affected_rows: 0,
        execution_time_ms: start.elapsed().as_millis() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str, data_type: &str, nullable: bool, default: Option<&str>) -> ColumnDefinition {
        ColumnDefinition {
            name: name.into(),
            data_type: data_type.into(),
            nullable,
            default_value: default.map(Into::into),
            primary_key: false,
        }
    }

    fn fixture() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE clientes (id INTEGER PRIMARY KEY AUTOINCREMENT, nome TEXT);
             CREATE TABLE pedidos (
                 id INTEGER PRIMARY KEY,
                 cliente_id INTEGER REFERENCES clientes(id),
                 status TEXT DEFAULT 'novo',
                 total REAL,
                 dobro REAL GENERATED ALWAYS AS (total * 2) VIRTUAL
             );
             CREATE INDEX idx_pedidos_status ON pedidos(status);
             CREATE TRIGGER trg_pedidos AFTER INSERT ON pedidos BEGIN SELECT 1; END;
             CREATE VIEW resumo AS SELECT status, count(*) AS n FROM pedidos GROUP BY status;
             INSERT INTO clientes (nome) VALUES ('Ana');
             INSERT INTO pedidos (cliente_id, status, total) VALUES (1, 'pago', 10), (1, NULL, 5);",
        )
        .unwrap();
        conn
    }

    fn alter(original: ColumnDefinition, updated: ColumnDefinition) -> StructureChange {
        StructureChange::AlterColumn {
            schema: None,
            table: "pedidos".into(),
            original,
            updated,
        }
    }

    fn schema_sql(conn: &Connection, name: &str) -> String {
        conn.query_row("SELECT sql FROM sqlite_schema WHERE name = ?1", [name], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn creates_alters_and_drops_with_plain_alter_table() {
        let mut conn = fixture();
        let mut id = column("id", "INTEGER", false, None);
        id.primary_key = true;
        let create = StructureChange::CreateTable {
            schema: None,
            table: "notas".into(),
            columns: vec![id, column("texto", "TEXT", true, Some("''"))],
        };
        assert_eq!(
            preview(&mut conn, &create).unwrap(),
            ["CREATE TABLE \"notas\" (\n  \"id\" INTEGER NOT NULL,\n  \"texto\" TEXT DEFAULT '',\n  PRIMARY KEY (\"id\")\n)"]
        );
        apply(&mut conn, &create).unwrap();

        let add = StructureChange::AddColumn {
            schema: None,
            table: "notas".into(),
            column: column("autor", "TEXT", true, None),
        };
        apply(&mut conn, &add).unwrap();

        // Só renomear não precisa recriar
        let rename = StructureChange::AlterColumn {
            schema: None,
            table: "notas".into(),
            original: column("autor", "TEXT", true, None),
            updated: column("escritor", "TEXT", true, None),
        };
        assert_eq!(preview(&mut conn, &rename).unwrap().len(), 1);
        apply(&mut conn, &rename).unwrap();

        apply(
            &mut conn,
            &StructureChange::DropColumn {
                schema: None,
                table: "notas".into(),
                column: "escritor".into(),
                cascade: false,
            },
        )
        .unwrap();
        apply(
            &mut conn,
            &StructureChange::RenameTable {
                schema: None,
                table: "notas".into(),
                new_name: "anotacoes".into(),
            },
        )
        .unwrap();
        assert!(schema_sql(&conn, "anotacoes").contains("\"texto\" TEXT DEFAULT ''"));

        apply(
            &mut conn,
            &StructureChange::DropTable {
                schema: None,
                table: "anotacoes".into(),
                kind: RelationKind::Table,
                cascade: false,
            },
        )
        .unwrap();
        let left: i64 = conn
            .query_row("SELECT count(*) FROM sqlite_schema WHERE name = 'anotacoes'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0);
    }

    #[test]
    fn changing_type_or_null_rebuilds_and_keeps_everything_else() {
        let mut conn = fixture();
        let change = alter(
            column("status", "TEXT", true, Some("'novo'")),
            column("situacao", "VARCHAR(20)", false, Some("'aberto'")),
        );

        // A prévia não muda nada — nem o rename que ela precisou fazer
        let planned = preview(&mut conn, &change).unwrap();
        assert!(planned[0].starts_with("ALTER TABLE \"pedidos\" RENAME COLUMN \"status\" TO \"situacao\""));
        assert!(planned.iter().any(|sql| sql.starts_with("CREATE TABLE \"_octapus_new_pedidos\"")));
        assert!(planned.iter().any(|sql| sql.contains("idx_pedidos_status")));
        assert!(schema_sql(&conn, "pedidos").contains("status TEXT"));

        // NOT NULL com uma linha nula: falha e não deixa nada pela metade
        let err = apply(&mut conn, &change).unwrap_err();
        assert!(err.to_string().contains("NOT NULL"), "erro inesperado: {err}");
        assert!(schema_sql(&conn, "pedidos").contains("status TEXT"));

        conn.execute_batch("UPDATE pedidos SET status = 'novo' WHERE status IS NULL").unwrap();
        apply(&mut conn, &change).unwrap();

        let sql = schema_sql(&conn, "pedidos");
        assert!(sql.contains("\"situacao\" VARCHAR(20) NOT NULL DEFAULT 'aberto'"), "{sql}");
        // O resto da definição não mudou
        assert!(sql.contains("REFERENCES clientes (id)"), "{sql}");
        assert!(sql.contains("GENERATED ALWAYS AS"), "{sql}");
        // Índice, trigger e view sobreviveram, já com o nome novo
        assert!(schema_sql(&conn, "idx_pedidos_status").contains("situacao"));
        schema_sql(&conn, "trg_pedidos");
        let rows: i64 = conn.query_row("SELECT sum(n) FROM resumo", [], |r| r.get(0)).unwrap();
        assert_eq!(rows, 2);
        let dobro: f64 = conn
            .query_row("SELECT dobro FROM pedidos WHERE total = 10", [], |r| r.get(0))
            .unwrap();
        assert_eq!(dobro, 20.0);

        // As foreign keys voltaram a valer
        let fk: i64 = conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0)).unwrap();
        assert_eq!(fk, 1);
        let legacy: i64 = conn.query_row("PRAGMA legacy_alter_table", [], |r| r.get(0)).unwrap();
        assert_eq!(legacy, 0);
    }

    #[test]
    fn autoincrement_survives_a_rebuild() {
        let mut conn = fixture();
        apply(
            &mut conn,
            &StructureChange::AlterColumn {
                schema: None,
                table: "clientes".into(),
                original: column("nome", "TEXT", true, None),
                updated: column("nome", "TEXT", true, Some("'sem nome'")),
            },
        )
        .unwrap();
        let sql = schema_sql(&conn, "clientes");
        assert!(sql.to_uppercase().contains("AUTOINCREMENT"), "{sql}");
        assert!(sql.contains("DEFAULT 'sem nome'"), "{sql}");
        let nome: String = conn.query_row("SELECT nome FROM clientes", [], |r| r.get(0)).unwrap();
        assert_eq!(nome, "Ana");
    }
}
