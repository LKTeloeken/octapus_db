//! Objetos de um schema fora das relações soltas: views, views materializadas,
//! índices, funções, sequences, triggers e chaves estrangeiras — as pastas da
//! árvore.
//!
//! Views e views materializadas já estão no catálogo (são relações). O resto
//! vem da fonte sob demanda, um schema (ou uma tabela) por vez, e fica aqui só
//! **em memória**: não vai para o disco (o arquivo do catálogo não muda) e some
//! a cada sincronização ou refresh. O índice do Mongo é por coleção (o banco
//! não lista os índices de todas), então a chave do cache leva a relação
//! quando veio de uma só.
//!
//! O assistente (parte 2) pede pelas mesmas funções: índices de uma tabela,
//! funções de um schema e a definição de um objeto.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::base::Catalog;
use super::model::{Page, RelKind};
use super::query::{paginate, Needle};

/// Tipo de objeto de uma pasta.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ObjectKind {
    View,
    MaterializedView,
    Index,
    Function,
    Sequence,
    Trigger,
    ForeignKey,
}

impl ObjectKind {
    /// A relação do catálogo que este tipo lista (views); `None` = vem da fonte.
    fn relation(self) -> Option<RelKind> {
        match self {
            Self::View => Some(RelKind::View),
            Self::MaterializedView => Some(RelKind::MaterializedView),
            Self::Index | Self::Function | Self::Sequence | Self::Trigger | Self::ForeignKey => None,
        }
    }

    /// Objeto que pertence a uma tabela (a pasta dentro dela filtra por ela)
    fn per_table(self) -> bool {
        matches!(self, Self::Index | Self::Trigger | Self::ForeignKey)
    }
}

