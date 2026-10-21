//! O que deu errado numa consulta do assistente: a classe da falha
//! (`FailureClass` do contrato, RN-30) e o erro sanitizado (RN-10), que são o
//! que vai para a API no outcome e na tentativa de correção.
//!
//! Os dois são determinísticos e não olham o banco: só o erro e o texto da
//! consulta que o modelo gerou.

use serde::{Deserialize, Serialize};

use crate::adapters::STATEMENT_TIMEOUT_MESSAGE;
use crate::error::Error;

/// Teto do `errorSanitized` no contrato (`StageResult`).
pub const SANITIZED_MAX_CHARS: usize = 2000;

/// `FailureClass` do contrato 1.4.0 (`openapi.yaml`).
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    /// O guard de leitura recusou (ou o banco recusou uma escrita que passou)
    GuardRejected,
    /// Tabela, coluna, função, schema ou coleção que não existe
    UnknownObject,
    Syntax,
    TypeMismatch,
    /// Erro de execução que não cabe nos outros
    RuntimeError,
    Timeout,
    /// O paradigma não expressa o pedido: o plano do Redis passou de um teto
    /// do executor (B9). A separação fina fica na planilha (RN-30)
    ParadigmLimitation,
    /// O modelo respondeu fora do contrato (B5)
    InvalidOutput,
}

/// A classe da falha de uma consulta. `None` quando o erro não é da consulta
/// (conexão, cancelamento, banco sem suporte): aí não há o que corrigir, e o
/// turno para.
pub fn classify(error: &Error) -> Option<FailureClass> {
    match error {
        Error::GuardRejected(_) => Some(FailureClass::GuardRejected),
        Error::DbQuery { sql_state, message } => Some(classify_postgres(sql_state, message)),
        Error::MongoQuery { code, message, .. } => Some(classify_mongo(*code, message)),
        Error::RedisQuery { code, message } => Some(classify_redis(code, message)),
        Error::PlanLimit(_) => Some(FailureClass::ParadigmLimitation),
        Error::Query(message) => Some(classify_message(message)),
        Error::InvalidQuery(message) if message.contains("only one statement") => {
            Some(FailureClass::GuardRejected)
        }
        Error::InvalidQuery(_) => Some(FailureClass::Syntax),
        _ => None,
    }
}

/// Postgres: pelo SQLSTATE (apêndice A da documentação), olhando a mensagem
/// só onde um código cobre mais de um caso.
fn classify_postgres(sql_state: &str, message: &str) -> FailureClass {
    match sql_state {
        // Escrita barrada pela transação READ ONLY: o guard deixou passar
        "25006" => FailureClass::GuardRejected,
        // statement_timeout; o mesmo código cobre o cancelamento pedido
        "57014" if message.contains("statement timeout") => FailureClass::Timeout,
        "55P03" => FailureClass::Timeout, // lock_timeout
        // undefined_column, undefined_table, undefined_object,
        // invalid_schema_name, undefined_parameter
        "42703" | "42P01" | "42704" | "3F000" | "42P02" => FailureClass::UnknownObject,
        // undefined_function: "operator does not exist: integer = text" é tipo
        "42883" if message.starts_with("operator does not exist") => FailureClass::TypeMismatch,
        "42883" => FailureClass::UnknownObject,
        // datatype_mismatch, cannot_coerce, indeterminate_datatype,
        // ambiguous_function, wrong_object_type
        "42804" | "42846" | "42P18" | "42725" | "42809" => FailureClass::TypeMismatch,
        // Valor que não converte para o tipo: invalid_text_representation,
        // invalid_datetime_format, datetime_field_overflow,
        // invalid_character_value_for_cast, numeric_value_out_of_range
        "22P02" | "22007" | "22008" | "22018" | "22003" => FailureClass::TypeMismatch,
        "42501" => FailureClass::RuntimeError, // insufficient_privilege
        // O resto da classe 42: consulta mal formada (sintaxe, GROUP BY,
        // coluna ambígua, janela…)
        state if state.starts_with("42") => FailureClass::Syntax,
        _ => FailureClass::RuntimeError,
    }
}

