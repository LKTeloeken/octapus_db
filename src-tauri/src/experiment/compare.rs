//! O veredito de um teste: o resultado obtido contra o resultado do gabarito
//! (nunca o texto da consulta). As regras são do conjunto de perguntas, com
//! padrão no arquivo e ajuste por pergunta ou por gabarito ([`RulesPatch`]):
//!
//! | regra               | valores                          | padrão   |
//! |---------------------|----------------------------------|----------|
//! | `order`             | `ignore`, `strict`               | `ignore` |
//! | `columns`           | `any`, `position`, `name`        | `any`    |
//! | `extra_columns`     | `forbid`, `allow`                | `forbid` |
//! | `decimals`          | casas no arredondamento          | `4`      |
//! | `null_equals_empty` | `true`, `false`                  | `false`  |
//! | `truncated`         | `fail`, `subset`                 | `fail`   |
//!
//! - **Ordem das linhas** (`order`): `ignore` compara como multiconjunto (a
//!   mesma linha duas vezes conta duas vezes); `strict` exige a mesma sequência.
//! - **Colunas** (`columns`): `any` ignora os nomes e procura um pareamento de
//!   colunas em que as linhas batem; `position` compara a i-ésima com a
//!   i-ésima; `name` pareia pelo nome, sem diferenciar maiúsculas.
//! - **Colunas extras** (`extra_columns`): `forbid` exige o mesmo número de
//!   colunas; `allow` aceita colunas a mais no obtido. Faltar coluna é erro.
//! - **Arredondamento** (`decimals`): células que são números nos dois lados
//!   são arredondadas a N casas antes de comparar (`1`, `1.0` e `1.00001` são
//!   iguais com 4 casas). Inteiros são comparados dígito a dígito, sem perder
//!   precisão. O resto é texto, comparado como veio do banco.
//! - **Nulos**: nulo só é igual a nulo; com `null_equals_empty`, também ao
//!   texto vazio.
//! - **Resultado incompleto** (`truncated`): o obtido passou da página do
//!   assistente. `fail` reprova; `subset` aceita se toda linha obtida está no
//!   esperado (com `order = strict`, se é um prefixo dele). Um obtido com
//!   menos linhas que o esperado, sem ter sido cortado, sempre reprova.
//!
//! A correspondência exata com a consulta de referência ([`exact_match`]) é
//! outro indicador, só do PostgreSQL, e não entra no veredito.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::tokenizer::{Token, Tokenizer};

use crate::models::QueryResult;

/// Pareamentos completos de colunas testados antes de desistir (`columns =
/// any`). O filtro por conteúdo de cada coluna deixa pouquíssimos candidatos;
/// o teto só protege de um resultado com muitas colunas idênticas.
const MAX_MAPPINGS: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowOrder {
    Ignore,
    Strict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnMatch {
    Any,
    Position,
    Name,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtraColumns {
    Forbid,
    Allow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Truncated {
    Fail,
    Subset,
}

/// As regras em vigor para um gabarito.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rules {
    pub order: RowOrder,
    pub columns: ColumnMatch,
    pub extra_columns: ExtraColumns,
    pub decimals: u32,
    pub null_equals_empty: bool,
    pub truncated: Truncated,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            order: RowOrder::Ignore,
            columns: ColumnMatch::Any,
            extra_columns: ExtraColumns::Forbid,
            decimals: 4,
            null_equals_empty: false,
            truncated: Truncated::Fail,
        }
    }
}

/// O que um nível (arquivo, pergunta, gabarito) muda nas regras.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulesPatch {
    pub order: Option<RowOrder>,
    pub columns: Option<ColumnMatch>,
    pub extra_columns: Option<ExtraColumns>,
    pub decimals: Option<u32>,
    pub null_equals_empty: Option<bool>,
    pub truncated: Option<Truncated>,
}

impl Rules {
    pub fn with(self, patch: Option<&RulesPatch>) -> Self {
        let Some(patch) = patch else {
            return self;
        };
        Self {
            order: patch.order.unwrap_or(self.order),
            columns: patch.columns.unwrap_or(self.columns),
            extra_columns: patch.extra_columns.unwrap_or(self.extra_columns),
            decimals: patch.decimals.unwrap_or(self.decimals),
            null_equals_empty: patch.null_equals_empty.unwrap_or(self.null_equals_empty),
            truncated: patch.truncated.unwrap_or(self.truncated),
        }
    }
}

