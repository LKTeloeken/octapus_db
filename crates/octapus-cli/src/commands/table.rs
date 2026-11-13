//! `octapus table`: as linhas de uma tabela, pelo mesmo caminho do navegador
//! de tabelas do app (`fetch_table_data`). É leitura — e o `--where` de um
//! banco SQL passa antes pelo guard de leitura, para a condição não carregar
//! um segundo comando nem uma função que escreve.

use octapus_core::adapters::ReadOnlyOp;
use octapus_core::assistant::guard;
use octapus_core::models::{QueryLanguage, SortDirection, SortSpec, TableDataRequest};

use crate::cli::TableArgs;
use crate::commands::query::output_of;
use crate::context::App;
use crate::error::{CliError, CliResult};

/// Teto de linhas por chamada.
const MAX_LIMIT: i64 = 10_000;

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn parse_order(raw: &str) -> CliResult<SortSpec> {
    let (column, direction) = match raw.rsplit_once(':') {
        Some((column, direction)) => (column, direction.to_ascii_lowercase()),
        None => (raw, "asc".to_string()),
    };
    let direction = match direction.as_str() {
        "asc" => SortDirection::Asc,
        "desc" => SortDirection::Desc,
        other => return Err(CliError::Usage(format!("ordenação \"{other}\" inválida: use coluna ou coluna:desc"))),
    };
    if column.trim().is_empty() {
        return Err(CliError::Usage("falta o nome da coluna em --order".into()));
    }
    Ok(SortSpec {
        column: column.trim().to_string(),
        direction,
    })
}

pub async fn run(app: &App, args: &TableArgs) -> CliResult<()> {
    let connected = app.connect(&args.target).await?;
    let (schema, table) = connected.split_table(&args.table, args.schema.as_deref());
    let sort = args.order.iter().map(|raw| parse_order(raw)).collect::<CliResult<Vec<_>>>()?;

    if let (Some(condition), QueryLanguage::Sql) = (&args.r#where, connected.capabilities.query_language) {
        let relation = match schema.is_empty() {
            true => quote_ident(&table),
            false => format!("{}.{}", quote_ident(&schema), quote_ident(&table)),
        };
        // As quebras de linha impedem que um `--` na condição comente o resto
        let probe = format!("SELECT * FROM {relation} WHERE (\n{condition}\n)");
        guard::check(&ReadOnlyOp::Sql(probe), connected.server.db_type)?;
    }

    let request = TableDataRequest {
        schema: (!schema.is_empty()).then_some(schema),
        table,
        where_expr: args.r#where.clone(),
        sort,
        limit: args.limit.clamp(1, MAX_LIMIT),
        offset: args.offset.max(0),
        count_total: false,
        unlimited: false,
    };
    let result = connected.adapter.fetch_table_data(request).await?;
    app.out.query(&output_of(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_accepts_column_and_direction() {
        let spec = parse_order("created_at:DESC").unwrap();
        assert_eq!(spec.column, "created_at");
        assert!(matches!(spec.direction, SortDirection::Desc));
        assert!(matches!(parse_order("name").unwrap().direction, SortDirection::Asc));
        assert!(parse_order("name:sideways").is_err());
        assert!(parse_order(":desc").is_err());
    }
}
