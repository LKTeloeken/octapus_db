use std::cmp::Ordering;
use std::time::Instant;

use redis::aio::ConnectionManager;

use crate::error::{Error, Result};
use crate::models::{QueryColumnInfo, QueryResult, SortDirection, TableDataRequest};

use super::metadata::{scan_keys, ROOT_GROUP, SCAN_CAP};

/// Max elements rendered for collection values (lists, hashes, sets, zsets).
const VALUE_ELEMENT_CAP: isize = 100;

const COLUMNS: [&str; 4] = ["key", "type", "ttl", "value"];

struct KeyEntry {
    key: String,
    type_name: String,
    ttl: Option<i64>,
    value: Option<String>,
}

impl KeyEntry {
    fn column(&self, name: &str) -> Option<String> {
        match name {
            "key" => Some(self.key.clone()),
            "type" => Some(self.type_name.clone()),
            "ttl" => self.ttl.map(|t| t.to_string()),
            "value" => self.value.clone(),
            _ => None,
        }
    }
}

/// Browse a key-prefix group: SCAN candidates, sort client-side over
/// the synthetic columns (key/type/ttl/value), then paginate. The sweep is
/// capped at [`SCAN_CAP`] keys, so results over huge keyspaces are partial.
pub async fn fetch_table_data(
    conn: &mut ConnectionManager,
    request: TableDataRequest,
) -> Result<QueryResult> {
    if request
        .where_expr
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty())
    {
        return Err(Error::InvalidQuery(
            "Raw WHERE is only supported for PostgreSQL".into(),
        ));
    }

    for s in &request.sort {
        validate_column(&s.column)?;
    }

    let start = Instant::now();

    let pattern = if request.table == ROOT_GROUP {
        "*".to_string()
    } else {
        format!("{}:*", request.table)
    };

    let mut keys = scan_keys(conn, &pattern, SCAN_CAP).await?;

    // The "*" pattern also matches prefixed keys; keep only true root keys
    if request.table == ROOT_GROUP {
        keys.retain(|k| !k.contains(':'));
    }

    keys.sort();

    let needs_details_early = request.sort.iter().any(|s| s.column != "key");

    let mut entries: Vec<KeyEntry> = if needs_details_early {
        let mut all = hydrate(conn, &keys).await?;
        sort_entries(&mut all, &request);
        all
    } else {
        // Sort is by key (or absent): paginate on names, hydrate only the page
        Vec::new()
    };

    let (total, page_entries) = if needs_details_early {
        let total = entries.len() as i64;
        let from = (request.offset.max(0) as usize).min(entries.len());
        let to = (from + request.limit as usize + 1).min(entries.len());
        (total, entries.drain(from..to).collect::<Vec<_>>())
    } else {
        if request
            .sort
            .first()
            .map(|s| s.direction == SortDirection::Desc)
            .unwrap_or(false)
        {
            keys.reverse();
        }
        let total = keys.len() as i64;
        let from = (request.offset.max(0) as usize).min(keys.len());
        let to = (from + request.limit as usize + 1).min(keys.len());
        (total, hydrate(conn, &keys[from..to]).await?)
    };

    let has_more = page_entries.len() as i64 > request.limit;
    let page = &page_entries[..page_entries.len().min(request.limit as usize)];

    let rows: Vec<Vec<Option<String>>> = page
        .iter()
        .map(|e| COLUMNS.iter().map(|c| e.column(c)).collect())
        .collect();

    Ok(QueryResult {
        columns: COLUMNS
            .iter()
            .map(|name| QueryColumnInfo {
                name: name.to_string(),
                type_name: if *name == "ttl" { "long" } else { "string" }.to_string(),
                type_oid: None,
            })
            .collect(),
        row_count: rows.len(),
        rows,
        total_count: request.count_total.then_some(total),
        has_more,
        execution_time_ms: start.elapsed().as_millis() as u64,
        editable_info: None, // edits go through native commands
    })
}

