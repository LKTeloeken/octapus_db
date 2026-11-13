//! A saída do `octapus`: o mesmo dado em tabela (para ler), JSON, CSV ou
//! NDJSON (para outro programa). O formato de cada um é estável — mudar é
//! quebrar script de alguém.
//!
//! - `json`: listas viram um array de objetos; o resultado de uma consulta vira
//!   `{ columns, rows, rowCount, hasMore, … }`, com as linhas como arrays (dois
//!   campos podem ter o mesmo nome);
//! - `ndjson`: um objeto por linha, nome da coluna → valor;
//! - `csv`: cabeçalho e linhas, `NULL` como campo vazio;
//! - toda célula é `string | null`, como no app.

use std::io::{self, IsTerminal, Write};

use serde_json::{json, Map, Value};
use unicode_width::UnicodeWidthStr;

use crate::cli::Format;
use crate::error::CliResult;

/// Largura máxima de uma célula na tabela (o resto vira `…`).
const MAX_CELL_WIDTH: usize = 60;

/// Colunas e linhas, o que toda saída tabular tem.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Grid {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
}

impl Grid {
    pub fn new<const N: usize>(columns: [&str; N]) -> Self {
        Self {
            columns: columns.iter().map(|name| name.to_string()).collect(),
            rows: Vec::new(),
        }
    }

    pub fn push<const N: usize>(&mut self, row: [Option<String>; N]) {
        self.rows.push(row.into());
    }
}

/// O resultado de uma consulta: a grade e o que se sabe dela.
#[derive(Debug, Default)]
pub struct QueryOutput {
    pub grid: Grid,
    pub types: Vec<String>,
    pub has_more: bool,
    pub total_count: Option<i64>,
    pub elapsed_ms: u64,
}

pub struct Output {
    pub format: Format,
    pub color: bool,
}

impl Output {
    /// `table` num terminal, `json` fora dele; cor só no terminal, sem
    /// `NO_COLOR` e sem `--no-color`.
    pub fn new(format: Option<Format>, no_color: bool) -> Self {
        let tty = io::stdout().is_terminal();
        Self {
            format: format.unwrap_or(if tty { Format::Table } else { Format::Json }),
            color: tty && !no_color && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty()),
        }
    }

    /// Uma lista (servidores, tabelas, colunas): a grade para ler e, para
    /// programa, os mesmos itens como objetos (camelCase, como no app).
    pub fn list(&self, grid: &Grid, items: &[Value]) -> CliResult<()> {
        let mut out = io::stdout().lock();
        match self.format {
            Format::Table => write_table(&mut out, grid, self.color)?,
            Format::Csv => write_csv(&mut out, grid)?,
            Format::Json => writeln!(out, "{}", serde_json::to_string_pretty(items).unwrap_or_default())?,
            Format::Ndjson => {
                for item in items {
                    writeln!(out, "{item}")?;
                }
            }
        }
        out.flush()?;
        Ok(())
    }

    /// O resultado de uma consulta.
    pub fn query(&self, result: &QueryOutput) -> CliResult<()> {
        let mut out = io::stdout().lock();
        let grid = &result.grid;
        match self.format {
            Format::Table => {
                write_table(&mut out, grid, self.color)?;
                // O rodapé vai para o stderr: o stdout fica só com o dado
                let more = if result.has_more { " (há mais; use --limit/--offset)" } else { "" };
                eprintln!("{} linha(s) · {} ms{more}", grid.rows.len(), result.elapsed_ms);
            }
            Format::Csv => write_csv(&mut out, grid)?,
            Format::Json => {
                let columns: Vec<Value> = grid
                    .columns
                    .iter()
                    .zip(result.types.iter().map(Some).chain(std::iter::repeat(None)))
                    .map(|(name, type_name)| json!({ "name": name, "type": type_name }))
                    .collect();
                let document = json!({
                    "columns": columns,
                    "rows": grid.rows,
                    "rowCount": grid.rows.len(),
                    "hasMore": result.has_more,
                    "totalCount": result.total_count,
                    "executionTimeMs": result.elapsed_ms,
                });
                writeln!(out, "{}", serde_json::to_string_pretty(&document).unwrap_or_default())?;
            }
            Format::Ndjson => {
                let keys: Vec<&str> = grid.columns.iter().map(String::as_str).collect();
                for row in &grid.rows {
                    writeln!(out, "{}", object(&keys, row))?;
                }
            }
        }
        out.flush()?;
        Ok(())
    }

    /// Um objeto só (`server show`, `paths`): em tabela, uma linha por campo
    /// (rótulo, valor); para programa, o objeto.
    pub fn record(&self, fields: &[(&str, Option<String>)], value: &Value) -> CliResult<()> {
        let mut out = io::stdout().lock();
        match self.format {
            Format::Table => {
                let width = fields.iter().map(|(label, _)| label.width()).max().unwrap_or(0);
                for (label, field) in fields {
                    let pad = " ".repeat(width - label.width());
                    writeln!(out, "{label}{pad}  {}", field.as_deref().unwrap_or("—"))?;
                }
            }
            Format::Csv => {
                let grid = Grid {
                    columns: fields.iter().map(|(label, _)| label.to_string()).collect(),
                    rows: vec![fields.iter().map(|(_, field)| field.clone()).collect()],
                };
                write_csv(&mut out, &grid)?;
            }
            Format::Json => writeln!(out, "{}", serde_json::to_string_pretty(value).unwrap_or_default())?,
            Format::Ndjson => writeln!(out, "{value}")?,
        }
        out.flush()?;
        Ok(())
    }

    /// Uma confirmação curta ("excluído", "ok"): texto para gente, objeto para
    /// programa.
    pub fn done(&self, message: &str, extra: Value) -> CliResult<()> {
        let mut out = io::stdout().lock();
        match self.format {
            Format::Table | Format::Csv => writeln!(out, "{message}")?,
            Format::Json | Format::Ndjson => {
                let mut value = json!({ "ok": true });
                if let (Some(target), Value::Object(extra)) = (value.as_object_mut(), extra) {
                    target.extend(extra);
                }
                writeln!(out, "{value}")?;
            }
        }
        out.flush()?;
        Ok(())
    }
}