/// Um resultado como fica gravado na rodada: as células como o adapter deu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
    /// Havia mais linhas além da página
    pub truncated: bool,
}

impl From<&QueryResult> for Table {
    fn from(result: &QueryResult) -> Self {
        Self {
            columns: result.columns.iter().map(|column| column.name.clone()).collect(),
            rows: result.rows.clone(),
            truncated: result.has_more,
        }
    }
}

/// Por que o obtido não bate com o esperado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mismatch {
    /// O obtido foi cortado pela página (`truncated = fail`, ou não é parte
    /// do esperado)
    Truncated,
    /// Falta coluna, ou sobra com `extra_columns = forbid`
    ColumnCount,
    /// `columns = name` e um nome esperado não veio
    ColumnNames,
    RowCount,
    Values,
    /// As mesmas linhas, em outra ordem (`order = strict`)
    Order,
}

/// O veredito e o que a tela precisa para marcar as diferenças.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub equal: bool,
    pub mismatch: Option<Mismatch>,
    /// Para cada coluna esperada, a coluna obtida pareada (`null` = sem par)
    pub mapping: Vec<Option<usize>>,
    /// Linhas esperadas que o obtido não tem (índices no esperado)
    pub missing_rows: Vec<usize>,
    /// Linhas obtidas que o esperado não tem (índices no obtido)
    pub extra_rows: Vec<usize>,
}

/// Uma célula normalizada pelas regras, comparável por igualdade.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Cell {
    Null,
    Number(String),
    Text(String),
}

fn cell(value: Option<&str>, rules: &Rules) -> Cell {
    let Some(text) = value else {
        return Cell::Null;
    };
    if text.is_empty() && rules.null_equals_empty {
        return Cell::Null;
    }
    match number(text, rules.decimals as usize) {
        Some(number) => Cell::Number(number),
        None => Cell::Text(text.to_string()),
    }
}

/// O número na forma canônica com `decimals` casas, ou `None` se o texto não
/// é um número decimal simples (`1e5`, `inf` e `0x10` ficam como texto).
fn number(text: &str, decimals: usize) -> Option<String> {
    let text = text.trim();
    let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
    let negative = text.starts_with('-');
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    let all_digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
    if (whole.is_empty() && fraction.is_empty()) || !all_digits(whole) || !all_digits(fraction) {
        return None;
    }

    let canonical = if fraction.bytes().all(|byte| byte == b'0') {
        // Inteiro: os dígitos como estão (um `bigint` não cabe num f64)
        let whole = whole.trim_start_matches('0');
        let whole = if whole.is_empty() { "0" } else { whole };
        format!("{whole}.{}", "0".repeat(decimals))
    } else {
        let value: f64 = digits.parse().ok()?;
        format!("{value:.decimals$}")
    };
    let zero = canonical.bytes().all(|byte| byte == b'0' || byte == b'.');
    Some(if negative && !zero { format!("-{canonical}") } else { canonical })
}

type Grid = Vec<Vec<Cell>>;

fn grid(table: &Table, rules: &Rules) -> Grid {
    table
        .rows
        .iter()
        .map(|row| row.iter().map(|value| cell(value.as_deref(), rules)).collect())
        .collect()
}

fn column(grid: &Grid, index: usize) -> Vec<&Cell> {
    grid.iter().filter_map(|row| row.get(index)).collect()
}

/// As linhas obtidas na ordem das colunas esperadas.
fn project<'a>(grid: &'a Grid, mapping: &[usize]) -> Vec<Vec<&'a Cell>> {
    static NULL: Cell = Cell::Null;
    grid.iter()
        .map(|row| mapping.iter().map(|&index| row.get(index).unwrap_or(&NULL)).collect())
        .collect()
}

