use serde::{Deserialize, Serialize};

use crate::catalog::ObjectKind;

/// Describes what a given adapter supports, so the frontend can decide what to
/// render (schema tree levels, editable grids, free-form editor, etc.) without
/// branching on `dbType` directly.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
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

    /// Estrutura pelo catálogo de metadados (`catalog_*`) em vez do
    /// `list_schemas_with_tables` — o mesmo que `adapters::has_catalog`.
    pub has_catalog: bool,

    /// Criar/renomear/remover tabela e coluna (`structure_change_sql`,
    /// `apply_structure_change`).
    pub supports_structure_changes: bool,

    /// `DROP ... CASCADE` ao remover uma tabela.
    pub supports_drop_cascade: bool,

    /// Comparar dois schemas do database (`catalog_compare_schemas`).
    pub supports_schema_compare: bool,

    /// Backup e restauração pelas ferramentas nativas (`pg_dump`/`pg_restore`).
    pub supports_backup: bool,

    /// Gravar as alterações pendentes da grade numa chamada só
    /// (`save_row_changes`), que desfaz tudo se uma etapa falhar. No Mongo isso
    /// depende da topologia: replica set/mongos usa transação; num standalone
    /// as etapas valem uma a uma (com aviso no log), como nas chamadas separadas.
    pub supports_atomic_save: bool,

    /// O assistente de consulta atende este banco (a tela e os comandos
    /// `assistant_*` de turno). Todos os com adapter: Postgres, SQLite, Mongo
    /// e Redis (este pelo plano de passos, sem catálogo).
    pub supports_assistant: bool,

    /// O servidor pode mandar ao assistente os valores de exemplo dos campos
    /// categóricos (`Server::assistant_sample_values`, desligado por padrão).
    /// Só o Mongo: sem eles o modelo não tem como saber os valores de um
    /// `status` num documento sem esquema.
    pub assistant_sample_values: bool,

    /// Em que o assistente escreve a consulta deste banco — o `language` da
    /// saída do modelo no contrato da API (`output.schema.json`).
    pub query_language: QueryLanguage,

    /// Pastas de objetos que a árvore mostra num schema aberto, nesta ordem,
    /// depois da pasta das tabelas (`catalog_folders`/`catalog_objects`).
    /// Vazio = nenhuma.
    #[cfg_attr(test, ts(as = "Vec<ObjectKind>"))]
    pub structure_folders: &'static [ObjectKind],

    /// Pastas de objetos dentro de cada tabela, depois da pasta das colunas
    /// (a view materializada só tem a de índices): vêm por relação
    /// (`catalog_objects` com `table`), nunca do schema inteiro.
    #[cfg_attr(test, ts(as = "Vec<ObjectKind>"))]
    pub relation_folders: &'static [ObjectKind],
}

/// Linguagem da consulta gerada pelo assistente.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryLanguage {
    /// SQL (Postgres e SQLite seguem o mesmo caminho)
    Sql,
    /// Operação declarativa do Mongo (`mongo-operation.schema.json`)
    Mongo,
    /// Plano de passos do Redis (`redis-plan.schema.json`): o modelo não vê
    /// chaves nem valores (RN-09)
    RedisPlan,
}

