//! Estrutura para o contexto do assistente: quais relações mandar ao modelo
//! (ranking léxico com a pergunta e as vizinhas pelas chaves estrangeiras) e
//! os detalhes que a fonte devolve delas (colunas, chaves, comentários).
//!
//! Parte pura: nada aqui vai ao banco. O catálogo sabe as relações; colunas e
//! chaves estrangeiras vêm da fonte (`CatalogSource`), só das relações
//! escolhidas — nunca da estrutura inteira.

use std::collections::HashSet;

use super::model::RelKind;

/// Uma chave estrangeira: `schema.table(columns)` → `ref_schema.ref_table(ref_columns)`.
/// `ref_columns` vazio = a chave primária da referenciada (SQLite sem `to`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignKey {
    pub schema: String,
    pub table: String,
    pub columns: Vec<String>,
    pub ref_schema: String,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
}

/// Uma coluna, como o contexto do assistente precisa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnDetail {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub primary_key: bool,
    pub comment: Option<String>,
}

/// Uma relação com as colunas na ordem da tabela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationDetail {
    pub schema: String,
    pub table: String,
    pub comment: Option<String>,
    pub columns: Vec<ColumnDetail>,
}

/// Uma relação candidata, com a pontuação contra a pergunta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub schema: String,
    pub table: String,
    pub kind: RelKind,
    pub score: u32,
}

impl Candidate {
    pub fn key(&self) -> (String, String) {
        (self.schema.clone(), self.table.clone())
    }
}

/// Palavras que não ajudam a achar tabela (português e inglês).
const STOPWORDS: &[&str] = &[
    "the", "and", "for", "from", "with", "what", "which", "how", "many", "much", "all", "each",
    "per", "list", "show", "give", "get", "find", "that", "this", "are", "was", "were", "have",
    "has", "por", "para", "com", "que", "dos", "das", "nos", "nas", "uma", "uns", "umas", "qual",
    "quais", "quanto", "quantos", "quanta", "quantas", "todo", "todos", "toda", "todas", "cada",
    "mostre", "mostrar", "liste", "listar", "traga", "trazer", "quero", "ver", "sao", "foi",
    "foram", "tem", "ter", "esta", "estao", "isso", "esse", "essa", "este", "como", "onde",
    "quando", "entre", "mais", "menos", "maior", "menor", "ultimo", "ultimos", "ultima", "ultimas",
    "last", "first", "top", "most", "least",
];

/// Minúsculas e sem acento (o suficiente para português).
fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}

/// Singular aproximado: `categories` → `category`, `pedidos` → `pedido`,
/// `transacoes` → `transacao`.
fn stem(word: &str) -> String {
    let len = word.chars().count();
    if len > 4 && word.ends_with("ies") {
        format!("{}y", &word[..word.len() - 3])
    } else if len > 4 && word.ends_with("oes") {
        format!("{}ao", &word[..word.len() - 3])
    } else if len > 3 && word.ends_with('s') && !word.ends_with("ss") {
        word[..word.len() - 1].to_string()
    } else {
        word.to_string()
    }
}

/// As palavras de um texto livre (pergunta, histórico), já no singular.
pub fn question_terms(text: &str) -> Vec<String> {
    fold(text)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| word.chars().count() >= 3 && !STOPWORDS.contains(word))
        .filter(|word| !word.chars().all(|c| c.is_ascii_digit()))
        .map(stem)
        .collect()
}