/// Mongo: pelo código do servidor (`error_codes.yml` do MongoDB), e pela
/// mensagem nos códigos genéricos (`Location<n>`) de cada operador.
fn classify_mongo(code: i32, message: &str) -> FailureClass {
    match code {
        // MaxTimeMSExpired, ExceededTimeLimit
        50 | 262 => FailureClass::Timeout,
        // NamespaceNotFound (também o do preflight, coleção que não existe)
        26 => FailureClass::UnknownObject,
        // Unauthorized, erro do Atlas (permissão)
        13 | 8000 => FailureClass::RuntimeError,
        // TypeMismatch, ConversionFailure ($toInt, $convert…)
        14 | 241 => FailureClass::TypeMismatch,
        // BadValue, FailedToParse, InvalidOptions, InvalidPipelineOperator,
        // estágio desconhecido ou mal formado, colisão de caminhos, FieldPath
        2 | 9 | 72 | 168 | 15952 | 16410 | 16411 | 16412 | 17276 | 28646 | 31249 | 31250 | 31253
        | 31254 | 40323 | 40324 | 40352 | 40353 => FailureClass::Syntax,
        _ => {
            let message = message.to_ascii_lowercase();
            let any = |needles: &[&str]| needles.iter().any(|needle| message.contains(needle));
            if any(&["can't convert", "cannot convert", "only supports", "unsupported conversion", "failed to parse number", "failed to parse objectid"]) {
                FailureClass::TypeMismatch
            } else if any(&["unrecognized", "unknown operator", "unknown top level operator", "unknown expression", "unknown argument", "unknown group operator", "invalid", "fieldpath", "path collision", "expected"]) {
                FailureClass::Syntax
            } else {
                FailureClass::RuntimeError
            }
        }
    }
}

/// Redis: pelo código da resposta (a primeira palavra do erro) e, no `ERR`
/// genérico, pela mensagem.
fn classify_redis(code: &str, message: &str) -> FailureClass {
    let message = message.to_ascii_lowercase();
    let any = |needles: &[&str]| needles.iter().any(|needle| message.contains(needle));
    match code {
        "WRONGTYPE" => FailureClass::TypeMismatch,
        // Réplica recusou uma escrita: o guard deixou passar
        "READONLY" => FailureClass::GuardRejected,
        "ERR" if any(&["not an integer", "not a float", "not a valid float", "out of range", "not a valid"]) => {
            FailureClass::TypeMismatch
        }
        "ERR" if any(&["unknown command", "wrong number of arguments", "syntax error", "unknown subcommand", "invalid", "unsupported"]) => {
            FailureClass::Syntax
        }
        // NOPERM, NOAUTH, MOVED, CROSSSLOT, BUSY…: do servidor, não da consulta
        _ => FailureClass::RuntimeError,
    }
}

/// Sem SQLSTATE (SQLite, ou o Postgres sem erro do servidor): pela mensagem.
fn classify_message(message: &str) -> FailureClass {
    let message = message.to_ascii_lowercase();
    if message.contains(STATEMENT_TIMEOUT_MESSAGE) || message.contains("interrupted") {
        FailureClass::Timeout
    } else if message.contains("readonly database") || message.contains("read-only") {
        FailureClass::GuardRejected
    } else if message.contains("no such table")
        || message.contains("no such column")
        || message.contains("no such function")
        || message.contains("unknown database")
    {
        FailureClass::UnknownObject
    } else if message.contains("datatype mismatch") {
        FailureClass::TypeMismatch
    } else if message.contains("syntax error")
        || message.contains("incomplete input")
        || message.contains("unrecognized token")
        || message.contains("wrong number of arguments")
        || message.contains("misuse of")
        || message.contains("ambiguous column")
    {
        FailureClass::Syntax
    } else {
        FailureClass::RuntimeError
    }
}

