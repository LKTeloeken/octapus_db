use serde::Serialize;

/// Describes what a given adapter supports, so the frontend can decide what to
/// render (schema tree levels, editable grids, free-form editor, etc.) without
/// branching on `dbType` directly.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterCapabilities {
    /// Whether the database exposes a schema level between database and table
    /// (Postgres: yes; MongoDB/Redis: no).
    pub has_schemas: bool,

    /// Whether rows/documents have a stable primary key usable for editing
    /// (Postgres: per-table PK; MongoDB: `_id`; Redis: no).
    pub has_primary_keys: bool,

    /// Whether the free-form editor accepts SQL (Postgres) or native commands
    /// (MongoDB shell-style / Redis commands).
    pub supports_sql: bool,

    /// Whether multi-statement transactions are supported.
    pub supports_transactions: bool,

    /// Whether secondary indexes can be listed.
    pub supports_indexes: bool,

    /// Whether `fetch_table_data` (paginated/sorted/filtered browse) is available.
    pub browsable: bool,
}

impl AdapterCapabilities {
    pub const fn postgres() -> Self {
        Self {
            has_schemas: true,
            has_primary_keys: true,
            supports_sql: true,
            supports_transactions: true,
            supports_indexes: true,
            browsable: true,
        }
    }

    pub const fn mongodb() -> Self {
        Self {
            has_schemas: false,
            has_primary_keys: true, // `_id`
            supports_sql: false,
            supports_transactions: false,
            supports_indexes: true,
            browsable: true,
        }
    }

    pub const fn redis() -> Self {
        Self {
            has_schemas: false,
            has_primary_keys: false,
            supports_sql: false,
            supports_transactions: false,
            supports_indexes: false,
            browsable: true,
        }
    }

    /// Capabilities by database type, without requiring a live connection.
    /// `None` for types without an adapter yet.
    pub fn for_db_type(db_type: super::DatabaseType) -> Option<Self> {
        match db_type {
            super::DatabaseType::Postgres => Some(Self::postgres()),
            super::DatabaseType::Mongodb => Some(Self::mongodb()),
            super::DatabaseType::Redis => Some(Self::redis()),
            super::DatabaseType::Mysql | super::DatabaseType::Sqlite => None,
        }
    }
}