/// Função, procedure, agregada ou de janela (`prokind` do Postgres).
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FunctionKind {
    Function,
    Procedure,
    Aggregate,
    Window,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexObject {
    pub name: String,
    /// A relação indexada
    pub table: String,
    /// Método (`btree`, `gin`…); no Mongo, o tipo da chave (`regular`, `text`…)
    pub method: String,
    pub unique: bool,
    pub primary: bool,
    /// Colunas (ou expressões) na ordem do índice
    pub columns: Vec<String>,
    /// Índice parcial: a condição (`status <> 'void'`), completa — o
    /// assistente troca os literais antes de mandar ao modelo
    pub predicate: Option<String>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionObject {
    pub name: String,
    /// Tipos dos argumentos, que distinguem as sobrecargas
    /// (`pg_get_function_identity_arguments`): a identidade para pedir a definição
    pub signature: String,
    /// Argumentos como se declaram (com nomes e defaults), para mostrar
    pub arguments: String,
    /// Tipo de retorno; `None` numa procedure
    pub returns: Option<String>,
    pub function_kind: FunctionKind,
    pub language: String,
    /// `COMMENT ON FUNCTION`, quando há
    pub comment: Option<String>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SequenceObject {
    pub name: String,
    /// `bigint`, `integer`…
    pub data_type: String,
    /// A coluna dona (`serial`/`identity`), como `tabela.coluna`
    pub owned_by: Option<String>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerObject {
    pub name: String,
    pub table: String,
    /// `BEFORE`, `AFTER` ou `INSTEAD OF`
    pub timing: String,
    /// `INSERT`, `UPDATE`, `DELETE`, `TRUNCATE`
    pub events: Vec<String>,
    /// A função chamada (Postgres); `None` no SQLite (o corpo vai inline)
    pub function: Option<String>,
    /// Desligado (`ALTER TABLE … DISABLE TRIGGER`)
    pub enabled: bool,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignKeyObject {
    /// O nome da constraint (no SQLite, que não tem, `fk_<tabela>_<n>`)
    pub name: String,
    pub table: String,
    pub columns: Vec<String>,
    pub ref_schema: String,
    pub ref_table: String,
    /// Vazio no SQLite quando a referência é a chave primária implícita
    pub ref_columns: Vec<String>,
    /// `CASCADE`, `SET NULL`, `RESTRICT`, `NO ACTION`, `SET DEFAULT`
    pub on_delete: String,
    pub on_update: String,
}

/// Um item de pasta.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CatalogObject {
    View { name: String },
    MaterializedView { name: String },
    Index(IndexObject),
    Function(FunctionObject),
    Sequence(SequenceObject),
    Trigger(TriggerObject),
    ForeignKey(ForeignKeyObject),
}

impl CatalogObject {
    pub fn name(&self) -> &str {
        match self {
            Self::View { name } | Self::MaterializedView { name } => name,
            Self::Index(index) => &index.name,
            Self::Function(function) => &function.name,
            Self::Sequence(sequence) => &sequence.name,
            Self::Trigger(trigger) => &trigger.name,
            Self::ForeignKey(key) => &key.name,
        }
    }

    /// A tabela do objeto que pertence a uma (índice, trigger, chave estrangeira)
    fn table(&self) -> Option<&str> {
        match self {
            Self::Index(index) => Some(&index.table),
            Self::Trigger(trigger) => Some(&trigger.table),
            Self::ForeignKey(key) => Some(&key.table),
            _ => None,
        }
    }

    /// O filtro da pasta: o nome ou a tabela do objeto (e, na chave
    /// estrangeira, também a tabela referenciada).
    fn matches(&self, needle: &Needle) -> bool {
        needle.matches(self.name())
            || self.table().is_some_and(|table| needle.matches(table))
            || matches!(self, Self::ForeignKey(key) if needle.matches(&key.ref_table))
    }
}

/// Um objeto de que se quer a definição. `table` só no índice do Mongo (o nome
/// é por coleção); `signature` na função (distingue as sobrecargas).
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "CatalogObjectRef"))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectRef {
    pub kind: ObjectKind,
    pub schema: String,
    pub name: String,
    #[serde(default)]
    #[cfg_attr(test, ts(optional = nullable))]
    pub table: Option<String>,
    #[serde(default)]
    #[cfg_attr(test, ts(optional = nullable))]
    pub signature: Option<String>,
}

/// Uma pasta de um schema e quantos itens tem. `count: None` = ainda não se
/// sabe (índices e funções só vêm ao abrir a pasta); `Some(0)` = vazia, a
/// árvore esconde.
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "CatalogFolder"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderSummary {
    pub kind: ObjectKind,
    pub count: Option<usize>,
}

/// `(tipo, schema, relação)`: a relação só quando a fonte leu uma só.
type ObjectKey = (ObjectKind, Box<str>, Option<Box<str>>);

/// Índices e funções lidos, por schema. Só em memória.
#[derive(Debug, Default)]
pub(super) struct ObjectCache {
    lists: HashMap<ObjectKey, Arc<[CatalogObject]>>,
}

impl ObjectCache {
    fn get(&self, kind: ObjectKind, schema: &str, table: Option<&str>) -> Option<Arc<[CatalogObject]>> {
        let key = (kind, Box::from(schema), table.map(Box::from));
        self.lists.get(&key).cloned()
    }

    pub fn approx_heap_bytes(&self) -> usize {
        self.lists
            .iter()
            .map(|((_, schema, table), list)| {
                64 + schema.len()
                    + table.as_ref().map_or(0, |table| table.len())
                    + list.len() * (size_of::<CatalogObject>() + 48)
            })
            .sum()
    }
}

impl Catalog {
    /// Views ou views materializadas de um schema carregado, filtradas e
    /// paginadas, direto das relações. `None` = schema desconhecido; ainda não
    /// carregado devolve uma página vazia.
    fn relation_objects(
        &self,
        schema: &str,
        kind: ObjectKind,
        rel: RelKind,
        needle: Option<&Needle>,
        offset: usize,
        limit: usize,
    ) -> Option<Page<CatalogObject>> {
        self.schemas.get(schema)?;
        let Some(shape) = self.shape_of(schema) else {
            return Some(paginate(std::iter::empty::<CatalogObject>(), offset, limit, |item| {
                item
            }));
        };
        let to_object = |name: &str| match kind {
            ObjectKind::MaterializedView => CatalogObject::MaterializedView { name: name.to_string() },
            _ => CatalogObject::View { name: name.to_string() },
        };
        Some(paginate(
            shape
                .tables
                .iter()
                .filter(|table| table.kind == rel && table.parent.is_none())
                .map(|table| self.names.resolve(table.name))
                .filter(|name| needle.is_none_or(|needle| needle.matches(name))),
            offset,
            limit,
            to_object,
        ))
    }

    /// Uma janela de uma pasta. Views vêm das relações; índices e funções, do
    /// que a fonte já trouxe — `None` quando ainda não trouxe (ou, nas views,
    /// quando o schema não existe). Com `table`, só os índices dessa relação:
    /// a lista do schema inteiro, se já veio, serve filtrada.
    pub fn objects(
        &self,
        kind: ObjectKind,
        schema: &str,
        table: Option<&str>,
        filter: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> Option<Page<CatalogObject>> {
        let needle = filter.and_then(Needle::new);
        if let Some(rel) = kind.relation() {
            return self.relation_objects(schema, kind, rel, needle.as_ref(), offset, limit);
        }

        let (list, by_table) = match self.objects.get(kind, schema, table) {
            Some(list) => (list, None),
            None => (self.objects.get(kind, schema, None)?, table.filter(|_| kind.per_table())),
        };
        Some(paginate(
            list.iter()
                .filter(|object| by_table.is_none_or(|table| object.table() == Some(table)))
                .filter(|object| needle.as_ref().is_none_or(|needle| object.matches(needle))),
            offset,
            limit,
            Clone::clone,
        ))
    }

    /// Guarda o que a fonte leu (índices ou funções; views não passam por
    /// aqui). Ordena por nome (e assinatura, nas sobrecargas) em ordem de bytes.
    pub fn set_objects(
        &mut self,
        kind: ObjectKind,
        schema: &str,
        table: Option<&str>,
        mut objects: Vec<CatalogObject>,
    ) {
        objects.sort_by(|a, b| {
            a.name().cmp(b.name()).then_with(|| match (a, b) {
                (CatalogObject::Function(a), CatalogObject::Function(b)) => a.signature.cmp(&b.signature),
                _ => std::cmp::Ordering::Equal,
            })
        });
        let key = (kind, Box::from(schema), table.map(Box::from));
        self.objects.lists.insert(key, objects.into());
    }

    /// Esquece índices e funções de um schema (refresh, mudança de estrutura)
    /// ou, sem schema, de todos (sincronização).
    pub fn forget_objects(&mut self, schema: Option<&str>) {
        match schema {
            Some(schema) => self.objects.lists.retain(|(_, cached, _), _| cached.as_ref() != schema),
            None => self.objects.lists.clear(),
        }
    }

    /// As pastas de um schema e quantos itens cada uma tem, sem ir ao banco:
    /// views pelas relações, índices e funções pelo que já veio (`None` se
    /// ainda não veio). `None` = schema desconhecido.
    pub fn folders(&self, schema: &str, kinds: &[ObjectKind]) -> Option<Vec<FolderSummary>> {
        self.schemas.get(schema)?;
        let loaded = self.shape_of(schema).is_some();
        Some(
            kinds
                .iter()
                .map(|&kind| FolderSummary {
                    kind,
                    count: match kind.relation() {
                        Some(rel) => loaded.then(|| {
                            self.shape_of(schema).map_or(0, |shape| {
                                shape
                                    .tables
                                    .iter()
                                    .filter(|table| table.kind == rel && table.parent.is_none())
                                    .count()
                            })
                        }),
                        None => self.objects.get(kind, schema, None).map(|list| list.len()),
                    },
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::TableRecord;

    fn index(name: &str, table: &str) -> CatalogObject {
        CatalogObject::Index(IndexObject {
            name: name.into(),
            table: table.into(),
            method: "btree".into(),
            unique: false,
            primary: false,
            columns: vec!["id".into()],
            predicate: None,
        })
    }

    fn function(name: &str, signature: &str) -> CatalogObject {
        CatalogObject::Function(FunctionObject {
            name: name.into(),
            signature: signature.into(),
            arguments: signature.into(),
            returns: Some("integer".into()),
            function_kind: FunctionKind::Function,
            language: "sql".into(),
            comment: None,
        })
    }

    fn names(page: &Page<CatalogObject>) -> Vec<&str> {
        page.items.iter().map(CatalogObject::name).collect()
    }

    fn catalog() -> Catalog {
        let mut catalog = Catalog::new();
        catalog.set_tables(
            "app",
            vec![
                TableRecord::new("orders", RelKind::Table),
                TableRecord::new("active_orders", RelKind::View),
                TableRecord::new("totals", RelKind::MaterializedView),
                TableRecord::new("big_orders", RelKind::View),
            ],
            None,
        );
        catalog.insert_unloaded("later");
        catalog
    }

    #[test]
    fn views_come_from_the_relations() {
        let catalog = catalog();
        let views = catalog.objects(ObjectKind::View, "app", None, None, 0, 10).unwrap();
        assert_eq!(names(&views), ["active_orders", "big_orders"]);
        assert!(matches!(views.items[0], CatalogObject::View { .. }));

        let filtered = catalog
            .objects(ObjectKind::View, "app", None, Some("BIG"), 0, 10)
            .unwrap();
        assert_eq!((filtered.total, names(&filtered)), (1, vec!["big_orders"]));

        let materialized = catalog
            .objects(ObjectKind::MaterializedView, "app", None, None, 0, 10)
            .unwrap();
        assert!(matches!(
            materialized.items[..],
            [CatalogObject::MaterializedView { .. }]
        ));

        // Schema ainda não carregado: vazio; desconhecido: None
        assert_eq!(
            catalog
                .objects(ObjectKind::View, "later", None, None, 0, 10)
                .unwrap()
                .total,
            0
        );
        assert!(catalog.objects(ObjectKind::View, "nope", None, None, 0, 10).is_none());
    }

    #[test]
    fn cached_objects_page_filter_and_sort() {
        let mut catalog = catalog();
        assert!(catalog.objects(ObjectKind::Index, "app", None, None, 0, 10).is_none());

        let indexes = (0..120).map(|i| index(&format!("idx_{i:03}"), if i % 2 == 0 { "orders" } else { "users" }));
        catalog.set_objects(ObjectKind::Index, "app", None, indexes.rev().collect());

        let page = catalog.objects(ObjectKind::Index, "app", None, None, 100, 50).unwrap();
        assert_eq!((page.total, page.offset, page.items.len()), (120, 100, 20));
        assert_eq!(page.items[0].name(), "idx_100");

        // O filtro casa o nome ou a tabela
        let by_table = catalog
            .objects(ObjectKind::Index, "app", None, Some("user"), 0, 500)
            .unwrap();
        assert_eq!(by_table.total, 60);
        let by_name = catalog
            .objects(ObjectKind::Index, "app", None, Some("idx_01"), 0, 500)
            .unwrap();
        assert_eq!(by_name.total, 10);

        // Os índices de uma tabela saem da lista do schema
        let of_orders = catalog
            .objects(ObjectKind::Index, "app", Some("orders"), None, 0, 500)
            .unwrap();
        assert_eq!(of_orders.total, 60);
        assert!(of_orders
            .items
            .iter()
            .all(|item| matches!(item, CatalogObject::Index(i) if i.table == "orders")));

        // Sobrecargas ficam juntas, pela assinatura
        catalog.set_objects(
            ObjectKind::Function,
            "app",
            None,
            vec![
                function("total", "integer"),
                function("add", "text"),
                function("add", "integer, integer"),
            ],
        );
        let functions = catalog.objects(ObjectKind::Function, "app", None, None, 0, 10).unwrap();
        let signatures: Vec<&str> = functions
            .items
            .iter()
            .map(|item| match item {
                CatalogObject::Function(f) => f.signature.as_str(),
                _ => "",
            })
            .collect();
        assert_eq!(signatures, ["integer, integer", "text", "integer"]);
    }

    #[test]
    fn per_relation_lists_stay_apart() {
        let mut catalog = catalog();
        catalog.set_objects(ObjectKind::Index, "", Some("orders"), vec![index("_id_", "orders")]);
        assert_eq!(
            catalog
                .objects(ObjectKind::Index, "", Some("orders"), None, 0, 10)
                .unwrap()
                .total,
            1
        );
        assert!(catalog
            .objects(ObjectKind::Index, "", Some("users"), None, 0, 10)
            .is_none());
        // Uma coleção lida não é a lista do schema
        assert!(catalog.objects(ObjectKind::Index, "", None, None, 0, 10).is_none());
    }

    #[test]
    fn forget_drops_one_schema_or_all() {
        let mut catalog = catalog();
        catalog.set_objects(ObjectKind::Index, "app", None, vec![index("a", "orders")]);
        catalog.set_objects(ObjectKind::Function, "app", None, vec![function("f", "")]);
        catalog.set_objects(ObjectKind::Index, "other", None, vec![index("b", "orders")]);
        assert!(catalog.stats().approx_heap_bytes > catalog_without_objects_bytes());

        catalog.forget_objects(Some("app"));
        assert!(catalog.objects(ObjectKind::Index, "app", None, None, 0, 10).is_none());
        assert!(catalog
            .objects(ObjectKind::Function, "app", None, None, 0, 10)
            .is_none());
        assert!(catalog.objects(ObjectKind::Index, "other", None, None, 0, 10).is_some());

        catalog.forget_objects(None);
        assert!(catalog.objects(ObjectKind::Index, "other", None, None, 0, 10).is_none());

        // Sair o schema leva junto o que se leu dele
        catalog.set_objects(ObjectKind::Index, "app", None, vec![index("a", "orders")]);
        catalog.remove_schema("app");
        assert!(catalog.objects(ObjectKind::Index, "app", None, None, 0, 10).is_none());
    }

    fn catalog_without_objects_bytes() -> usize {
        catalog().stats().approx_heap_bytes
    }

    #[test]
    fn folders_count_without_the_database() {
        let mut catalog = catalog();
        let kinds = [
            ObjectKind::View,
            ObjectKind::MaterializedView,
            ObjectKind::Index,
            ObjectKind::Function,
        ];
        let counts = |catalog: &Catalog, schema: &str| -> Vec<Option<usize>> {
            catalog
                .folders(schema, &kinds)
                .unwrap()
                .into_iter()
                .map(|folder| folder.count)
                .collect()
        };
        assert_eq!(counts(&catalog, "app"), [Some(2), Some(1), None, None]);

        catalog.set_objects(ObjectKind::Function, "app", None, Vec::new());
        assert_eq!(counts(&catalog, "app"), [Some(2), Some(1), None, Some(0)]);

        // Não carregado: nada se sabe ainda
        assert_eq!(counts(&catalog, "later"), [None, None, None, None]);
        assert!(catalog.folders("nope", &kinds).is_none());
    }

    #[test]
    fn serializes_tagged_by_kind() {
        let json = serde_json::to_value(index("idx", "orders")).unwrap();
        assert_eq!(json["kind"], "index");
        assert_eq!(json["table"], "orders");
        let view = serde_json::to_value(CatalogObject::MaterializedView { name: "m".into() }).unwrap();
        assert_eq!(view, serde_json::json!({ "kind": "materializedView", "name": "m" }));
        let target: ObjectRef =
            serde_json::from_value(serde_json::json!({ "kind": "function", "schema": "app", "name": "add" })).unwrap();
        assert_eq!((target.table, target.signature), (None, None));
    }

    #[test]
    fn table_folders_filter_triggers_and_foreign_keys_by_table() {
        let mut catalog = catalog();
        let trigger = |name: &str, table: &str| {
            CatalogObject::Trigger(TriggerObject {
                name: name.into(),
                table: table.into(),
                timing: "AFTER".into(),
                events: vec!["INSERT".into()],
                function: None,
                enabled: true,
            })
        };
        catalog.set_objects(
            ObjectKind::Trigger,
            "app",
            None,
            vec![trigger("t2", "orders"), trigger("t1", "users")],
        );
        let of_orders = catalog.objects(ObjectKind::Trigger, "app", Some("orders"), None, 0, 10).unwrap();
        assert_eq!(names(&of_orders), ["t2"]);
        // O filtro também acha pela tabela, e a de uma FK pela referenciada
        let by_table = catalog.objects(ObjectKind::Trigger, "app", None, Some("user"), 0, 10).unwrap();
        assert_eq!(names(&by_table), ["t1"]);
        let key = CatalogObject::ForeignKey(ForeignKeyObject {
            name: "orders_user_fkey".into(),
            table: "orders".into(),
            columns: vec!["user_id".into()],
            ref_schema: "app".into(),
            ref_table: "customers".into(),
            ref_columns: vec!["id".into()],
            on_delete: "CASCADE".into(),
            on_update: "NO ACTION".into(),
        });
        catalog.set_objects(ObjectKind::ForeignKey, "app", Some("orders"), vec![key]);
        let found = catalog
            .objects(ObjectKind::ForeignKey, "app", Some("orders"), Some("custom"), 0, 10)
            .unwrap();
        assert_eq!(names(&found), ["orders_user_fkey"]);
        // As sequences são do schema: o `table` não filtra
        assert!(catalog.objects(ObjectKind::Sequence, "app", None, None, 0, 10).is_none());
    }
}
