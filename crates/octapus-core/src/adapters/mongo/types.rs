use mongodb::bson::Bson;

use crate::error::{Error, Result};

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

/// Campo que a grade mostra como Extended JSON (`bson_type_name` "object"/"array").
pub fn is_compound(value: &Bson) -> bool {
    matches!(value, Bson::Document(_) | Bson::Array(_))
}

/// Texto com cara de documento/array JSON — o que vale a pena conferir contra
/// o tipo do campo na coleção quando o documento não diz.
pub fn looks_like_compound(value: &str) -> bool {
    matches!(value.trim_start().as_bytes().first(), Some(b'{' | b'['))
}

/// Converte o texto editado na grade para BSON conforme o tipo do campo.
/// Num campo object/array (`compound`) o texto é o Extended JSON que a grade
/// mostrou (`bson_to_string`) e volta a ser documento/array — `$oid`, `$date`
/// etc. aninhados viram os tipos BSON. Fora disso vale `parse_scalar`, então
/// um campo string com texto JSON continua string. JSON inválido num campo
/// object/array é erro, para não trocar o tipo do campo em silêncio.
pub fn parse_field_value(field: &str, value: &str, compound: bool) -> Result<Bson> {
    if !compound {
        return Ok(parse_scalar(value));
    }
    let json: serde_json::Value = serde_json::from_str(value).map_err(|e| {
        Error::InvalidQuery(format!(
            "O campo `{field}` é um documento/array: o valor precisa ser JSON válido ({e})"
        ))
    })?;
    Bson::try_from(json).map_err(|e| {
        Error::InvalidQuery(format!(
            "O campo `{field}` tem Extended JSON inválido ({e})"
        ))
    })
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
    fn parse_field_value_object() {
        let parsed = parse_field_value("atributos", r#"{"cor": "azul", "peso": 1.5}"#, true).unwrap();
        let Bson::Document(doc) = parsed else {
            panic!("esperava documento: {parsed:?}")
        };
        assert_eq!(doc.get_str("cor").unwrap(), "azul");
        assert_eq!(doc.get_f64("peso").unwrap(), 1.5);
    }

    #[test]
    fn parse_field_value_array() {
        let parsed = parse_field_value("categorias", r#"["casa", "moda"]"#, true).unwrap();
        assert_eq!(
            parsed,
            Bson::Array(vec![Bson::String("casa".into()), Bson::String("moda".into())])
        );
    }

    #[test]
    fn parse_field_value_nested_extjson() {
        let raw = r#"{"autor": {"$oid": "507f1f77bcf86cd799439011"}, "em": {"$date": "2025-01-02T03:04:05Z"}, "itens": [{"$oid": "507f1f77bcf86cd799439012"}]}"#;
        let Bson::Document(doc) = parse_field_value("meta", raw, true).unwrap() else {
            panic!("esperava documento")
        };
        assert_eq!(doc.get_object_id("autor").unwrap().to_hex(), "507f1f77bcf86cd799439011");
        assert_eq!(doc.get_datetime("em").unwrap().timestamp_millis(), 1_735_787_045_000);
        assert!(matches!(doc.get_array("itens").unwrap()[0], Bson::ObjectId(_)));
    }

    /// O que a grade mostra (`bson_to_string`) volta ao mesmo valor.
    #[test]
    fn parse_field_value_round_trips_display() {
        let original = Bson::Document(mongodb::bson::doc! {
            "autor": mongodb::bson::oid::ObjectId::parse_str("507f1f77bcf86cd799439011").unwrap(),
            "em": mongodb::bson::DateTime::from_millis(1_735_787_045_000),
            "tags": ["a", "b"],
            "ativo": true,
        });
        let shown = bson_to_string(&original).unwrap();
        assert_eq!(parse_field_value("meta", &shown, true).unwrap(), original);
    }

    #[test]
    fn parse_field_value_string_field_keeps_json_text() {
        let raw = r#"{"nao": "sou documento"}"#;
        assert_eq!(parse_field_value("obs", raw, false).unwrap(), Bson::String(raw.into()));
        assert_eq!(parse_field_value("obs", "[1,2]", false).unwrap(), Bson::String("[1,2]".into()));
    }

    #[test]
    fn parse_field_value_invalid_json_in_compound_field_is_error() {
        let err = parse_field_value("atributos", "{cor: azul", true).unwrap_err();
        assert!(matches!(err, Error::InvalidQuery(_)));
    }

    #[test]
    fn looks_like_compound_checks_first_char() {
        assert!(looks_like_compound("  {\"a\": 1}"));
        assert!(looks_like_compound("[]"));
        assert!(!looks_like_compound("texto"));
        assert!(!looks_like_compound(""));
    }
}