/// As capabilities de um tipo de banco — a tabela do `get_db_capabilities`.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbTypeCapabilities {
    pub db_type: super::DatabaseType,
    pub capabilities: AdapterCapabilities,
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
            has_catalog: true,
            supports_structure_changes: true,
            supports_drop_cascade: true,
            supports_schema_compare: true,
            supports_backup: true,
            supports_atomic_save: true,
            supports_assistant: true,
            assistant_sample_values: false,
            query_language: QueryLanguage::Sql,
            structure_folders: &[
                ObjectKind::View,
                ObjectKind::MaterializedView,
                ObjectKind::Function,
                ObjectKind::Sequence,
            ],
            relation_folders: &[ObjectKind::Index, ObjectKind::Trigger, ObjectKind::ForeignKey],
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
            has_catalog: true,
            supports_structure_changes: false,
            supports_drop_cascade: false,
            supports_schema_compare: false,
            supports_backup: false,
            supports_atomic_save: true,
            supports_assistant: true,
            assistant_sample_values: true,
            query_language: QueryLanguage::Mongo,
            structure_folders: &[ObjectKind::View],
            relation_folders: &[ObjectKind::Index],
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
            has_catalog: false,
            supports_structure_changes: false,
            supports_drop_cascade: false,
            supports_schema_compare: false,
            supports_backup: false,
            supports_atomic_save: false,
            supports_assistant: true,
            assistant_sample_values: false,
            query_language: QueryLanguage::RedisPlan,
            structure_folders: &[],
            relation_folders: &[],
        }
    }

    pub const fn sqlite() -> Self {
        Self {
            // Um arquivo SQLite é um banco só, sem nível de schema
            has_schemas: false,
            has_primary_keys: true,
            supports_sql: true,
            supports_transactions: true,
            supports_indexes: true,
            browsable: true,
            has_catalog: true,
            supports_structure_changes: true,
            supports_drop_cascade: false,
            supports_schema_compare: false,
            supports_backup: false,
            supports_atomic_save: true,
            supports_assistant: true,
            assistant_sample_values: false,
            query_language: QueryLanguage::Sql,
            structure_folders: &[ObjectKind::View],
            relation_folders: &[ObjectKind::Index, ObjectKind::Trigger, ObjectKind::ForeignKey],
        }
    }

    /// Capabilities by database type, without requiring a live connection.
    /// `None` for types without an adapter yet.
    pub fn for_db_type(db_type: super::DatabaseType) -> Option<Self> {
        match db_type {
            super::DatabaseType::Postgres => Some(Self::postgres()),
            super::DatabaseType::Mongodb => Some(Self::mongodb()),
            super::DatabaseType::Redis => Some(Self::redis()),
            super::DatabaseType::Sqlite => Some(Self::sqlite()),
            super::DatabaseType::Mysql => None,
        }
    }

    /// A tabela de todos os tipos com adapter, para o front decidir por
    /// servidor sem uma consulta por servidor.
    pub fn all() -> Vec<DbTypeCapabilities> {
        use super::DatabaseType::*;
        [Postgres, Mongodb, Redis, Sqlite]
            .into_iter()
            .filter_map(|db_type| {
                Self::for_db_type(db_type).map(|capabilities| DbTypeCapabilities {
                    db_type,
                    capabilities,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::has_catalog;
    use crate::models::DatabaseType;

    #[test]
    fn catalog_flag_matches_the_adapters() {
        for entry in AdapterCapabilities::all() {
            assert_eq!(
                entry.capabilities.has_catalog,
                has_catalog(entry.db_type),
                "{:?}",
                entry.db_type
            );
        }
    }

    #[test]
    fn schema_level_features_need_schemas() {
        for entry in AdapterCapabilities::all() {
            let caps = entry.capabilities;
            if caps.supports_schema_compare {
                assert!(caps.has_schemas && caps.has_catalog, "{:?}", entry.db_type);
            }
            if caps.supports_drop_cascade {
                assert!(caps.supports_structure_changes, "{:?}", entry.db_type);
            }
        }
        assert_eq!(AdapterCapabilities::all().len(), 4);
        assert!(AdapterCapabilities::for_db_type(DatabaseType::Mysql).is_none());
    }

    #[test]
    fn folders_need_the_catalog() {
        for entry in AdapterCapabilities::all() {
            let caps = entry.capabilities;
            if !caps.structure_folders.is_empty() || !caps.relation_folders.is_empty() {
                assert!(caps.has_catalog, "{:?}", entry.db_type);
            }
            // Funções, sequences e views materializadas só onde há schema
            for kind in [ObjectKind::Function, ObjectKind::Sequence, ObjectKind::MaterializedView] {
                if caps.structure_folders.contains(&kind) {
                    assert!(caps.has_schemas, "{:?}", entry.db_type);
                }
            }
        }
        let json = serde_json::to_value(AdapterCapabilities::sqlite()).unwrap();
        assert_eq!(json["structureFolders"], serde_json::json!(["view"]));
        assert_eq!(json["relationFolders"], serde_json::json!(["index", "trigger", "foreignKey"]));
    }

    #[test]
    fn sql_language_matches_the_sql_editor() {
        for entry in AdapterCapabilities::all() {
            let caps = entry.capabilities;
            assert_eq!(
                caps.query_language == QueryLanguage::Sql,
                caps.supports_sql,
                "{:?}",
                entry.db_type
            );
        }
    }
}
