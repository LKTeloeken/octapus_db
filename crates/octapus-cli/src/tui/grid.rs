//! A grade de resultados da TUI. Só as linhas e colunas que cabem na tela são
//! desenhadas, e as páginas seguintes chegam quando o cursor se aproxima do
//! fim — um resultado enorme custa o mesmo que um pequeno.
//!
//! A edição segue o modelo do app: nada vai ao banco na hora. As alterações
//! ficam **pendentes** (verde = linha nova, vermelho = removida, amarelo =
//! célula editada) e só são gravadas, numa transação, com o salvar.

use std::collections::{BTreeMap, BTreeSet};

use octapus_core::models::{EditableInfo, QueryResult, RowChanges, RowEdit, RowInsert};
use unicode_width::UnicodeWidthStr;

use super::config::{CellKind, Pending};

/// Largura máxima de uma coluna (o resto da célula aparece com `Enter`).
const MAX_COLUMN_WIDTH: usize = 40;
/// Largura mínima: cabe um `NULL`.
const MIN_COLUMN_WIDTH: usize = 4;
/// O que cada coluna gasta além do conteúdo: ` valor │`.
pub const COLUMN_CHROME: usize = 3;
/// Linhas olhadas para decidir a largura das colunas.
const WIDTH_SAMPLE: usize = 200;
/// Teto de linhas em memória: depois disto, as páginas param de chegar.
pub const MAX_ROWS: usize = 50_000;
/// A que distância do fim o cursor pede a próxima página.
const LOAD_AHEAD: usize = 50;

#[derive(Debug, Default)]
pub struct Grid {
    pub columns: Vec<String>,
    pub types: Vec<String>,
    /// A família do tipo de cada coluna (cor e alinhamento)
    kinds: Vec<CellKind>,
    pub rows: Vec<Vec<Option<String>>>,
    pub has_more: bool,
    pub editable: Option<EditableInfo>,
    pub elapsed_ms: u64,
    widths: Vec<usize>,

    pub row: usize,
    pub col: usize,
    pub scroll_row: usize,
    pub scroll_col: usize,

    /// Células editadas: (linha, coluna) → valor novo
    edits: BTreeMap<(usize, usize), Option<String>>,
    /// Linhas marcadas para excluir
    deleted: BTreeSet<usize>,
    /// A partir desta linha, são linhas novas (ainda não existem no banco)
    first_new_row: usize,
}

impl Grid {
    pub fn from_result(result: QueryResult) -> Self {
        let mut grid = Self {
            types: result.columns.iter().map(|column| column.type_name.clone()).collect(),
            kinds: result.columns.iter().map(|column| CellKind::of(&column.type_name)).collect(),
            columns: result.columns.into_iter().map(|column| column.name).collect(),
            first_new_row: result.rows.len(),
            rows: result.rows,
            has_more: result.has_more,
            editable: result.editable_info,
            elapsed_ms: result.execution_time_ms,
            ..Self::default()
        };
        grid.measure();
        grid
    }

    /// Junta a página seguinte. Com linhas novas pendentes no fim, a página
    /// entra antes delas.
    pub fn append(&mut self, result: QueryResult) {
        self.has_more = result.has_more && self.first_new_row + result.rows.len() < MAX_ROWS;
        let added = result.rows.len();
        let tail = self.rows.split_off(self.first_new_row);
        self.rows.extend(result.rows);
        self.rows.extend(tail);

        // As pendências das linhas novas acompanham o deslocamento
        let first_new_row = self.first_new_row;
        let shift = |row: usize| if row >= first_new_row { row + added } else { row };
        self.edits = std::mem::take(&mut self.edits)
            .into_iter()
            .map(|((row, col), value)| ((shift(row), col), value))
            .collect();
        self.deleted = self.deleted.iter().map(|row| shift(*row)).collect();
        if self.row >= self.first_new_row {
            self.row += added;
        }
        self.first_new_row += added;
    }

