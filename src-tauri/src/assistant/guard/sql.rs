//! Guard das consultas SQL geradas pelo assistente (Postgres e SQLite).
//!
//! Falha fechado: na dúvida, recusa. São duas camadas sobre o `sqlparser`:
//!
//! 1. **AST:** o texto precisa parsear e ser exatamente um statement do tipo
//!    `Query` (`SELECT`, `WITH … SELECT`, `VALUES`, `TABLE`). DDL, DML,
//!    `COPY`, `SET`, `PRAGMA`, `ATTACH`, controle de transação: tudo fora.
//! 2. **Tokens:** a varredura do texto tokenizado (strings, `$$`, comentários
//!    e identificadores entre aspas já separados pelo tokenizer) acha em
//!    qualquer profundidade o que cabe dentro de uma `Query`, sem depender de
//!    percorrer a AST inteira: CTE que modifica dados (`INSERT`/`UPDATE`/
//!    `DELETE`/`MERGE`), `SELECT INTO`, `FOR UPDATE/SHARE` e chamadas a
//!    funções com efeito colateral.
//!
//! O guard não é a única barreira: o executor roda tudo numa transação
//! `READ ONLY` (ou `query_only`), num statement só. A denylist mira o que essa
//! transação **não** impede — encerrar sessões, lock de sessão, ler arquivo do
//! servidor, abrir conexão para fora (`dblink`), SQL dinâmico.

use std::fmt;

use sqlparser::ast::Statement;
use sqlparser::dialect::{Dialect, PostgreSqlDialect, SQLiteDialect};
use sqlparser::parser::Parser;
use sqlparser::tokenizer::{Token, Tokenizer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlDialect {
    Postgres,
    Sqlite,
}

/// Por que o guard recusou a consulta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    Empty,
    /// O parser não entendeu: na dúvida, não executa
    Unparseable,
    MultipleStatements,
    /// Um statement que não é consulta (DDL, DML, `SET`, `PRAGMA`…)
    NotAQuery,
    /// `INSERT`/`UPDATE`/`DELETE`/`MERGE` dentro da consulta (CTE)
    WriteKeyword(String),
    SelectInto,
    /// `FOR UPDATE`, `FOR SHARE`, `FOR NO KEY UPDATE`, `FOR KEY SHARE`
    LockingClause,
    ForbiddenFunction(String),
    /// Identificador com escape Unicode (`U&"…"`): esconderia o nome real
    UnicodeEscape,
}

impl Rejection {
    /// Código estável, para log e classificação (nunca o texto da consulta).
    pub fn code(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Unparseable => "unparseable",
            Self::MultipleStatements => "multiple_statements",
            Self::NotAQuery => "not_a_query",
            Self::WriteKeyword(_) => "write_keyword",
            Self::SelectInto => "select_into",
            Self::LockingClause => "locking_clause",
            Self::ForbiddenFunction(_) => "forbidden_function",
            Self::UnicodeEscape => "unicode_escape",
        }
    }
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("empty query"),
            Self::Unparseable => f.write_str("the query could not be parsed"),
            Self::MultipleStatements => f.write_str("only one statement is allowed"),
            Self::NotAQuery => f.write_str("only read queries (SELECT) are allowed"),
            Self::WriteKeyword(keyword) => write!(f, "{keyword} is not allowed in a read query"),
            Self::SelectInto => f.write_str("SELECT INTO is not allowed"),
            Self::LockingClause => f.write_str("locking clauses (FOR UPDATE/SHARE) are not allowed"),
            Self::ForbiddenFunction(name) => write!(f, "function {name} is not allowed"),
            Self::UnicodeEscape => f.write_str("Unicode-escaped identifiers are not allowed"),
        }
    }
}

/// Palavras-chave de escrita que cabem dentro de uma `Query` (numa CTE).
const WRITE_KEYWORDS: &[&str] = &["INSERT", "UPDATE", "DELETE", "MERGE"];

/// Depois de `FOR`, abrem uma cláusula de lock.
const LOCK_STARTERS: &[&str] = &["UPDATE", "SHARE", "NO", "KEY"];