/// Uma linha de resultado como objeto: toda célula é `string | null`.
fn object(keys: &[&str], row: &[Option<String>]) -> Value {
    let mut map = Map::with_capacity(keys.len());
    for (key, value) in keys.iter().zip(row) {
        map.insert(key.to_string(), value.clone().map_or(Value::Null, Value::String));
    }
    Value::Object(map)
}

fn write_csv(out: &mut impl Write, grid: &Grid) -> io::Result<()> {
    let line = |cells: Vec<String>| cells.join(",");
    writeln!(out, "{}", line(grid.columns.iter().map(|name| csv_field(name)).collect()))?;
    for row in &grid.rows {
        writeln!(
            out,
            "{}",
            line(row.iter().map(|cell| cell.as_deref().map(csv_field).unwrap_or_default()).collect())
        )?;
    }
    Ok(())
}

/// Aspas só quando preciso (RFC 4180): vírgula, aspas, quebra de linha ou
/// espaço nas pontas.
fn csv_field(value: &str) -> String {
    let needs_quotes = value.contains([',', '"', '\n', '\r']) || value.starts_with(' ') || value.ends_with(' ');
    if needs_quotes {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// A célula como aparece na tabela: uma linha só, até [`MAX_CELL_WIDTH`].
fn table_cell(value: &str) -> String {
    let mut out = String::new();
    let mut width = 0;
    for ch in value.chars() {
        let ch = match ch {
            '\n' => '⏎',
            '\r' | '\t' => ' ',
            other => other,
        };
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width > MAX_CELL_WIDTH - 1 && value.width() > MAX_CELL_WIDTH {
            out.push('…');
            return out;
        }
        width += ch_width;
        out.push(ch);
    }
    out
}

fn write_table(out: &mut impl Write, grid: &Grid, color: bool) -> io::Result<()> {
    let (bold, dim, reset) = if color { ("\x1b[1m", "\x1b[2m", "\x1b[0m") } else { ("", "", "") };

    let cells: Vec<Vec<Option<String>>> = grid
        .rows
        .iter()
        .map(|row| row.iter().map(|cell| cell.as_deref().map(table_cell)).collect())
        .collect();
    let mut widths: Vec<usize> = grid.columns.iter().map(|name| name.width()).collect();
    for row in &cells {
        for (index, cell) in row.iter().enumerate() {
            let width = cell.as_deref().map_or(4, UnicodeWidthStr::width);
            if let Some(slot) = widths.get_mut(index) {
                *slot = (*slot).max(width);
            }
        }
    }

    let last = grid.columns.len().saturating_sub(1);
    let mut line = String::new();
    for (index, name) in grid.columns.iter().enumerate() {
        line.push_str(name);
        if index < last {
            line.push_str(&" ".repeat(widths[index] - name.width() + 2));
        }
    }
    writeln!(out, "{bold}{line}{reset}")?;

    for row in &cells {
        let mut line = String::new();
        for (index, cell) in row.iter().enumerate() {
            let (text, is_null) = match cell {
                Some(text) => (text.as_str(), false),
                None => ("NULL", true),
            };
            if is_null {
                line.push_str(dim);
            }
            line.push_str(text);
            if is_null {
                line.push_str(reset);
            }
            if index < last {
                let width = widths.get(index).copied().unwrap_or(0);
                line.push_str(&" ".repeat(width.saturating_sub(text.width()) + 2));
            }
        }
        writeln!(out, "{line}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Grid {
        let mut grid = Grid::new(["id", "name"]);
        grid.push([Some("1".into()), Some("Ana, a \"primeira\"".into())]);
        grid.push([Some("2".into()), None]);
        grid
    }

    #[test]
    fn csv_quotes_only_when_needed_and_null_is_empty() {
        let mut out = Vec::new();
        write_csv(&mut out, &sample()).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "id,name\n1,\"Ana, a \"\"primeira\"\"\"\n2,\n");
    }

    #[test]
    fn table_aligns_columns_and_marks_null() {
        let mut out = Vec::new();
        write_table(&mut out, &sample(), false).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "id  name\n1   Ana, a \"primeira\"\n2   NULL\n");
    }

    #[test]
    fn long_and_multiline_cells_stay_on_one_line() {
        assert_eq!(table_cell("a\nb"), "a⏎b");
        let long = "x".repeat(200);
        let cell = table_cell(&long);
        assert_eq!(cell.width(), MAX_CELL_WIDTH);
        assert!(cell.ends_with('…'));
    }

    #[test]
    fn rows_become_objects_with_string_or_null_values() {
        let grid = sample();
        assert_eq!(object(&["id", "name"], &grid.rows[1]), json!({ "id": "2", "name": null }));
    }
}
