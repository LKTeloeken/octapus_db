use std::collections::{HashMap, HashSet};
use std::num::NonZeroU32;

use super::interner::{Interner, Sym};
use super::model::{RelKind, TableRecord};

/// Índice de um [`Shape`] no catálogo. Estável durante a sessão; os slots de
/// formatos sem nenhum schema são reaproveitados. Índice + 1, para
/// `Option<ShapeId>` caber em 4 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ShapeId(NonZeroU32);

impl ShapeId {
    pub(super) fn index(self) -> usize {
        self.0.get() as usize - 1
    }

    pub(super) fn from_index(index: usize) -> Self {
        ShapeId(NonZeroU32::new(index as u32 + 1).expect("mais de u32::MAX formatos"))
    }
}

/// Nome de relação → formatos que o têm. Denso por [`Sym`]: o caso comum (o
/// nome está num formato só — sempre, no pior caso sem tenants) não aloca nada
/// além de 4 bytes; só nomes em vários formatos usam o mapa extra.
#[derive(Debug, Default)]
pub(super) struct TableIndex {
    first: Vec<Option<ShapeId>>,
    more: HashMap<Sym, Vec<ShapeId>>,
    live: usize,
}

impl TableIndex {
    pub fn add(&mut self, sym: Sym, shape: ShapeId) {
        let index = sym.index();
        if self.first.len() <= index {
            self.first.resize(index + 1, None);
        }
        match self.first[index] {
            None => {
                self.first[index] = Some(shape);
                self.live += 1;
            }
            Some(_) => self.more.entry(sym).or_default().push(shape),
        }
    }

    pub fn remove(&mut self, sym: Sym, shape: ShapeId) {
        let Some(slot) = self.first.get_mut(sym.index()) else {
            return;
        };
        if *slot == Some(shape) {
            // Promove um dos extras, se houver
            let next = self.more.get_mut(&sym).and_then(Vec::pop);
            if self.more.get(&sym).is_some_and(Vec::is_empty) {
                self.more.remove(&sym);
            }
            *slot = next;
            if next.is_none() {
                self.live -= 1;
            }
        } else if let Some(extra) = self.more.get_mut(&sym) {
            extra.retain(|&other| other != shape);
            if extra.is_empty() {
                self.more.remove(&sym);
            }
        }
    }

    /// Formatos que têm `sym` (vazio se nenhum).
    pub fn shapes(&self, sym: Sym) -> Vec<ShapeId> {
        let Some(Some(first)) = self.first.get(sym.index()) else {
            return Vec::new();
        };
        let mut shapes = vec![*first];
        if let Some(extra) = self.more.get(&sym) {
            shapes.extend(extra);
        }
        shapes
    }

    #[cfg(test)]
    pub fn contains(&self, sym: Sym) -> bool {
        matches!(self.first.get(sym.index()), Some(Some(_)))
    }