/// O erro para a API (RN-10): o SQLSTATE e só a mensagem principal — nunca o
/// `DETAIL`, o `HINT` nem o `WHERE`, onde os valores aparecem —, com cada
/// literal que **não** veio da própria consulta trocado por `?`.
///
/// Um trecho entre aspas ou um número fica só se aparece como token na
/// consulta que o modelo escreveu (nome de coluna, literal da pergunta): nesse
/// caso não é dado do banco. `None` para erros que não são da consulta.
pub fn sanitize(error: &Error, query: &str) -> Option<String> {
    let (sql_state, message) = match error {
        Error::DbQuery { sql_state, message } => (Some(sql_state.clone()), message.as_str()),
        // Mongo: o código vem no lugar do SQLSTATE (`BadValue (2): …`)
        Error::MongoQuery { code, code_name, message } => (
            Some(if code_name.is_empty() {
                code.to_string()
            } else {
                format!("{code_name} ({code})")
            }),
            message.as_str(),
        ),
        // Redis: o código da resposta no lugar do SQLSTATE (`WRONGTYPE: …`)
        Error::RedisQuery { code, message } => (Some(code.clone()), message.as_str()),
        // Teto do executor do Redis: texto nosso, só com id de passo, nome de
        // comando e os tetos (que o modelo precisa ver para refazer o plano)
        Error::PlanLimit(message) => return Some(truncate_chars(message, SANITIZED_MAX_CHARS)),
        // Mensagens do próprio guard: texto nosso, com no máximo um nome de
        // função tirado da consulta
        Error::GuardRejected(message) | Error::InvalidQuery(message) | Error::Query(message) => {
            (None, message.as_str())
        }
        _ => return None,
    };

    let known = known_tokens(query);
    let message = scrub(message, &known);
    let sanitized = match sql_state {
        Some(state) => format!("{state}: {message}"),
        // Sem SQLSTATE (SQLite): o valor pode vir sem aspas depois do `: `
        None => scrub_tail(&message, &known),
    };
    Some(truncate_chars(&sanitized, SANITIZED_MAX_CHARS))
}

/// Os tokens da consulta, em minúsculas: da operação JSON do Mongo ou do
/// plano do Redis (chaves, textos e números) ou do SQL.
fn known_tokens(query: &str) -> Vec<String> {
    match serde_json::from_str::<serde_json::Value>(query) {
        Ok(value @ serde_json::Value::Object(_)) => {
            let mut tokens = Vec::new();
            json_tokens(&value, &mut tokens);
            tokens
        }
        _ => query_tokens(query),
    }
}

/// Chaves e textos entram inteiros, sem o `$` de caminho (`"$total"` →
/// `total`) e por partes (`customer.name` → `customer`, `name`).
fn json_tokens(value: &serde_json::Value, tokens: &mut Vec<String>) {
    use serde_json::Value;

    let push_text = |text: &str, tokens: &mut Vec<String>| {
        let lower = text.to_lowercase();
        let bare = lower.trim_start_matches('$').to_string();
        tokens.extend(bare.split('.').filter(|part| !part.is_empty()).map(str::to_owned));
        tokens.push(bare);
        tokens.push(lower);
    };
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                push_text(key, tokens);
                json_tokens(value, tokens);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| json_tokens(item, tokens)),
        Value::String(text) => push_text(text, tokens),
        Value::Number(number) => tokens.push(number.to_string()),
        Value::Bool(_) | Value::Null => {}
    }
}

/// Os tokens da consulta (identificadores, literais e números), em minúsculas.
fn query_tokens(query: &str) -> Vec<String> {
    use sqlparser::dialect::GenericDialect;
    use sqlparser::tokenizer::{Token, Tokenizer};

    let Ok(tokens) = Tokenizer::new(&GenericDialect {}, query).tokenize() else {
        // Consulta que nem tokeniza: nada dela conta como conhecido, e todo
        // literal da mensagem vira `?`
        return Vec::new();
    };
    tokens
        .into_iter()
        .filter_map(|token| match token {
            Token::Word(word) => Some(word.value),
            Token::Number(number, _) => Some(number),
            Token::SingleQuotedString(text)
            | Token::DoubleQuotedString(text)
            | Token::NationalStringLiteral(text)
            | Token::EscapedStringLiteral(text) => Some(text),
            Token::DollarQuotedString(text) => Some(text.value),
            _ => None,
        })
        .map(|text| text.to_lowercase())
        .collect()
}

/// Veio da consulta: o texto inteiro, ou cada parte de um nome qualificado
/// (`relation "public.users"`), é um token dela.
fn from_query(text: &str, known: &[String]) -> bool {
    let text = text.to_lowercase();
    !text.is_empty()
        && (known.contains(&text)
            || (text.contains('.') && text.split('.').all(|part| known.iter().any(|k| k == part))))
}

