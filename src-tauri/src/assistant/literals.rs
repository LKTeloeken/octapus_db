//! Literais trocados por `?` nas definições que vão ao modelo (views,
//! funções, índices): o texto de um `CREATE VIEW … WHERE status = 'pago'` ou o
//! corpo de uma função pode carregar dado do usuário (RN-08), e o modelo só
//! precisa da forma. A sidebar mostra a definição completa; só o assistente
//! passa por aqui.
//!
//! Não é um parser: um varredor de tokens que sabe o bastante de SQL
//! (Postgres e SQLite) e de JSON para não confundir identificador com literal.
//! Na dúvida, troca — perder um número de exemplo custa menos que vazar um
//! valor.

/// Tipos cujo modificador entre parênteses é tamanho, não dado
/// (`varchar(20)`, `numeric(10,2)`): esses números ficam.
const SIZED_TYPES: [&str; 14] = [
    "bit",
    "char",
    "character",
    "decimal",
    "float",
    "interval",
    "numeric",
    "time",
    "timestamp",
    "timestamptz",
    "timetz",
    "varbit",
    "varchar",
    "varying",
];

/// SQL com strings, números e comentários trocados: string (`'…'`, `E'…'`,
/// `X'…'`, `U&'…'`) e número viram `?`; comentário sai. Identificador entre
/// aspas, parâmetro (`$1`) e o tamanho de um tipo ficam. O corpo de uma função
/// (`AS $function$ … $function$`) é código: os literais de dentro também são
/// trocados; um dollar-quote dentro do corpo é literal e vira `?` inteiro.
pub fn mask_sql(sql: &str) -> String {
    mask_sql_at(sql, 0)
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

fn mask_sql_at(sql: &str, depth: usize) -> String {
    let chars: Vec<char> = sql.chars().collect();
    let mut out = String::with_capacity(sql.len());
    // Para cada parêntese aberto: se é o modificador de um tipo
    let mut parens: Vec<bool> = Vec::new();
    let mut last_ident = String::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            '-' if next == Some('-') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            '/' if next == Some('*') => {
                // Comentários de bloco aninham no Postgres
                let mut level = 0;
                while i < chars.len() {
                    if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                        level += 1;
                        i += 2;
                    } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                        level -= 1;
                        i += 2;
                        if level == 0 {
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
                out.push(' ');
                last_ident.clear();
                continue;
            }
            '\'' => {
                i = skip_string(&chars, i, false);
                out.push('?');
                last_ident.clear();
                continue;
            }
            '"' | '`' => {
                let end = skip_quoted(&chars, i, c);
                out.extend(&chars[i..end]);
                i = end;
                last_ident.clear();
                continue;
            }
            '$' => {
                if next.is_some_and(|d| d.is_ascii_digit()) {
                    // Parâmetro posicional
                    out.push('$');
                    i += 1;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        out.push(chars[i]);
                        i += 1;
                    }
                    continue;
                }
                if let Some((tag_end, body_end, close_end)) = dollar_quote(&chars, i) {
                    if depth == 0 {
                        out.extend(&chars[i..tag_end]);
                        let body: String = chars[tag_end..body_end].iter().collect();
                        out.push_str(&mask_sql_at(&body, depth + 1));
                        out.extend(&chars[body_end..close_end]);
                    } else {
                        out.push('?');
                    }
                    i = close_end;
                    last_ident.clear();
                    continue;
                }
                out.push(c);
                i += 1;
                continue;
            }
            '(' => {
                parens.push(SIZED_TYPES.contains(&last_ident.as_str()));
                out.push(c);
                last_ident.clear();
                i += 1;
                continue;
            }
            ')' => {
                parens.pop();
                out.push(c);
                last_ident.clear();
                i += 1;
                continue;
            }
            _ => {}
        }

        if is_ident_start(c) {
            let start = i;
            while i < chars.len() && is_ident_char(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            // Prefixo de string: E'…' (com escapes de barra), B'…', X'…', N'…', U&'…'
            let prefix = word.to_ascii_lowercase();
            if chars.get(i) == Some(&'\'') && matches!(prefix.as_str(), "e" | "b" | "x" | "n") {
                i = skip_string(&chars, i, prefix == "e");
                out.push('?');
                last_ident.clear();
                continue;
            }
            if prefix == "u" && chars.get(i) == Some(&'&') && chars.get(i + 1) == Some(&'\'') {
                i = skip_string(&chars, i + 1, false);
                out.push('?');
                last_ident.clear();
                continue;
            }
            out.push_str(&word);
            last_ident = prefix;
            continue;
        }

        let starts_number = c.is_ascii_digit() || (c == '.' && next.is_some_and(|d| d.is_ascii_digit()));
        if starts_number {
            let start = i;
            i = skip_number(&chars, i);
            if parens.last() == Some(&true) {
                out.extend(&chars[start..i]);
            } else {
                out.push('?');
            }
            continue;
        }

        if !c.is_whitespace() && c != ',' {
            last_ident.clear();
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Fim (exclusivo) de uma string `'…'` que começa em `start`, com `''`
/// escapado e, em `E'…'`, a barra. Sem fechamento, vai até o fim.
fn skip_string(chars: &[char], start: usize, backslash: bool) -> usize {
    let mut i = start + 1;
    while i < chars.len() {
        match chars[i] {
            '\\' if backslash => i += 2,
            '\'' if chars.get(i + 1) == Some(&'\'') => i += 2,
            '\'' => return i + 1,
            _ => i += 1,
        }
    }
    chars.len()
}

/// Fim (exclusivo) de um identificador entre `quote` (`"…"` ou `` `…` ``),
/// com a aspa dobrada como escape.
fn skip_quoted(chars: &[char], start: usize, quote: char) -> usize {
    let mut i = start + 1;
    while i < chars.len() {
        if chars[i] == quote {
            if chars.get(i + 1) == Some(&quote) {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    chars.len()
}

/// `$tag$ … $tag$` a partir de `start`: (fim da abertura, início do
/// fechamento, fim do fechamento). `None` se não é um dollar-quote fechado.
fn dollar_quote(chars: &[char], start: usize) -> Option<(usize, usize, usize)> {
    let mut i = start + 1;
    while i < chars.len() && chars[i] != '$' {
        if !(is_ident_char(chars[i]) && chars[i] != '$') || (i == start + 1 && chars[i].is_ascii_digit()) {
            return None;
        }
        i += 1;
    }
    if i >= chars.len() {
        return None;
    }
    let tag: Vec<char> = chars[start..=i].to_vec();
    let body_start = i + 1;
    let mut j = body_start;
    while j + tag.len() <= chars.len() {
        if chars[j..j + tag.len()] == tag[..] {
            return Some((body_start, j, j + tag.len()));
        }
        j += 1;
    }
    None
}

/// Fim de um número: dígitos (com `_` do PG 16), ponto, expoente.
fn skip_number(chars: &[char], start: usize) -> usize {
    let mut i = start;
    while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '_' || chars[i] == '.') {
        i += 1;
    }
    if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
        let mut j = i + 1;
        if j < chars.len() && (chars[j] == '+' || chars[j] == '-') {
            j += 1;
        }
        if j < chars.len() && chars[j].is_ascii_digit() {
            i = j;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
        }
    }
    i
}

/// JSON (o pipeline de uma view do Mongo) com os valores trocados: string e
/// número viram `?`. Ficam as chaves, `true`/`false`/`null` e as strings que
/// começam com `$` (caminho de campo, `$$NOW`), que são forma, não dado.
pub fn mask_json(json: &str) -> String {
    let chars: Vec<char> = json.chars().collect();
    let mut out = String::with_capacity(json.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            let mut end = i + 1;
            while end < chars.len() {
                match chars[end] {
                    '\\' => end += 2,
                    '"' => break,
                    _ => end += 1,
                }
            }
            let end = (end + 1).min(chars.len());
            let mut after = end;
            while after < chars.len() && chars[after].is_whitespace() {
                after += 1;
            }
            let is_key = chars.get(after) == Some(&':');
            let is_path = chars.get(i + 1) == Some(&'$');
            if is_key || is_path {
                out.extend(&chars[i..end]);
            } else {
                out.push('?');
            }
            i = end;
            continue;
        }
        if c == '-' || c.is_ascii_digit() {
            let mut end = i + 1;
            while end < chars.len()
                && (chars[end].is_ascii_digit() || matches!(chars[end], '.' | 'e' | 'E' | '+' | '-'))
            {
                end += 1;
            }
            out.push('?');
            i = end;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_numbers_and_comments_go_away() {
        let view = "CREATE OR REPLACE VIEW app.big AS\n SELECT id, 'CANARY_a'::text AS tag -- CANARY_comment\n   FROM app.orders /* CANARY /* nested */ block */\n  WHERE total > 1000::numeric AND code = E'CANARY\\'s' AND ratio < .5e-3;\n";
        let masked = mask_sql(view);
        assert!(!masked.contains("CANARY"), "{masked}");
        assert!(!masked.contains("1000") && !masked.contains(".5"), "{masked}");
        assert!(masked.contains("?::text AS tag"), "{masked}");
        assert!(
            masked.contains("total > ?::numeric AND code = ? AND ratio < ?;"),
            "{masked}"
        );
        assert!(
            masked.starts_with("CREATE OR REPLACE VIEW app.big AS\n SELECT id,"),
            "{masked}"
        );
    }

    #[test]
    fn identifiers_parameters_and_type_sizes_stay() {
        let sql = r#"SELECT "Order 2024"."total_1", t2.x FROM "a""b" t2 WHERE id = $1 AND v::varchar(20) <> ? AND n::numeric(10, 2) > 3"#;
        let masked = mask_sql(sql);
        assert_eq!(
            masked,
            r#"SELECT "Order 2024"."total_1", t2.x FROM "a""b" t2 WHERE id = $1 AND v::varchar(20) <> ? AND n::numeric(10, 2) > ?"#
        );
        assert_eq!(
            mask_sql("x character varying(40) DEFAULT 'CANARY'::character varying"),
            "x character varying(40) DEFAULT ?::character varying"
        );
        assert_eq!(
            mask_sql("round(total, 2) + coalesce(x, 0)"),
            "round(total, ?) + coalesce(x, ?)"
        );
    }

    #[test]
    fn prefixed_strings_are_literals_too() {
        assert_eq!(
            mask_sql("SELECT X'CAFE', B'0101', N'CANARY', U&'d\\0061t', e'a\\'b' , x"),
            "SELECT ?, ?, ?, ?, ? , x"
        );
        // Uma palavra terminada em e/x não é prefixo
        assert_eq!(mask_sql("SELECT type, max FROM t"), "SELECT type, max FROM t");
    }

    #[test]
    fn function_bodies_are_code_and_inner_dollar_quotes_are_literals() {
        let function = "CREATE OR REPLACE FUNCTION app.f(days integer DEFAULT 90)\n RETURNS text\n LANGUAGE plpgsql\nAS $function$\nBEGIN\n  IF days > 30 THEN RAISE NOTICE 'CANARY_msg %', days; END IF;\n  EXECUTE $q$SELECT 'CANARY_inner'$q$;\n  RETURN 'CANARY_ret';\nEND;\n$function$\n";
        let masked = mask_sql(function);
        assert!(!masked.contains("CANARY"), "{masked}");
        assert!(!masked.contains("90") && !masked.contains("30"), "{masked}");
        assert!(masked.contains("AS $function$\nBEGIN\n  IF days > ? THEN RAISE NOTICE ?, days; END IF;\n  EXECUTE ?;\n  RETURN ?;\nEND;\n$function$"), "{masked}");
        // `$$` sem tag e dollar-quote sem fechamento
        assert_eq!(mask_sql("AS $$SELECT 1$$"), "AS $$SELECT ?$$");
        assert_eq!(mask_sql("a $b c"), "a $b c");
    }

    #[test]
    fn unterminated_strings_do_not_leak() {
        assert_eq!(mask_sql("WHERE a = 'CANARY"), "WHERE a = ?");
        assert_eq!(mask_sql("WHERE a = \"open"), "WHERE a = \"open");
    }

    #[test]
    fn non_ascii_survives() {
        assert_eq!(
            mask_sql("SELECT préço FROM pedidos WHERE situação = 'pago'"),
            "SELECT préço FROM pedidos WHERE situação = ?"
        );
    }

    #[test]
    fn json_values_go_away_keys_and_paths_stay() {
        let pipeline = r#"[
  {
    "$match": {
      "status": "CANARY_paid",
      "total": { "$gte": 1500.5 },
      "tags": ["CANARY_a", "CANARY_b"],
      "active": true,
      "deleted": null,
      "note": "say \"CANARY\""
    }
  },
  { "$project": { "total": "$amount", "now": "$$NOW", "n": -1 } }
]"#;
        let masked = mask_json(pipeline);
        assert!(!masked.contains("CANARY") && !masked.contains("1500"), "{masked}");
        assert!(masked.contains(r#""status": ?"#), "{masked}");
        assert!(masked.contains(r#""$gte": ?"#), "{masked}");
        assert!(masked.contains(r#""tags": [?, ?]"#), "{masked}");
        assert!(
            masked.contains(r#""active": true"#) && masked.contains(r#""deleted": null"#),
            "{masked}"
        );
        assert!(
            masked.contains(r#""total": "$amount", "now": "$$NOW", "n": ?"#),
            "{masked}"
        );
    }
}