/// Funções com efeito colateral que a transação `READ ONLY` não impede, ou que
/// não deveriam rodar a pedido de um modelo. Escrita a partir da documentação
/// do Postgres (funções de administração do sistema, objetos grandes,
/// sequências, locks consultivos, XML) e das funções perigosas do SQLite.
/// Nome em minúsculas; vale com qualquer qualificação (`pg_catalog.setval`).
const FORBIDDEN_FUNCTIONS: &[&str] = &[
    // Configuração e sessões (Postgres: Admin Functions)
    "set_config",
    "pg_cancel_backend",
    "pg_terminate_backend",
    "pg_reload_conf",
    "pg_rotate_logfile",
    "pg_log_backend_memory_contexts",
    "pg_promote",
    // Sequências: a escrita delas não volta no ROLLBACK
    "nextval",
    "setval",
    // Arquivos do servidor
    "pg_read_file",
    "pg_read_binary_file",
    "pg_stat_file",
    // WAL, backup e recuperação
    "pg_switch_wal",
    "pg_backup_start",
    "pg_backup_stop",
    "pg_start_backup",
    "pg_stop_backup",
    "pg_wal_replay_pause",
    "pg_wal_replay_resume",
    // Replicação
    "pg_drop_replication_slot",
    "pg_copy_physical_replication_slot",
    "pg_copy_logical_replication_slot",
    "pg_replication_slot_advance",
    "pg_logical_emit_message",
    // Índices (manutenção que escreve)
    "gin_clean_pending_list",
    "brin_summarize_new_values",
    "brin_summarize_range",
    "brin_desummarize_range",
    // Notificação, espera e transação
    "pg_notify",
    "pg_sleep",
    "pg_sleep_for",
    "pg_sleep_until",
    "txid_current",
    "pg_current_xact_id",
    "pg_import_system_collations",
    // SQL dinâmico: executam o texto que recebem, fora do alcance do guard
    "query_to_xml",
    "query_to_xmlschema",
    "query_to_xml_and_xmlschema",
    "cursor_to_xml",
    "cursor_to_xmlschema",
    // Objetos grandes (os `lo_*` vão pelo prefixo)
    "loread",
    "lowrite",
    // SQLite: extensões e arquivos (shell e extensões comuns)
    "load_extension",
    "readfile",
    "writefile",
    "edit",
    "fts3_tokenizer",
];

/// Famílias inteiras, pelo prefixo do nome.
const FORBIDDEN_PREFIXES: &[&str] = &[
    "lo_",
    "dblink",
    "pg_advisory",
    "pg_try_advisory",
    "pg_ls_",
    "pg_create_",
    "pg_replication_origin_",
    "pg_logical_slot_",
    "pg_stat_reset",
    "pg_stat_statements_reset",
    "pg_file_",
    "binary_upgrade_",
];

fn is_forbidden_function(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    FORBIDDEN_FUNCTIONS.contains(&name.as_str())
        || FORBIDDEN_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
}

/// Confere se `sql` é uma consulta de leitura que pode ir ao executor.
pub fn check(sql: &str, dialect: SqlDialect) -> Result<(), Rejection> {
    if sql.trim().trim_end_matches(';').trim().is_empty() {
        return Err(Rejection::Empty);
    }
    let dialect: &dyn Dialect = match dialect {
        SqlDialect::Postgres => &PostgreSqlDialect {},
        SqlDialect::Sqlite => &SQLiteDialect {},
    };

    let statements = Parser::parse_sql(dialect, sql).map_err(|_| Rejection::Unparseable)?;
    match statements.as_slice() {
        [] => return Err(Rejection::Empty),
        [Statement::Query(_)] => {}
        [_] => return Err(Rejection::NotAQuery),
        _ => return Err(Rejection::MultipleStatements),
    }

    let tokens = Tokenizer::new(dialect, sql)
        .tokenize()
        .map_err(|_| Rejection::Unparseable)?;
    check_tokens(&tokens)
}

