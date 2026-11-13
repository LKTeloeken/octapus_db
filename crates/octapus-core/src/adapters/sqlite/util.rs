use rusqlite::types::{Value, ValueRef};
use rusqlite::Connection;

use crate::error::{Error, Result};

/// Erro vindo do banco do usuário.
///
/// O `From<rusqlite::Error>` global vira `Error::Storage`, que no app quer dizer
/// "o app.db local falhou" — aqui o problema é a query contra o banco aberto.
pub fn db_err(error: rusqlite::Error) -> Error {
    Error::Query(error.to_string())
}

/// Escapa um identificador do SQLite (mesma regra do Postgres: aspas duplas,
/// dobradas quando aparecem no nome).
pub fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

/// Metadados de uma coluna, na ordem física (`cid` do `PRAGMA table_info`).
pub struct SqliteColumn {
    pub name: String,
    /// Tipo declarado no `CREATE TABLE`. Pode vir vazio: o SQLite aceita coluna
    /// sem tipo, e nesse caso a afinidade é BLOB.
    pub declared_type: String,
    pub not_null: bool,
    pub default_value: Option<String>,
    /// Posição na chave primária (1-based); `0` quando a coluna não faz parte dela.
    pub pk_position: i32,
}

/// Colunas da tabela, na ordem do `CREATE TABLE`.
///
/// Usa a função de tabela `pragma_table_info`, que aceita parâmetro — o
/// `PRAGMA table_info(x)` exigiria interpolar o nome da tabela no SQL.
pub fn table_columns(conn: &Connection, table: &str) -> Result<Vec<SqliteColumn>> {
    let mut stmt = conn
        .prepare(
            "SELECT name, type, \"notnull\", dflt_value, pk \
             FROM pragma_table_info(?1) ORDER BY cid",
        )
        .map_err(db_err)?;

    let columns: Vec<SqliteColumn> = stmt
        .query_map([table], |row| {
            Ok(SqliteColumn {
                name: row.get(0)?,
                declared_type: row.get(1)?,
                not_null: row.get::<_, i32>(2)? != 0,
                // O default é o literal cru do CREATE TABLE: pode ser número,
                // texto ou expressão, então lemos sem exigir um tipo.
                default_value: value_to_string(row.get_ref(3)?),
                pk_position: row.get(4)?,
            })
        })
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_err)?;

    if columns.is_empty() {
        return Err(Error::InvalidQuery(format!(
            "Table {table} not found or has no columns"
        )));
    }

    Ok(columns)
}

/// Nomes das colunas da chave primária, na ordem declarada. Vazio quando a
/// tabela não tem PK — aí ela não é editável, igual ao Postgres.
pub fn pk_columns(columns: &[SqliteColumn]) -> Vec<String> {
    let mut pk: Vec<&SqliteColumn> = columns.iter().filter(|c| c.pk_position > 0).collect();
    pk.sort_by_key(|c| c.pk_position);
    pk.into_iter().map(|c| c.name.clone()).collect()
}

