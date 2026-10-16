//! Linha de comando das ferramentas do Postgres — funções puras, testáveis sem
//! banco nem binário.
//!
//! A senha nunca entra no argv (qualquer usuário da máquina o vê no `ps`): sai
//! em [`ConnectionArgs::password`] e vai por `PGPASSWORD` no ambiente do filho.

use chrono::NaiveDateTime;

use crate::models::{
    BackupOptions, BackupTarget, DumpContent, DumpFormat, RestoreFormat, RestoreOptions, Server,
};

/// Como o processo chega ao banco: argumentos sem segredo + a senha à parte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionArgs {
    pub args: Vec<String>,
    pub password: Option<String>,
    /// `PGSSLMODE` padrão; um `sslmode` na própria URI prevalece sobre ele
    pub ssl_mode: &'static str,
    /// CA própria (`PGSSLROOTCERT`) dos modos `verify-*`
    pub ssl_root_cert: Option<String>,
}

/// O TLS do processo, pelo ambiente: o modo do servidor e, nos `verify-*`, a
/// CA — a do servidor ou `system`, as CAs do sistema (libpq 16+; uma libpq
/// mais velha lê `system` como caminho de arquivo e recusa com erro claro).
pub fn ssl_env(connection: &ConnectionArgs) -> Vec<(&'static str, String)> {
    let mut env = vec![("PGSSLMODE", connection.ssl_mode.to_string())];
    if matches!(connection.ssl_mode, "verify-ca" | "verify-full") {
        let root = connection.ssl_root_cert.clone().unwrap_or_else(|| "system".into());
        env.push(("PGSSLROOTCERT", root));
    }
    env
}

/// Argumentos de conexão para `database` no servidor. Com `connection_uri`, a
/// URI vai no `--dbname` sem a senha e com o database trocado pelo escolhido
/// (como faz o pool); sem ela, `--host`/`--port`/`--username`.
pub fn connection_args(server: &Server, database: &str) -> ConnectionArgs {
    // O mesmo modo do pool (pool.rs), com os nomes da libpq
    let ssl_mode = server.ssl_mode.as_str();
    let ssl_root_cert = server
        .ssl_root_cert
        .clone()
        .filter(|_| server.ssl_mode.verifies());
    let server_password = Some(server.password.clone()).filter(|p| !p.is_empty());

    if let Some(uri) = server.connection_uri.as_deref().map(str::trim) {
        if !uri.is_empty() {
            let (dbname, uri_password) = if is_uri(uri) {
                rewrite_uri(uri, database)
            } else {
                rewrite_conninfo(uri, database)
            };
            return ConnectionArgs {
                args: vec![format!("--dbname={dbname}")],
                password: uri_password.or(server_password),
                ssl_mode,
                ssl_root_cert,
            };
        }
    }

    let mut args = Vec::new();
    if !server.host.is_empty() {
        args.push(format!("--host={}", server.host));
    }
    if server.port != 0 {
        args.push(format!("--port={}", server.port));
    }
    if !server.username.is_empty() {
        args.push(format!("--username={}", server.username));
    }
    args.push(format!("--dbname={}", dbname_value(database)));
    ConnectionArgs {
        args,
        password: server_password,
        ssl_mode,
        ssl_root_cert,
    }
}

/// Argumentos do `pg_dump` (sem o programa). `file` é onde ele grava — o
/// `.partial`, que vira o arquivo final só no sucesso.
pub fn dump_args(
    connection: &ConnectionArgs,
    target: &BackupTarget,
    options: &BackupOptions,
    file: &str,
) -> Result<Vec<String>, String> {
    validate(options)?;

    let mut args = vec![
        // O log do `--verbose` é o progresso que o front mostra
        "--verbose".to_string(),
        // Sem terminal para perguntar a senha: falha clara em vez de travar
        "--no-password".to_string(),
        match options.format {
            DumpFormat::Custom => "--format=custom".to_string(),
            DumpFormat::Plain => "--format=plain".to_string(),
        },
        format!("--file={file}"),
    ];
    match options.content {
        DumpContent::All => {}
        DumpContent::SchemaOnly => args.push("--schema-only".into()),
        DumpContent::DataOnly => args.push("--data-only".into()),
    }
    if options.no_owner {
        args.push("--no-owner".into());
        args.push("--no-privileges".into());
    }
    if options.clean {
        args.push("--clean".into());
        args.push("--if-exists".into());
    }
    if options.column_inserts {
        args.push("--column-inserts".into());
    }
    match target {
        BackupTarget::Database => {}
        BackupTarget::Schema { schema } => {
            args.push(format!("--schema={}", quote_pattern(schema)));
        }
        BackupTarget::Table { schema, table } => {
            args.push(format!(
                "--table={}.{}",
                quote_pattern(schema),
                quote_pattern(table)
            ));
        }
    }
    args.extend(connection.args.iter().cloned());
    Ok(args)
}