/// Troca por `?` os trechos entre aspas e os números da mensagem que não
/// vieram da consulta. Quebras de linha viram espaço (a mensagem é uma linha).
fn scrub(message: &str, known: &[String]) -> String {
    let chars: Vec<char> = message.chars().collect();
    let mut out = String::with_capacity(message.len());
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // Apóstrofo no meio da palavra (`can't`, `$arrayElemAt's`): não abre
        // trecho entre aspas
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let apostrophe = c == '\''
            && i > 0
            && is_word(chars[i - 1])
            && chars.get(i + 1).is_some_and(|next| next.is_alphabetic());

        // Trecho entre aspas: até a aspa que fecha (ou o fim, sem vazar nada).
        // Uma aspa colada numa palavra não fecha: o valor pode ter aspas
        // dentro (`"ab"CANARY"`), e o resto dele vazaria
        if (c == '"' || c == '\'') && !apostrophe {
            let close = (i + 1..chars.len())
                .find(|&j| chars[j] == c && !chars.get(j + 1).is_some_and(|&next| is_word(next)));
            let inner_end = close.unwrap_or(chars.len());
            let inner: String = chars[i + 1..inner_end].iter().collect();
            out.push(c);
            if from_query(&inner, known) {
                out.push_str(&inner);
            } else {
                out.push('?');
            }
            if close.is_some() {
                out.push(c);
            }
            i = inner_end + 1;
            continue;
        }

        // Número solto (não parte de um nome como `tenant_00042`)
        let starts_word = i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
        if c.is_ascii_digit() && !starts_word {
            let mut end = i;
            while end < chars.len() && (chars[end].is_ascii_digit() || chars[end] == '.') {
                end += 1;
            }
            // Um número colado a letras é nome, não valor (`int4`, `2nd`)
            let glued = end < chars.len() && (chars[end].is_alphabetic() || chars[end] == '_');
            let number: String = chars[i..end].iter().collect();
            if glued || from_query(number.trim_end_matches('.'), known) {
                out.push_str(&number);
            } else {
                out.push('?');
                if number.ends_with('.') {
                    out.push('.');
                }
            }
            i = end;
            continue;
        }

        // SQLite: "no such column: nome" — o nome vem sem aspas
        if let Some(rest) = sqlite_object_name(&chars, i) {
            let (name, end) = rest;
            out.push_str(if from_query(&name, known) { &name } else { "?" });
            i = end;
            continue;
        }

        out.push(if c.is_control() { ' ' } else { c });
        i += 1;
    }
    out
}

/// Mensagem sem SQLSTATE: o que vem depois do primeiro `: ` só fica se cada
/// palavra veio da consulta (`misuse of aggregate: count()`) ou se é o
/// `syntax error` fixo do SQLite; senão vira `?`
/// (`CHECK constraint failed: <valor>` → `CHECK constraint failed: ?`).
fn scrub_tail(message: &str, known: &[String]) -> String {
    let Some(colon) = message.find(": ") else {
        return message.to_string();
    };
    let (head, tail) = (&message[..colon], &message[colon + 2..]);
    let safe = tail == "syntax error"
        || tail
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .filter(|word| !word.is_empty())
            .all(|word| known.contains(&word.to_lowercase()));
    if safe {
        message.to_string()
    } else {
        format!("{head}: ?")
    }
}

/// No SQLite, `no such table: x`, `no such column: t.x` e `no such function: f`
/// trazem o nome sem aspas logo depois do `: `. Devolve o nome e onde acaba.
fn sqlite_object_name(chars: &[char], i: usize) -> Option<(String, usize)> {
    if i < 2 || chars[i - 2] != ':' || chars[i - 1] != ' ' {
        return None;
    }
    let before: String = chars[..i - 2].iter().collect();
    let before = before.to_ascii_lowercase();
    let prefixed = ["no such table", "no such column", "no such function"]
        .iter()
        .any(|prefix| before.ends_with(prefix));
    if !prefixed {
        return None;
    }
    let end = chars[i..]
        .iter()
        .position(|c| c.is_whitespace())
        .map_or(chars.len(), |p| i + p);
    Some((chars[i..end].iter().collect(), end))
}

