//! O editor de consulta da TUI: um buffer de linhas com cursor, realce de
//! SQL e autocompletar. Simples de propósito — digitar não pode esperar nada.
//!
//! O realce é um varredor de tokens por linha (palavra-chave, texto, número,
//! comentário); o que atravessa linhas (`/* … */`) é levado de uma linha para
//! a seguinte pelo estado inicial de cada uma, recalculado só quando o texto
//! muda.

use unicode_width::UnicodeWidthChar;

use super::complete::Item;

/// O papel de um trecho, para a cor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Plain,
    Keyword,
    String,
    Number,
    Comment,
}

pub const KEYWORDS: &[&str] = &[
    "all", "alter", "and", "any", "as", "asc", "begin", "between", "by", "case", "cast", "commit", "create", "cross",
    "delete", "desc", "distinct", "drop", "else", "end", "except", "exists", "explain", "false", "from", "full",
    "group", "having", "ilike", "in", "index", "inner", "insert", "intersect", "into", "is", "join", "left", "like",
    "limit", "not", "null", "offset", "on", "or", "order", "outer", "primary", "returning", "right", "rollback",
    "select", "set", "table", "then", "true", "union", "unique", "update", "using", "values", "view", "when",
    "where", "with",
];

pub fn is_keyword(word: &str) -> bool {
    KEYWORDS.binary_search(&word.to_ascii_lowercase().as_str()).is_ok()
}

fn is_word(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// Os trechos de uma linha: (início em chars, fim, papel). `in_comment` diz
/// se a linha começa dentro de um `/* … */`; devolve se termina dentro.
pub fn highlight(line: &str, mut in_comment: bool) -> (Vec<(usize, usize, Token)>, bool) {
    let chars: Vec<char> = line.chars().collect();
    let mut spans = Vec::new();
    let mut index = 0;

    while index < chars.len() {
        let start = index;
        if in_comment {
            while index < chars.len() {
                if chars[index] == '*' && chars.get(index + 1) == Some(&'/') {
                    index += 2;
                    in_comment = false;
                    break;
                }
                index += 1;
            }
            spans.push((start, index, Token::Comment));
            continue;
        }

        let ch = chars[index];
        let next = chars.get(index + 1).copied();
        match ch {
            '-' if next == Some('-') => {
                spans.push((start, chars.len(), Token::Comment));
                break;
            }
            '/' if next == Some('*') => {
                in_comment = true;
                index += 2;
                // O laço de cima fecha o trecho (nesta linha ou em outra)
                while index < chars.len() {
                    if chars[index] == '*' && chars.get(index + 1) == Some(&'/') {
                        index += 2;
                        in_comment = false;
                        break;
                    }
                    index += 1;
                }
                spans.push((start, index, Token::Comment));
            }
            '\'' | '"' => {
                index += 1;
                while index < chars.len() {
                    if chars[index] == ch {
                        index += 1;
                        // Aspas dobradas continuam o texto
                        if chars.get(index) == Some(&ch) {
                            index += 1;
                            continue;
                        }
                        break;
                    }
                    index += 1;
                }
                let token = if ch == '\'' { Token::String } else { Token::Plain };
                spans.push((start, index, token));
            }
            ch if ch.is_ascii_digit() => {
                while index < chars.len() && (chars[index].is_ascii_alphanumeric() || chars[index] == '.') {
                    index += 1;
                }
                spans.push((start, index, Token::Number));
            }
            ch if is_word(ch) => {
                while index < chars.len() && is_word(chars[index]) {
                    index += 1;
                }
                let word: String = chars[start..index].iter().collect();
                let token = if is_keyword(&word) { Token::Keyword } else { Token::Plain };
                spans.push((start, index, token));
            }
            _ => {
                index += 1;
                spans.push((start, index, Token::Plain));
            }
        }
    }
    (spans, in_comment)
}

#[derive(Debug, Clone, Default)]
pub struct Completion {
    pub items: Vec<Item>,
    pub selected: usize,
    /// Onde começa a palavra que está sendo completada (coluna em chars)
    pub start: usize,
}

#[derive(Debug)]
pub struct Editor {
    lines: Vec<String>,
    /// Linha e coluna do cursor, em chars
    pub row: usize,
    pub col: usize,
    pub scroll: usize,
    /// Estado inicial de cada linha (dentro de `/* … */`?), para o realce
    comment_state: Vec<bool>,
    pub completion: Option<Completion>,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            lines: vec![String::new()],
            row: 0,
            col: 0,
            scroll: 0,
            comment_state: vec![false],
            completion: None,
        }
    }
}

fn byte_of(line: &str, col: usize) -> usize {
    line.char_indices().nth(col).map_or(line.len(), |(index, _)| index)
}