/// Programa e argumentos da restauração de `file`: o `.dump` vai pelo
/// `pg_restore`, o script SQL pelo `psql` (o executor do app não sabe o
/// `COPY ... FROM stdin` que o script usa).
pub fn restore_args(
    connection: &ConnectionArgs,
    format: RestoreFormat,
    options: &RestoreOptions,
    file: &str,
) -> Result<(&'static str, Vec<String>), String> {
    match format {
        RestoreFormat::Custom => {
            let mut args = vec!["--verbose".to_string(), "--no-password".to_string()];
            if options.no_owner {
                args.push("--no-owner".into());
                args.push("--no-privileges".into());
            }
            if options.clean {
                args.push("--clean".into());
                args.push("--if-exists".into());
            }
            if options.single_transaction {
                args.push("--single-transaction".into());
                args.push("--exit-on-error".into());
            }
            args.extend(connection.args.iter().cloned());
            args.push(file.to_string());
            Ok(("pg_restore", args))
        }
        RestoreFormat::Plain => {
            // Dono, GRANTs e DROPs já estão escritos no script
            if options.no_owner || options.clean {
                return Err(
                    "No script SQL, dono, permissões e DROP são os que estão no arquivo.".into(),
                );
            }
            let mut args = vec![
                // Um ~/.psqlrc do usuário não pode mudar o comportamento
                "--no-psqlrc".to_string(),
                "--no-password".to_string(),
            ];
            if options.single_transaction {
                args.push("--set=ON_ERROR_STOP=1".into());
                args.push("--single-transaction".into());
            }
            args.push(format!("--file={file}"));
            args.extend(connection.args.iter().cloned());
            Ok(("psql", args))
        }
    }
}

/// De onde o `psql` lê, quando a restauração passa pelo filtro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanInput {
    /// O script SQL, lido pelo app
    File(String),
    /// O `pg_restore` gerando o SQL do `.dump` (estes argumentos)
    PgRestore(Vec<String>),
}

/// Como uma restauração roda: o programa principal, os argumentos e, com o
/// filtro, o que alimenta o stdin dele.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestorePlan {
    pub tool: &'static str,
    pub args: Vec<String>,
    pub input: Option<PlanInput>,
    /// `pg_restore` direto, sem "tudo ou nada": sai com 1 depois de pular erros
    pub tolerate_ignored_errors: bool,
}

/// O plano da restauração. Sem o filtro, cada formato vai pela sua ferramenta
/// direto no banco; com ele (ferramentas 17+ num servidor anterior ao 17), o
/// SQL passa pelo app até o `psql`, sem o `SET transaction_timeout` que esse
/// servidor recusaria — e que, no "tudo ou nada", derrubaria tudo.
pub fn restore_plan(
    connection: &ConnectionArgs,
    format: RestoreFormat,
    options: &RestoreOptions,
    file: &str,
    filter: bool,
) -> Result<RestorePlan, String> {
    let (tool, direct) = restore_args(connection, format, options, file)?;
    if !filter {
        return Ok(RestorePlan {
            tool,
            args: direct,
            input: None,
            tolerate_ignored_errors: format == RestoreFormat::Custom && !options.single_transaction,
        });
    }

    let mut psql = vec!["--no-psqlrc".to_string(), "--no-password".to_string()];
    if options.single_transaction {
        psql.push("--set=ON_ERROR_STOP=1".into());
        psql.push("--single-transaction".into());
    }
    psql.push("--file=-".into());
    psql.extend(connection.args.iter().cloned());

    let input = match format {
        RestoreFormat::Plain => PlanInput::File(file.to_string()),
        RestoreFormat::Custom => {
            // Só gera o SQL (sem conexão): quem aplica e controla a transação é o psql
            let mut producer = vec!["--verbose".to_string()];
            if options.no_owner {
                producer.push("--no-owner".into());
                producer.push("--no-privileges".into());
            }
            if options.clean {
                producer.push("--clean".into());
                producer.push("--if-exists".into());
            }
            producer.push("--file=-".into());
            producer.push(file.to_string());
            PlanInput::PgRestore(producer)
        }
    };
    Ok(RestorePlan {
        tool: "psql",
        args: psql,
        input: Some(input),
        tolerate_ignored_errors: false,
    })
}