/// Compara o obtido com o esperado pelas regras.
pub fn compare(expected: &Table, obtained: &Table, rules: &Rules) -> Comparison {
    let want = grid(expected, rules);
    let got = grid(obtained, rules);
    let (want_width, got_width) = (expected.columns.len(), obtained.columns.len());
    let subset = obtained.truncated && rules.truncated == Truncated::Subset;

    // O que impede a igualdade antes de olhar as linhas
    let blocked = if obtained.truncated && rules.truncated == Truncated::Fail {
        Some(Mismatch::Truncated)
    } else if got_width < want_width || (got_width > want_width && rules.extra_columns == ExtraColumns::Forbid) {
        Some(Mismatch::ColumnCount)
    } else if !subset && got.len() != want.len() {
        Some(Mismatch::RowCount)
    } else {
        None
    };

    let by_name = names_mapping(expected, obtained);
    if blocked.is_none() {
        let found = match rules.columns {
            ColumnMatch::Position => Some((0..want_width).collect::<Vec<_>>()),
            ColumnMatch::Name => by_name.clone(),
            ColumnMatch::Any => search(&want, &got, want_width, got_width, rules, subset),
        };
        if let Some(mapping) = &found {
            if rows_match(&want, &project(&got, mapping), rules.order, subset) {
                return Comparison {
                    equal: true,
                    mismatch: None,
                    mapping: mapping.iter().copied().map(Some).collect(),
                    missing_rows: Vec::new(),
                    extra_rows: Vec::new(),
                };
            }
        }
    }

    // Não bate: o melhor pareamento para a tela marcar as diferenças (pelo
    // nome se todos os nomes vieram, senão pela posição)
    let display: Vec<Option<usize>> = match (&by_name, rules.columns) {
        (Some(mapping), ColumnMatch::Any | ColumnMatch::Name) => mapping.iter().copied().map(Some).collect(),
        _ => (0..want_width).map(|index| (index < got_width).then_some(index)).collect(),
    };
    let complete: Vec<usize> = display.iter().map(|index| index.unwrap_or(usize::MAX)).collect();
    let projected = project(&got, &complete);
    let (missing_rows, extra_rows) = match rules.order {
        RowOrder::Strict => positional_diff(&want, &projected),
        RowOrder::Ignore => multiset_diff(&want, &projected),
    };

    let mismatch = blocked.unwrap_or_else(|| {
        if rules.columns == ColumnMatch::Name && by_name.is_none() {
            Mismatch::ColumnNames
        } else if subset {
            Mismatch::Truncated
        } else if rules.order == RowOrder::Strict && rows_match(&want, &projected, RowOrder::Ignore, false) {
            Mismatch::Order
        } else {
            Mismatch::Values
        }
    });
    Comparison {
        equal: false,
        mismatch: Some(mismatch),
        mapping: display,
        missing_rows,
        extra_rows,
    }
}

/// Cada coluna esperada na coluna obtida de mesmo nome (sem diferenciar
/// maiúsculas, a primeira ainda livre); `None` se algum nome não veio.
fn names_mapping(expected: &Table, obtained: &Table) -> Option<Vec<usize>> {
    let mut used = vec![false; obtained.columns.len()];
    expected
        .columns
        .iter()
        .map(|name| {
            let index = obtained
                .columns
                .iter()
                .enumerate()
                .position(|(index, other)| !used[index] && other.eq_ignore_ascii_case(name))?;
            used[index] = true;
            Some(index)
        })
        .collect()
}

fn rows_match<'a>(want: &'a Grid, got: &[Vec<&'a Cell>], order: RowOrder, subset: bool) -> bool {
    let want: Vec<Vec<&Cell>> = want.iter().map(|row| row.iter().collect()).collect();
    match (order, subset) {
        (RowOrder::Strict, false) => want == got,
        (RowOrder::Strict, true) => got.len() <= want.len() && want[..got.len()] == *got,
        (RowOrder::Ignore, false) => {
            let (mut want, mut got) = (want, got.to_vec());
            want.sort();
            got.sort();
            want == got
        }
        (RowOrder::Ignore, true) => {
            let mut left: HashMap<&Vec<&Cell>, usize> = HashMap::new();
            for row in &want {
                *left.entry(row).or_default() += 1;
            }
            got.iter().all(|row| match left.get_mut(row) {
                Some(count) if *count > 0 => {
                    *count -= 1;
                    true
                }
                _ => false,
            })
        }
    }
}

/// Procura um pareamento de colunas em que as linhas batem (`columns = any`).
/// Uma coluna obtida só é candidata de uma esperada com o mesmo conteúdo.
fn search(want: &Grid, got: &Grid, want_width: usize, got_width: usize, rules: &Rules, subset: bool) -> Option<Vec<usize>> {
    let got_columns: Vec<Vec<&Cell>> = (0..got_width).map(|index| signature(got, index, rules.order)).collect();
    let candidates: Vec<Vec<usize>> = (0..want_width)
        .map(|index| {
            let wanted = signature(want, index, rules.order);
            (0..got_width)
                // Com o obtido cortado o conteúdo é só uma parte: sem filtro
                .filter(|&other| subset || got_columns[other] == wanted)
                .collect()
        })
        .collect();

    let mut mapping = Vec::with_capacity(want_width);
    let mut used = vec![false; got_width];
    let mut tried = 0;
    assign(want, got, &candidates, rules.order, subset, &mut mapping, &mut used, &mut tried).then_some(mapping)
}

