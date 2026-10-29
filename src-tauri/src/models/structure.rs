use serde::{Deserialize, Serialize};

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    pub name: String,
    pub size_bytes: Option<i64>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaInfo {
    pub name: String,
    pub table_count: Option<i64>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableInfo {
    pub name: String,
    pub schema: String,
    pub table_type: TableType,
    pub row_estimate: Option<i64>,
    /// Mesmo significado de [`TableStructure::size_bytes`]
    pub size_bytes: Option<i64>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TableType {
    Table,
    View,
    MaterializedView,
    Foreign,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub name: String,
    pub ordinal: i32,
    pub data_type: String,
    pub is_nullable: bool,
    pub default_value: Option<String>,
    /// O banco gera o valor sem um DEFAULT declarado: identity do Postgres,
    /// `INTEGER PRIMARY KEY` do SQLite (apelido do rowid), `_id` do Mongo.
    /// Junto com `default_value`, diz que a coluna pode ficar fora do INSERT.
    pub is_auto_generated: bool,
    pub is_primary_key: bool,
    pub is_foreign_key: bool,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub is_unique: bool,
    pub is_primary: bool,
    pub index_type: String,
}

/// Full structure for caching (used sparingly)
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseStructure {
    pub schemas: Vec<SchemaStructure>,
    pub fetched_at: i64,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaStructure {
    pub name: String,
    pub tables: Vec<TableStructure>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableStructure {
    pub name: String,
    pub table_type: TableType,
    /// Tamanho total em disco (dados + índices), quando o banco informa.
    /// `None` para views e bancos sem suporte (Redis, SQLite).
    pub size_bytes: Option<i64>,
}
// ─────────────────────────────────────────────────────────────────────────────
// Edição da estrutura (DDL) pela árvore
// ─────────────────────────────────────────────────────────────────────────────

/// Uma coluna como o diálogo da árvore descreve. Tipo e default são trechos de
/// SQL do próprio banco (`varchar(80)`, `now()`): o backend valida que são um
/// tipo e uma expressão, não monta nem traduz.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnDefinition {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    #[serde(default)]
    pub default_value: Option<String>,
    /// Só vale na criação (tabela ou coluna nova)
    #[serde(default)]
    pub primary_key: bool,
}

/// O que se remove: a árvore lista tabelas e views juntas.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RelationKind {
    #[default]
    Table,
    View,
    MaterializedView,
    Foreign,
}

/// Uma mudança de estrutura pedida pela árvore. `schema` é `null` nos bancos
/// sem schema (SQLite). Cada uma vira um ou mais statements, aplicados numa
/// transação — e mostrados antes, no diálogo.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum StructureChange {
    CreateTable {
        schema: Option<String>,
        table: String,
        columns: Vec<ColumnDefinition>,
    },
    RenameTable {
        schema: Option<String>,
        table: String,
        new_name: String,
    },
    DropTable {
        schema: Option<String>,
        table: String,
        #[serde(default)]
        kind: RelationKind,
        /// Postgres: leva junto o que depende dela (views, FKs)
        #[serde(default)]
        cascade: bool,
    },
    AddColumn {
        schema: Option<String>,
        table: String,
        column: ColumnDefinition,
    },
    /// `original` é como a coluna estava quando o diálogo abriu: só o que
    /// mudou vira statement
    AlterColumn {
        schema: Option<String>,
        table: String,
        original: ColumnDefinition,
        updated: ColumnDefinition,
    },
    DropColumn {
        schema: Option<String>,
        table: String,
        column: String,
        #[serde(default)]
        cascade: bool,
    },
}

impl StructureChange {
    pub fn schema(&self) -> Option<&str> {
        match self {
            Self::CreateTable { schema, .. }
            | Self::RenameTable { schema, .. }
            | Self::DropTable { schema, .. }
            | Self::AddColumn { schema, .. }
            | Self::AlterColumn { schema, .. }
            | Self::DropColumn { schema, .. } => schema.as_deref(),
        }
    }
}