/// Converte uma célula do SQLite no `string | null` que a fronteira exige.
///
/// É o análogo do `::text` que o Postgres faz no servidor, só que em Rust: o
/// SQLite não tem um cast universal para texto que preserve blobs.
pub fn value_to_string(value: ValueRef<'_>) -> Option<String> {
    match value {
        ValueRef::Null => None,
        ValueRef::Integer(i) => Some(i.to_string()),
        ValueRef::Real(f) => Some(f.to_string()),
        ValueRef::Text(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
        // Blob não tem representação textual: sai como literal do próprio SQLite
        // (`x'..'`), que é o mesmo formato aceito de volta na edição.
        ValueRef::Blob(bytes) => Some(format!("x'{}'", hex::encode(bytes))),
    }
}

/// Nome do tipo quando a coluna não tem tipo declarado (expressões do editor
/// livre): cai na classe de armazenamento do valor.
pub fn storage_class_name(value: ValueRef<'_>) -> &'static str {
    match value {
        ValueRef::Null => "null",
        ValueRef::Integer(_) => "integer",
        ValueRef::Real(_) => "real",
        ValueRef::Text(_) => "text",
        ValueRef::Blob(_) => "blob",
    }
}

/// Afinidade de uma coluna, pelas regras oficiais do SQLite
/// (<https://sqlite.org/datatype3.html#determination_of_column_affinity>).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Affinity {
    Integer,
    Text,
    Blob,
    Real,
    Numeric,
}

pub fn affinity_of(declared_type: &str) -> Affinity {
    let t = declared_type.to_uppercase();

    if t.contains("INT") {
        Affinity::Integer
    } else if t.contains("CHAR") || t.contains("CLOB") || t.contains("TEXT") {
        Affinity::Text
    } else if t.is_empty() || t.contains("BLOB") {
        Affinity::Blob
    } else if t.contains("REAL") || t.contains("FLOA") || t.contains("DOUB") {
        Affinity::Real
    } else {
        Affinity::Numeric
    }
}

/// Converte o texto vindo do front no valor tipado que a coluna espera.
///
/// O SQLite é dinamicamente tipado: gravar tudo como texto faria uma coluna
/// inteira guardar `'42'`, quebrando ordenação, comparação e índices. Esta
/// função é o análogo do `$N::text::<tipo>` que o adapter do Postgres monta.
pub fn coerce_value(raw: Option<&str>, declared_type: &str) -> Result<Value> {
    let Some(raw) = raw else {
        return Ok(Value::Null);
    };

    let invalid = |kind: &str| Error::InvalidQuery(format!("'{raw}' is not a valid {kind} value"));
    let trimmed = raw.trim();

    Ok(match affinity_of(declared_type) {
        Affinity::Integer => match bool_to_int(trimmed) {
            Some(i) => Value::Integer(i),
            None => Value::Integer(trimmed.parse().map_err(|_| invalid("integer"))?),
        },
        Affinity::Real => Value::Real(trimmed.parse().map_err(|_| invalid("real"))?),
        // NUMERIC guarda como inteiro, senão como real, senão como texto — é o
        // que o próprio SQLite faz ao inserir nessa afinidade. O booleano entra
        // aqui porque `BOOLEAN` não casa com nenhuma outra regra de afinidade.
        Affinity::Numeric => match bool_to_int(trimmed) {
            Some(i) => Value::Integer(i),
            None => match trimmed.parse::<i64>() {
                Ok(i) => Value::Integer(i),
                Err(_) => match trimmed.parse::<f64>() {
                    Ok(f) => Value::Real(f),
                    Err(_) => Value::Text(raw.to_string()),
                },
            },
        },
        Affinity::Text => Value::Text(raw.to_string()),
        // Aceita de volta o literal `x'..'` produzido por `value_to_string`;
        // qualquer outra coisa entra como texto (o SQLite permite).
        Affinity::Blob => match parse_blob_literal(trimmed) {
            Some(bytes) => Value::Blob(bytes),
            None => Value::Text(raw.to_string()),
        },
    })
}

/// O editor de checkbox do front manda "true"/"false"; no SQLite booleano é 1/0.
fn bool_to_int(raw: &str) -> Option<i64> {
    match raw.to_ascii_lowercase().as_str() {
        "true" | "t" => Some(1),
        "false" | "f" => Some(0),
        _ => None,
    }
}

fn parse_blob_literal(raw: &str) -> Option<Vec<u8>> {
    let rest = raw
        .strip_prefix("x'")
        .or_else(|| raw.strip_prefix("X'"))?;
    hex::decode(rest.strip_suffix('\'')?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affinity_follows_sqlite_rules() {
        assert_eq!(affinity_of("INTEGER"), Affinity::Integer);
        assert_eq!(affinity_of("BIGINT"), Affinity::Integer);
        // "INT" vence "CHAR" porque a primeira regra do SQLite é a do INT
        assert_eq!(affinity_of("UNSIGNED BIG INT"), Affinity::Integer);
        assert_eq!(affinity_of("VARCHAR(50)"), Affinity::Text);
        assert_eq!(affinity_of("TEXT"), Affinity::Text);
        assert_eq!(affinity_of(""), Affinity::Blob);
        assert_eq!(affinity_of("BLOB"), Affinity::Blob);
        assert_eq!(affinity_of("DOUBLE"), Affinity::Real);
        assert_eq!(affinity_of("FLOAT"), Affinity::Real);
        assert_eq!(affinity_of("NUMERIC"), Affinity::Numeric);
        assert_eq!(affinity_of("DECIMAL(10,2)"), Affinity::Numeric);
        assert_eq!(affinity_of("BOOLEAN"), Affinity::Numeric);
        assert_eq!(affinity_of("DATETIME"), Affinity::Numeric);
    }

    #[test]
    fn null_is_preserved_for_any_type() {
        assert_eq!(coerce_value(None, "INTEGER").unwrap(), Value::Null);
        assert_eq!(coerce_value(None, "TEXT").unwrap(), Value::Null);
    }

    #[test]
    fn integer_column_keeps_integer_storage() {
        assert_eq!(coerce_value(Some("42"), "INTEGER").unwrap(), Value::Integer(42));
        assert_eq!(coerce_value(Some(" -7 "), "int").unwrap(), Value::Integer(-7));
    }

    #[test]
    fn integer_column_rejects_garbage() {
        let err = coerce_value(Some("abc"), "INTEGER").unwrap_err();
        assert!(err.to_string().contains("not a valid integer"));
    }

    #[test]
    fn real_column_keeps_real_storage() {
        assert_eq!(coerce_value(Some("19.9"), "REAL").unwrap(), Value::Real(19.9));
    }

    #[test]
    fn numeric_column_narrows_to_the_tightest_type() {
        assert_eq!(coerce_value(Some("10"), "NUMERIC").unwrap(), Value::Integer(10));
        assert_eq!(coerce_value(Some("10.5"), "NUMERIC").unwrap(), Value::Real(10.5));
        assert_eq!(
            coerce_value(Some("dez"), "NUMERIC").unwrap(),
            Value::Text("dez".into())
        );
    }

    /// O checkbox do front manda "true"/"false" — sem isso uma coluna BOOLEAN
    /// guardaria a string e pararia de casar com `WHERE flag = 1`.
    #[test]
    fn boolean_editor_values_become_one_and_zero() {
        assert_eq!(coerce_value(Some("true"), "BOOLEAN").unwrap(), Value::Integer(1));
        assert_eq!(coerce_value(Some("false"), "BOOLEAN").unwrap(), Value::Integer(0));
        assert_eq!(coerce_value(Some("t"), "INTEGER").unwrap(), Value::Integer(1));
    }

    #[test]
    fn text_column_keeps_the_raw_string() {
        assert_eq!(
            coerce_value(Some("  com espaço  "), "TEXT").unwrap(),
            Value::Text("  com espaço  ".into())
        );
    }

    #[test]
    fn blob_round_trips_through_the_hex_literal() {
        let rendered = value_to_string(ValueRef::Blob(b"hi")).unwrap();
        assert_eq!(rendered, "x'6869'");
        assert_eq!(
            coerce_value(Some(&rendered), "BLOB").unwrap(),
            Value::Blob(b"hi".to_vec())
        );
    }

    #[test]
    fn cells_are_rendered_as_strings() {
        assert_eq!(value_to_string(ValueRef::Null), None);
        assert_eq!(value_to_string(ValueRef::Integer(7)).as_deref(), Some("7"));
        assert_eq!(value_to_string(ValueRef::Text(b"oi")).as_deref(), Some("oi"));
    }

    #[test]
    fn identifiers_are_quoted() {
        assert_eq!(quote_ident("users"), "\"users\"");
        assert_eq!(quote_ident("we\"ird"), "\"we\"\"ird\"");
    }
}
