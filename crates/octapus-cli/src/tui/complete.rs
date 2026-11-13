//! O que o autocompletar do editor oferece, e onde.
//!
//! [`analyze`] olha o texto em volta do cursor e diz o que cabe ali (uma
//! tabela depois de `FROM`, uma coluna depois de `WHERE`, os membros de
//! `alias.`…) e quais tabelas a consulta cita, com os apelidos. É um
//! varredor de tokens feito à mão, sem banco e sem analisar o SQL inteiro:
//! cobre o que se digita no dia a dia e, quando não entende, cai na lista
//! geral — nunca trava a digitação nem escreve sozinho.
//!
//! [`rank`] ordena os candidatos que a TUI juntou (do catálogo, das colunas
//! já lidas e das palavras-chave) pelo que foi digitado.

use super::editor::is_keyword;

/// O que cabe na posição do cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Dentro de um texto entre aspas ou de um comentário: nada
    Nothing,
    /// Sem contexto claro (o começo de um comando, depois de uma expressão)
    Any,
    /// Uma tabela ou um schema (`FROM`, `JOIN`, `UPDATE`, `INTO`)
    Table,
    /// Uma coluna (`SELECT`, `WHERE`, `ON`, `ORDER BY`…)
    Column,
    /// O que vem depois de `nome.`: colunas de uma tabela ou apelido, ou as
    /// tabelas de um schema
    Member(String),
}

/// Uma tabela citada na consulta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRef {
    pub schema: Option<String>,
    pub name: String,
    pub alias: Option<String>,
}

