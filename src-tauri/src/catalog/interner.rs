use std::collections::HashMap;
use std::hash::{BuildHasher, RandomState};
use std::num::NonZeroU32;

use hashbrown::HashTable;

/// Nome internado: um índice no [`Interner`].
///
/// Em multi-tenant, 750 mil tabelas costumam ter ~150 nomes distintos — cada
/// nome mora uma vez só e o resto do catálogo guarda só o id. Guardado como
/// índice + 1 para `Option<Sym>` caber nos mesmos 4 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Sym(NonZeroU32);

impl Sym {
    pub(super) fn index(self) -> usize {
        self.0.get() as usize - 1
    }

    pub(super) fn from_index(index: usize) -> Self {
        Sym(NonZeroU32::new(index as u32 + 1).expect("mais de u32::MAX nomes"))
    }
}

/// Tabela de strings com busca exata e sem caixa (o Postgres dobra
/// identificadores sem aspas para minúsculas, então `Orders` e `orders` se
/// resolvem pelo mesmo caminho).
///
/// Feita para o pior caso — centenas de milhares de nomes únicos: o texto mora
/// uma vez num buffer contínuo, o índice exato guarda só o id de cada nome, e o
/// índice sem caixa só tem os nomes com maiúsculas (os todo-minúsculos já se
/// acham pelo exato).
#[derive(Debug, Default)]
pub struct Interner {
    text: String,
    /// Início e fim de cada nome em `text`
    spans: Vec<(u32, u32)>,
    exact: HashTable<Sym>,
    /// Caixa-baixa → nomes com maiúsculas que dobram para ela
    folded: HashMap<Box<str>, Vec<Sym>>,
    hasher: RandomState,
}

fn slice<'a>(text: &'a str, spans: &[(u32, u32)], sym: Sym) -> &'a str {
    let (start, end) = spans[sym.index()];
    &text[start as usize..end as usize]
}

impl Interner {
    pub fn intern(&mut self, name: &str) -> Sym {
        let hash = self.hasher.hash_one(name);
        let Self {
            text,
            spans,
            exact,
            folded,
            hasher,
        } = self;

        if let Some(&sym) = exact.find(hash, |&sym| slice(text, spans, sym) == name) {
            return sym;
        }

        let sym = Sym::from_index(spans.len());
        let start = text.len() as u32;
        text.push_str(name);
        spans.push((start, text.len() as u32));
        exact.insert_unique(hash, sym, |&sym| hasher.hash_one(slice(text, spans, sym)));

        let lower = name.to_lowercase();
        if lower != name {
            folded.entry(lower.into_boxed_str()).or_default().push(sym);
        }
        sym
    }

    pub fn get(&self, name: &str) -> Option<Sym> {
        let hash = self.hasher.hash_one(name);
        self.exact
            .find(hash, |&sym| self.resolve(sym) == name)
            .copied()
    }

    /// Todas as grafias que, em caixa-baixa, são iguais a `name` em caixa-baixa:
    /// a toda minúscula primeiro, depois as com maiúsculas na ordem de chegada.
    pub fn get_folded(&self, name: &str) -> Vec<Sym> {
        let lower = name.to_lowercase();
        let mut syms: Vec<Sym> = self.get(&lower).into_iter().collect();
        if let Some(mixed) = self.folded.get(lower.as_str()) {
            syms.extend(mixed);
        }
        syms
    }

    pub fn resolve(&self, sym: Sym) -> &str {
        slice(&self.text, &self.spans, sym)
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.spans.len()
    }

    #[cfg(test)]
    pub fn iter(&self) -> impl Iterator<Item = (Sym, &str)> {
        (0..self.spans.len()).map(|index| {
            let sym = Sym::from_index(index);
            (sym, self.resolve(sym))
        })
    }

    /// Bytes aproximados no heap (para as estatísticas e o diagnóstico).
    pub fn approx_heap_bytes(&self) -> usize {
        let folded: usize = self
            .folded
            .iter()
            .map(|(key, syms)| key.len() + 16 + 24 + syms.capacity() * 4 + 8)
            .sum();
        self.text.capacity()
            + self.spans.capacity() * 8
            // Um slot de 4 bytes + 1 de controle por posição da tabela
            + self.exact.capacity() * 5
            + folded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interns_once_and_resolves() {
        let mut interner = Interner::default();
        let a = interner.intern("orders");
        let b = interner.intern("customers");

        assert_eq!(interner.intern("orders"), a);
        assert_ne!(a, b);
        assert_eq!(interner.resolve(a), "orders");
        assert_eq!(interner.get("customers"), Some(b));
        assert_eq!(interner.get("missing"), None);
        assert_eq!(interner.len(), 2);
        assert_eq!(
            interner.iter().map(|(_, name)| name).collect::<Vec<_>>(),
            ["orders", "customers"]
        );
    }

    #[test]
    fn folds_case_to_every_spelling() {
        let mut interner = Interner::default();
        let mixed = interner.intern("Orders");
        let lower = interner.intern("orders");
        let upper = interner.intern("ORDERS");

        assert_eq!(interner.get_folded("oRdErS"), [lower, mixed, upper]);
        assert_eq!(interner.get_folded("orders"), [lower, mixed, upper]);
        assert!(interner.get_folded("invoices").is_empty());
    }

    #[test]
    fn survives_growth_and_rehashing() {
        let mut interner = Interner::default();
        let syms: Vec<Sym> = (0..10_000).map(|i| interner.intern(&format!("t_{i}"))).collect();
        for (i, sym) in syms.iter().enumerate() {
            assert_eq!(interner.get(&format!("t_{i}")), Some(*sym));
            assert_eq!(interner.resolve(*sym), format!("t_{i}"));
        }
        assert_eq!(std::mem::size_of::<Option<Sym>>(), 4);
    }
}