fn truncate_chars(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((cut, _)) => text[..cut].to_string(),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pg(sql_state: &str, message: &str) -> Error {
        Error::DbQuery {
            sql_state: sql_state.into(),
            message: message.into(),
        }
    }

    // ── Classificação ───────────────────────────────────────────────────────

    #[test]
    fn postgres_errors_by_sqlstate() {
        use FailureClass::*;
        let cases = [
            ("42703", "column \"nome\" does not exist", UnknownObject),
            ("42P01", "relation \"usuarios\" does not exist", UnknownObject),
            ("3F000", "schema \"x\" does not exist", UnknownObject),
            ("42883", "function lowerr(text) does not exist", UnknownObject),
            ("42883", "operator does not exist: integer = text", TypeMismatch),
            ("42804", "argument of WHERE must be type boolean, not type integer", TypeMismatch),
            ("22P02", "invalid input syntax for type integer: \"abc\"", TypeMismatch),
            ("22007", "invalid input syntax for type date: \"x\"", TypeMismatch),
            ("42601", "syntax error at or near \"FORM\"", Syntax),
            ("42803", "column \"u.name\" must appear in the GROUP BY clause", Syntax),
            ("42702", "column reference \"id\" is ambiguous", Syntax),
            ("57014", STATEMENT_TIMEOUT_MESSAGE, Timeout),
            ("55P03", "canceling statement due to lock timeout", Timeout),
            ("25006", "cannot execute INSERT in a read-only transaction", GuardRejected),
            ("22012", "division by zero", RuntimeError),
            ("42501", "permission denied for table salaries", RuntimeError),
            ("57014", "canceling statement due to user request", RuntimeError),
            ("XX000", "unexpected", RuntimeError),
        ];
        for (state, message, expected) in cases {
            assert_eq!(classify(&pg(state, message)), Some(expected), "{state} {message}");
        }
    }

    #[test]
    fn sqlite_errors_by_message() {
        use FailureClass::*;
        let cases = [
            ("no such table: usuarios", UnknownObject),
            ("no such column: nome", UnknownObject),
            ("no such function: lowerr", UnknownObject),
            ("near \"FORM\": syntax error", Syntax),
            ("incomplete input", Syntax),
            ("wrong number of arguments to function substr()", Syntax),
            ("misuse of aggregate: count()", Syntax),
            ("datatype mismatch", TypeMismatch),
            (STATEMENT_TIMEOUT_MESSAGE, Timeout),
            ("attempt to write a readonly database", GuardRejected),
            ("disk I/O error", RuntimeError),
        ];
        for (message, expected) in cases {
            assert_eq!(classify(&Error::Query(message.into())), Some(expected), "{message}");
        }
    }

    #[test]
    fn guard_and_non_query_errors() {
        assert_eq!(
            classify(&Error::GuardRejected("only read queries".into())),
            Some(FailureClass::GuardRejected)
        );
        assert_eq!(
            classify(&Error::InvalidQuery("only one statement is allowed".into())),
            Some(FailureClass::GuardRejected)
        );
        assert_eq!(classify(&Error::InvalidQuery("Empty query".into())), Some(FailureClass::Syntax));
        // Não são da consulta: nada a corrigir
        for error in [
            Error::Connection("refused".into()),
            Error::ConnectionTimeout,
            Error::PoolExhausted,
            Error::Cancelled,
            Error::UnsupportedType("mongo".into()),
        ] {
            assert_eq!(classify(&error), None, "{error:?}");
        }
    }

    #[test]
    fn classes_serialize_as_in_the_contract() {
        assert_eq!(
            serde_json::to_value(FailureClass::GuardRejected).unwrap(),
            serde_json::json!("guard_rejected")
        );
        assert_eq!(
            serde_json::to_value(FailureClass::ParadigmLimitation).unwrap(),
            serde_json::json!("paradigm_limitation")
        );
    }

    // ── Sanitização ─────────────────────────────────────────────────────────

    /// Valores que só existem no banco. Nenhum pode sair na mensagem.
    const CANARIES: &[&str] = &["CANARY_7f3a", "ana@example.com", "48213", "2026-13-45", "s3cr3t"];

    #[test]
    fn database_values_never_leave() {
        let query = "SELECT email::int, birth::date FROM users WHERE id = 42";
        let errors = [
            pg("22P02", "invalid input syntax for type integer: \"CANARY_7f3a\""),
            pg("22P02", "invalid input syntax for type integer: \"ana@example.com\""),
            pg("22007", "invalid input syntax for type date: \"2026-13-45\""),
            pg("22003", "value \"48213\" is out of range for type smallint"),
            pg("22P02", "invalid input value for enum mood: 's3cr3t'"),
            pg("22P02", "malformed array literal: \"{CANARY_7f3a\""),
            pg("XX000", "unexpected value 48213 in row"),
            Error::Query("CHECK constraint failed: s3cr3t".into()),
        ];
        for error in errors {
            let sanitized = sanitize(&error, query).unwrap();
            for canary in CANARIES {
                assert!(!sanitized.contains(canary), "{canary} vazou em: {sanitized}");
            }
        }
    }

    #[test]
    fn keeps_what_came_from_the_query() {
        let query = "SELECT nome, total FROM public.pedidos WHERE status = 'pago' LIMIT 10";
        assert_eq!(
            sanitize(&pg("42703", "column \"nome\" does not exist"), query).as_deref(),
            Some("42703: column \"nome\" does not exist")
        );
        assert_eq!(
            sanitize(&pg("42P01", "relation \"public.pedidos\" does not exist"), query).as_deref(),
            Some("42P01: relation \"public.pedidos\" does not exist")
        );
        // Identificador sem aspas na consulta: o Postgres devolve em minúsculas
        assert_eq!(
            sanitize(&pg("42703", "column \"nome\" does not exist"), "SELECT Nome FROM t").as_deref(),
            Some("42703: column \"nome\" does not exist")
        );
        // Literal escrito na consulta pode voltar; o que não está nela, não
        assert_eq!(
            sanitize(
                &pg("22P02", "invalid input value for enum st: \"pago\""),
                query
            )
            .as_deref(),
            Some("22P02: invalid input value for enum st: \"pago\"")
        );
        assert_eq!(
            sanitize(&pg("42703", "column \"secreto\" does not exist"), query).as_deref(),
            Some("42703: column \"?\" does not exist")
        );
    }

    #[test]
    fn numbers_and_type_names() {
        let query = "SELECT x FROM tenant_00042.t";
        assert_eq!(
            sanitize(&pg("22001", "value too long for type character varying(10)"), query).as_deref(),
            Some("22001: value too long for type character varying(?)")
        );
        // Número colado em nome continua: int4, tenant_00042
        assert_eq!(
            sanitize(&pg("42883", "operator does not exist: int4 = text"), query).as_deref(),
            Some("42883: operator does not exist: int4 = text")
        );
        assert_eq!(
            sanitize(&pg("42P01", "relation \"tenant_00042.t\" does not exist"), query).as_deref(),
            Some("42P01: relation \"tenant_00042.t\" does not exist")
        );
    }

    #[test]
    fn sqlite_names_without_quotes() {
        let query = "SELECT nome FROM usuarios";
        assert_eq!(
            sanitize(&Error::Query("no such column: nome".into()), query).as_deref(),
            Some("no such column: nome")
        );
        assert_eq!(
            sanitize(&Error::Query("no such table: main.CANARY_7f3a".into()), query).as_deref(),
            Some("no such table: ?")
        );
        assert_eq!(
            sanitize(&Error::Query("near \"FORM\": syntax error".into()), "SELECT 1 FORM t").as_deref(),
            Some("near \"FORM\": syntax error")
        );
        assert_eq!(
            sanitize(
                &Error::Query("misuse of aggregate: count()".into()),
                "SELECT name FROM t WHERE count(*) > 1"
            )
            .as_deref(),
            Some("misuse of aggregate: count()")
        );
        assert_eq!(
            sanitize(&Error::Query("CHECK constraint failed: s3cr3t".into()), query).as_deref(),
            Some("CHECK constraint failed: ?")
        );
    }

    #[test]
    fn unterminated_quotes_and_long_messages() {
        assert_eq!(
            sanitize(&pg("22P02", "bad value \"CANARY_7f3a"), "SELECT 1").as_deref(),
            Some("22P02: bad value \"?")
        );
        let long = pg("XX000", &"é".repeat(5000));
        assert_eq!(
            sanitize(&long, "SELECT 1").unwrap().chars().count(),
            SANITIZED_MAX_CHARS
        );
        assert_eq!(
            sanitize(&pg("XX000", "linha um\nlinha dois"), "SELECT 1").as_deref(),
            Some("XX000: linha um linha dois")
        );
    }

    #[test]
    fn quotes_inside_values_do_not_leak_the_rest() {
        // O valor tem aspas dentro: a aspa colada no resto não fecha o trecho
        assert_eq!(
            sanitize(&pg("22P02", "invalid input syntax for type integer: \"ab\"CANARY_7f3a\""), "SELECT 1").as_deref(),
            Some("22P02: invalid input syntax for type integer: \"?\"")
        );
        assert_eq!(
            sanitize(&pg("22P02", "invalid input value for enum e: 'O'Brien'"), "SELECT 1").as_deref(),
            Some("22P02: invalid input value for enum e: '?'")
        );
    }

    // ── Mongo ───────────────────────────────────────────────────────────────

    fn mongo(code: i32, code_name: &str, message: &str) -> Error {
        Error::MongoQuery {
            code,
            code_name: code_name.into(),
            message: message.into(),
        }
    }

    #[test]
    fn mongo_errors_by_code() {
        use FailureClass::*;
        let cases = [
            (50, "MaxTimeMSExpired", "operation exceeded time limit", Timeout),
            (26, "NamespaceNotFound", "collection \"pedidos\" does not exist", UnknownObject),
            (2, "BadValue", "unknown operator: $bogus", Syntax),
            (40324, "Location40324", "Unrecognized pipeline stage name: '$matc'", Syntax),
            (168, "InvalidPipelineOperator", "Unrecognized expression '$sumx'", Syntax),
            (241, "ConversionFailure", "Failed to parse number 'abc' in $convert with no onError value", TypeMismatch),
            (14, "TypeMismatch", "$add only supports numeric or date types, not string", TypeMismatch),
            (16020, "Location16020", "Expression $toLower takes exactly 1 arguments. 2 were passed in.", RuntimeError),
            (28651, "Location28651", "can't convert from BSON type string to Date", TypeMismatch),
            (16608, "Location16608", "can't $divide by zero", RuntimeError),
            (13, "Unauthorized", "not authorized on shop to execute command", RuntimeError),
        ];
        for (code, name, message, expected) in cases {
            assert_eq!(classify(&mongo(code, name, message)), Some(expected), "{code} {message}");
        }
    }

    /// RN-10 no Mongo: valores do banco nunca saem; o que veio da operação
    /// (coleção, operador, campo, literal da pergunta) pode voltar.
    #[test]
    fn mongo_values_never_leave() {
        let query = r#"{"collection":"orders","operation":"aggregate","pipeline":[
            {"$match":{"status":"pago"}},{"$project":{"n":{"$toInt":"$label"}}}]}"#;
        let errors = [
            mongo(241, "ConversionFailure", "Failed to parse number 'CANARY_7f3a' in $convert with no onError value: Did not consume whole string."),
            mongo(241, "ConversionFailure", "Failed to parse objectId 'ana@example.com' in $convert with no onError value"),
            mongo(9, "FailedToParse", "Error parsing date string '2026-13-45'; 0: passing a time zone identifier"),
            mongo(2, "BadValue", "value \"s3cr3t\" is invalid, found 48213"),
            mongo(241, "ConversionFailure", "Failed to parse number 'ab'CANARY_7f3a' in $convert"),
        ];
        for error in errors {
            let sanitized = sanitize(&error, query).unwrap();
            for canary in CANARIES {
                assert!(!sanitized.contains(canary), "{canary} vazou em: {sanitized}");
            }
        }

        assert_eq!(
            sanitize(&mongo(26, "NamespaceNotFound", "collection \"orders\" does not exist"), query).as_deref(),
            Some("NamespaceNotFound (26): collection \"orders\" does not exist")
        );
        assert_eq!(
            sanitize(&mongo(168, "InvalidPipelineOperator", "Unrecognized expression '$toInt'"), query).as_deref(),
            Some("InvalidPipelineOperator (168): Unrecognized expression '$toInt'")
        );
        assert_eq!(
            sanitize(&mongo(2, "BadValue", "$arrayElemAt's first argument must be an array, but is 'label'"), query).as_deref(),
            Some("BadValue (2): $arrayElemAt's first argument must be an array, but is 'label'")
        );
        assert_eq!(
            sanitize(&mongo(2, "", "enum value 'pago' is invalid"), query).as_deref(),
            Some("2: enum value 'pago' is invalid")
        );
    }

    fn redis(code: &str, message: &str) -> Error {
        Error::RedisQuery {
            code: code.into(),
            message: message.into(),
        }
    }

    #[test]
    fn redis_errors_by_reply_code() {
        use FailureClass::*;
        let cases = [
            ("WRONGTYPE", "Operation against a key holding the wrong kind of value", TypeMismatch),
            ("ERR", "value is not an integer or out of range", TypeMismatch),
            ("ERR", "min or max is not a float", TypeMismatch),
            ("ERR", "wrong number of arguments for 'hget' command", Syntax),
            ("ERR", "syntax error", Syntax),
            ("ERR", "unknown command 'LPOS' in this Redis version", Syntax),
            ("ERR", "Invalid stream ID specified as stream command argument", Syntax),
            ("READONLY", "You can't write against a read only replica.", GuardRejected),
            ("NOPERM", "this user has no permissions to run the 'hgetall' command", RuntimeError),
            ("CROSSSLOT", "Keys in request don't hash to the same slot", RuntimeError),
            ("ERR", "something else", RuntimeError),
        ];
        for (code, message, expected) in cases {
            assert_eq!(classify(&redis(code, message)), Some(expected), "{code} {message}");
        }
        assert_eq!(
            classify(&Error::PlanLimit("the plan needs more than 5000 commands".into())),
            Some(ParadigmLimitation)
        );
    }

    /// RN-10 no Redis: a chave que entrou no lugar de `$item` (dado do banco)
    /// e os valores nunca saem; o que veio do plano (comando, padrão, campo)
    /// pode voltar.
    #[test]
    fn redis_keys_and_values_never_leave() {
        let query = r#"{"steps":[
            {"id":"users","command":"SCAN","args":["0","MATCH","user:*"],"foreach":null},
            {"id":"ages","command":"HINCRBY","args":["$item","age","x"],"foreach":"users"}],"output":"ages"}"#;
        let errors = [
            redis("ERR", "unknown command 'HINCRBY', with args beginning with: 'user:CANARY_7f3a' 'age' "),
            redis("ERR", "value 's3cr3t' is not an integer"),
            redis("ERR", "Error at key \"ana@example.com\": 48213 is out of range"),
            redis("MOVED", "48213 10.0.0.7:6379"),
        ];
        for error in errors {
            let sanitized = sanitize(&error, query).unwrap();
            for canary in CANARIES.iter().chain(&["10.0.0.7", "user:"]) {
                assert!(!sanitized.contains(canary), "{canary} vazou em: {sanitized}");
            }
        }
        assert_eq!(
            sanitize(&redis("ERR", "wrong number of arguments for 'hincrby' command"), query).as_deref(),
            Some("ERR: wrong number of arguments for 'hincrby' command")
        );
        assert_eq!(
            sanitize(&redis("WRONGTYPE", "Operation against a key holding the wrong kind of value"), query).as_deref(),
            Some("WRONGTYPE: Operation against a key holding the wrong kind of value")
        );
        // O teto é texto nosso: vai inteiro, com os números que o modelo precisa
        assert_eq!(
            sanitize(&Error::PlanLimit("step \"ages\" iterates over more than 1000 items".into()), query).as_deref(),
            Some("step \"ages\" iterates over more than 1000 items")
        );
    }

    #[test]
    fn non_query_errors_have_nothing_to_send() {
        assert_eq!(sanitize(&Error::Connection("host=x password=y".into()), "SELECT 1"), None);
        assert_eq!(sanitize(&Error::Cancelled, "SELECT 1"), None);
    }
}
