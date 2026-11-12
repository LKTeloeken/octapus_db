//! Leitura léxica mínima de SQL, para o que o editor livre precisa saber do
//! texto antes de mandá-lo ao banco — sem interpretar a consulta.

/// O miolo do SQL: sem espaços, comentários e `;` nas pontas.
///
/// O editor pagina uma consulta embrulhando-a numa subquery
/// (`SELECT * FROM (…) AS __q LIMIT …`) e decide se ela devolve linhas pela
/// primeira palavra. Um comentário de linha no fim engolia o `) AS __q …`, um
/// `;` antes dele ficava no meio da subquery, e um comentário no começo
/// escondia o `SELECT`. O que está entre aspas, cifrões (`$tag$ … $tag$`) ou
/// no meio do texto fica como veio: `SELECT '--'` e `a; b` não mudam.
pub fn statement_body(sql: &str) -> &str {
    let bytes = sql.as_bytes();
    let mut start: Option<usize> = None;
    let mut end = 0;
    let mut i = 0;

    while i < bytes.len() {
        let token = i;
        let significant = match bytes[i] {
            byte if byte.is_ascii_whitespace() => {
                i += 1;
                false
            }
            b';' => {
                i += 1;
                false
            }
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                i = line_comment_end(bytes, i);
                false
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i = block_comment_end(bytes, i);
                false
            }
            b'\'' => {
                i = quoted_end(bytes, i, b'\'', is_escape_string(bytes, i));
                true
            }
            b'"' => {
                i = quoted_end(bytes, i, b'"', false);
                true
            }
            b'$' => {
                i = dollar_quoted_end(sql, i).unwrap_or(i + 1);
                true
            }
            _ => {
                i += 1;
                true
            }
        };

        if significant {
            start.get_or_insert(token);
            end = i;
        }
    }

    // Os cortes caem sempre num byte ASCII ou no fim de um caractere inteiro
    start.map_or("", |start| &sql[start..end])
}

/// `-- …` até o fim da linha (a quebra fica de fora).
fn line_comment_end(bytes: &[u8], from: usize) -> usize {
    bytes[from..]
        .iter()
        .position(|&byte| byte == b'\n')
        .map_or(bytes.len(), |offset| from + offset)
}

/// `/* … */`, com aninhamento (como no Postgres); sem fechar, vai até o fim.
fn block_comment_end(bytes: &[u8], from: usize) -> usize {
    let mut depth = 0usize;
    let mut i = from;
    while i < bytes.len() {
        match (bytes[i], bytes.get(i + 1)) {
            (b'/', Some(b'*')) => {
                depth += 1;
                i += 2;
            }
            (b'*', Some(b'/')) => {
                depth -= 1;
                i += 2;
                if depth == 0 {
                    return i;
                }
            }
            _ => i += 1,
        }
    }
    bytes.len()
}

/// Fim de um trecho entre `quote`s: a aspa dobrada (`''`, `""`) não fecha, e
/// numa string `E'…'` a barra invertida escapa o caractere seguinte.
fn quoted_end(bytes: &[u8], from: usize, quote: u8, backslash_escapes: bool) -> usize {
    let mut i = from + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if backslash_escapes => i += 2,
            byte if byte == quote => {
                if bytes.get(i + 1) == Some(&quote) {
                    i += 2;
                } else {
                    return i + 1;
                }
            }
            _ => i += 1,
        }
    }
    bytes.len()
}

/// A aspa em `at` abre uma string `E'…'` (o `E` solto, não o fim de um nome)?
fn is_escape_string(bytes: &[u8], at: usize) -> bool {
    at >= 1
        && matches!(bytes[at - 1], b'e' | b'E')
        && (at < 2 || !is_identifier_byte(bytes[at - 2]))
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80
}

/// Fim de um `$tag$ … $tag$` aberto em `at`; `None` se o `$` não abre um
/// (parâmetro `$1`, `$` dentro de um nome). Sem fechar, vai até o fim.
fn dollar_quoted_end(sql: &str, at: usize) -> Option<usize> {
    let bytes = sql.as_bytes();
    if at > 0 && is_identifier_byte(bytes[at - 1]) {
        return None;
    }

    let tag_len = bytes[at + 1..]
        .iter()
        .position(|&byte| !is_identifier_byte(byte))?;
    let tag_end = at + 1 + tag_len;
    if bytes[tag_end] != b'$' || bytes.get(at + 1).is_some_and(u8::is_ascii_digit) {
        return None;
    }

    let delimiter = &sql[at..=tag_end];
    let body = tag_end + 1;
    Some(
        sql[body..]
            .find(delimiter)
            .map_or(sql.len(), |offset| body + offset + delimiter.len()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_a_trailing_line_comment() {
        assert_eq!(statement_body("select * from t\n-- teste"), "select * from t");
        assert_eq!(statement_body("select * from t -- teste\n"), "select * from t");
        assert_eq!(statement_body("select 1; -- fim"), "select 1");
        assert_eq!(statement_body("select 1 /* a /* b */ c */ ;\n"), "select 1");
    }

    #[test]
    fn drops_leading_comments_so_the_first_word_is_the_statement() {
        assert_eq!(statement_body("-- lista\nselect 1"), "select 1");
        assert_eq!(statement_body("/* x */ ;\n  select 1"), "select 1");
    }

    #[test]
    fn keeps_what_is_in_the_middle() {
        assert_eq!(statement_body("select 1 -- um\n, 2"), "select 1 -- um\n, 2");
        assert_eq!(statement_body("insert into t values (1); select 2;"), "insert into t values (1); select 2");
    }

    #[test]
    fn comment_markers_inside_quotes_are_text() {
        assert_eq!(statement_body("select '--' ;"), "select '--'");
        assert_eq!(statement_body("select 'it''s -- ok'"), "select 'it''s -- ok'");
        assert_eq!(statement_body("select \"a--b\" -- x"), "select \"a--b\"");
        assert_eq!(statement_body(r"select E'a\'--b' -- x"), r"select E'a\'--b'");
        assert_eq!(statement_body("select $$ -- ; $$ -- x"), "select $$ -- ; $$");
        assert_eq!(statement_body("select $fn$ a; $fn$;"), "select $fn$ a; $fn$");
    }

    #[test]
    fn dollar_signs_that_are_not_quotes() {
        assert_eq!(statement_body("select $1 -- x"), "select $1");
        assert_eq!(statement_body("select a$b$c -- x"), "select a$b$c");
    }

    #[test]
    fn only_noise_is_empty() {
        assert_eq!(statement_body("  ;; \n"), "");
        assert_eq!(statement_body("-- só comentário"), "");
        assert_eq!(statement_body("/* aberto"), "");
    }

    #[test]
    fn unterminated_quote_runs_to_the_end() {
        assert_eq!(statement_body("select 'abc -- x"), "select 'abc -- x");
        assert_eq!(statement_body("select 'não' -- é"), "select 'não'");
    }
}