impl TableRef {
    /// `name` é como a consulta chama esta tabela? (o apelido, ou o nome
    /// dela quando não há apelido — e o nome também vale com apelido)
    pub fn is_called(&self, name: &str) -> bool {
        self.alias.as_deref().is_some_and(|alias| alias.eq_ignore_ascii_case(name)) || self.name.eq_ignore_ascii_case(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    pub scope: Scope,
    /// O que já foi digitado da palavra atual (pode ser vazio)
    pub prefix: String,
    pub tables: Vec<TableRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Word(String),
    Dot,
    Comma,
    Open,
    Close,
    Semi,
    /// Texto entre aspas simples, comentário, operador: não é nome
    Other,
}

struct Token {
    tok: Tok,
    start: usize,
    end: usize,
}

fn is_word(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// Os tokens do texto e se `cursor` caiu dentro de aspas ou de comentário.
fn tokenize(chars: &[char], cursor: usize) -> (Vec<Token>, bool) {
    let mut tokens = Vec::new();
    let mut opaque = false;
    let mut index = 0;
    while index < chars.len() {
        let start = index;
        let ch = chars[index];
        let next = chars.get(index + 1).copied();
        let tok = match ch {
            ch if ch.is_whitespace() => {
                index += 1;
                continue;
            }
            '-' if next == Some('-') => {
                while index < chars.len() && chars[index] != '\n' {
                    index += 1;
                }
                opaque |= cursor > start && cursor <= index;
                Tok::Other
            }
            '/' if next == Some('*') => {
                index += 2;
                let mut closed = false;
                while index < chars.len() {
                    if chars[index] == '*' && chars.get(index + 1) == Some(&'/') {
                        index += 2;
                        closed = true;
                        break;
                    }
                    index += 1;
                }
                opaque |= cursor > start && (cursor < index || !closed);
                Tok::Other
            }
            '\'' | '"' => {
                index += 1;
                let mut closed = false;
                while index < chars.len() {
                    if chars[index] == ch {
                        index += 1;
                        // Aspas dobradas continuam o texto
                        if chars.get(index) == Some(&ch) {
                            index += 1;
                            continue;
                        }
                        closed = true;
                        break;
                    }
                    index += 1;
                }
                opaque |= cursor > start && (cursor < index || !closed);
                match (ch, closed) {
                    // Um identificador entre aspas é um nome como outro
                    ('"', true) => Tok::Word(chars[start + 1..index - 1].iter().collect::<String>().replace("\"\"", "\"")),
                    _ => Tok::Other,
                }
            }
            ch if is_word(ch) => {
                while index < chars.len() && is_word(chars[index]) {
                    index += 1;
                }
                Tok::Word(chars[start..index].iter().collect())
            }
            '.' => {
                index += 1;
                Tok::Dot
            }
            ',' => {
                index += 1;
                Tok::Comma
            }
            '(' => {
                index += 1;
                Tok::Open
            }
            ')' => {
                index += 1;
                Tok::Close
            }
            ';' => {
                index += 1;
                Tok::Semi
            }
            _ => {
                index += 1;
                Tok::Other
            }
        };
        tokens.push(Token { tok, start, end: index });
    }
    (tokens, opaque)
}

fn keyword(token: &Token) -> Option<String> {
    match &token.tok {
        Tok::Word(word) if is_keyword(word) => Some(word.to_ascii_lowercase()),
        _ => None,
    }
}

/// Um nome que não é palavra-chave.
fn name(token: Option<&Token>) -> Option<String> {
    match &token?.tok {
        Tok::Word(word) if !is_keyword(word) => Some(word.clone()),
        _ => None,
    }
}

/// `[schema.]tabela [AS] [apelido]` a partir de `at`; devolve onde parou.
fn table_ref(tokens: &[Token], at: usize) -> Option<(TableRef, usize)> {
    let first = name(tokens.get(at))?;
    let mut next = at + 1;
    let (schema, table) = match (tokens.get(next).map(|token| &token.tok), name(tokens.get(next + 1))) {
        (Some(Tok::Dot), Some(table)) => {
            next += 2;
            (Some(first), table)
        }
        // `schema.` ainda sem a tabela: não é uma tabela chamada `schema`
        (Some(Tok::Dot), None) => return None,
        _ => (None, first),
    };
    if tokens.get(next).and_then(keyword).as_deref() == Some("as") {
        next += 1;
    }
    let alias = name(tokens.get(next));
    if alias.is_some() {
        next += 1;
    }
    Some((
        TableRef {
            schema,
            name: table,
            alias,
        },
        next,
    ))
}

/// As tabelas que o comando cita (depois de `FROM`, `JOIN`, `UPDATE`, `INTO`).
fn tables(tokens: &[Token]) -> Vec<TableRef> {
    let mut found: Vec<TableRef> = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let word = keyword(&tokens[index]);
        index += 1;
        if !matches!(word.as_deref(), Some("from" | "join" | "update" | "into")) {
            continue;
        }
        // Depois de `FROM`, uma lista separada por vírgulas
        while let Some((table, next)) = table_ref(tokens, index) {
            index = next;
            if !found.contains(&table) {
                found.push(table);
            }
            match (word.as_deref(), tokens.get(index).map(|token| &token.tok)) {
                (Some("from"), Some(Tok::Comma)) => index += 1,
                _ => break,
            }
        }
    }
    found
}

/// O que cabe em `cursor` (posição em caracteres dentro de `text`).
pub fn analyze(text: &str, cursor: usize) -> Context {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());
    let (tokens, opaque) = tokenize(&chars, cursor);

    let mut prefix_start = cursor;
    while prefix_start > 0 && is_word(chars[prefix_start - 1]) {
        prefix_start -= 1;
    }
    let prefix: String = chars[prefix_start..cursor].iter().collect();

    // Só o comando em que o cursor está
    let first = tokens.iter().rposition(|token| token.tok == Tok::Semi && token.end <= prefix_start).map_or(0, |at| at + 1);
    let last = tokens
        .iter()
        .position(|token| token.tok == Tok::Semi && token.start >= cursor)
        .unwrap_or(tokens.len());
    let statement = &tokens[first..last.max(first)];
    let tables = tables(statement);
    if opaque {
        return Context {
            scope: Scope::Nothing,
            prefix,
            tables,
        };
    }

    let before: Vec<&Token> = statement.iter().filter(|token| token.end <= prefix_start).collect();
    let scope = match before.last().map(|token| &token.tok) {
        Some(Tok::Dot) => match before.len().checked_sub(2).and_then(|at| name(Some(before[at]))) {
            Some(qualifier) => Scope::Member(qualifier),
            None => Scope::Any,
        },
        _ => clause(&before),
    };
    Context { scope, prefix, tables }
}

/// A cláusula em que o cursor está: a última palavra-chave que manda, de
/// trás para a frente, pulando o que está entre parênteses já fechados.
fn clause(before: &[&Token]) -> Scope {
    let mut depth = 0usize;
    for (index, token) in before.iter().enumerate().rev() {
        match &token.tok {
            Tok::Close => depth += 1,
            Tok::Open => depth = depth.saturating_sub(1),
            _ if depth > 0 => {}
            _ => match keyword(token).as_deref() {
                Some("from" | "join" | "update" | "into" | "table") => {
                    // Logo depois da palavra (ou da vírgula de uma lista) vem
                    // uma tabela; depois do nome dela, o apelido ou a próxima
                    // cláusula
                    let expects_table = match before.last().map(|token| &token.tok) {
                        Some(Tok::Comma) => true,
                        _ => index == before.len() - 1,
                    };
                    return if expects_table { Scope::Table } else { Scope::Any };
                }
                Some(
                    "select" | "where" | "on" | "and" | "or" | "by" | "having" | "set" | "returning" | "when" | "then"
                    | "else" | "distinct" | "not" | "between" | "like" | "ilike" | "in" | "case" | "using",
                ) => return Scope::Column,
                _ => {}
            },
        }
    }
    Scope::Any
}

// ── Candidatos ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Column,
    Table,
    View,
    Schema,
    Keyword,
}