/// O conteúdo de uma coluna: na ordem das linhas, ou ordenado quando a ordem
/// não conta.
fn signature(grid: &Grid, index: usize, order: RowOrder) -> Vec<&Cell> {
    let mut cells = column(grid, index);
    if order == RowOrder::Ignore {
        cells.sort();
    }
    cells
}

#[allow(clippy::too_many_arguments)]
fn assign(
    want: &Grid,
    got: &Grid,
    candidates: &[Vec<usize>],
    order: RowOrder,
    subset: bool,
    mapping: &mut Vec<usize>,
    used: &mut [bool],
    tried: &mut usize,
) -> bool {
    if mapping.len() == candidates.len() {
        *tried += 1;
        return rows_match(want, &project(got, mapping), order, subset);
    }
    for &index in &candidates[mapping.len()] {
        if used[index] || *tried >= MAX_MAPPINGS {
            continue;
        }
        used[index] = true;
        mapping.push(index);
        if assign(want, got, candidates, order, subset, mapping, used, tried) {
            return true;
        }
        mapping.pop();
        used[index] = false;
    }
    false
}

/// Linhas sem par, contando repetições: (do esperado, do obtido).
fn multiset_diff<'a>(want: &'a Grid, got: &[Vec<&'a Cell>]) -> (Vec<usize>, Vec<usize>) {
    let mut free: HashMap<&Vec<&Cell>, Vec<usize>> = HashMap::new();
    for (index, row) in got.iter().enumerate().rev() {
        free.entry(row).or_default().push(index);
    }
    let mut missing = Vec::new();
    for (index, row) in want.iter().enumerate() {
        let row: Vec<&Cell> = row.iter().collect();
        if free.get_mut(&row).and_then(Vec::pop).is_none() {
            missing.push(index);
        }
    }
    let mut extra: Vec<usize> = free.into_values().flatten().collect();
    extra.sort_unstable();
    (missing, extra)
}

/// Linhas diferentes na mesma posição, e as que sobram de um lado.
fn positional_diff(want: &Grid, got: &[Vec<&Cell>]) -> (Vec<usize>, Vec<usize>) {
    let differs = |index: usize| match (want.get(index), got.get(index)) {
        (Some(want), Some(got)) => want.iter().collect::<Vec<_>>() != *got,
        _ => true,
    };
    let missing = (0..want.len()).filter(|&index| differs(index)).collect();
    let extra = (0..got.len()).filter(|&index| differs(index)).collect();
    (missing, extra)
}

/// A consulta gerada é a de referência? Só no PostgreSQL. Compara a sequência
/// de tokens, sem contar espaços, comentários, o `;` do fim e a caixa das
/// palavras-chave e dos identificadores sem aspas. `None` se uma das duas não
/// passa pelo tokenizador.
pub fn exact_match(generated: &str, reference: &str) -> Option<bool> {
    Some(tokens(generated)? == tokens(reference)?)
}