fn validate_column(column: &str) -> Result<()> {
    if COLUMNS.contains(&column) {
        Ok(())
    } else {
        Err(Error::InvalidQuery(format!(
            "Unknown column: {column} (available: key, type, ttl, value)"
        )))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Entry hydration (TYPE / TTL / value per key)
// ─────────────────────────────────────────────────────────────────────────────

async fn hydrate(conn: &mut ConnectionManager, keys: &[String]) -> Result<Vec<KeyEntry>> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }

    // Batch TYPE + TTL in a single pipeline round-trip
    let mut pipe = redis::pipe();
    for key in keys {
        pipe.cmd("TYPE").arg(key);
        pipe.cmd("TTL").arg(key);
    }
    let meta: Vec<redis::Value> = pipe.query_async(conn).await?;

    let mut entries = Vec::with_capacity(keys.len());
    for (i, key) in keys.iter().enumerate() {
        let type_name = match meta.get(i * 2) {
            Some(redis::Value::SimpleString(s)) => s.clone(),
            Some(redis::Value::BulkString(b)) => String::from_utf8_lossy(b).into_owned(),
            _ => "unknown".to_string(),
        };
        let ttl = match meta.get(i * 2 + 1) {
            Some(redis::Value::Int(t)) if *t >= 0 => Some(*t),
            _ => None, // -1 (no expiry) / -2 (missing) → NULL
        };

        let value = fetch_value(conn, key, &type_name).await?;

        entries.push(KeyEntry {
            key: key.clone(),
            type_name,
            ttl,
            value,
        });
    }

    Ok(entries)
}

/// Fetch a displayable value for a key according to its type, capping
/// collection types at [`VALUE_ELEMENT_CAP`] elements.
async fn fetch_value(
    conn: &mut ConnectionManager,
    key: &str,
    type_name: &str,
) -> Result<Option<String>> {
    let value = match type_name {
        "string" => redis::cmd("GET").arg(key).query_async(conn).await?,
        "list" => {
            redis::cmd("LRANGE")
                .arg(key)
                .arg(0)
                .arg(VALUE_ELEMENT_CAP - 1)
                .query_async(conn)
                .await?
        }
        "set" => {
            redis::cmd("SRANDMEMBER")
                .arg(key)
                .arg(VALUE_ELEMENT_CAP)
                .query_async(conn)
                .await?
        }
        "zset" => {
            redis::cmd("ZRANGE")
                .arg(key)
                .arg(0)
                .arg(VALUE_ELEMENT_CAP - 1)
                .arg("WITHSCORES")
                .query_async(conn)
                .await?
        }
        "hash" => {
            redis::cmd("HGETALL")
                .arg(key)
                .query_async(conn)
                .await?
        }
        // stream and others: show type only
        _ => redis::Value::Nil,
    };

    Ok(super::command::value_to_string(&value))
}

// ─────────────────────────────────────────────────────────────────────────────
// Client-side sorting over synthetic columns
// ─────────────────────────────────────────────────────────────────────────────

/// Numeric comparison when both sides parse as numbers, else lexicographic.
fn compare(a: &str, b: &str) -> Ordering {
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        _ => a.cmp(b),
    }
}

