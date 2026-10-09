use serde::Serialize;

// ─────────────────────────────────────────────────────────────────────────────
// Entrada — o que o introspector entrega ao catálogo
// ─────────────────────────────────────────────────────────────────────────────

/// Tipo de relação. `Partitioned` é o pai de uma tabela particionada (`relkind
/// 'p'` no Postgres), que a árvore antiga nem listava.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RelKind {
    Table,
    View,
    MaterializedView,
    Foreign,
    Partitioned,
}

impl RelKind {
    pub(super) const ALL: [RelKind; 5] = [
        RelKind::Table,
        RelKind::View,
        RelKind::MaterializedView,
        RelKind::Foreign,
        RelKind::Partitioned,
    ];

    pub(super) fn code(self) -> u8 {
        self as u8
    }

    pub(super) fn from_code(code: u8) -> Option<Self> {
        Self::ALL.get(code as usize).copied()
    }
}

/// Uma relação de um schema, como chega do banco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRecord {
    pub name: String,
    pub kind: RelKind,
    /// Tabela-pai, quando esta relação é uma partição
    pub partition_of: Option<String>,
}

/// Atalhos para montar entradas nos testes (o introspector monta a struct).
#[cfg(test)]
impl TableRecord {
    pub fn new(name: impl Into<String>, kind: RelKind) -> Self {
        Self {
            name: name.into(),
            kind,
            partition_of: None,
        }
    }

    pub fn partition(name: impl Into<String>, parent: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: RelKind::Table,
            partition_of: Some(parent.into()),
        }
    }
}

/// Um schema na camada 0 (só o nome), com o fingerprint quando o banco tem um
/// — no Postgres, o hash dos `(oid, xmin)` das relações do schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaHeader {
    pub name: String,
    pub fingerprint: Option<i64>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Estado
// ─────────────────────────────────────────────────────────────────────────────

/// Situação das tabelas de um schema no catálogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LoadState {
    /// Só o nome é conhecido (camada 0)
    Unloaded,
    /// Tabelas carregadas e de acordo com o último fingerprint visto
    Loaded,
    /// Tabelas carregadas, mas o banco mudou desde então — continuam visíveis
    /// até a recarga
    Stale,
}

impl LoadState {
    pub(super) fn code(self) -> u8 {
        self as u8
    }

    pub(super) fn from_code(code: u8) -> Option<Self> {
        [LoadState::Unloaded, LoadState::Loaded, LoadState::Stale]
            .get(code as usize)
            .copied()
    }
}

/// O que mudou ao aplicar uma nova lista de schemas ([`super::Catalog::set_schemas`]).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SchemaDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    /// Schemas carregados cujo fingerprint mudou (viraram [`LoadState::Stale`])
    pub changed: Vec<String>,
}

#[cfg(test)]
impl SchemaDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Saída — fatias que o front pede
// ─────────────────────────────────────────────────────────────────────────────

/// Nó cujos filhos se quer listar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogPath {
    /// Os schemas do database
    Schemas,
    /// As relações de um schema (partições ficam dentro do pai)
    Schema(String),
    /// As partições de uma tabela particionada
    Partitions { schema: String, table: String },
    /// Os schemas de um grupo de formato ([`ShapeGroup::key`])
    ShapeSchemas(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeKind {
    Schema,
    Table,
    View,
    MaterializedView,
    Foreign,
    Partitioned,
}

impl From<RelKind> for NodeKind {
    fn from(kind: RelKind) -> Self {
        match kind {
            RelKind::Table => NodeKind::Table,
            RelKind::View => NodeKind::View,
            RelKind::MaterializedView => NodeKind::MaterializedView,
            RelKind::Foreign => NodeKind::Foreign,
            RelKind::Partitioned => NodeKind::Partitioned,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogNode {
    pub name: String,
    pub kind: NodeKind,
    /// Filhos diretos (tabelas de um schema, partições de um pai); `None`
    /// quando ainda não se sabe (schema não carregado)
    pub child_count: Option<u32>,
    /// Só para schemas
    pub state: Option<LoadState>,
    /// Só para schemas: quanto o formato deste schema difere do dominante
    /// (um tenant com migração pendente). `None` = igual, ou sem molde
    pub drift: Option<SchemaDrift>,
}

/// Diferença de um schema para o formato dominante, em relações.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaDrift {
    /// Relações do dominante que faltam aqui
    pub missing: u32,
    /// Relações que este schema tem a mais
    pub extra: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ShapeRole {
    /// O formato com mais schemas: o "molde" dos tenants
    Dominant,
    /// Parecido com o dominante (tenant atrasado ou adiantado)
    Variant,
    /// O resto: schemas sem relação com o molde (`public`, auditoria…) e os
    /// ainda não carregados
    Other,
}

/// Um grupo da árvore agrupada por formato.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeGroup {
    /// Estável entre sincronizações enquanto o conteúdo do formato for o
    /// mesmo (hash das relações); `"other"` para o resto
    pub key: String,
    pub role: ShapeRole,
    /// Relações do formato (`None` no grupo "outros")
    pub tables: Option<usize>,
    /// Diferença para o dominante (só nas variações)
    pub missing: Vec<String>,
    pub extra: Vec<String>,
    pub schemas: usize,
}

/// Uma janela de uma lista; `total` já considera o filtro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub total: usize,
    pub offset: usize,
    pub items: Vec<T>,
}

/// Em quantos schemas um resultado aparece, com alguns exemplos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaGroup {
    pub total: usize,
    pub sample: Vec<String>,
}

/// Resultado da busca. Tabelas iguais em vários schemas viram **um** resultado
/// ("orders em 5.000 schemas", em `schemas`); numa busca `schema.tabela` cada
/// par é um resultado, com o schema em `schema`. Um resultado que é o próprio
/// schema não tem nenhum dos dois.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub name: String,
    pub kind: NodeKind,
    pub score: u32,
    pub schema: Option<String>,
    pub schemas: Option<SchemaGroup>,
}

/// A que relação um nome sem schema se refere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum Resolution {
    Found {
        schema: String,
        table: String,
        kind: NodeKind,
    },
    /// Fora do `search_path` e em mais de um schema
    Ambiguous {
        table: String,
        schemas: SchemaGroup,
    },
    NotFound,
}

/// Formato dominante (o "molde" dos tenants) e os formatos que divergem dele.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftReport {
    pub dominant: Option<ShapeSummary>,
    pub divergent: Vec<DriftGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeSummary {
    pub tables: usize,
    pub schemas: SchemaGroup,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftGroup {
    pub schemas: SchemaGroup,
    /// Relações do formato dominante que faltam neste grupo
    pub missing: Vec<String>,
    /// Relações que este grupo tem e o dominante não
    pub extra: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogStats {
    pub schemas: usize,
    pub loaded: usize,
    pub stale: usize,
    pub unloaded: usize,
    pub shapes: usize,
    pub distinct_names: usize,
    /// Relações somadas em todos os schemas carregados
    pub relations: usize,
    pub approx_heap_bytes: usize,
}