fn tokens(sql: &str) -> Option<Vec<String>> {
    let mut tokens: Vec<String> = Tokenizer::new(&PostgreSqlDialect {}, sql)
        .tokenize()
        .ok()?
        .into_iter()
        .filter_map(|token| match token {
            Token::Whitespace(_) | Token::EOF => None,
            Token::Word(word) if word.quote_style.is_none() => Some(word.value.to_lowercase()),
            other => Some(other.to_string()),
        })
        .collect();
    while tokens.last().is_some_and(|token| token == ";") {
        tokens.pop();
    }
    Some(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(columns: &[&str], rows: &[&[Option<&str>]]) -> Table {
        Table {
            columns: columns.iter().map(|name| name.to_string()).collect(),
            rows: rows
                .iter()
                .map(|row| row.iter().map(|value| value.map(str::to_string)).collect())
                .collect(),
            truncated: false,
        }
    }

    fn rules(patch: RulesPatch) -> Rules {
        Rules::default().with(Some(&patch))
    }

    #[test]
    fn row_order_only_matters_when_strict() {
        let expected = table(&["nome"], &[&[Some("ana")], &[Some("bia")], &[Some("ana")]]);
        let shuffled = table(&["nome"], &[&[Some("bia")], &[Some("ana")], &[Some("ana")]]);
        assert!(compare(&expected, &shuffled, &Rules::default()).equal);

        let strict = rules(RulesPatch {
            order: Some(RowOrder::Strict),
            ..Default::default()
        });
        let verdict = compare(&expected, &shuffled, &strict);
        assert_eq!((verdict.equal, verdict.mismatch), (false, Some(Mismatch::Order)));
        assert_eq!((verdict.missing_rows, verdict.extra_rows), (vec![0, 1], vec![0, 1]));
        assert!(compare(&expected, &expected, &strict).equal);

        // Multiconjunto: uma repetição a menos não é igual
        let fewer = table(&["nome"], &[&[Some("ana")], &[Some("bia")], &[Some("bia")]]);
        let verdict = compare(&expected, &fewer, &Rules::default());
        assert_eq!(verdict.mismatch, Some(Mismatch::Values));
        assert_eq!((verdict.missing_rows, verdict.extra_rows), (vec![2], vec![2]));
    }

    #[test]
    fn columns_pair_by_content_position_or_name() {
        let expected = table(&["cidade", "total"], &[&[Some("Poa"), Some("3")], &[Some("Rio"), Some("7")]]);
        let swapped = table(&["count", "city"], &[&[Some("3"), Some("Poa")], &[Some("7"), Some("Rio")]]);

        let verdict = compare(&expected, &swapped, &Rules::default());
        assert!(verdict.equal);
        assert_eq!(verdict.mapping, [Some(1), Some(0)]);

        let position = rules(RulesPatch {
            columns: Some(ColumnMatch::Position),
            ..Default::default()
        });
        assert_eq!(compare(&expected, &swapped, &position).mismatch, Some(Mismatch::Values));

        let name = rules(RulesPatch {
            columns: Some(ColumnMatch::Name),
            ..Default::default()
        });
        assert_eq!(compare(&expected, &swapped, &name).mismatch, Some(Mismatch::ColumnNames));
        let renamed = table(&["TOTAL", "Cidade"], &[&[Some("3"), Some("Poa")], &[Some("7"), Some("Rio")]]);
        assert!(compare(&expected, &renamed, &name).equal);
    }

    /// Colunas com o mesmo conteúdo em outra ordem de linhas: o pareamento
    /// vale pela linha inteira, não coluna a coluna.
    #[test]
    fn any_columns_checks_whole_rows() {
        let expected = table(&["a", "b"], &[&[Some("1"), Some("2")], &[Some("2"), Some("1")]]);
        let same = table(&["x", "y"], &[&[Some("2"), Some("1")], &[Some("1"), Some("2")]]);
        assert!(compare(&expected, &same, &Rules::default()).equal);
        let other = table(&["x", "y"], &[&[Some("1"), Some("1")], &[Some("2"), Some("2")]]);
        assert!(!compare(&expected, &other, &Rules::default()).equal);
    }

    #[test]
    fn extra_columns_are_forbidden_unless_allowed() {
        let expected = table(&["nome"], &[&[Some("ana")], &[Some("bia")]]);
        let wider = table(&["id", "nome"], &[&[Some("1"), Some("ana")], &[Some("2"), Some("bia")]]);
        assert_eq!(compare(&expected, &wider, &Rules::default()).mismatch, Some(Mismatch::ColumnCount));

        let allow = rules(RulesPatch {
            extra_columns: Some(ExtraColumns::Allow),
            ..Default::default()
        });
        let verdict = compare(&expected, &wider, &allow);
        assert!(verdict.equal);
        assert_eq!(verdict.mapping, [Some(1)]);

        // Faltar coluna reprova sempre
        assert_eq!(compare(&wider, &expected, &allow).mismatch, Some(Mismatch::ColumnCount));
    }

    #[test]
    fn numbers_are_rounded_and_text_is_not() {
        let expected = table(&["media", "total", "cep"], &[&[Some("10.5"), Some("3"), Some("01000-000")]]);
        let close = table(&["avg", "count", "zip"], &[&[Some("10.50004"), Some("3.0"), Some("01000-000")]]);
        assert!(compare(&expected, &close, &Rules::default()).equal);

        let far = table(&["avg", "count", "zip"], &[&[Some("10.51"), Some("3"), Some("01000-000")]]);
        assert!(!compare(&expected, &far, &Rules::default()).equal);
        let coarse = rules(RulesPatch {
            decimals: Some(1),
            ..Default::default()
        });
        assert!(compare(&expected, &far, &coarse).equal);

        assert_eq!(number("-0.00001", 4).as_deref(), Some("0.0000"));
        assert_eq!(number("007", 2).as_deref(), Some("7.00"));
        assert_eq!(number("+2.50", 2).as_deref(), Some("2.50"));
        // Um bigint além do f64 não vira outro número
        assert_ne!(number("9007199254740993", 0), number("9007199254740992", 0));
        for text in ["1e5", "inf", "NaN", "0x10", "", ".", "1.2.3", "12 kg"] {
            assert_eq!(number(text, 2), None, "{text}");
        }
    }

    #[test]
    fn null_only_equals_empty_when_asked() {
        let expected = table(&["apelido"], &[&[None], &[Some("bia")]]);
        let empty = table(&["apelido"], &[&[Some("")], &[Some("bia")]]);
        assert!(!compare(&expected, &empty, &Rules::default()).equal);
        assert!(compare(&expected, &expected, &Rules::default()).equal);

        let lenient = rules(RulesPatch {
            null_equals_empty: Some(true),
            ..Default::default()
        });
        assert!(compare(&expected, &empty, &lenient).equal);
    }

    #[test]
    fn incomplete_results_fail_or_count_as_subset() {
        let expected = table(&["n"], &[&[Some("1")], &[Some("2")], &[Some("3")]]);
        let mut page = table(&["n"], &[&[Some("1")], &[Some("2")]]);

        // Menos linhas, sem corte: reprova nas duas regras
        assert_eq!(compare(&expected, &page, &Rules::default()).mismatch, Some(Mismatch::RowCount));

        page.truncated = true;
        assert_eq!(compare(&expected, &page, &Rules::default()).mismatch, Some(Mismatch::Truncated));

        let subset = rules(RulesPatch {
            truncated: Some(Truncated::Subset),
            ..Default::default()
        });
        assert!(compare(&expected, &page, &subset).equal);

        let mut stranger = table(&["n"], &[&[Some("1")], &[Some("9")]]);
        stranger.truncated = true;
        let verdict = compare(&expected, &stranger, &subset);
        assert_eq!((verdict.equal, verdict.mismatch), (false, Some(Mismatch::Truncated)));
        assert_eq!(verdict.extra_rows, [1]);

        // Com ordem estrita, o cortado precisa ser um prefixo
        let strict_subset = rules(RulesPatch {
            truncated: Some(Truncated::Subset),
            order: Some(RowOrder::Strict),
            ..Default::default()
        });
        assert!(compare(&expected, &page, &strict_subset).equal);
        let mut tail = table(&["n"], &[&[Some("2")], &[Some("3")]]);
        tail.truncated = true;
        assert!(!compare(&expected, &tail, &strict_subset).equal);
    }

    #[test]
    fn empty_results_match_each_other() {
        let expected = table(&["n"], &[]);
        assert!(compare(&expected, &table(&["total"], &[]), &Rules::default()).equal);
        assert_eq!(
            compare(&expected, &table(&["n"], &[&[Some("1")]]), &Rules::default()).mismatch,
            Some(Mismatch::RowCount)
        );
    }

    #[test]
    fn patches_override_only_what_they_set() {
        let file = RulesPatch {
            decimals: Some(2),
            order: Some(RowOrder::Strict),
            ..Default::default()
        };
        let question = RulesPatch {
            order: Some(RowOrder::Ignore),
            ..Default::default()
        };
        let merged = Rules::default().with(Some(&file)).with(Some(&question)).with(None);
        assert_eq!((merged.decimals, merged.order), (2, RowOrder::Ignore));
        assert_eq!(merged.columns, ColumnMatch::Any);
    }

    #[test]
    fn exact_match_ignores_layout_case_and_comments() {
        let reference = "SELECT name FROM customers WHERE city = 'Poa' ORDER BY name";
        let generated = "select Name\nfrom customers -- todos\nwhere city = 'Poa'\norder by name;";
        assert_eq!(exact_match(generated, reference), Some(true));

        // Literal e identificador entre aspas guardam a caixa
        assert_eq!(exact_match("SELECT name FROM customers WHERE city = 'poa' ORDER BY name", reference), Some(false));
        assert_eq!(exact_match("SELECT \"Name\" FROM t", "SELECT name FROM t"), Some(false));
        assert_eq!(exact_match("SELECT count(*) FROM t", "SELECT count(1) FROM t"), Some(false));
        assert_eq!(exact_match("SELECT 'aberta", reference), None);
    }
}
