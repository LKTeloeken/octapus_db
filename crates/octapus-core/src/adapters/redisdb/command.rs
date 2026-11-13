use redis::Value;

use crate::error::{Error, Result};
use crate::models::{QueryColumnInfo, QueryResult};

/// Tokenize a Redis command line, honoring single/double quotes and escapes,
/// e.g. `SET "my key" 'a value'` → ["SET", "my key", "a value"].
pub fn tokenize(line: &str) -> Result<Vec<String>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_string: Option<char> = None;
    let mut escaped = false;
    let mut has_token = false;

    for c in line.chars() {
        if let Some(quote) = in_string {
            if escaped {
                current.push(c);
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == quote {
                in_string = None;
            } else {
                current.push(c);
            }
            continue;
        }

        match c {
            '"' | '\'' => {
                in_string = Some(c);
                has_token = true;
            }
            c if c.is_whitespace() => {
                if has_token {
                    tokens.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            _ => {
                current.push(c);
                has_token = true;
            }
        }
    }

    if in_string.is_some() {
        return Err(Error::InvalidQuery("Unterminated string in command".into()));
    }
    if has_token {
        tokens.push(current);
    }

    if tokens.is_empty() {
        return Err(Error::InvalidQuery("Empty command".into()));
    }

    Ok(tokens)
}

/// Convert a Redis reply value to a display string.
pub fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Nil => None,
        Value::Int(v) => Some(v.to_string()),
        Value::Double(v) => Some(v.to_string()),
        Value::Boolean(v) => Some(v.to_string()),
        Value::SimpleString(s) => Some(s.clone()),
        Value::Okay => Some("OK".to_string()),
        Value::BulkString(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
        Value::BigNumber(n) => Some(n.to_string()),
        Value::VerbatimString { text, .. } => Some(text.clone()),
        Value::Array(items) | Value::Set(items) => {
            let parts: Vec<String> = items
                .iter()
                .map(|v| value_to_string(v).unwrap_or_else(|| "nil".into()))
                .collect();
            Some(format!("[{}]", parts.join(", ")))
        }
        Value::Map(pairs) => {
            let parts: Vec<String> = pairs
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{}: {}",
                        value_to_string(k).unwrap_or_else(|| "nil".into()),
                        value_to_string(v).unwrap_or_else(|| "nil".into())
                    )
                })
                .collect();
            Some(format!("{{{}}}", parts.join(", ")))
        }
        other => Some(format!("{other:?}")),
    }
}

/// Render a Redis reply as a tabular result. Pair-shaped replies (RESP3 maps,
/// or RESP2 flat field/value arrays from commands like HGETALL / CONFIG GET)
/// become two columns; arrays become one row per element; scalars one cell.
pub fn value_to_result(command_name: &str, value: Value) -> QueryResult {
    let upper = command_name.to_uppercase();
    let pairwise = matches!(upper.as_str(), "HGETALL" | "CONFIG" | "XPENDING");

    let (columns, rows): (Vec<&str>, Vec<Vec<Option<String>>>) = match value {
        Value::Map(pairs) => (
            vec!["field", "value"],
            pairs
                .iter()
                .map(|(k, v)| vec![value_to_string(k), value_to_string(v)])
                .collect(),
        ),
        Value::Array(items) if pairwise && items.len() % 2 == 0 => (
            vec!["field", "value"],
            items
                .chunks(2)
                .map(|pair| vec![value_to_string(&pair[0]), value_to_string(&pair[1])])
                .collect(),
        ),
        Value::Array(items) | Value::Set(items) => (
            vec!["value"],
            items.iter().map(|v| vec![value_to_string(v)]).collect(),
        ),
        Value::Nil => (vec!["value"], vec![]),
        scalar => (vec!["value"], vec![vec![value_to_string(&scalar)]]),
    };

    QueryResult {
        columns: columns
            .into_iter()
            .map(|name| QueryColumnInfo {
                name: name.to_string(),
                type_name: "string".to_string(),
                type_oid: None,
            })
            .collect(),
        row_count: rows.len(),
        rows,
        total_count: None,
        has_more: false,
        execution_time_ms: 0,
        editable_info: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_simple() {
        assert_eq!(tokenize("GET foo").unwrap(), vec!["GET", "foo"]);
    }

    #[test]
    fn tokenize_quoted_strings() {
        assert_eq!(
            tokenize(r#"SET "my key" 'a value'"#).unwrap(),
            vec!["SET", "my key", "a value"]
        );
    }

    #[test]
    fn tokenize_escapes() {
        assert_eq!(
            tokenize(r#"SET k "say \"hi\"""#).unwrap(),
            vec!["SET", "k", r#"say "hi""#]
        );
    }

    #[test]
    fn tokenize_empty_rejected() {
        assert!(tokenize("   ").is_err());
    }

    #[test]
    fn tokenize_unterminated_rejected() {
        assert!(tokenize(r#"SET k "oops"#).is_err());
    }

    #[test]
    fn hgetall_flat_array_becomes_pairs() {
        let value = Value::Array(vec![
            Value::BulkString(b"name".to_vec()),
            Value::BulkString(b"ana".to_vec()),
            Value::BulkString(b"age".to_vec()),
            Value::BulkString(b"30".to_vec()),
        ]);
        let result = value_to_result("HGETALL", value);
        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.row_count, 2);
        assert_eq!(result.rows[0][0].as_deref(), Some("name"));
        assert_eq!(result.rows[1][1].as_deref(), Some("30"));
    }

    #[test]
    fn generic_array_one_per_row() {
        let value = Value::Array(vec![
            Value::BulkString(b"a".to_vec()),
            Value::BulkString(b"b".to_vec()),
        ]);
        let result = value_to_result("LRANGE", value);
        assert_eq!(result.columns.len(), 1);
        assert_eq!(result.row_count, 2);
    }

    #[test]
    fn scalar_reply() {
        let result = value_to_result("GET", Value::BulkString(b"hello".to_vec()));
        assert_eq!(result.row_count, 1);
        assert_eq!(result.rows[0][0].as_deref(), Some("hello"));
    }
}