fn check_tokens(tokens: &[Token]) -> Result<(), Rejection> {
    // Identificador com escape Unicode: `U&"\0073etval"` é `setval`. O
    // tokenizer não junta o `U&` ao identificador, então o padrão é visto nos
    // tokens crus (sem espaço entre eles).
    for pair in tokens.windows(2) {
        if let [Token::Word(word), Token::Ampersand] = pair {
            if word.quote_style.is_none() && word.value.eq_ignore_ascii_case("u") {
                return Err(Rejection::UnicodeEscape);
            }
        }
    }

    // Daqui em diante, só o que conta: espaços e comentários saem
    let significant: Vec<&Token> = tokens
        .iter()
        .filter(|token| !matches!(token, Token::Whitespace(_)))
        .collect();

    for (index, token) in significant.iter().enumerate() {
        let Token::Word(word) = token else { continue };
        let next = significant.get(index + 1);

        // Chamada de função: nome (com ou sem aspas) seguido de `(`
        if matches!(next, Some(Token::LParen)) && is_forbidden_function(&word.value) {
            return Err(Rejection::ForbiddenFunction(word.value.to_ascii_lowercase()));
        }

        // Palavras-chave só valem sem aspas: `"update"` é um identificador
        if word.quote_style.is_some() {
            continue;
        }
        let keyword = word.value.to_ascii_uppercase();

        if keyword == "UESCAPE" {
            return Err(Rejection::UnicodeEscape);
        }
        if keyword == "INTO" {
            return Err(Rejection::SelectInto);
        }
        if keyword == "FOR" {
            let starts_lock = matches!(
                next,
                Some(Token::Word(next)) if next.quote_style.is_none()
                    && LOCK_STARTERS.contains(&next.value.to_ascii_uppercase().as_str())
            );
            if starts_lock {
                return Err(Rejection::LockingClause);
            }
        }
        if WRITE_KEYWORDS.contains(&keyword.as_str()) {
            return Err(Rejection::WriteKeyword(keyword));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use SqlDialect::{Postgres, Sqlite};

    fn allowed(sql: &str, dialect: SqlDialect) {
        if let Err(rejection) = check(sql, dialect) {
            panic!("devia passar ({dialect:?}): {sql}\n  recusada: {rejection}");
        }
    }

    fn rejected(sql: &str, dialect: SqlDialect) -> Rejection {
        match check(sql, dialect) {
            Ok(()) => panic!("devia recusar ({dialect:?}): {sql}"),
            Err(rejection) => rejection,
        }
    }

    fn both(sql: &str) -> [Rejection; 2] {
        [rejected(sql, Postgres), rejected(sql, Sqlite)]
    }

    // ── O que passa ─────────────────────────────────────────────────────────

    #[test]
    fn plain_reads_pass() {
        let reads = [
            "SELECT 1",
            "select * from users",
            "SELECT id, name FROM users WHERE age > 30 ORDER BY name LIMIT 10;",
            "SELECT count(*) FROM orders GROUP BY status HAVING count(*) > 2",
            "WITH recent AS (SELECT * FROM orders WHERE created_at > '2026-01-01') \
             SELECT customer_id, sum(total) FROM recent GROUP BY 1",
            "SELECT u.name, o.total FROM users u JOIN orders o ON o.user_id = u.id",
            "SELECT * FROM a UNION ALL SELECT * FROM b",
            "VALUES (1, 'a'), (2, 'b')",
            "SELECT lower(name), coalesce(email, '-'), length(name) FROM users",
            "SELECT * FROM users WHERE id IN (SELECT user_id FROM orders)",
            "SELECT 1 -- comentário no fim",
            "/* cabeçalho */ SELECT 1",
        ];
        for sql in reads {
            allowed(sql, Postgres);
            allowed(sql, Sqlite);
        }
    }

    #[test]
    fn postgres_specific_reads_pass() {
        for sql in [
            "SELECT now()::date, 'abc'::text, ARRAY[1,2,3]",
            "SELECT jsonb_build_object('a', 1) ->> 'a'",
            "SELECT * FROM generate_series(1, 10) AS g(n)",
            "SELECT currval('orders_id_seq')",
            "SELECT current_setting('search_path')",
            "SELECT $$ dollar ; DROP TABLE x $$",
            "SELECT * FROM \"Users\" WHERE \"Name\" = 'x'",
            "SELECT substring(name FROM 1 FOR 3) FROM users",
            "SELECT pg_size_pretty(pg_relation_size('users'))",
        ] {
            allowed(sql, Postgres);
        }
    }

    #[test]
    fn sqlite_specific_reads_pass() {
        for sql in [
            "SELECT * FROM `users` WHERE [name] = 'x'",
            "SELECT name FROM pragma_table_info('users')",
            "SELECT replace(name, 'a', 'b') FROM users",
            "SELECT datetime('now'), json_extract(data, '$.a') FROM events",
        ] {
            allowed(sql, Sqlite);
        }
    }

    #[test]
    fn keywords_inside_strings_comments_and_quotes_pass() {
        for sql in [
            "SELECT 'DELETE FROM users; DROP TABLE users' AS texto",
            "SELECT 'it''s FOR UPDATE' AS x",
            "SELECT 1 /* INSERT INTO t VALUES (1) */",
            "SELECT 1 -- ; DELETE FROM users",
            "SELECT \"update\", \"delete\" FROM audit",
            "SELECT 'setval(''x'', 1)'",
        ] {
            allowed(sql, Postgres);
            allowed(sql, Sqlite);
        }
    }

    // ── O que não passa ─────────────────────────────────────────────────────

    #[test]
    fn empty_and_garbage_are_rejected() {
        assert_eq!(both("   ;  "), [Rejection::Empty, Rejection::Empty]);
        assert!(check("SELEKT 1", Postgres).is_err());
        assert_eq!(rejected("SELECT FROM WHERE (", Postgres), Rejection::Unparseable);
        assert_eq!(rejected("SELECT 'unterminated", Sqlite), Rejection::Unparseable);
    }

    #[test]
    fn writes_and_ddl_are_not_queries() {
        for sql in [
            "DELETE FROM users",
            "delete from users where id = 1",
            "UPDATE users SET name = 'x'",
            "INSERT INTO users (name) VALUES ('x')",
            "DROP TABLE users",
            "CREATE TABLE t (id int)",
            "ALTER TABLE users ADD COLUMN x int",
            "TRUNCATE users",
            "GRANT ALL ON users TO public",
            "BEGIN",
            "COMMIT",
            "SET search_path TO evil",
            "EXPLAIN ANALYZE DELETE FROM users",
            "COPY users TO '/tmp/x'",
        ] {
            assert_eq!(rejected(sql, Postgres), Rejection::NotAQuery, "{sql}");
        }
        for sql in [
            "PRAGMA writable_schema = ON",
            "PRAGMA journal_mode = DELETE",
            "ATTACH DATABASE '/tmp/x.db' AS x",
            "VACUUM",
            "REPLACE INTO users (id) VALUES (1)",
        ] {
            assert!(check(sql, Sqlite).is_err(), "{sql}");
        }
    }

    #[test]
    fn multiple_statements_are_rejected() {
        for sql in [
            "SELECT 1; DELETE FROM users",
            "SELECT 1; SELECT 2",
            "SELECT 1;COMMIT;DROP TABLE users",
            "SELECT * FROM users; -- só um comentário\nDROP TABLE users",
        ] {
            assert_eq!(both(sql)[0], Rejection::MultipleStatements, "{sql}");
            assert!(check(sql, Sqlite).is_err(), "{sql}");
        }
    }

    #[test]
    fn commit_in_the_middle_is_rejected() {
        // A tentativa de sair da transação READ ONLY e voltar a escrever
        let sql = "SELECT 1) AS x; COMMIT; DROP TABLE users; SELECT * FROM (SELECT 1";
        assert!(check(sql, Postgres).is_err());
        assert!(check(sql, Sqlite).is_err());
    }

    #[test]
    fn data_modifying_ctes_are_rejected() {
        for sql in [
            "WITH gone AS (DELETE FROM users RETURNING *) SELECT * FROM gone",
            "WITH x AS (UPDATE users SET name = 'a' RETURNING id) SELECT * FROM x",
            "WITH x AS (INSERT INTO users (name) VALUES ('a') RETURNING id) SELECT 1",
        ] {
            let rejection = check(sql, Postgres).unwrap_err();
            assert!(
                matches!(rejection, Rejection::WriteKeyword(_) | Rejection::Unparseable | Rejection::NotAQuery),
                "{sql}: {rejection:?}"
            );
        }
        assert_eq!(
            rejected("WITH gone AS (DeLeTe FROM users RETURNING *) SELECT * FROM gone", Postgres),
            Rejection::WriteKeyword("DELETE".into())
        );
    }

    #[test]
    fn select_into_is_rejected() {
        assert_eq!(rejected("SELECT * INTO copia FROM users", Postgres), Rejection::SelectInto);
        assert_eq!(
            rejected("select id into temporary t from users", Postgres),
            Rejection::SelectInto
        );
    }

    #[test]
    fn locking_clauses_are_rejected_at_any_depth() {
        for sql in [
            "SELECT * FROM users FOR UPDATE",
            "SELECT * FROM users for share",
            "SELECT * FROM users FOR NO KEY UPDATE",
            "SELECT * FROM users FOR KEY SHARE SKIP LOCKED",
            "SELECT * FROM (SELECT * FROM users FOR UPDATE) AS s",
            "SELECT * FROM users FOR\n  /* x */ UPDATE NOWAIT",
        ] {
            // O parser não conhece todas as formas (`FOR NO KEY UPDATE`): aí
            // a recusa vem antes, como `Unparseable` — o que importa é recusar
            let rejection = rejected(sql, Postgres);
            assert!(
                matches!(rejection, Rejection::LockingClause | Rejection::Unparseable),
                "{sql}: {rejection:?}"
            );
        }
        assert_eq!(
            rejected("SELECT * FROM users FOR UPDATE", Postgres),
            Rejection::LockingClause
        );
    }

    #[test]
    fn syntax_the_parser_does_not_know_fails_closed() {
        // `TABLE t` é leitura válida no Postgres, mas o sqlparser não parseia:
        // fica recusada em vez de passar sem conferência
        assert_eq!(rejected("TABLE users", Postgres), Rejection::Unparseable);
    }

    #[test]
    fn forbidden_functions_are_rejected_in_any_form() {
        for (sql, name) in [
            ("SELECT nextval('orders_id_seq')", "nextval"),
            ("SELECT SETVAL('orders_id_seq', 1)", "setval"),
            ("SELECT pg_catalog.setval('s', 1)", "setval"),
            ("SELECT \"setval\"('s', 1)", "setval"),
            ("SELECT setval /* x */ ('s', 1)", "setval"),
            ("SELECT pg_terminate_backend(pid) FROM pg_stat_activity", "pg_terminate_backend"),
            ("SELECT set_config('search_path', 'evil', false)", "set_config"),
            ("SELECT * FROM dblink('host=x', 'DELETE FROM t') AS t(a int)", "dblink"),
            ("SELECT dblink_exec('DROP TABLE t')", "dblink_exec"),
            ("SELECT lo_import('/etc/passwd')", "lo_import"),
            ("SELECT lo_export(1, '/tmp/x')", "lo_export"),
            ("SELECT pg_advisory_lock(1)", "pg_advisory_lock"),
            ("SELECT pg_read_file('/etc/passwd')", "pg_read_file"),
            ("SELECT * FROM pg_ls_dir('.')", "pg_ls_dir"),
            ("SELECT query_to_xml('DELETE FROM t', true, true, '')", "query_to_xml"),
            ("SELECT pg_sleep(100)", "pg_sleep"),
            ("SELECT 1 WHERE (SELECT nextval('s')) > 0", "nextval"),
            ("WITH x AS (SELECT pg_cancel_backend(1)) SELECT * FROM x", "pg_cancel_backend"),
        ] {
            assert_eq!(
                rejected(sql, Postgres),
                Rejection::ForbiddenFunction(name.into()),
                "{sql}"
            );
        }
        for (sql, name) in [
            ("SELECT load_extension('/tmp/evil')", "load_extension"),
            ("SELECT writefile('/tmp/x', 'y')", "writefile"),
            ("SELECT readfile('/etc/passwd')", "readfile"),
        ] {
            assert_eq!(rejected(sql, Sqlite), Rejection::ForbiddenFunction(name.into()), "{sql}");
        }
    }

    #[test]
    fn function_names_as_plain_columns_pass() {
        // Sem `(` não é chamada: uma coluna chamada `nextval` é só dado
        allowed("SELECT nextval FROM config", Postgres);
        allowed("SELECT t.lo_size FROM t", Postgres);
    }

    #[test]
    fn unicode_escaped_identifiers_are_rejected() {
        assert_eq!(
            rejected("SELECT U&\"\\0073etval\"('s', 1)", Postgres),
            Rejection::UnicodeEscape
        );
        assert!(check("SELECT U&\"!0073etval\" UESCAPE '!' ('s', 1)", Postgres).is_err());
    }

    #[test]
    fn rejections_have_stable_codes_and_messages() {
        let rejection = Rejection::ForbiddenFunction("setval".into());
        assert_eq!(rejection.code(), "forbidden_function");
        assert_eq!(rejection.to_string(), "function setval is not allowed");
    }
}