impl Editor {
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    #[cfg(test)]
    pub fn set_text(&mut self, text: &str) {
        self.lines = text.split('\n').map(|line| line.trim_end_matches('\r').to_string()).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.row = self.lines.len() - 1;
        self.col = self.lines[self.row].chars().count();
        self.changed();
    }

    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(|line| line.trim().is_empty())
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn line(&self, row: usize) -> Option<&str> {
        self.lines.get(row).map(String::as_str)
    }

    /// Os trechos coloridos de uma linha.
    pub fn spans(&self, row: usize) -> Vec<(usize, usize, Token)> {
        match self.lines.get(row) {
            Some(line) => highlight(line, self.comment_state.get(row).copied().unwrap_or(false)).0,
            None => Vec::new(),
        }
    }

    fn changed(&mut self) {
        self.completion = None;
        let mut state = false;
        self.comment_state.clear();
        for line in &self.lines {
            self.comment_state.push(state);
            state = highlight(line, state).1;
        }
    }

    fn line_len(&self, row: usize) -> usize {
        self.lines.get(row).map_or(0, |line| line.chars().count())
    }

    pub fn insert_char(&mut self, ch: char) {
        let at = byte_of(&self.lines[self.row], self.col);
        self.lines[self.row].insert(at, ch);
        self.col += 1;
        self.changed();
    }

    /// Texto colado (pode ter várias linhas).
    pub fn insert_str(&mut self, text: &str) {
        for ch in text.chars() {
            match ch {
                '\n' => self.newline_raw(),
                '\r' => {}
                '\t' => {
                    self.insert_raw(' ');
                    self.insert_raw(' ');
                }
                ch => self.insert_raw(ch),
            }
        }
        self.changed();
    }

    fn insert_raw(&mut self, ch: char) {
        let at = byte_of(&self.lines[self.row], self.col);
        self.lines[self.row].insert(at, ch);
        self.col += 1;
    }

    fn newline_raw(&mut self) {
        let at = byte_of(&self.lines[self.row], self.col);
        let rest = self.lines[self.row].split_off(at);
        self.lines.insert(self.row + 1, rest);
        self.row += 1;
        self.col = 0;
    }

    /// Enter: quebra a linha mantendo a indentação.
    pub fn newline(&mut self) {
        let indent: String = self.lines[self.row].chars().take_while(|ch| *ch == ' ').collect();
        let indent: String = indent.chars().take(self.col).collect();
        self.newline_raw();
        self.lines[self.row].insert_str(0, &indent);
        self.col = indent.chars().count();
        self.changed();
    }

    pub fn backspace(&mut self) {
        if self.col > 0 {
            let at = byte_of(&self.lines[self.row], self.col - 1);
            self.lines[self.row].remove(at);
            self.col -= 1;
        } else if self.row > 0 {
            let line = self.lines.remove(self.row);
            self.row -= 1;
            self.col = self.line_len(self.row);
            self.lines[self.row].push_str(&line);
        }
        self.changed();
    }