/// As partes de um nome de relação: `order_items` → `order`, `item`;
/// `OrderItems` → `order`, `item`; `tenant_00042` → `tenant`.
pub fn name_parts(name: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut previous_lower = false;
    for c in name.chars() {
        let boundary = !c.is_alphanumeric() || c.is_ascii_digit() || (c.is_uppercase() && previous_lower);
        if boundary && !current.is_empty() {
            parts.push(std::mem::take(&mut current));
        }
        if c.is_alphabetic() {
            current.push(c);
        }
        previous_lower = c.is_lowercase();
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
        .into_iter()
        .map(|part| stem(&fold(&part)))
        .filter(|part| part.chars().count() >= 2)
        .collect()
}

/// Prefixo comum de pelo menos 5 letras: `customer` e `customers`,
/// `pagamento` e `pagamentos` (já cobertos pelo singular), `produto` e `produtos_estoque`.
fn shares_prefix(a: &str, b: &str) -> bool {
    a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count() >= 5
}

/// Pontuação de um nome contra os termos da pergunta: nome inteiro igual vale
/// mais que uma parte igual, que vale mais que um prefixo comum.
pub fn score_name(name: &str, terms: &HashSet<String>) -> u32 {
    if terms.is_empty() {
        return 0;
    }
    let parts = name_parts(name);
    let whole = parts.join("");
    let mut score = 0;
    if terms.contains(&whole) {
        score += 15;
    }
    for part in &parts {
        if part.chars().count() < 3 {
            continue;
        }
        if terms.contains(part) {
            score += 10;
        } else if terms.iter().any(|term| shares_prefix(term, part)) {
            score += 4;
        }
    }
    score
}

/// As escolhidas (`seeds`, na ordem do ranking) e depois as vizinhas pelas
/// chaves estrangeiras — que apontam para elas ou que elas apontam —, até
/// `cap` relações. As vizinhas entram na ordem das sementes: as da primeira
/// antes das da segunda. Uma vizinha só entra se está entre as candidatas
/// (`allowed`), o que mantém o contexto dentro dos schemas pedidos.
pub fn with_neighbors(
    seeds: &[(String, String)],
    foreign_keys: &[ForeignKey],
    allowed: &HashSet<(String, String)>,
    cap: usize,
) -> Vec<(String, String)> {
    let mut chosen: Vec<(String, String)> = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for seed in seeds.iter().take(cap) {
        if seen.insert(seed.clone()) {
            chosen.push(seed.clone());
        }
    }

    for seed in seeds {
        if chosen.len() >= cap {
            break;
        }
        for fk in foreign_keys {
            let from = (fk.schema.clone(), fk.table.clone());
            let to = (fk.ref_schema.clone(), fk.ref_table.clone());
            let neighbor = if &from == seed {
                to
            } else if &to == seed {
                from
            } else {
                continue;
            };
            if chosen.len() >= cap {
                break;
            }
            if allowed.contains(&neighbor) && seen.insert(neighbor.clone()) {
                chosen.push(neighbor);
            }
        }
    }
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terms(text: &str) -> HashSet<String> {
        question_terms(text).into_iter().collect()
    }

    fn key(table: &str) -> (String, String) {
        ("public".to_string(), table.to_string())
    }

    #[test]
    fn question_terms_fold_accents_drop_stopwords_and_plurals() {
        assert_eq!(
            question_terms("Quais são as transações dos clientes em 2026?"),
            ["transacao", "cliente"]
        );
        assert_eq!(question_terms("How many ORDERS per category"), ["order", "category"]);
    }

    #[test]
    fn name_parts_split_snake_camel_and_digits() {
        assert_eq!(name_parts("order_items"), ["order", "item"]);
        assert_eq!(name_parts("OrderItems"), ["order", "item"]);
        assert_eq!(name_parts("tenant_00042"), ["tenant"]);
        assert_eq!(name_parts("Pagamentos"), ["pagamento"]);
    }

    #[test]
    fn scores_rank_whole_names_over_parts_over_prefixes() {
        let question = terms("total dos pedidos por cliente");
        let pedidos = score_name("pedidos", &question);
        let itens = score_name("pedido_itens", &question);
        let clientes = score_name("clientes", &question);
        let produtos = score_name("produtos", &question);
        assert!(pedidos > itens, "{pedidos} > {itens}");
        assert!(clientes > 0 && itens > 0);
        assert_eq!(produtos, 0);

        let english = terms("revenue by customer");
        assert!(score_name("customer_accounts", &english) > 0);
        assert_eq!(score_name("orders", &terms("")), 0);
    }

    #[test]
    fn neighbors_follow_foreign_keys_both_ways_within_the_cap() {
        let fk = |table: &str, references: &str| ForeignKey {
            schema: "public".into(),
            table: table.into(),
            columns: vec!["x".into()],
            ref_schema: "public".into(),
            ref_table: references.into(),
            ref_columns: vec!["id".into()],
        };
        let fks = [
            fk("orders", "customers"),
            fk("order_items", "orders"),
            fk("order_items", "products"),
            fk("invoices", "orders"),
            fk("audit", "elsewhere"),
        ];
        let allowed: HashSet<_> = ["orders", "customers", "order_items", "products", "invoices", "audit"]
            .into_iter()
            .map(key)
            .collect();

        let chosen = with_neighbors(&[key("orders")], &fks, &allowed, 10);
        assert_eq!(
            chosen,
            [key("orders"), key("customers"), key("order_items"), key("invoices")]
        );

        // Teto: as sementes primeiro, depois as vizinhas até completar
        let chosen = with_neighbors(&[key("orders"), key("products")], &fks, &allowed, 3);
        assert_eq!(chosen, [key("orders"), key("products"), key("customers")]);

        // Vizinha fora das candidatas (outro schema) não entra
        let chosen = with_neighbors(&[key("audit")], &fks, &allowed, 10);
        assert_eq!(chosen, [key("audit")]);
    }
}