    fn measure(&mut self) {
        self.widths = self
            .columns
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let kind = self.kind_of(index);
                let cells = self.rows.iter().take(WIDTH_SAMPLE).map(|row| {
                    row.get(index)
                        .and_then(Option::as_deref)
                        .map_or(4, |value| first_line_width(shown(kind, value)))
                });
                // O cabeçalho tem o nome e, embaixo, o tipo
                let header = name.width().max(self.types.get(index).map_or(0, |name| name.width()));
                cells.fold(header, usize::max).clamp(MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH)
            })
            .collect();
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    pub fn kind_of(&self, col: usize) -> CellKind {
        self.kinds.get(col).copied().unwrap_or(CellKind::Text)
    }

    /// Largura da coluna do número da linha: o marcador da pendência, os
    /// dígitos e um espaço.
    pub fn gutter_width(&self) -> usize {
        self.rows.len().max(1).to_string().len() + 2
    }

    /// O marcador da linha na coluna dos números: nova, removida ou com
    /// alguma célula editada.
    pub fn row_marker(&self, row: usize) -> Option<Pending> {
        self.row_pending(row)
            .or_else(|| self.edits.range((row, 0)..(row + 1, 0)).next().map(|_| Pending::Edited))
    }

    pub fn width_of(&self, col: usize) -> usize {
        self.widths.get(col).copied().unwrap_or(MIN_COLUMN_WIDTH)
    }

    /// O valor que a célula mostra: o editado, se houver, senão o do banco.
    pub fn cell(&self, row: usize, col: usize) -> Option<&str> {
        match self.edits.get(&(row, col)) {
            Some(value) => value.as_deref(),
            None => self.rows.get(row)?.get(col)?.as_deref(),
        }
    }

    pub fn row_pending(&self, row: usize) -> Option<Pending> {
        if self.deleted.contains(&row) {
            Some(Pending::Removed)
        } else if row >= self.first_new_row {
            Some(Pending::Added)
        } else {
            None
        }
    }

    pub fn cell_pending(&self, row: usize, col: usize) -> Option<Pending> {
        self.row_pending(row)
            .or_else(|| self.edits.contains_key(&(row, col)).then_some(Pending::Edited))
    }

    pub fn pending_count(&self) -> usize {
        let edited_rows: BTreeSet<usize> = self
            .edits
            .keys()
            .map(|(row, _)| *row)
            .filter(|row| *row < self.first_new_row && !self.deleted.contains(row))
            .collect();
        edited_rows.len() + self.deleted.len() + (self.rows.len() - self.first_new_row)
    }

    /// Por que a grade não aceita edição, se não aceita.
    pub fn read_only_reason(&self) -> Option<&'static str> {
        match &self.editable {
            None => Some("este resultado não é editável (abra a tabela pela árvore)"),
            Some(info) if info.primary_key_columns.is_empty() => Some("a tabela não tem chave primária"),
            Some(_) => None,
        }
    }

    pub fn set_cell(&mut self, row: usize, col: usize, value: Option<String>) {
        if row >= self.rows.len() || col >= self.columns.len() || self.deleted.contains(&row) {
            return;
        }
        // A coluna cresce para o valor novo caber
        if let (Some(width), Some(value)) = (self.widths.get_mut(col), value.as_deref()) {
            *width = (*width).max(first_line_width(value)).min(MAX_COLUMN_WIDTH);
        }
        if row >= self.first_new_row {
            // Linha nova: o valor é o próprio conteúdo dela
            self.rows[row][col] = value;
            return;
        }
        // Voltou ao valor do banco: deixa de ser pendência
        if self.rows[row][col] == value {
            self.edits.remove(&(row, col));
        } else {
            self.edits.insert((row, col), value);
        }
    }

    /// Marca (ou desmarca) a linha para excluir. Uma linha nova some na hora.
    pub fn toggle_delete(&mut self, row: usize) {
        if row >= self.rows.len() {
            return;
        }
        if row >= self.first_new_row {
            self.rows.remove(row);
            self.row = self.row.min(self.rows.len().saturating_sub(1));
            return;
        }
        if !self.deleted.remove(&row) {
            self.deleted.insert(row);
        }
    }

    /// Acrescenta uma linha vazia no fim e leva o cursor a ela.
    pub fn add_row(&mut self) {
        self.rows.push(vec![None; self.columns.len()]);
        self.row = self.rows.len() - 1;
    }

    /// Desfaz as pendências da linha.
    pub fn undo_row(&mut self, row: usize) {
        if row >= self.first_new_row {
            self.toggle_delete(row);
            return;
        }
        self.deleted.remove(&row);
        self.edits.retain(|(edited, _), _| *edited != row);
    }

    /// As pendências no formato que o adapter grava (`save_row_changes`):
    /// exclusões e edições pela chave primária **original** da linha.
    pub fn changes(&self) -> Option<(EditableInfo, RowChanges)> {
        let info = self.editable.clone()?;
        let pk_of = |row: usize| -> Vec<Option<String>> {
            info.primary_key_column_indices
                .iter()
                .map(|index| self.rows[row].get(*index).cloned().flatten())
                .collect()
        };

        let deletes = self.deleted.iter().map(|row| pk_of(*row)).collect();
        let inserts = self.rows[self.first_new_row..]
            .iter()
            .map(|row| RowInsert {
                values: row
                    .iter()
                    .enumerate()
                    .filter(|(_, value)| value.is_some())
                    .map(|(index, value)| (self.columns[index].clone(), value.clone()))
                    .collect(),
            })
            .collect();

        let mut by_row: BTreeMap<usize, Vec<(String, Option<String>)>> = BTreeMap::new();
        for ((row, col), value) in &self.edits {
            if !self.deleted.contains(row) {
                by_row.entry(*row).or_default().push((self.columns[*col].clone(), value.clone()));
            }
        }
        let edits = by_row
            .into_iter()
            .map(|(row, changes)| RowEdit {
                pk_values: pk_of(row),
                changes,
            })
            .collect();

        Some((info, RowChanges { deletes, inserts, edits }))
    }

    pub fn move_by(&mut self, rows: isize, cols: isize) {
        let last_row = self.rows.len().saturating_sub(1) as isize;
        let last_col = self.columns.len().saturating_sub(1) as isize;
        self.row = (self.row as isize + rows).clamp(0, last_row.max(0)) as usize;
        self.col = (self.col as isize + cols).clamp(0, last_col.max(0)) as usize;
    }

    /// O cursor chegou perto do fim e ainda há páginas?
    pub fn wants_more(&self) -> bool {
        self.has_more && self.row + LOAD_AHEAD >= self.first_new_row
    }

    /// Quantas linhas do banco já estão aqui (o `offset` da próxima página).
    pub fn loaded_rows(&self) -> usize {
        self.first_new_row
    }

    /// Ajusta a rolagem para o cursor caber em `height` linhas e `width`
    /// colunas de tela.
    pub fn scroll_into_view(&mut self, height: usize, width: usize) {
        if height > 0 {
            if self.row < self.scroll_row {
                self.scroll_row = self.row;
            } else if self.row >= self.scroll_row + height {
                self.scroll_row = self.row + 1 - height;
            }
        }
        if self.col < self.scroll_col {
            self.scroll_col = self.col;
        }
        // Avança a primeira coluna até a do cursor caber inteira
        while self.scroll_col < self.col && self.span_width(self.scroll_col, self.col) > width {
            self.scroll_col += 1;
        }
    }

    /// A largura na tela das colunas `from..=to`, com os separadores.
    fn span_width(&self, from: usize, to: usize) -> usize {
        (from..=to).map(|col| self.width_of(col) + COLUMN_CHROME).sum()
    }
}