impl ItemKind {
    pub fn glyph(self) -> &'static str {
        match self {
            ItemKind::Column => "▪",
            ItemKind::Table => "▤",
            ItemKind::View => "◫",
            ItemKind::Schema => "◇",
            ItemKind::Keyword => "·",
        }
    }
}

/// Um candidato do autocompletar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub label: String,
    pub kind: ItemKind,
    /// O tipo da coluna, o schema da tabela…
    pub detail: String,
}

impl Item {
    pub fn new(label: impl Into<String>, kind: ItemKind, detail: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            kind,
            detail: detail.into(),
        }
    }
}

/// Os candidatos que casam com `prefix`, na ordem em que aparecem na lista:
/// o nome exato, os que começam por ele e, por fim, os que o contêm. Dentro
/// de cada grupo vale a ordem de entrada (colunas antes de tabelas, antes de
/// palavras-chave — quem monta a lista decide).
pub fn rank(prefix: &str, candidates: Vec<Item>, limit: usize) -> Vec<Item> {
    let needle = prefix.to_lowercase();
    let mut groups: [Vec<Item>; 3] = Default::default();
    for item in candidates {
        let label = item.label.to_lowercase();
        let group = if label == needle {
            0
        } else if label.starts_with(&needle) {
            1
        } else if needle.len() >= 2 && label.contains(&needle) {
            2
        } else {
            continue;
        };
        let seen = groups.iter().flatten().any(|other| other.label.eq_ignore_ascii_case(&item.label) && other.kind == item.kind);
        if !seen {
            groups[group].push(item);
        }
    }
    groups.into_iter().flatten().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `|` marca o cursor.
    fn at(text: &str) -> Context {
        let cursor = text.chars().position(|ch| ch == '|').expect("falta o |");
        analyze(&text.replace('|', ""), cursor)
    }

    fn table(schema: Option<&str>, name: &str, alias: Option<&str>) -> TableRef {
        TableRef {
            schema: schema.map(str::to_string),
            name: name.to_string(),
            alias: alias.map(str::to_string),
        }
    }

    #[test]
    fn the_clause_decides_what_fits() {
        assert_eq!(at("select * from |").scope, Scope::Table);
        assert_eq!(at("select * from cu|").scope, Scope::Table);
        assert_eq!(at("select * from a, |").scope, Scope::Table);
        assert_eq!(at("select * from a left join |").scope, Scope::Table);
        assert_eq!(at("update |").scope, Scope::Table);
        assert_eq!(at("insert into |").scope, Scope::Table);
        // Depois do nome da tabela vem o apelido ou a próxima cláusula
        assert_eq!(at("select * from orders |").scope, Scope::Any);
        assert_eq!(at("select * from orders o wh|").scope, Scope::Any);

        assert_eq!(at("select |").scope, Scope::Column);
        assert_eq!(at("select a, to| from orders").scope, Scope::Column);
        assert_eq!(at("select * from orders where |").scope, Scope::Column);
        assert_eq!(at("select * from orders where a = 1 and to|").scope, Scope::Column);
        assert_eq!(at("select * from a join b on |").scope, Scope::Column);
        assert_eq!(at("select * from orders order by |").scope, Scope::Column);
        // Dentro de uma função continua valendo a cláusula de fora
        assert_eq!(at("select count(|) from orders").scope, Scope::Column);
        assert_eq!(at("select * from orders where id in (select max(id) from |").scope, Scope::Table);

        assert_eq!(at("|").scope, Scope::Any);
        assert_eq!(at("sel|").prefix, "sel");
    }

    #[test]
    fn a_dot_asks_for_the_members_of_what_comes_before() {
        let context = at("select o.to| from public.orders o");
        assert_eq!(context.scope, Scope::Member("o".into()));
        assert_eq!(context.prefix, "to");
        assert_eq!(at("select * from public.|").scope, Scope::Member("public".into()));
        assert_eq!(at("select \"Order Items\".| from \"Order Items\"").scope, Scope::Member("Order Items".into()));
    }

    #[test]
    fn tables_and_aliases_come_from_the_whole_statement() {
        let context = at("select | from public.orders o join customers as c on c.id = o.customer_id, audit where 1 = 1");
        assert_eq!(
            context.tables,
            [
                table(Some("public"), "orders", Some("o")),
                table(None, "customers", Some("c")),
            ]
        );
        assert!(context.tables[0].is_called("O") && context.tables[0].is_called("orders"));

        assert_eq!(at("select * from a, b x, c where |").tables.len(), 3);
        assert_eq!(at("update orders set |").tables, [table(None, "orders", None)]);
        // `schema.` pela metade não vira uma tabela com o nome do schema
        assert_eq!(at("select * from tenant_1.|").tables, []);
        assert_eq!(at("select * from tenant_1.| join plans p on true").tables, [table(None, "plans", Some("p"))]);
        // Uma subconsulta no FROM não é uma tabela
        assert_eq!(at("select | from (select 1) t").tables, []);
        // Só o comando do cursor
        assert_eq!(at("select * from a; select | from b; select * from c").tables, [table(None, "b", None)]);
    }

    #[test]
    fn nothing_is_offered_inside_text_or_comments() {
        assert_eq!(at("select * from orders where name = 'An|").scope, Scope::Nothing);
        assert_eq!(at("select * from orders where name = 'Ana|' and x").scope, Scope::Nothing);
        assert_eq!(at("select 1 -- from |").scope, Scope::Nothing);
        assert_eq!(at("select /* from | */ 1").scope, Scope::Nothing);
        // Depois de fechar, volta a valer
        assert_eq!(at("select * from orders where name = 'Ana' and |").scope, Scope::Column);
        assert_eq!(at("select 1 -- nota\nfrom |").scope, Scope::Table);
    }

    #[test]
    fn ranking_puts_the_exact_name_first_then_prefixes_then_the_rest() {
        let items = vec![
            Item::new("subtotal", ItemKind::Column, "numeric"),
            Item::new("total_tax", ItemKind::Column, "numeric"),
            Item::new("total", ItemKind::Column, "numeric"),
            Item::new("TOTAL", ItemKind::Column, "numeric"),
            Item::new("tokens", ItemKind::Table, "public"),
            Item::new("id", ItemKind::Column, "int4"),
        ];
        let labels = |prefix: &str| -> Vec<String> { rank(prefix, items.clone(), 50).into_iter().map(|item| item.label).collect() };
        assert_eq!(labels("total"), ["total", "total_tax", "subtotal"]);
        assert_eq!(labels("to"), ["total_tax", "total", "tokens", "subtotal"]);
        // Uma letra só: o que começa por ela, sem o "contém"
        assert_eq!(labels("t"), ["total_tax", "total", "tokens"]);
        assert_eq!(labels("").len(), 5);
        assert_eq!(rank("", items.clone(), 2).len(), 2);
    }
}
