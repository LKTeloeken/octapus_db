use tokio_postgres::types::Type;
use tokio_postgres::Row;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};

/// Extract a PostgreSQL value as a String for JSON transport
pub fn extract_value(row: &Row, idx: usize, pg_type: &Type) -> Option<String> {
    // Macro to reduce boilerplate
    macro_rules! try_get {
        ($t:ty) => {
            row.get::<_, Option<$t>>(idx).map(|v| v.to_string())
        };
    }

    match *pg_type {
        // Integers
        Type::INT2 => try_get!(i16),
        Type::INT4 => try_get!(i32),
        Type::INT8 => try_get!(i64),
        Type::OID => try_get!(u32),

        // Floats
        Type::FLOAT4 => try_get!(f32),
        Type::FLOAT8 => try_get!(f64),

        // Boolean
        Type::BOOL => try_get!(bool),

        // Date/Time
        Type::TIMESTAMP => row.get::<_, Option<NaiveDateTime>>(idx).map(|v| v.to_string()),
        Type::TIMESTAMPTZ => row.get::<_, Option<DateTime<Utc>>>(idx).map(|v| v.to_rfc3339()),
        Type::DATE => row.get::<_, Option<NaiveDate>>(idx).map(|v| v.to_string()),
        Type::TIME | Type::TIMETZ => row.get::<_, Option<NaiveTime>>(idx).map(|v| v.to_string()),

        // Binary
        Type::BYTEA => row.get::<_, Option<Vec<u8>>>(idx).map(hex::encode),

        // Arrays
        Type::INT4_ARRAY => row.get::<_, Option<Vec<i32>>>(idx).map(|v| format!("{v:?}")),
        Type::INT8_ARRAY => row.get::<_, Option<Vec<i64>>>(idx).map(|v| format!("{v:?}")),
        Type::TEXT_ARRAY | Type::VARCHAR_ARRAY => {
            row.get::<_, Option<Vec<String>>>(idx).map(|v| format!("{v:?}"))
        }

        // Text types (most common - try this for unknown types too)
        _ => row.try_get::<_, Option<String>>(idx).ok().flatten(),
    }
}

/// Get type info for column metadata
pub fn type_category(pg_type: &Type) -> &'static str {
    match *pg_type {
        Type::INT2 | Type::INT4 | Type::INT8 | Type::OID => "integer",
        Type::FLOAT4 | Type::FLOAT8 | Type::NUMERIC => "number",
        Type::BOOL => "boolean",
        Type::TIMESTAMP | Type::TIMESTAMPTZ => "timestamp",
        Type::DATE => "date",
        Type::TIME | Type::TIMETZ => "time",
        Type::BYTEA => "binary",
        Type::JSON | Type::JSONB => "json",
        Type::UUID => "uuid",
        _ => "text",
    }
}