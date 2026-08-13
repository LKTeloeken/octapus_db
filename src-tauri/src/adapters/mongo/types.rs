use mongodb::bson::Bson;

/// Convert a BSON value to a display string for JSON transport.
/// `None` represents null/missing (rendered as NULL in the grid).
pub fn bson_to_string(value: &Bson) -> Option<String> {
    match value {
        Bson::Null | Bson::Undefined => None,
        Bson::String(s) => Some(s.clone()),
        Bson::Int32(v) => Some(v.to_string()),
        Bson::Int64(v) => Some(v.to_string()),
        Bson::Double(v) => Some(v.to_string()),
        Bson::Boolean(v) => Some(v.to_string()),
        Bson::ObjectId(oid) => Some(oid.to_hex()),
        Bson::DateTime(dt) => Some(
            dt.try_to_rfc3339_string()
                .unwrap_or_else(|_| dt.timestamp_millis().to_string()),
        ),
        Bson::Decimal128(v) => Some(v.to_string()),
        Bson::Binary(b) => Some(hex::encode(&b.bytes)),
        Bson::RegularExpression(r) => Some(format!("/{}/{}", r.pattern, r.options)),
        Bson::Timestamp(t) => Some(format!("{}:{}", t.time, t.increment)),
        // Nested documents/arrays: relaxed extended JSON
        other => Some(
            other
                .clone()
                .into_relaxed_extjson()
                .to_string(),
        ),
    }
}

/// Human-readable BSON type name (used as `data_type` / `type_name`).
pub fn bson_type_name(value: &Bson) -> &'static str {
    match value {
        Bson::Double(_) => "double",
        Bson::String(_) => "string",
        Bson::Array(_) => "array",
        Bson::Document(_) => "object",
        Bson::Boolean(_) => "bool",
        Bson::Null => "null",
        Bson::Int32(_) => "int",
        Bson::Int64(_) => "long",
        Bson::ObjectId(_) => "objectId",
        Bson::DateTime(_) => "date",
        Bson::Decimal128(_) => "decimal",
        Bson::Binary(_) => "binData",
        Bson::RegularExpression(_) => "regex",
        Bson::Timestamp(_) => "timestamp",
        Bson::Undefined => "undefined",
        _ => "mixed",
    }
}

/// Best-effort parse of a frontend-provided string into a typed BSON value.
/// Used for filters and row edits, where the grid only has strings.
pub fn parse_scalar(value: &str) -> Bson {
    if value == "null" {
        return Bson::Null;
    }
    if value == "true" {
        return Bson::Boolean(true);
    }
    if value == "false" {
        return Bson::Boolean(false);
    }
    if let Ok(v) = value.parse::<i64>() {
        return Bson::Int64(v);
    }
    if let Ok(v) = value.parse::<f64>() {
        return Bson::Double(v);
    }
    if let Ok(oid) = mongodb::bson::oid::ObjectId::parse_str(value) {
        return Bson::ObjectId(oid);
    }
    Bson::String(value.to_string())
}

/// SQL LIKE pattern → anchored regex (`%` → `.*`, `_` → `.`), escaping
/// everything else so user input can't inject regex syntax.
pub fn like_to_regex(pattern: &str) -> String {
    let mut regex = String::with_capacity(pattern.len() + 2);
    regex.push('^');
    for c in pattern.chars() {
        match c {
            '%' => regex.push_str(".*"),
            '_' => regex.push('.'),
            // Escape regex metacharacters
            '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|'
            | '\\' => {
                regex.push('\\');
                regex.push(c);
            }
            _ => regex.push(c),
        }
    }
    regex.push('$');
    regex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_scalar_types() {
        assert_eq!(parse_scalar("42"), Bson::Int64(42));
        assert_eq!(parse_scalar("4.5"), Bson::Double(4.5));
        assert_eq!(parse_scalar("true"), Bson::Boolean(true));
        assert_eq!(parse_scalar("null"), Bson::Null);
        assert_eq!(parse_scalar("hello"), Bson::String("hello".into()));
        assert!(matches!(
            parse_scalar("507f1f77bcf86cd799439011"),
            Bson::ObjectId(_)
        ));
    }

    #[test]
    fn like_pattern_conversion() {
        assert_eq!(like_to_regex("%ana%"), "^.*ana.*$");
        assert_eq!(like_to_regex("a_c"), "^a.c$");
        assert_eq!(like_to_regex("100% (a.b)"), "^100.* \\(a\\.b\\)$");
    }
}
