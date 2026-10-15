//! Peças comuns da edição de estrutura (DDL) pela árvore, para o Postgres e o
//! SQLite: os dois citam identificadores com aspas duplas e aceitam a mesma
//! forma de coluna (`"nome" tipo NOT NULL DEFAULT expr`).
//!
//! Tipo e default chegam como trechos de SQL digitados pelo usuário. Não dá
//! para montar parametrizado (DDL não aceita parâmetro), então cada trecho é
//! validado para ser só aquilo que diz ser: um tipo não pode fechar o statement
//! e começar outro, e o default tem que ser uma única expressão.

use sqlparser::dialect::Dialect;
use sqlparser::parser::Parser;
use sqlparser::tokenizer::Token;

use crate::error::{Error, Result};
use crate::models::ColumnDefinition;

/// Cita um identificador (aspas duplas, dobradas quando aparecem no nome).
pub fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

/// Nome de tabela ou coluna: o que vier, desde que exista — a citação cuida
/// de espaço, maiúscula e aspas.
pub fn validate_name<'a>(name: &'a str, what: &str) -> Result<&'a str> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::InvalidQuery(format!("{what} name is required")));
    }
    if name.contains('\0') {
        return Err(Error::InvalidQuery(format!("{what} name contains an invalid character")));
    }
    Ok(name)
}

/// Um tipo (`integer`, `varchar(80)`, `numeric(10, 2)`, `timestamp with time
/// zone`, `text[]`, `"MeuTipo"`, `public.status`): letras, dígitos, espaço e
/// `_ ( ) [ ] , . "`. Nada de `;`, aspas simples ou comentário.
pub fn validate_data_type(data_type: &str) -> Result<&str> {
    let data_type = data_type.trim();
    if data_type.is_empty() {
        return Err(Error::InvalidQuery("Column type is required".into()));
    }
    let allowed = |c: char| c.is_alphanumeric() || " _()[],.\"".contains(c);
    if !data_type.chars().all(allowed) {
        return Err(Error::InvalidQuery(format!("Invalid column type: {data_type}")));
    }
    Ok(data_type)
}

/// Um default: uma única expressão (`0`, `'ativo'`, `now()`,
/// `gen_random_uuid()`). Vazio = sem default.
pub fn validate_default<'a>(raw: Option<&'a str>, dialect: &dyn Dialect) -> Result<Option<&'a str>> {
    let Some(expr) = raw.map(str::trim).filter(|expr| !expr.is_empty()) else {
        return Ok(None);
    };

    let mut parser = Parser::new(dialect)
        .try_with_sql(expr)
        .map_err(|e| Error::InvalidQuery(format!("Invalid default value: {e}")))?;
    parser
        .parse_expr()
        .map_err(|e| Error::InvalidQuery(format!("Invalid default value: {e}")))?;
    if !matches!(parser.peek_token().token, Token::EOF) {
        return Err(Error::InvalidQuery(
            "Default value must be a single expression".into(),
        ));
    }
    Ok(Some(expr))
}

/// `"nome" tipo [NOT NULL] [DEFAULT expr] [PRIMARY KEY]`. A chave primária só
/// sai aqui com `inline_pk`; na criação de tabela ela vai numa constraint no
/// fim (cobre a chave composta).
pub fn column_sql(column: &ColumnDefinition, dialect: &dyn Dialect, inline_pk: bool) -> Result<String> {
    let name = validate_name(&column.name, "Column")?;
    let data_type = validate_data_type(&column.data_type)?;
    let mut sql = format!("{} {data_type}", quote_ident(name));
    if !column.nullable {
        sql.push_str(" NOT NULL");
    }
    if let Some(default) = validate_default(column.default_value.as_deref(), dialect)? {
        sql.push_str(" DEFAULT ");
        sql.push_str(default);
    }
    if inline_pk && column.primary_key {
        sql.push_str(" PRIMARY KEY");
    }
    Ok(sql)
}

/// O corpo de um `CREATE TABLE`: as colunas e, se houver, a chave primária.
pub fn create_table_body(columns: &[ColumnDefinition], dialect: &dyn Dialect) -> Result<String> {
    if columns.is_empty() {
        return Err(Error::InvalidQuery("A table needs at least one column".into()));
    }

    let mut seen = std::collections::HashSet::new();
    let mut parts = Vec::with_capacity(columns.len() + 1);
    for column in columns {
        if !seen.insert(column.name.trim()) {
            return Err(Error::InvalidQuery(format!(
                "Duplicate column: {}",
                column.name.trim()
            )));
        }
        parts.push(format!("  {}", column_sql(column, dialect, false)?));
    }

    let keys: Vec<String> = columns
        .iter()
        .filter(|column| column.primary_key)
        .map(|column| quote_ident(column.name.trim()))
        .collect();
    if !keys.is_empty() {
        parts.push(format!("  PRIMARY KEY ({})", keys.join(", ")));
    }
    Ok(parts.join(",\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlparser::dialect::PostgreSqlDialect;

    fn column(name: &str, data_type: &str) -> ColumnDefinition {
        ColumnDefinition {
            name: name.into(),
            data_type: data_type.into(),
            nullable: true,
            default_value: None,
            primary_key: false,
        }
    }

    #[test]
    fn types_cannot_smuggle_another_statement() {
        for ok in ["integer", "varchar(80)", "numeric(10, 2)", "timestamp with time zone", "text[]", "\"MeuTipo\"", "public.status"] {
            assert!(validate_data_type(ok).is_ok(), "{ok} devia passar");
        }
        for bad in ["", "int; DROP TABLE x", "text -- x", "text /* x */", "varchar(1)'"] {
            assert!(validate_data_type(bad).is_err(), "{bad} devia falhar");
        }
    }

    #[test]
    fn defaults_are_a_single_expression() {
        let dialect = PostgreSqlDialect {};
        assert_eq!(validate_default(Some("  "), &dialect).unwrap(), None);
        assert_eq!(validate_default(Some("now()"), &dialect).unwrap(), Some("now()"));
        assert_eq!(validate_default(Some("'ativo'"), &dialect).unwrap(), Some("'ativo'"));
        assert!(validate_default(Some("1; DROP TABLE x"), &dialect).is_err());
        assert!(validate_default(Some("1 2"), &dialect).is_err());
    }

    #[test]
    fn create_table_puts_the_key_at_the_end() {
        let dialect = PostgreSqlDialect {};
        let mut id = column("id", "integer");
        id.nullable = false;
        id.primary_key = true;
        let mut name = column("na\"me", "text");
        name.default_value = Some("'x'".into());

        let body = create_table_body(&[id, name], &dialect).unwrap();
        assert_eq!(
            body,
            "  \"id\" integer NOT NULL,\n  \"na\"\"me\" text DEFAULT 'x',\n  PRIMARY KEY (\"id\")"
        );

        assert!(create_table_body(&[], &dialect).is_err());
        assert!(create_table_body(&[column("a", "int"), column("a", "int")], &dialect).is_err());
        assert!(create_table_body(&[column(" ", "int")], &dialect).is_err());
    }
}