/// A linha de comando do plano, como o usuário rodaria num terminal.
pub fn display_plan(plan: &RestorePlan) -> String {
    let main = display_command(plan.tool, &plan.args);
    match &plan.input {
        None => main,
        Some(input) => {
            let command = match input {
                PlanInput::File(file) => format!("{main} < {}", shell_quote(file)),
                PlanInput::PgRestore(args) => {
                    format!("{} | {main}", display_command("pg_restore", args))
                }
            };
            format!(
                "{command}\n# sem a linha \"SET transaction_timeout = 0;\": o servidor é anterior ao 17 e não a conhece"
            )
        }
    }
}

/// Combinações que o `pg_dump` recusa ou ignora em silêncio viram erro aqui,
/// para o usuário não achar que pediu algo que não aconteceu.
fn validate(options: &BackupOptions) -> Result<(), String> {
    if options.clean && options.format != DumpFormat::Plain {
        return Err(
            "\"Apagar antes de criar\" só vale no script SQL; no .dump, escolha isso na restauração."
                .into(),
        );
    }
    if options.clean && options.content == DumpContent::DataOnly {
        return Err("\"Apagar antes de criar\" não combina com \"Só dados\".".into());
    }
    if options.column_inserts && options.format != DumpFormat::Plain {
        return Err("\"INSERT em vez de COPY\" só vale no script SQL.".into());
    }
    if options.column_inserts && options.content == DumpContent::SchemaOnly {
        return Err("\"INSERT em vez de COPY\" não combina com \"Só estrutura\".".into());
    }
    Ok(())
}

/// Nome sugerido no diálogo de salvar: `loja_public_pedidos_20261015_1432.dump`.
pub fn suggested_file_name(
    database: &str,
    target: &BackupTarget,
    format: DumpFormat,
    now: NaiveDateTime,
) -> String {
    let mut parts = vec![database];
    match target {
        BackupTarget::Database => {}
        BackupTarget::Schema { schema } => parts.push(schema),
        BackupTarget::Table { schema, table } => {
            parts.push(schema);
            parts.push(table);
        }
    }
    let stem = parts
        .iter()
        .map(|part| sanitize_file_part(part))
        .collect::<Vec<_>>()
        .join("_");
    format!(
        "{stem}_{}.{}",
        now.format("%Y%m%d_%H%M"),
        format.extension()
    )
}

/// O arquivo enquanto o processo escreve; vira o final só no sucesso.
pub fn partial_path(path: &str) -> String {
    format!("{path}.partial")
}

