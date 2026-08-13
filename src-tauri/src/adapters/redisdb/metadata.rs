use std::collections::BTreeMap;

use chrono::Utc;
use redis::aio::ConnectionManager;

use crate::error::Result;
use crate::models::{
    ColumnInfo, DatabaseInfo, DatabaseStructure, SchemaStructure, TableInfo, TableStructure,
    TableType,
};

/// Group name for keys without a `:`-separated prefix.
pub const ROOT_GROUP: &str = "(root)";

/// Safety cap on how many keys a single SCAN sweep will collect.
pub const SCAN_CAP: usize = 50_000;

const SCAN_BATCH: usize = 1000;

/// Collect keys matching `pattern` via cursor-based SCAN (never KEYS),
/// capped at `cap` keys.
pub async fn scan_keys(
    conn: &mut ConnectionManager,
    pattern: &str,
    cap: usize,
) -> Result<Vec<String>> {
    let mut keys = Vec::new();
    let mut cursor: u64 = 0;

    loop {
        let (next, batch): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(SCAN_BATCH)
            .query_async(conn)
            .await?;

        keys.extend(batch);
        cursor = next;

        if cursor == 0 || keys.len() >= cap {
            break;
        }
    }

    keys.truncate(cap);
    Ok(keys)
}

/// Logical databases are the numeric Redis databases (0..N).
pub async fn list_databases(conn: &mut ConnectionManager) -> Result<Vec<DatabaseInfo>> {
    // CONFIG GET may be disabled on managed Redis; fall back to the default 16
    let count: i64 = redis::cmd("CONFIG")
        .arg("GET")
        .arg("databases")
        .query_async::<Vec<String>>(conn)
        .await
        .ok()
        .and_then(|reply| reply.get(1).and_then(|v| v.parse().ok()))
        .unwrap_or(16);

    Ok((0..count)
        .map(|i| DatabaseInfo {
            name: i.to_string(),
            size_bytes: None,
        })
        .collect())
}

/// "Tables" are key groups: keys sharing the prefix before the first `:`.
/// Keys without a separator land in [`ROOT_GROUP`].
pub async fn list_tables(
    conn: &mut ConnectionManager,
    schema_name: &str,
) -> Result<Vec<TableInfo>> {
    let keys = scan_keys(conn, "*", SCAN_CAP).await?;
    let groups = group_by_prefix(&keys);

    Ok(groups
        .into_iter()
        .map(|(prefix, count)| TableInfo {
            name: prefix,
            schema: schema_name.to_string(),
            table_type: TableType::Table,
            row_estimate: Some(count),
        })
        .collect())
}

pub fn group_by_prefix(keys: &[String]) -> BTreeMap<String, i64> {
    let mut groups: BTreeMap<String, i64> = BTreeMap::new();
    for key in keys {
        let prefix = key
            .split_once(':')
            .map(|(p, _)| p.to_string())
            .unwrap_or_else(|| ROOT_GROUP.to_string());
        *groups.entry(prefix).or_insert(0) += 1;
    }
    groups
}

/// Synthetic columns: every key row exposes key / type / ttl / value.
pub fn list_columns() -> Vec<ColumnInfo> {
    let spec: [(&str, &str, bool); 4] = [
        ("key", "string", false),
        ("type", "string", false),
        ("ttl", "long", true), // -1 (no expiry) shown as NULL
        ("value", "string", true),
    ];

    spec.iter()
        .enumerate()
        .map(|(i, (name, data_type, nullable))| ColumnInfo {
            name: name.to_string(),
            ordinal: i as i32 + 1,
            data_type: data_type.to_string(),
            is_nullable: *nullable,
            default_value: None,
            is_primary_key: *name == "key",
            is_foreign_key: false,
        })
        .collect()
}

pub async fn list_schemas_with_tables(
    conn: &mut ConnectionManager,
    schema_name: &str,
) -> Result<DatabaseStructure> {
    let tables = list_tables(conn, schema_name)
        .await?
        .into_iter()
        .map(|t| TableStructure {
            name: t.name,
            table_type: t.table_type,
        })
        .collect();

    Ok(DatabaseStructure {
        schemas: vec![SchemaStructure {
            name: schema_name.to_string(),
            tables,
        }],
        fetched_at: Utc::now().timestamp_millis(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_by_first_colon_segment() {
        let keys = vec![
            "user:1".to_string(),
            "user:2".to_string(),
            "session:a".to_string(),
            "counter".to_string(),
            "user:profile:3".to_string(),
        ];
        let groups = group_by_prefix(&keys);
        assert_eq!(groups.get("user"), Some(&3));
        assert_eq!(groups.get("session"), Some(&1));
        assert_eq!(groups.get(ROOT_GROUP), Some(&1));
    }
}
