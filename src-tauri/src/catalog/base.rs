use std::collections::{BTreeMap, HashMap};

use super::interner::{Interner, Sym};
use super::model::{LoadState, SchemaDiff, SchemaHeader, TableRecord};
use super::shape::{Shape, ShapeId, TableIndex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SchemaEntry {
    pub shape: Option<ShapeId>,
    /// Fingerprint dos dados **carregados** — não o último visto, para que uma
    /// recarga que falhe continue aparecendo como pendente no próximo diff
    pub fingerprint: Option<i64>,
    pub state: LoadState,
}

impl SchemaEntry {
    fn unloaded() -> Self {
        Self {
            shape: None,
            fingerprint: None,
            state: LoadState::Unloaded,
        }
    }
}

/// Catálogo de metadados de **um** database: schemas, o formato (conjunto de
/// relações) de cada um e os índices para listar, buscar e resolver nomes sem
/// voltar ao banco.
///
/// Não conhece banco nem rede: o introspector alimenta ([`Catalog::set_schemas`],
/// [`Catalog::set_tables`], [`Catalog::share_shape`]) e o front consulta
/// (`children`, `search`, `resolve`, `drift`). Quem compartilha entre threads
/// embrulha num lock.
#[derive(Debug, Default)]
pub struct Catalog {
    pub(super) names: Interner,
    /// Ordenado por nome em ordem de bytes, como o `ORDER BY nspname`
    pub(super) schemas: BTreeMap<Box<str>, SchemaEntry>,
    pub(super) shapes: Vec<Shape>,
    /// Quantos schemas usam cada formato; zero = slot livre
    pub(super) members: Vec<u32>,
    shape_lookup: HashMap<u64, Vec<ShapeId>>,
    free_shapes: Vec<ShapeId>,
    /// Nome de relação → formatos que a têm (a busca e a resolução partem daqui)
    pub(super) by_table: TableIndex,
    fetched_at: Option<i64>,
    /// A lista completa de schemas (camada 0) já chegou alguma vez — cargas
    /// prioritárias inserem schemas soltos, então "tem schema" não basta
    listed: bool,
}

impl Catalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Quando o catálogo foi revalidado com o banco pela última vez (ms epoch).
    pub fn fetched_at(&self) -> Option<i64> {
        self.fetched_at
    }

    pub fn set_fetched_at(&mut self, at: i64) {
        self.fetched_at = Some(at);
    }

    /// Camada 0: a lista completa de schemas. Schemas novos entram como
    /// [`LoadState::Unloaded`], os que sumiram saem, e os carregados cujo
    /// fingerprint mudou viram [`LoadState::Stale`] (as tabelas antigas continuam
    /// visíveis até a recarga).
    pub fn set_schemas(&mut self, headers: impl IntoIterator<Item = SchemaHeader>) -> SchemaDiff {
        let incoming: HashMap<String, Option<i64>> = headers
            .into_iter()
            .map(|header| (header.name, header.fingerprint))
            .collect();

        let mut diff = SchemaDiff::default();
        self.listed = true;

        let gone: Vec<Box<str>> = self
            .schemas
            .keys()
            .filter(|name| !incoming.contains_key(name.as_ref()))
            .cloned()
            .collect();
        for name in gone {
            self.remove_schema(&name);
            diff.removed.push(name.into());
        }

        for (name, fingerprint) in incoming {
            match self.schemas.get_mut(name.as_str()) {
                Some(entry) => {
                    let changed = entry.state == LoadState::Loaded
                        && fingerprint.is_some()
                        && entry.fingerprint != fingerprint;
                    if changed {
                        entry.state = LoadState::Stale;
                        diff.changed.push(name);
                    }
                }
                None => {
                    self.schemas.insert(name.as_str().into(), SchemaEntry::unloaded());
                    diff.added.push(name);
                }
            }
        }

        diff.added.sort();
        diff.removed.sort();
        diff.changed.sort();
        diff
    }

    /// As relações de um schema (cria o schema se ainda não existia).
    pub fn set_tables(&mut self, schema: &str, tables: Vec<TableRecord>, fingerprint: Option<i64>) {
        let shape = Shape::build(tables, &mut self.names);
        self.assign_built(schema, shape, fingerprint, LoadState::Loaded);
    }

    /// Shape-first: `schema` tem exatamente as relações de `like`, que já está
    /// carregado. Devolve `false` (e não muda nada) se `like` não tem formato.
    pub fn share_shape(&mut self, schema: &str, like: &str, fingerprint: Option<i64>) -> bool {
        let Some(shape) = self.schemas.get(like).and_then(|entry| entry.shape) else {
            return false;
        };
        self.assign(schema, shape, fingerprint, LoadState::Loaded);
        true
    }

    /// Marca um schema carregado para recarga (refresh manual).
    pub fn invalidate(&mut self, schema: &str) -> bool {
        match self.schemas.get_mut(schema) {
            Some(entry) if entry.state == LoadState::Loaded => {
                entry.state = LoadState::Stale;
                true
            }
            _ => false,
        }
    }

    pub fn remove_schema(&mut self, schema: &str) -> bool {
        let Some(entry) = self.schemas.remove(schema) else {
            return false;
        };
        if let Some(shape) = entry.shape {
            self.release(shape);
        }
        true
    }

    /// A lista completa de schemas (camada 0) já chegou alguma vez: um nome
    /// fora dela não existe.
    pub fn knows_schemas(&self) -> bool {
        self.listed
    }

    pub(super) fn set_listed(&mut self, listed: bool) {
        self.listed = listed;
    }

    pub fn schema_state(&self, schema: &str) -> Option<LoadState> {
        self.schemas.get(schema).map(|entry| entry.state)
    }

    /// Schemas cujas relações faltam ou estão velhas.
    #[cfg(test)]
    pub fn pending_schemas(&self) -> Vec<String> {
        self.schemas
            .iter()
            .filter(|(_, entry)| entry.state != LoadState::Loaded)
            .map(|(name, _)| name.to_string())
            .collect()
    }

    pub(super) fn shape_of(&self, schema: &str) -> Option<&Shape> {
        self.schemas
            .get(schema)
            .and_then(|entry| entry.shape)
            .map(|shape| &self.shapes[shape.index()])
    }

    /// Formatos vivos (com ao menos um schema).
    pub(super) fn live_shapes(&self) -> impl Iterator<Item = (ShapeId, &Shape, u32)> {
        self.shapes
            .iter()
            .zip(&self.members)
            .enumerate()
            .filter(|(_, (_, &members))| members > 0)
            .map(|(index, (shape, &members))| (ShapeId::from_index(index), shape, members))
    }

    /// Para a carga do arquivo: entra com um formato já montado e o estado salvo.
    pub(super) fn assign_built(
        &mut self,
        schema: &str,
        shape: Shape,
        fingerprint: Option<i64>,
        state: LoadState,
    ) {
        let shape = self.intern_shape(shape);
        self.assign(schema, shape, fingerprint, state);
    }

    pub(super) fn insert_unloaded(&mut self, schema: &str) {
        self.schemas
            .entry(schema.into())
            .or_insert_with(SchemaEntry::unloaded);
    }

    pub(super) fn intern_name(&mut self, name: &str) -> Sym {
        self.names.intern(name)
    }

    pub(super) fn assign(
        &mut self,
        schema: &str,
        shape: ShapeId,
        fingerprint: Option<i64>,
        state: LoadState,
    ) {
        // Retém o novo antes de soltar o antigo: se forem o mesmo, não libera
        self.members[shape.index()] += 1;

        let entry = self
            .schemas
            .entry(schema.into())
            .or_insert_with(SchemaEntry::unloaded);
        let previous = entry.shape.replace(shape);
        entry.fingerprint = fingerprint;
        entry.state = state;

        if let Some(previous) = previous {
            self.release(previous);
        }
    }

    /// Devolve o formato idêntico já existente ou registra este (sem membros
    /// ainda — quem chama faz o `assign`).
    pub(super) fn intern_shape(&mut self, shape: Shape) -> ShapeId {
        if let Some(candidates) = self.shape_lookup.get(&shape.hash) {
            if let Some(&existing) = candidates
                .iter()
                .find(|id| self.shapes[id.index()].tables == shape.tables)
            {
                return existing;
            }
        }

        let id = match self.free_shapes.pop() {
            Some(id) => id,
            None => {
                self.shapes.push(Shape::default());
                self.members.push(0);
                ShapeId::from_index(self.shapes.len() - 1)
            }
        };

        for table in shape.tables.iter() {
            self.by_table.add(table.name, id);
        }
        self.shape_lookup.entry(shape.hash).or_default().push(id);
        self.shapes[id.index()] = shape;
        id
    }

    fn release(&mut self, id: ShapeId) {
        let members = &mut self.members[id.index()];
        *members -= 1;
        if *members > 0 {
            return;
        }

        let shape = std::mem::take(&mut self.shapes[id.index()]);
        for table in shape.tables.iter() {
            self.by_table.remove(table.name, id);
        }
        if let Some(bucket) = self.shape_lookup.get_mut(&shape.hash) {
            bucket.retain(|&other| other != id);
            if bucket.is_empty() {
                self.shape_lookup.remove(&shape.hash);
            }
        }
        self.free_shapes.push(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::model::RelKind;

    fn header(name: &str, fingerprint: Option<i64>) -> SchemaHeader {
        SchemaHeader {
            name: name.into(),
            fingerprint,
        }
    }

    fn tables(names: &[&str]) -> Vec<TableRecord> {
        names
            .iter()
            .map(|name| TableRecord::new(*name, RelKind::Table))
            .collect()
    }

    #[test]
    fn tenants_with_the_same_tables_share_one_shape() {
        let mut catalog = Catalog::new();
        for tenant in ["t1", "t2", "t3"] {
            catalog.set_tables(tenant, tables(&["orders", "customers"]), Some(1));
        }
        catalog.set_tables("public", tables(&["plans"]), Some(2));

        assert_eq!(catalog.live_shapes().count(), 2);
        assert_eq!(catalog.names.len(), 3);
        let orders = catalog.names.get("orders").unwrap();
        assert_eq!(catalog.by_table.shapes(orders).len(), 1);
    }

    #[test]
    fn shape_first_links_without_rebuilding() {
        let mut catalog = Catalog::new();
        catalog.set_tables("t1", tables(&["orders"]), Some(1));

        assert!(catalog.share_shape("t2", "t1", Some(7)));
        assert!(!catalog.share_shape("t3", "missing", None));
        assert_eq!(catalog.schema_state("t2"), Some(LoadState::Loaded));
        assert_eq!(catalog.schema_state("t3"), None);
        assert_eq!(catalog.members.iter().sum::<u32>(), 2);
    }

    #[test]
    fn set_schemas_diffs_added_removed_and_changed() {
        let mut catalog = Catalog::new();
        catalog.set_schemas([header("a", None), header("b", None), header("c", None)]);
        catalog.set_tables("a", tables(&["x"]), Some(10));
        catalog.set_tables("b", tables(&["x"]), Some(20));

        let diff = catalog.set_schemas([
            header("a", Some(10)), // igual
            header("b", Some(21)), // mudou
            header("d", None),     // novo
        ]);

        assert_eq!(diff.added, ["d"]);
        assert_eq!(diff.removed, ["c"]);
        assert_eq!(diff.changed, ["b"]);
        assert_eq!(catalog.schema_state("a"), Some(LoadState::Loaded));
        assert_eq!(catalog.schema_state("b"), Some(LoadState::Stale));
        assert_eq!(catalog.schema_state("d"), Some(LoadState::Unloaded));
        assert_eq!(catalog.pending_schemas(), ["b", "d"]);

        // Recarregar com o fingerprint novo encerra a pendência
        catalog.set_tables("b", tables(&["x", "y"]), Some(21));
        assert!(catalog.set_schemas([header("a", Some(10)), header("b", Some(21)), header("d", None)]).is_empty());
    }

    #[test]
    fn released_shapes_leave_no_index_behind() {
        let mut catalog = Catalog::new();
        catalog.set_tables("t1", tables(&["orders"]), None);
        catalog.set_tables("t2", tables(&["orders"]), None);
        catalog.set_tables("t1", tables(&["invoices"]), None);
        catalog.remove_schema("t2");

        let orders = catalog.names.get("orders").unwrap();
        assert!(!catalog.by_table.contains(orders));
        assert_eq!(catalog.live_shapes().count(), 1);

        // O slot livre é reaproveitado
        catalog.set_tables("t3", tables(&["payments"]), None);
        assert_eq!(catalog.shapes.len(), 2);
    }

    #[test]
    fn reassigning_the_same_shape_keeps_it_alive() {
        let mut catalog = Catalog::new();
        catalog.set_tables("t1", tables(&["orders"]), None);
        catalog.set_tables("t1", tables(&["orders"]), Some(5));
        assert_eq!(catalog.live_shapes().count(), 1);
        assert_eq!(catalog.members.iter().sum::<u32>(), 1);
    }

    #[test]
    fn invalidate_only_touches_loaded_schemas() {
        let mut catalog = Catalog::new();
        catalog.set_schemas([header("a", None)]);
        assert!(!catalog.invalidate("a"));
        catalog.set_tables("a", tables(&["x"]), None);
        assert!(catalog.invalidate("a"));
        assert_eq!(catalog.schema_state("a"), Some(LoadState::Stale));
    }
}