    pub fn delete(&mut self) {
        if self.col < self.line_len(self.row) {
            let at = byte_of(&self.lines[self.row], self.col);
            self.lines[self.row].remove(at);
        } else if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].push_str(&next);
        }
        self.changed();
    }

    pub fn move_left(&mut self) {
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.line_len(self.row);
        }
        self.completion = None;
    }

    pub fn move_right(&mut self) {
        if self.col < self.line_len(self.row) {
            self.col += 1;
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        }
        self.completion = None;
    }

    pub fn move_vertical(&mut self, delta: isize) {
        let last = self.lines.len() as isize - 1;
        self.row = (self.row as isize + delta).clamp(0, last) as usize;
        self.col = self.col.min(self.line_len(self.row));
        self.completion = None;
    }

    pub fn move_home(&mut self) {
        self.col = 0;
        self.completion = None;
    }

    pub fn move_end(&mut self) {
        self.col = self.line_len(self.row);
        self.completion = None;
    }

    pub fn scroll_into_view(&mut self, height: usize) {
        if height == 0 {
            return;
        }
        if self.row < self.scroll {
            self.scroll = self.row;
        } else if self.row >= self.scroll + height {
            self.scroll = self.row + 1 - height;
        }
    }

    /// A coluna do cursor na tela (caracteres largos contam dois).
    pub fn cursor_x(&self) -> usize {
        self.lines[self.row]
            .chars()
            .take(self.col)
            .map(|ch| ch.width().unwrap_or(0))
            .sum()
    }

    /// O cursor como posição (em caracteres) dentro de [`Editor::text`].
    pub fn cursor_offset(&self) -> usize {
        let before: usize = self.lines[..self.row].iter().map(|line| line.chars().count() + 1).sum();
        before + self.col.min(self.line_len(self.row))
    }

    /// Abre a lista do autocompletar para a palavra que começa em `start`
    /// (coluna em chars, na linha do cursor). Sem candidatos, fecha.
    pub fn open_completion(&mut self, start: usize, items: Vec<Item>) {
        self.completion = (!items.is_empty()).then_some(Completion {
            items,
            selected: 0,
            start,
        });
    }

    pub fn move_completion(&mut self, delta: isize) {
        if let Some(completion) = self.completion.as_mut() {
            let count = completion.items.len() as isize;
            completion.selected = (completion.selected as isize + delta).rem_euclid(count) as usize;
        }
    }

    /// Troca a palavra atual pelo candidato escolhido.
    pub fn accept_completion(&mut self) {
        let Some(completion) = self.completion.take() else {
            return;
        };
        let Some(item) = completion.items.get(completion.selected) else {
            return;
        };
        let from = byte_of(&self.lines[self.row], completion.start);
        let to = byte_of(&self.lines[self.row], self.col);
        // O que já está escrito igual fica como a pessoa digitou (`FROM`)
        if self.lines[self.row][from..to].eq_ignore_ascii_case(&item.label) {
            return;
        }
        self.lines[self.row].replace_range(from..to, &item.label);
        self.col = completion.start + item.label.chars().count();
        self.changed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(line: &str) -> Vec<(String, Token)> {
        let chars: Vec<char> = line.chars().collect();
        highlight(line, false)
            .0
            .into_iter()
            .filter(|(start, end, _)| !chars[*start..*end].iter().all(|ch| ch.is_whitespace()))
            .map(|(start, end, token)| (chars[start..end].iter().collect(), token))
            .collect()
    }

    #[test]
    fn keywords_are_sorted_for_the_binary_search() {
        let mut sorted = KEYWORDS.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, KEYWORDS);
    }

    #[test]
    fn highlights_keywords_strings_numbers_and_comments() {
        assert_eq!(
            tokens("SELECT nome, 42 FROM t WHERE x = 'it''s' -- fim"),
            [
                ("SELECT".into(), Token::Keyword),
                ("nome".into(), Token::Plain),
                (",".into(), Token::Plain),
                ("42".into(), Token::Number),
                ("FROM".into(), Token::Keyword),
                ("t".into(), Token::Plain),
                ("WHERE".into(), Token::Keyword),
                ("x".into(), Token::Plain),
                ("=".into(), Token::Plain),
                ("'it''s'".into(), Token::String),
                ("-- fim".into(), Token::Comment),
            ]
        );
        // Identificador entre aspas não é palavra-chave nem texto
        assert_eq!(tokens("\"select\""), [("\"select\"".into(), Token::Plain)]);
    }

    #[test]
    fn block_comments_cross_lines() {
        let mut editor = Editor::default();
        editor.set_text("select /* começo\nmeio select\nfim */ from t");
        assert!(editor.spans(1).iter().all(|(_, _, token)| *token == Token::Comment));
        let last = editor.spans(2);
        assert_eq!(last.first().map(|span| span.2), Some(Token::Comment));
        assert!(last.iter().any(|(_, _, token)| *token == Token::Keyword));
    }

    #[test]
    fn editing_keeps_lines_and_cursor_consistent() {
        let mut editor = Editor::default();
        editor.insert_str("select *\n  from café");
        assert_eq!(editor.text(), "select *\n  from café");
        assert_eq!((editor.row, editor.col), (1, 11));

        editor.newline();
        assert_eq!(editor.line(2), Some("  "), "mantém a indentação");
        editor.backspace();
        editor.backspace();
        editor.backspace();
        assert_eq!(editor.text(), "select *\n  from café");

        editor.move_home();
        editor.move_left();
        assert_eq!((editor.row, editor.col), (0, 8));
        editor.delete();
        assert_eq!(editor.text(), "select *  from café");
        editor.move_vertical(5);
        assert_eq!(editor.row, 0);
    }

    #[test]
    fn accepting_a_candidate_replaces_the_word_being_typed() {
        use crate::tui::complete::ItemKind;
        let items = || vec![Item::new("customers", ItemKind::Table, ""), Item::new("cities", ItemKind::Table, "")];
        let mut editor = Editor::default();
        editor.insert_str("select 1;\nselect * from c");
        assert_eq!(editor.cursor_offset(), "select 1;\nselect * from c".chars().count());

        editor.open_completion(14, items());
        editor.move_completion(1);
        editor.accept_completion();
        assert_eq!(editor.text(), "select 1;\nselect * from cities");
        assert!(editor.completion.is_none());

        // O que já está escrito igual fica como foi digitado
        editor.insert_str(" WHERE");
        editor.open_completion(21, vec![Item::new("where", ItemKind::Keyword, "")]);
        editor.accept_completion();
        assert_eq!(editor.text(), "select 1;\nselect * from cities WHERE");

        editor.open_completion(0, Vec::new());
        assert!(editor.completion.is_none());
    }
}