/// O texto que a célula mostra para um valor: booleanos normalizados
/// (o Postgres devolve `t`/`f`; outros bancos, `1`/`0`).
pub fn shown(kind: CellKind, value: &str) -> &str {
    match (kind, value) {
        (CellKind::Bool, "t" | "true" | "1" | "TRUE") => "true",
        (CellKind::Bool, "f" | "false" | "0" | "FALSE") => "false",
        _ => value,
    }
}

/// A largura da primeira linha de um valor (o resto vira `⏎` na tela).
fn first_line_width(value: &str) -> usize {
    value.lines().next().unwrap_or_default().width()
}

#[cfg(test)]
mod tests {
    use octapus_core::models::QueryColumnInfo;

    use super::*;

    fn result(rows: &[[&str; 2]], has_more: bool) -> QueryResult {
        QueryResult {
            columns: ["id", "name"]
                .iter()
                .map(|name| QueryColumnInfo {
                    name: name.to_string(),
                    type_name: "text".into(),
                    type_oid: None,
                })
                .collect(),
            rows: rows.iter().map(|row| row.iter().map(|cell| Some(cell.to_string())).collect()).collect(),
            row_count: rows.len(),
            total_count: None,
            has_more,
            execution_time_ms: 1,
            editable_info: Some(EditableInfo {
                schema: "public".into(),
                table: "customers".into(),
                primary_key_columns: vec!["id".into()],
                primary_key_column_indices: vec![0],
            }),
        }
    }