    /// Nomes que estão em ao menos um formato vivo.
    pub fn names(&self) -> impl Iterator<Item = Sym> + '_ {
        self.first
            .iter()
            .enumerate()
            .filter(|(_, shape)| shape.is_some())
            .map(|(index, _)| Sym::from_index(index))
    }

    pub fn len(&self) -> usize {
        self.live
    }

    pub fn approx_heap_bytes(&self) -> usize {
        let more: usize = self
            .more
            .values()
            .map(|shapes| 4 + 24 + shapes.capacity() * 4 + 8)
            .sum();
        self.first.capacity() * 4 + more
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ShapeTable {
    pub name: Sym,
    pub kind: RelKind,
    /// Pai, quando é partição (sempre uma relação do mesmo formato)
    pub parent: Option<Sym>,
}

/// O conjunto de relações de um schema. Tenants com as mesmas tabelas apontam
/// para o mesmo formato — é o que faz 5.000 schemas × 150 tabelas caberem em
/// poucos formatos em vez de 750 mil registros.
#[derive(Debug, Clone, Default)]
pub(super) struct Shape {
    /// Ordenadas pelo nome em ordem de bytes (a mesma do `ORDER BY` sobre o tipo
    /// `name` do Postgres), o que permite busca binária por nome
    pub tables: Box<[ShapeTable]>,
    pub hash: u64,
    /// Relações fora as partições — o que aparece direto sob o schema
    pub top_level: u32,
}

impl Shape {
    /// Monta o formato a partir do que veio do banco. Nomes repetidos ficam com
    /// a primeira ocorrência; uma partição cujo pai não está na lista vira
    /// relação de topo (melhor aparecer solta do que sumir).
    pub fn build(mut records: Vec<TableRecord>, names: &mut Interner) -> Shape {
        records.sort_by(|a, b| a.name.cmp(&b.name));
        records.dedup_by(|a, b| a.name == b.name);

        let present: HashSet<&str> = records.iter().map(|r| r.name.as_str()).collect();
        let parents: Vec<Option<String>> = records
            .iter()
            .map(|r| {
                r.partition_of
                    .clone()
                    .filter(|parent| parent != &r.name && present.contains(parent.as_str()))
            })
            .collect();

        let tables = records
            .iter()
            .zip(parents)
            .map(|(record, parent)| ShapeTable {
                name: names.intern(&record.name),
                kind: record.kind,
                parent: parent.as_deref().map(|p| names.intern(p)),
            })
            .collect();

        Self::from_tables(tables, names)
    }

    /// Monta a partir de relações já internadas (a carga do arquivo). Reordena
    /// só se precisar, e calcula hash e contagem do mesmo jeito que [`Shape::build`].
    pub fn from_tables(mut tables: Vec<ShapeTable>, names: &Interner) -> Shape {
        let by_name = |a: &ShapeTable, b: &ShapeTable| names.resolve(a.name).cmp(names.resolve(b.name));
        if !tables.is_sorted_by(|a, b| by_name(a, b).is_le()) {
            tables.sort_by(by_name);
        }

        let mut hasher = Fnv::new();
        let mut top_level = 0;
        for table in &tables {
            hasher.write(names.resolve(table.name).as_bytes());
            hasher.write(&[0xff, table.kind.code()]);
            match table.parent {
                Some(parent) => hasher.write(names.resolve(parent).as_bytes()),
                None => top_level += 1,
            }
            hasher.write(&[0xfe]);
        }

        Shape {
            tables: tables.into_boxed_slice(),
            hash: hasher.finish(),
            top_level,
        }
    }

    pub fn find(&self, names: &Interner, name: &str) -> Option<&ShapeTable> {
        self.tables
            .binary_search_by(|table| names.resolve(table.name).cmp(name))
            .ok()
            .map(|index| &self.tables[index])
    }

    pub fn partitions_of(&self, parent: Sym) -> impl Iterator<Item = &ShapeTable> {
        self.tables
            .iter()
            .filter(move |table| table.parent == Some(parent))
    }

}

/// FNV-1a de 64 bits — estável entre execuções (o hasher padrão do Rust não
/// é), então serve também para nomear arquivos.
pub(super) struct Fnv(u64);

impl Fnv {
    pub fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }

    pub fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }

    pub fn finish(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names_of(shape: &Shape, names: &Interner) -> Vec<String> {
        shape
            .tables
            .iter()
            .map(|t| names.resolve(t.name).to_string())
            .collect()
    }

    #[test]
    fn sorts_dedups_and_hashes_by_content() {
        let mut names = Interner::default();
        let a = Shape::build(
            vec![
                TableRecord::new("orders", RelKind::Table),
                TableRecord::new("customers", RelKind::Table),
                TableRecord::new("orders", RelKind::View),
            ],
            &mut names,
        );
        let b = Shape::build(
            vec![
                TableRecord::new("customers", RelKind::Table),
                TableRecord::new("orders", RelKind::Table),
            ],
            &mut names,
        );

        assert_eq!(names_of(&a, &names), ["customers", "orders"]);
        assert_eq!(a.find(&names, "orders").unwrap().kind, RelKind::Table);
        assert_eq!(a.hash, b.hash);
        assert_eq!(a.tables, b.tables);
        assert!(a.find(&names, "invoices").is_none());
    }

    #[test]
    fn kind_and_partitioning_change_the_shape() {
        let mut names = Interner::default();
        let table = Shape::build(vec![TableRecord::new("events", RelKind::Table)], &mut names);
        let view = Shape::build(vec![TableRecord::new("events", RelKind::View)], &mut names);
        assert_ne!(table.hash, view.hash);

        let partitioned = Shape::build(
            vec![
                TableRecord::new("events", RelKind::Partitioned),
                TableRecord::partition("events_p1", "events"),
                TableRecord::partition("events_p2", "events"),
            ],
            &mut names,
        );
        assert_eq!(partitioned.top_level, 1);
        let parent = names.get("events").unwrap();
        assert_eq!(partitioned.partitions_of(parent).count(), 2);
    }

    #[test]
    fn table_index_promotes_and_counts_live_names() {
        let mut names = Interner::default();
        let orders = names.intern("orders");
        let (a, b, c) = (ShapeId::from_index(0), ShapeId::from_index(1), ShapeId::from_index(2));
        let mut index = TableIndex::default();

        index.add(orders, a);
        index.add(orders, b);
        index.add(orders, c);
        assert_eq!(index.shapes(orders), [a, b, c]);
        assert_eq!(index.len(), 1);

        index.remove(orders, a);
        index.remove(orders, b);
        assert_eq!(index.shapes(orders), [c]);
        index.remove(orders, c);
        assert!(!index.contains(orders));
        assert_eq!(index.len(), 0);
        assert_eq!(index.names().count(), 0);
        assert_eq!(std::mem::size_of::<ShapeTable>(), 12);
    }

    #[test]
    fn orphan_partition_becomes_top_level() {
        let mut names = Interner::default();
        let shape = Shape::build(vec![TableRecord::partition("events_p1", "events")], &mut names);
        assert_eq!(shape.top_level, 1);
        assert_eq!(shape.tables[0].parent, None);
    }
}