/// A linha de comando como o usuário colaria num terminal (POSIX).
pub fn display_command(program: &str, args: &[String]) -> String {
    std::iter::once(program.to_string())
        .chain(args.iter().map(|arg| shell_quote(arg)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A mensagem de erro de uma execução que falhou, a partir das últimas linhas
/// do stderr: as de erro (em inglês ou traduzidas), senão as últimas.
pub fn error_message(tool: &str, tail: &[String], exit_code: Option<i32>) -> String {
    let errors: Vec<&str> = tail
        .iter()
        .map(String::as_str)
        .filter(|line| is_error_line(line))
        .collect();
    let lines: Vec<&str> = if errors.is_empty() {
        tail.iter()
            .rev()
            .take(5)
            .rev()
            .map(String::as_str)
            .collect()
    } else {
        errors
    };
    if lines.is_empty() {
        match exit_code {
            Some(code) => format!("{tool} terminou com o código {code}."),
            None => format!("{tool} foi interrompido."),
        }
    } else {
        lines.join("\n")
    }
}

/// Linha de erro das ferramentas, em inglês ou traduzida (`pg_dump: error:`,
/// `psql:arq.sql:12: ERROR:`, `FATAL:`).
pub fn is_error_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.contains("error:") || lower.contains("erro:") || lower.contains("fatal")
}

/// O aviso final do `pg_restore` que seguiu depois de erros
/// (`pg_restore: warning: errors ignored on restore: 3`).
pub fn is_ignored_errors_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.contains("errors ignored on restore") || lower.contains("erros ignorados")
}

/// O `pg_dump`/`pg_restore` 17+ escrevem esta linha sempre, e um servidor
/// anterior ao 17 não conhece o parâmetro: na entrada do `psql` ela some.
const TRANSACTION_TIMEOUT_LINE: &[u8] = b"SET transaction_timeout = 0;";

/// Se uma linha da entrada filtrada segue para o `psql`.
pub fn keep_input_line(line: &[u8]) -> bool {
    line.trim_ascii_end() != TRANSACTION_TIMEOUT_LINE
}

/// Major de um número no formato do `server_version_num` (≥ 10).
fn major_of(num: u32) -> u32 {
    num / 10_000
}

/// Restaurar com ferramentas 17+ num servidor anterior ao 17 precisa do filtro
/// do `SET transaction_timeout` (ver [`restore_plan`]).
pub fn needs_timeout_filter(tools_num: u32, server_num: u32) -> bool {
    server_num > 0 && major_of(tools_num) >= 17 && major_of(server_num) < 17
}

/// Um nome literal no padrão do `-n`/`-t` (regras do psql): entre aspas,
/// `*`, `?`, `.` e maiúsculas valem ao pé da letra; aspas dobram.
fn quote_pattern(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// O `--dbname` trata como string de conexão o que tem `=` ou começa com
/// `postgres://`; um nome assim vai entre aspas de conninfo.
fn dbname_value(database: &str) -> String {
    if database.contains('=') || is_uri(database) {
        format!("dbname={}", conninfo_quote(database))
    } else {
        database.to_string()
    }
}

fn is_uri(value: &str) -> bool {
    value.starts_with("postgres://") || value.starts_with("postgresql://")
}

/// Tira a senha da URI (devolvida decodificada) e troca o database pelo
/// escolhido, mantendo usuário, hosts e parâmetros.
fn rewrite_uri(uri: &str, database: &str) -> (String, Option<String>) {
    let (scheme, rest) = uri.split_once("://").unwrap_or(("postgresql", uri));
    let authority_end = rest.find(['/', '?']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(authority_end);
    let query = tail.find('?').map(|at| &tail[at..]).unwrap_or("");

    let (authority, password) = match authority.rsplit_once('@') {
        Some((userinfo, hosts)) => match userinfo.split_once(':') {
            Some((user, password)) => (
                format!("{user}@{hosts}"),
                Some(percent_decode(password)).filter(|p| !p.is_empty()),
            ),
            None => (authority.to_string(), None),
        },
        None => (authority.to_string(), None),
    };

    (
        format!("{scheme}://{authority}/{}{query}", percent_encode(database)),
        password,
    )
}

/// Mesma coisa numa string `chave=valor` (a outra forma que o pool aceita).
fn rewrite_conninfo(conninfo: &str, database: &str) -> (String, Option<String>) {
    let mut password = None;
    let mut parts: Vec<String> = Vec::new();
    for (key, value) in parse_conninfo(conninfo) {
        match key.as_str() {
            "password" => password = Some(value).filter(|p| !p.is_empty()),
            "dbname" => {}
            _ => parts.push(format!("{key}={}", conninfo_quote(&value))),
        }
    }
    parts.push(format!("dbname={}", conninfo_quote(database)));
    (parts.join(" "), password)
}

/// Pares de uma string de conexão da libpq (`host=x port=5432 user='a b'`).
fn parse_conninfo(conninfo: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut chars = conninfo.chars().peekable();
    loop {
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        let key: String =
            std::iter::from_fn(|| chars.next_if(|c| *c != '=' && !c.is_whitespace())).collect();
        if key.is_empty() {
            break;
        }
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        if chars.next_if_eq(&'=').is_none() {
            break;
        }
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        let mut value = String::new();
        if chars.next_if_eq(&'\'').is_some() {
            while let Some(c) = chars.next() {
                match c {
                    '\\' => value.extend(chars.next()),
                    '\'' => break,
                    c => value.push(c),
                }
            }
        } else {
            while let Some(c) = chars.next_if(|c| !c.is_whitespace()) {
                if c == '\\' {
                    value.extend(chars.next());
                } else {
                    value.push(c);
                }
            }
        }
        pairs.push((key, value));
    }
    pairs
}

fn conninfo_quote(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |at: usize| (*bytes.get(at)? as char).to_digit(16);
        if bytes[i] == b'%' {
            if let (Some(high), Some(low)) = (hex(i + 1), hex(i + 2)) {
                out.push((high * 16 + low) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn sanitize_file_part(part: &str) -> String {
    let clean: String = part
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if clean.is_empty() {
        "_".into()
    } else {
        clean
    }
}

fn shell_quote(arg: &str) -> String {
    let safe = !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_=./:,@%+".contains(c));
    if safe {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DatabaseType, SslMode};

    fn server() -> Server {
        Server {
            id: Some(1),
            name: "local".into(),
            db_type: DatabaseType::Postgres,
            host: "db.local".into(),
            port: 5433,
            username: "app".into(),
            password: "s3gredo".into(),
            default_database: None,
            ssl_mode: crate::models::SslMode::Disable,
            ssl_root_cert: None,
            connection_uri: None,
            created_at: 0,
            scope_databases: None,
            scope_schemas: None,
            group_id: None,
        }
    }

    fn options(format: DumpFormat, content: DumpContent) -> BackupOptions {
        BackupOptions {
            format,
            content,
            no_owner: false,
            clean: false,
            column_inserts: false,
        }
    }

    fn joined(args: &[String]) -> String {
        args.join(" ")
    }

    #[test]
    fn host_port_user_and_password_apart() {
        let conn = connection_args(&server(), "loja");
        assert_eq!(
            conn.args,
            vec![
                "--host=db.local",
                "--port=5433",
                "--username=app",
                "--dbname=loja"
            ]
        );
        assert_eq!(conn.password.as_deref(), Some("s3gredo"));
        assert_eq!(conn.ssl_mode, "disable");
        assert_eq!(ssl_env(&conn), vec![("PGSSLMODE", "disable".to_string())]);

        let ssl = connection_args(
            &Server {
                ssl_mode: SslMode::Require,
                ssl_root_cert: Some("/ignorada/sem/verify.pem".into()),
                password: String::new(),
                ..server()
            },
            "loja",
        );
        assert_eq!(ssl.ssl_mode, "require");
        assert_eq!(ssl.ssl_root_cert, None);
        assert_eq!(ssl.password, None);
    }

    #[test]
    fn verify_modes_point_libpq_at_the_ca() {
        let own_ca = connection_args(
            &Server {
                ssl_mode: SslMode::VerifyCa,
                ssl_root_cert: Some("/certs/rds.pem".into()),
                ..server()
            },
            "loja",
        );
        assert_eq!(
            ssl_env(&own_ca),
            vec![
                ("PGSSLMODE", "verify-ca".to_string()),
                ("PGSSLROOTCERT", "/certs/rds.pem".to_string()),
            ]
        );

        let system = connection_args(
            &Server {
                ssl_mode: SslMode::VerifyFull,
                ..server()
            },
            "loja",
        );
        assert_eq!(
            ssl_env(&system),
            vec![
                ("PGSSLMODE", "verify-full".to_string()),
                ("PGSSLROOTCERT", "system".to_string()),
            ]
        );
    }

    #[test]
    fn database_names_that_look_like_conninfo_are_quoted() {
        assert_eq!(dbname_value("loja"), "loja");
        assert_eq!(dbname_value("a=b"), "dbname='a=b'");
        assert_eq!(dbname_value("it's"), "it's");
        assert_eq!(dbname_value("postgres://x"), "dbname='postgres://x'");
    }

    #[test]
    fn uri_loses_password_and_gets_the_chosen_database() {
        let (uri, password) = rewrite_uri(
            "postgresql://app:p%40ss%3Aw@db.example.com:6543/postgres?sslmode=verify-full",
            "loja vendas",
        );
        assert_eq!(
            uri,
            "postgresql://app@db.example.com:6543/loja%20vendas?sslmode=verify-full"
        );
        assert_eq!(password.as_deref(), Some("p@ss:w"));

        let (uri, password) = rewrite_uri("postgres://app@host", "loja");
        assert_eq!(uri, "postgres://app@host/loja");
        assert_eq!(password, None);

        let (uri, _) = rewrite_uri("postgres://host:5432?application_name=x", "loja");
        assert_eq!(uri, "postgres://host:5432/loja?application_name=x");
    }

    #[test]
    fn uri_password_wins_over_the_stored_one() {
        let conn = connection_args(
            &Server {
                connection_uri: Some("postgres://app:da-uri@host/db".into()),
                ..server()
            },
            "loja",
        );
        assert_eq!(conn.args, vec!["--dbname=postgres://app@host/loja"]);
        assert_eq!(conn.password.as_deref(), Some("da-uri"));

        // Sem senha na URI, vale a guardada no cofre
        let conn = connection_args(
            &Server {
                connection_uri: Some("postgres://app@host/db".into()),
                ..server()
            },
            "loja",
        );
        assert_eq!(conn.password.as_deref(), Some("s3gredo"));
    }

    #[test]
    fn conninfo_loses_password_and_gets_the_chosen_database() {
        let (conninfo, password) = rewrite_conninfo(
            "host=db port=5432 user='o\\'neil' password='a b' dbname=postgres",
            "loja",
        );
        assert_eq!(
            conninfo,
            "host='db' port='5432' user='o\\'neil' dbname='loja'"
        );
        assert_eq!(password.as_deref(), Some("a b"));
    }

    #[test]
    fn dump_of_a_table_with_every_option() {
        let conn = connection_args(&server(), "loja");
        let args = dump_args(
            &conn,
            &BackupTarget::Table {
                schema: "Vendas".into(),
                table: "pedido\"s.*".into(),
            },
            &BackupOptions {
                no_owner: true,
                clean: true,
                column_inserts: true,
                ..options(DumpFormat::Plain, DumpContent::All)
            },
            "/tmp/x.sql.partial",
        )
        .unwrap();
        assert_eq!(
            joined(&args),
            "--verbose --no-password --format=plain --file=/tmp/x.sql.partial \
             --no-owner --no-privileges --clean --if-exists --column-inserts \
             --table=\"Vendas\".\"pedido\"\"s.*\" \
             --host=db.local --port=5433 --username=app --dbname=loja"
        );
        assert!(!joined(&args).contains("s3gredo"));
    }

    #[test]
    fn dump_of_schema_and_database() {
        let conn = connection_args(&server(), "loja");
        let schema = dump_args(
            &conn,
            &BackupTarget::Schema {
                schema: "tenant_1".into(),
            },
            &options(DumpFormat::Custom, DumpContent::SchemaOnly),
            "f",
        )
        .unwrap();
        assert!(schema.contains(&"--format=custom".to_string()));
        assert!(schema.contains(&"--schema-only".to_string()));
        assert!(schema.contains(&"--schema=\"tenant_1\"".to_string()));

        let database = dump_args(
            &conn,
            &BackupTarget::Database,
            &options(DumpFormat::Custom, DumpContent::DataOnly),
            "f",
        )
        .unwrap();
        assert!(database.contains(&"--data-only".to_string()));
        assert!(!database
            .iter()
            .any(|a| a.starts_with("--schema=") || a.starts_with("--table=")));
    }

    fn restore_options(no_owner: bool, clean: bool, single_transaction: bool) -> RestoreOptions {
        RestoreOptions {
            no_owner,
            clean,
            single_transaction,
        }
    }

    #[test]
    fn restore_of_a_dump_goes_through_pg_restore() {
        let conn = connection_args(&server(), "destino");
        let (tool, args) = restore_args(
            &conn,
            RestoreFormat::Custom,
            &restore_options(true, true, true),
            "/tmp/loja.dump",
        )
        .unwrap();
        assert_eq!(tool, "pg_restore");
        assert_eq!(
            joined(&args),
            "--verbose --no-password --no-owner --no-privileges --clean --if-exists \
             --single-transaction --exit-on-error \
             --host=db.local --port=5433 --username=app --dbname=destino /tmp/loja.dump"
        );

        let (_, minimal) = restore_args(
            &conn,
            RestoreFormat::Custom,
            &restore_options(false, false, false),
            "f",
        )
        .unwrap();
        assert_eq!(
            joined(&minimal),
            "--verbose --no-password --host=db.local --port=5433 --username=app --dbname=destino f"
        );
    }

    #[test]
    fn restore_of_a_script_goes_through_psql() {
        let conn = connection_args(&server(), "destino");
        let (tool, args) = restore_args(
            &conn,
            RestoreFormat::Plain,
            &restore_options(false, false, true),
            "/tmp/loja.sql",
        )
        .unwrap();
        assert_eq!(tool, "psql");
        assert_eq!(
            joined(&args),
            "--no-psqlrc --no-password --set=ON_ERROR_STOP=1 --single-transaction \
             --file=/tmp/loja.sql --host=db.local --port=5433 --username=app --dbname=destino"
        );

        let (_, lenient) = restore_args(
            &conn,
            RestoreFormat::Plain,
            &restore_options(false, false, false),
            "f",
        )
        .unwrap();
        assert!(!lenient.iter().any(|a| a.contains("ON_ERROR_STOP")));

        // Opções que só existem no .dump
        assert!(restore_args(
            &conn,
            RestoreFormat::Plain,
            &restore_options(true, false, true),
            "f"
        )
        .is_err());
        assert!(restore_args(
            &conn,
            RestoreFormat::Plain,
            &restore_options(false, true, true),
            "f"
        )
        .is_err());
    }

    #[test]
    fn filter_is_needed_only_for_new_tools_on_old_servers() {
        assert!(needs_timeout_filter(180_006, 140_004));
        assert!(needs_timeout_filter(170_000, 90_624));
        assert!(!needs_timeout_filter(180_006, 170_002));
        assert!(!needs_timeout_filter(160_004, 140_004));
        // Versão do servidor desconhecida: direto
        assert!(!needs_timeout_filter(180_006, 0));
    }

    #[test]
    fn filtered_input_drops_only_the_timeout_line() {
        assert!(!keep_input_line(b"SET transaction_timeout = 0;\n"));
        assert!(!keep_input_line(b"SET transaction_timeout = 0;\r\n"));
        assert!(keep_input_line(b"SET statement_timeout = 0;\n"));
        assert!(keep_input_line(b"-- SET transaction_timeout = 0;\n"));
    }

    #[test]
    fn filtered_plans_go_through_psql() {
        let conn = connection_args(&server(), "destino");
        let options = restore_options(true, true, true);

        let direct =
            restore_plan(&conn, RestoreFormat::Custom, &options, "/b.dump", false).unwrap();
        assert_eq!(direct.tool, "pg_restore");
        assert_eq!(direct.input, None);
        assert_eq!(
            display_plan(&direct),
            display_command("pg_restore", &direct.args)
        );
        let lenient = restore_plan(
            &conn,
            RestoreFormat::Custom,
            &restore_options(true, false, false),
            "f",
            false,
        )
        .unwrap();
        assert!(lenient.tolerate_ignored_errors);

        let piped = restore_plan(&conn, RestoreFormat::Custom, &options, "/b.dump", true).unwrap();
        assert_eq!(piped.tool, "psql");
        assert_eq!(
            joined(&piped.args),
            "--no-psqlrc --no-password --set=ON_ERROR_STOP=1 --single-transaction --file=- \
             --host=db.local --port=5433 --username=app --dbname=destino"
        );
        assert_eq!(
            piped.input,
            Some(PlanInput::PgRestore(
                [
                    "--verbose",
                    "--no-owner",
                    "--no-privileges",
                    "--clean",
                    "--if-exists",
                    "--file=-",
                    "/b.dump"
                ]
                .map(String::from)
                .to_vec()
            ))
        );
        assert!(!piped.tolerate_ignored_errors);
        assert!(display_plan(&piped).starts_with("pg_restore --verbose"));
        assert!(display_plan(&piped).contains(" | psql --no-psqlrc"));

        let script = restore_plan(
            &conn,
            RestoreFormat::Plain,
            &restore_options(false, false, false),
            "/meu backup.sql",
            true,
        )
        .unwrap();
        assert_eq!(
            script.input,
            Some(PlanInput::File("/meu backup.sql".into()))
        );
        assert!(!script.args.iter().any(|a| a.contains("ON_ERROR_STOP")));
        assert!(display_plan(&script).contains("--dbname=destino < '/meu backup.sql'"));
    }

    #[test]
    fn recognizes_error_and_ignored_error_lines() {
        assert!(is_error_line(
            "psql:/tmp/x.sql:12: ERROR:  relation \"t\" already exists"
        ));
        assert!(is_error_line("pg_restore: error: could not execute query"));
        assert!(!is_error_line(
            "pg_restore: creating TABLE \"public.pedidos\""
        ));
        assert!(is_ignored_errors_line(
            "pg_restore: warning: errors ignored on restore: 3"
        ));
        assert!(is_ignored_errors_line(
            "pg_restore: aviso: erros ignorados na restauração: 3"
        ));
        assert!(!is_error_line(
            "pg_restore: warning: errors ignored on restore: 3"
        ));
    }

    #[test]
    fn rejects_options_that_would_be_ignored() {
        let conn = connection_args(&server(), "loja");
        let run = |options: BackupOptions| dump_args(&conn, &BackupTarget::Database, &options, "f");

        assert!(run(BackupOptions {
            clean: true,
            ..options(DumpFormat::Custom, DumpContent::All)
        })
        .is_err());
        assert!(run(BackupOptions {
            clean: true,
            ..options(DumpFormat::Plain, DumpContent::DataOnly)
        })
        .is_err());
        assert!(run(BackupOptions {
            column_inserts: true,
            ..options(DumpFormat::Custom, DumpContent::All)
        })
        .is_err());
        assert!(run(BackupOptions {
            column_inserts: true,
            ..options(DumpFormat::Plain, DumpContent::SchemaOnly)
        })
        .is_err());
        assert!(run(BackupOptions {
            clean: true,
            column_inserts: true,
            ..options(DumpFormat::Plain, DumpContent::All)
        })
        .is_ok());
    }

    #[test]
    fn file_names_are_safe_and_dated() {
        let now = chrono::NaiveDate::from_ymd_opt(2026, 10, 15)
            .unwrap()
            .and_hms_opt(14, 32, 0)
            .unwrap();
        assert_eq!(
            suggested_file_name("loja", &BackupTarget::Database, DumpFormat::Custom, now),
            "loja_20261015_1432.dump"
        );
        assert_eq!(
            suggested_file_name(
                "loja",
                &BackupTarget::Table {
                    schema: "public".into(),
                    table: "Itens/Pedido 2".into()
                },
                DumpFormat::Plain,
                now
            ),
            "loja_public_Itens_Pedido_2_20261015_1432.sql"
        );
        assert_eq!(partial_path("/a/b.dump"), "/a/b.dump.partial");
    }

    #[test]
    fn command_is_shell_quoted() {
        let args = vec![
            "--table=\"Vendas\".\"pedidos\"".to_string(),
            "--file=/tmp/meu backup.dump".to_string(),
            "--dbname=loja".to_string(),
            "--host=it's".to_string(),
        ];
        assert_eq!(
            display_command("pg_dump", &args),
            "pg_dump '--table=\"Vendas\".\"pedidos\"' '--file=/tmp/meu backup.dump' --dbname=loja '--host=it'\\''s'"
        );
    }

    #[test]
    fn error_message_prefers_error_lines() {
        let tail = vec![
            "pg_dump: last built-in OID is 16383".to_string(),
            "pg_dump: error: connection to server at \"db\" failed: FATAL:  password authentication failed for user \"app\"".to_string(),
        ];
        assert_eq!(error_message("pg_dump", &tail, Some(1)), tail[1]);

        let traduzido = vec!["pg_dump: erro: consulta falhou".to_string()];
        assert_eq!(error_message("pg_dump", &traduzido, Some(1)), traduzido[0]);

        let other: Vec<String> = (1..=8).map(|n| format!("linha {n}")).collect();
        assert_eq!(
            error_message("pg_dump", &other, Some(1)),
            "linha 4\nlinha 5\nlinha 6\nlinha 7\nlinha 8"
        );
        assert_eq!(
            error_message("pg_dump", &[], Some(2)),
            "pg_dump terminou com o código 2."
        );
        assert_eq!(
            error_message("pg_dump", &[], None),
            "pg_dump foi interrompido."
        );
    }

    #[test]
    fn percent_decoding_handles_edges() {
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%4"), "%4");
        assert_eq!(percent_decode("%zz"), "%zz");
        assert_eq!(percent_decode("%C3%A7"), "ç");
    }
}