    #[test]
    fn pending_changes_follow_the_app_model() {
        let mut grid = Grid::from_result(result(&[["1", "Ana"], ["2", "Bia"], ["3", "Caio"]], false));
        assert_eq!(grid.pending_count(), 0);

        grid.set_cell(0, 1, Some("Ana Maria".into()));
        grid.set_cell(0, 1, Some("Ana M.".into()));
        grid.toggle_delete(1);
        grid.add_row();
        grid.set_cell(3, 1, Some("Davi".into()));

        assert_eq!(grid.cell_pending(0, 1), Some(Pending::Edited));
        assert_eq!(grid.cell_pending(0, 0), None);
        assert_eq!(grid.row_pending(1), Some(Pending::Removed));
        assert_eq!(grid.row_pending(3), Some(Pending::Added));
        assert_eq!(grid.cell(0, 1), Some("Ana M."));
        assert_eq!(grid.pending_count(), 3);

        let (info, changes) = grid.changes().unwrap();
        assert_eq!(info.table, "customers");
        assert_eq!(changes.deletes, vec![vec![Some("2".to_string())]]);
        assert_eq!(changes.edits.len(), 1);
        assert_eq!(changes.edits[0].pk_values, vec![Some("1".to_string())]);
        assert_eq!(changes.edits[0].changes, vec![("name".to_string(), Some("Ana M.".to_string()))]);
        // A linha nova só leva o que foi preenchido
        assert_eq!(changes.inserts[0].values, vec![("name".to_string(), Some("Davi".to_string()))]);
    }

    #[test]
    fn restoring_the_value_and_undo_clear_the_pending_state() {
        let mut grid = Grid::from_result(result(&[["1", "Ana"], ["2", "Bia"]], false));
        grid.set_cell(0, 1, Some("x".into()));
        grid.set_cell(0, 1, Some("Ana".into()));
        assert_eq!(grid.pending_count(), 0, "voltou ao valor do banco");

        grid.set_cell(1, 1, None);
        grid.toggle_delete(1);
        // Linha excluída não aceita edição
        grid.set_cell(1, 0, Some("9".into()));
        grid.undo_row(1);
        assert_eq!(grid.pending_count(), 0);

        grid.add_row();
        grid.undo_row(2);
        assert_eq!(grid.rows.len(), 2, "linha nova desfeita some");
    }

    #[test]
    fn next_pages_go_before_the_new_rows() {
        let mut grid = Grid::from_result(result(&[["1", "Ana"]], true));
        grid.add_row();
        grid.set_cell(1, 1, Some("Nova".into()));
        grid.toggle_delete(0);

        grid.append(result(&[["2", "Bia"], ["3", "Caio"]], false));
        assert_eq!(grid.rows.len(), 4);
        assert_eq!(grid.cell(3, 1), Some("Nova"));
        assert_eq!(grid.row_pending(3), Some(Pending::Added));
        assert_eq!(grid.row_pending(0), Some(Pending::Removed));
        assert_eq!(grid.row, 3, "o cursor segue a linha nova");
        assert_eq!(grid.loaded_rows(), 3);
        assert!(!grid.wants_more());
    }

    #[test]
    fn scroll_follows_the_cursor_in_both_directions() {
        let rows: Vec<[&str; 2]> = (0..1000).map(|_| ["1", "x"]).collect();
        let mut grid = Grid::from_result(result(&rows, false));
        grid.move_by(500, 1);
        grid.scroll_into_view(20, 7);
        assert_eq!(grid.scroll_row, 481);
        // Só cabe uma coluna: a do cursor
        assert_eq!(grid.scroll_col, 1);
        grid.move_by(-1000, -5);
        grid.scroll_into_view(20, 7);
        assert_eq!((grid.scroll_row, grid.scroll_col), (0, 0));
    }
}
