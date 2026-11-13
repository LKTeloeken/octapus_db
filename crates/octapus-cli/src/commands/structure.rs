//! `octapus dbs`, `ls` e `describe`: a estrutura de um banco, um nível por
//! vez (nunca a estrutura inteira de uma vez — ver o catálogo no BACKEND.md).

use octapus_core::models::TableType;
use serde_json::{json, Value};

use crate::cli::{DescribeArgs, LsArgs, Target};
use crate::context::App;
use crate::error::CliResult;
use crate::output::Grid;

fn to_value<T: serde::Serialize>(item: &T) -> Value {
    serde_json::to_value(item).unwrap_or(Value::Null)
}

fn yes_no(flag: bool) -> Option<String> {
    Some(if flag { "sim" } else { "não" }.to_string())
}

pub async fn databases(app: &App, target: &Target) -> CliResult<()> {
    let connected = app.connect(target).await?;
    let databases = connected.adapter.list_databases().await?;

    let mut grid = Grid::new(["DATABASE"]);
    for database in &databases {
        grid.push([Some(database.name.clone())]);
    }
    app.out.list(&grid, &databases.iter().map(to_value).collect::<Vec<_>>())
}

pub async fn list(app: &App, args: &LsArgs) -> CliResult<()> {
    let connected = app.connect(&args.target).await?;

    // Banco com schemas e nenhum pedido: lista os schemas
    if connected.capabilities.has_schemas && args.schema.is_none() {
        let schemas = connected.adapter.list_schemas().await?;
        let mut grid = Grid::new(["SCHEMA", "TABELAS"]);
        for schema in &schemas {
            grid.push([Some(schema.name.clone()), schema.table_count.map(|count| count.to_string())]);
        }
        return app.out.list(&grid, &schemas.iter().map(to_value).collect::<Vec<_>>());
    }

    let schema = args.schema.clone().unwrap_or_default();
    let tables = connected.adapter.list_tables(&schema).await?;
    let mut grid = Grid::new(["NOME", "TIPO", "LINHAS (ESTIMATIVA)"]);
    for table in &tables {
        let kind = match table.table_type {
            TableType::Table => "tabela",
            TableType::View => "view",
            TableType::MaterializedView => "view materializada",
            TableType::Foreign => "tabela externa",
        };
        grid.push([
            Some(table.name.clone()),
            Some(kind.to_string()),
            table.row_estimate.map(|rows| rows.to_string()),
        ]);
    }
    app.out.list(&grid, &tables.iter().map(to_value).collect::<Vec<_>>())
}

pub async fn describe(app: &App, args: &DescribeArgs) -> CliResult<()> {
    let connected = app.connect(&args.target).await?;
    let (schema, table) = connected.split_table(&args.table, args.schema.as_deref());

    let columns = connected.adapter.list_columns(&schema, &table).await?;
    if columns.is_empty() {
        return Err(octapus_core::Error::NotFound(format!("Table \"{}\"", args.table)).into());
    }
    let indexes = connected.adapter.list_indexes(&schema, &table).await?;

    use crate::cli::Format;
    match app.out.format {
        // Para programa, um documento só com as duas listas
        Format::Json | Format::Ndjson => app.out.record(
            &[],
            &json!({
                "schema": (!schema.is_empty()).then_some(&schema),
                "table": table,
                "columns": columns.iter().map(to_value).collect::<Vec<_>>(),
                "indexes": indexes.iter().map(to_value).collect::<Vec<_>>(),
            }),
        ),
        Format::Table | Format::Csv => {
            let mut grid = Grid::new(["COLUNA", "TIPO", "NULO", "PADRÃO", "PK", "FK"]);
            for column in &columns {
                grid.push([
                    Some(column.name.clone()),
                    Some(column.data_type.clone()),
                    yes_no(column.is_nullable),
                    column.default_value.clone(),
                    yes_no(column.is_primary_key),
                    yes_no(column.is_foreign_key),
                ]);
            }
            app.out.list(&grid, &[])?;

            if !indexes.is_empty() && app.out.format == Format::Table {
                println!();
                let mut grid = Grid::new(["ÍNDICE", "COLUNAS", "ÚNICO", "TIPO"]);
                for index in &indexes {
                    grid.push([
                        Some(index.name.clone()),
                        Some(index.columns.join(", ")),
                        yes_no(index.is_unique),
                        Some(index.index_type.clone()),
                    ]);
                }
                app.out.list(&grid, &[])?;
            }
            Ok(())
        }
    }
}