fn sort_entries(entries: &mut [KeyEntry], request: &TableDataRequest) {
    if request.sort.is_empty() {
        return;
    }

    entries.sort_by(|a, b| {
        for spec in &request.sort {
            let (va, vb) = (a.column(&spec.column), b.column(&spec.column));
            let ord = match (va, vb) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Less,
                (Some(_), None) => Ordering::Greater,
                (Some(x), Some(y)) => compare(&x, &y),
            };
            let ord = match spec.direction {
                SortDirection::Asc => ord,
                SortDirection::Desc => ord.reverse(),
            };
            if ord != Ordering::Equal {
                return ord;
            }
        }
        Ordering::Equal
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_aware_compare() {
        assert_eq!(compare("9", "10"), Ordering::Less);
        assert_eq!(compare("abc", "abd"), Ordering::Less);
        assert_eq!(compare("2.5", "2.50"), Ordering::Equal);
    }

    // ── End-to-end (requires a local Redis; run with `cargo test -- --ignored`) ──

    #[tokio::test]
    #[ignore = "requires local redis on localhost:6380 (no auth)"]
    async fn e2e_redis_adapter() {
        use crate::adapters::redisdb::RedisAdapter;
        use crate::adapters::DatabaseAdapter;
        use crate::models::{DatabaseType, QueryOptions, Server, SortSpec};

        let server = Server {
            id: Some(1),
            name: "e2e".into(),
            db_type: DatabaseType::Redis,
            host: "localhost".into(),
            port: 6380,
            username: String::new(),
            password: String::new(),
            default_database: None,
            ssl_enabled: false,
            connection_uri: None,
            created_at: 0,
        };

        let adapter = RedisAdapter::new(&server, "0").unwrap();
        adapter.test_connection().await.unwrap();

        // Seed: user:1..30 hashes, session:a/b strings, one root key
        adapter
            .execute_statement("FLUSHDB")
            .await
            .unwrap();
        for i in 1..=30 {
            adapter
                .execute_statement(&format!("HSET user:{i} name user{i} age {}", i % 10))
                .await
                .unwrap();
        }
        adapter.execute_statement("SET session:a abc").await.unwrap();
        adapter.execute_statement("SET session:b def").await.unwrap();
        adapter.execute_statement("SET rootkey hello").await.unwrap();
        adapter.execute_statement("EXPIRE session:a 3600").await.unwrap();

        // Structure: prefix groups
        let tables = adapter.list_tables("0").await.unwrap();
        let user_group = tables.iter().find(|t| t.name == "user").unwrap();
        assert_eq!(user_group.row_estimate, Some(30));
        assert!(tables.iter().any(|t| t.name == "session"));
        assert!(tables.iter().any(|t| t.name == super::ROOT_GROUP));

        // Browse user group: paginate sorted by key desc
        let result = adapter
            .fetch_table_data(TableDataRequest {
                schema: None,
                table: "user".into(),
                where_expr: None,
                sort: vec![SortSpec {
                    column: "key".into(),
                    direction: crate::models::SortDirection::Desc,
                }],
                limit: 10,
                offset: 0,
                count_total: true,
            })
            .await
            .unwrap();

        assert_eq!(result.total_count, Some(30));
        assert_eq!(result.row_count, 10);
        assert!(result.has_more);
        assert_eq!(result.columns.len(), 4); // key, type, ttl, value
        assert_eq!(result.rows[0][0].as_deref(), Some("user:9")); // lexicographic desc
        assert_eq!(result.rows[0][1].as_deref(), Some("hash"));
        assert!(result.rows[0][3].as_deref().unwrap().contains("name"));

        // Root group only contains unprefixed keys
        let root = adapter
            .fetch_table_data(TableDataRequest {
                schema: None,
                table: super::ROOT_GROUP.into(),
                where_expr: None,
                sort: vec![],
                limit: 10,
                offset: 0,
                count_total: true,
            })
            .await
            .unwrap();
        assert_eq!(root.total_count, Some(1));
        assert_eq!(root.rows[0][0].as_deref(), Some("rootkey"));

        // Free-form editor commands
        let get = adapter
            .execute_query("GET session:a", QueryOptions::default())
            .await
            .unwrap();
        assert_eq!(get.rows[0][0].as_deref(), Some("abc"));

        let hgetall = adapter
            .execute_query("HGETALL user:1", QueryOptions::default())
            .await
            .unwrap();
        assert_eq!(hgetall.columns.len(), 2); // field/value pairs
        assert_eq!(hgetall.row_count, 2); // name + age

        let deleted = adapter.execute_statement("DEL rootkey").await.unwrap();
        assert_eq!(deleted.affected_rows, 1);

        // Cleanup
        adapter.execute_statement("FLUSHDB").await.unwrap();
    }
}
