//! Ponta a ponta com o `pg_dump`/`pg_restore`/`psql` de verdade contra um
//! Postgres local (localhost:5432, postgres/1234 — o mesmo dos outros e2e).
//! As ferramentas vêm de `OCTAPUS_PG_TOOLS_DIR` ou da detecção normal.
//!
//! `cargo test --lib backup::e2e -- --ignored`
//!
//! Cada teste cria e apaga os próprios databases `octapus_db_e2e_bk_*`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::adapters::postgres::PostgresAdapter;
use crate::adapters::DatabaseAdapter;
use crate::models::{
    BackupEvent, BackupEventKind, BackupOptions, BackupTarget, DatabaseType, DumpContent,
    DumpFormat, PgToolsInstall, QueryOptions, RestoreFormat, RestoreOptions, Server,
};

use super::{
    connection_args, detect, detect_format, dump_args, needs_timeout_filter, restore_plan,
    summarize, BackupService, JobInput, JobSpec, PlanInput,
};

fn server() -> Server {
    Server {
        id: Some(1),
        name: "e2e".into(),
        db_type: DatabaseType::Postgres,
        host: "localhost".into(),
        port: 5432,
        username: "postgres".into(),
        password: "1234".into(),
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

async fn tools() -> PgToolsInstall {
    let configured = std::env::var("OCTAPUS_PG_TOOLS_DIR").ok();
    detect(configured.as_deref()).await.selected.expect(
        "pg_dump não encontrado: instale (brew install libpq) ou defina OCTAPUS_PG_TOOLS_DIR",
    )
}

/// Database vazio (recriado), devolvendo o adapter dele.
async fn fresh_database(name: &str) -> PostgresAdapter {
    let admin = PostgresAdapter::new(&server(), "postgres").unwrap();
    admin
        .execute_statement(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"))
        .await
        .unwrap();
    admin
        .execute_statement(&format!("CREATE DATABASE {name}"))
        .await
        .unwrap();
    PostgresAdapter::new(&server(), name).unwrap()
}

async fn drop_database(name: &str) {
    let admin = PostgresAdapter::new(&server(), "postgres").unwrap();
    let _ = admin
        .execute_statement(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"))
        .await;
}

async fn scalar(adapter: &PostgresAdapter, sql: &str) -> String {
    let result = adapter
        .execute_query(sql, QueryOptions::default())
        .await
        .unwrap();
    result.rows[0][0].clone().unwrap_or_default()
}

/// Banco de origem: um schema com maiúscula (exige aspas no `-t`), uma tabela
/// com sequência, uma view e uma tabela em outro schema que não deve ir junto.
async fn seed(adapter: &PostgresAdapter) {
    for sql in [
        r#"CREATE SCHEMA "Vendas""#,
        r#"CREATE TABLE "Vendas".pedidos (id serial PRIMARY KEY, cliente text NOT NULL, total numeric(10,2))"#,
        r#"INSERT INTO "Vendas".pedidos (cliente, total) VALUES ('Ana', 10.5), ('Bia', 20), ('Caio', NULL)"#,
        r#"CREATE VIEW "Vendas".grandes AS SELECT * FROM "Vendas".pedidos WHERE total > 15"#,
        "CREATE TABLE public.outra (x int)",
        "INSERT INTO public.outra VALUES (1)",
    ] {
        adapter.execute_statement(sql).await.unwrap();
    }
}

fn scratch_file(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("octapus-e2e-backup-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// Roda o job pelo serviço até o evento final.
async fn run(spec: JobSpec) -> (BackupEventKind, Vec<String>) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let service = BackupService::default();
    service.init(Arc::new(move |event: BackupEvent| {
        let _ = tx.send(event.kind);
    }));
    service.start(spec);
    let mut log = Vec::new();
    while let Some(kind) = rx.recv().await {
        match kind {
            BackupEventKind::Log { lines } => log.extend(lines),
            other => return (other, log),
        }
    }
    unreachable!("o job sempre termina com um evento final");
}

fn env(password: &str) -> Vec<(String, String)> {
    vec![
        ("PGPASSWORD".into(), password.into()),
        ("PGSSLMODE".into(), "disable".into()),
    ]
}

async fn dump(
    install: &PgToolsInstall,
    database: &str,
    target: BackupTarget,
    options: BackupOptions,
    path: &Path,
) -> BackupEventKind {
    let partial = PathBuf::from(super::partial_path(&path.display().to_string()));
    let connection = connection_args(&server(), database);
    let args = dump_args(
        &connection,
        &target,
        &options,
        &partial.display().to_string(),
    )
    .unwrap();
    let (outcome, _) = run(JobSpec {
        tool: "pg_dump",
        program: PathBuf::from(&install.pg_dump),
        args,
        env: env("1234"),
        output: Some((partial, path.to_path_buf())),
        tolerate_ignored_errors: false,
        on_success: None,
        input: None,
    })
    .await;
    outcome
}

async fn restore(
    install: &PgToolsInstall,
    database: &str,
    format: RestoreFormat,
    options: RestoreOptions,
    path: &Path,
) -> BackupEventKind {
    // Como o comando: o filtro do `SET transaction_timeout` entra quando as
    // ferramentas são 17+ e o servidor é anterior
    let adapter = PostgresAdapter::new(&server(), database).unwrap();
    let server_num: u32 = scalar(&adapter, "SELECT current_setting('server_version_num')")
        .await
        .parse()
        .unwrap();
    let connection = connection_args(&server(), database);
    let plan = restore_plan(
        &connection,
        format,
        &options,
        &path.display().to_string(),
        needs_timeout_filter(install.version_num, server_num),
    )
    .unwrap();
    let pg_restore = || PathBuf::from(install.pg_restore.clone().expect("pg_restore"));
    let program = match plan.tool {
        "pg_restore" => pg_restore(),
        _ => PathBuf::from(install.psql.clone().expect("psql")),
    };
    let input = plan.input.map(|input| match input {
        PlanInput::File(file) => JobInput::File(PathBuf::from(file)),
        PlanInput::PgRestore(args) => JobInput::Process {
            tool: "pg_restore",
            program: pg_restore(),
            args,
        },
    });
    let (outcome, _) = run(JobSpec {
        tool: plan.tool,
        program,
        args: plan.args,
        env: env("1234"),
        output: None,
        tolerate_ignored_errors: plan.tolerate_ignored_errors,
        on_success: None,
        input,
    })
    .await;
    outcome
}

fn backup_options(format: DumpFormat) -> BackupOptions {
    BackupOptions {
        format,
        content: DumpContent::All,
        no_owner: true,
        clean: false,
        column_inserts: false,
    }
}

const ALL_OR_NOTHING: RestoreOptions = RestoreOptions {
    no_owner: true,
    clean: false,
    single_transaction: true,
};

#[tokio::test]
#[ignore = "requires local postgres on localhost:5432 (postgres/1234) and pg_dump"]
async fn e2e_table_backup_restores_into_another_database() {
    let install = tools().await;
    let source = fresh_database("octapus_db_e2e_bk_src1").await;
    seed(&source).await;

    let file = scratch_file("pedidos.dump");
    let outcome = dump(
        &install,
        "octapus_db_e2e_bk_src1",
        BackupTarget::Table {
            schema: "Vendas".into(),
            table: "pedidos".into(),
        },
        backup_options(DumpFormat::Custom),
        &file,
    )
    .await;
    assert!(
        matches!(outcome, BackupEventKind::Done { size_bytes, .. } if size_bytes > 0),
        "{outcome:?}"
    );
    assert!(!PathBuf::from(super::partial_path(&file.display().to_string())).exists());

    // O arquivo é reconhecido e resumido sem tocar em banco
    let (format, _) = detect_format(&file).await.unwrap();
    assert_eq!(format, RestoreFormat::Custom);
    let summary = summarize(Path::new(install.pg_restore.as_deref().unwrap()), &file)
        .await
        .unwrap();
    assert_eq!(summary.database.as_deref(), Some("octapus_db_e2e_bk_src1"));
    assert_eq!(summary.table_count, 1);
    assert!(summary.has_data);

    // O `-t` não leva o schema: o destino precisa tê-lo
    let target = fresh_database("octapus_db_e2e_bk_dst1").await;
    target
        .execute_statement(r#"CREATE SCHEMA "Vendas""#)
        .await
        .unwrap();
    let outcome = restore(
        &install,
        "octapus_db_e2e_bk_dst1",
        RestoreFormat::Custom,
        ALL_OR_NOTHING,
        &file,
    )
    .await;
    assert!(
        matches!(&outcome, BackupEventKind::Done { warnings, .. } if warnings.is_empty()),
        "{outcome:?}"
    );

    assert_eq!(
        scalar(&target, r#"SELECT count(*)::text FROM "Vendas".pedidos"#).await,
        "3"
    );
    // A sequência veio junto e continua de onde parou
    target
        .execute_statement(r#"INSERT INTO "Vendas".pedidos (cliente) VALUES ('Duda')"#)
        .await
        .unwrap();
    assert_eq!(
        scalar(&target, r#"SELECT max(id)::text FROM "Vendas".pedidos"#).await,
        "4"
    );
    // Só a tabela: nem a view nem a outra tabela
    assert_eq!(
        scalar(
            &target,
            "SELECT count(*)::text FROM pg_class WHERE relname IN ('grandes', 'outra')"
        )
        .await,
        "0"
    );

    drop_database("octapus_db_e2e_bk_src1").await;
    drop_database("octapus_db_e2e_bk_dst1").await;
    let _ = std::fs::remove_file(&file);
}

#[tokio::test]
#[ignore = "requires local postgres on localhost:5432 (postgres/1234) and pg_dump"]
async fn e2e_schema_script_restores_through_psql() {
    let install = tools().await;
    let source = fresh_database("octapus_db_e2e_bk_src2").await;
    seed(&source).await;

    let file = scratch_file("vendas.sql");
    let options = BackupOptions {
        column_inserts: true,
        ..backup_options(DumpFormat::Plain)
    };
    let outcome = dump(
        &install,
        "octapus_db_e2e_bk_src2",
        BackupTarget::Schema {
            schema: "Vendas".into(),
        },
        options,
        &file,
    )
    .await;
    assert!(
        matches!(outcome, BackupEventKind::Done { .. }),
        "{outcome:?}"
    );
    let script = std::fs::read_to_string(&file).unwrap();
    assert!(script.contains("INSERT INTO \"Vendas\".pedidos (id, cliente, total) VALUES"));
    assert!(!script.contains("public.outra"));
    assert_eq!(detect_format(&file).await.unwrap().0, RestoreFormat::Plain);

    let target = fresh_database("octapus_db_e2e_bk_dst2").await;
    let plain = RestoreOptions {
        no_owner: false,
        ..ALL_OR_NOTHING
    };
    let outcome = restore(
        &install,
        "octapus_db_e2e_bk_dst2",
        RestoreFormat::Plain,
        plain.clone(),
        &file,
    )
    .await;
    assert!(
        matches!(&outcome, BackupEventKind::Done { warnings, .. } if warnings.is_empty()),
        "{outcome:?}"
    );
    assert_eq!(
        scalar(&target, r#"SELECT count(*)::text FROM "Vendas".pedidos"#).await,
        "3"
    );
    assert_eq!(
        scalar(&target, r#"SELECT count(*)::text FROM "Vendas".grandes"#).await,
        "1"
    );

    // De novo, em "tudo ou nada": o schema já existe → erro, e nada muda
    let outcome = restore(
        &install,
        "octapus_db_e2e_bk_dst2",
        RestoreFormat::Plain,
        plain,
        &file,
    )
    .await;
    assert!(
        matches!(&outcome, BackupEventKind::Error { message } if message.contains("already exists")),
        "{outcome:?}"
    );
    assert_eq!(
        scalar(&target, r#"SELECT count(*)::text FROM "Vendas".pedidos"#).await,
        "3"
    );

    drop_database("octapus_db_e2e_bk_src2").await;
    drop_database("octapus_db_e2e_bk_dst2").await;
    let _ = std::fs::remove_file(&file);
}

#[tokio::test]
#[ignore = "requires local postgres on localhost:5432 (postgres/1234) and pg_dump"]
async fn e2e_restore_over_existing_objects() {
    let install = tools().await;
    let source = fresh_database("octapus_db_e2e_bk_src3").await;
    seed(&source).await;

    let file = scratch_file("inteiro.dump");
    let outcome = dump(
        &install,
        "octapus_db_e2e_bk_src3",
        BackupTarget::Database,
        backup_options(DumpFormat::Custom),
        &file,
    )
    .await;
    assert!(
        matches!(outcome, BackupEventKind::Done { .. }),
        "{outcome:?}"
    );

    let target = fresh_database("octapus_db_e2e_bk_dst3").await;
    let first = restore(
        &install,
        "octapus_db_e2e_bk_dst3",
        RestoreFormat::Custom,
        ALL_OR_NOTHING,
        &file,
    )
    .await;
    assert!(matches!(first, BackupEventKind::Done { .. }), "{first:?}");

    // Por cima, sem apagar e sem "tudo ou nada": segue, com os erros como aviso
    let lenient = RestoreOptions {
        single_transaction: false,
        ..ALL_OR_NOTHING
    };
    let outcome = restore(
        &install,
        "octapus_db_e2e_bk_dst3",
        RestoreFormat::Custom,
        lenient,
        &file,
    )
    .await;
    match &outcome {
        BackupEventKind::Done { warnings, .. } => {
            assert!(!warnings.is_empty());
            assert!(
                warnings.iter().any(|w| w.contains("already exists")),
                "{warnings:?}"
            );
        }
        other => panic!("esperava Done com avisos, veio {other:?}"),
    }

    // Apagando antes: limpo, e sem linhas duplicadas
    let clean = RestoreOptions {
        clean: true,
        ..ALL_OR_NOTHING
    };
    let outcome = restore(
        &install,
        "octapus_db_e2e_bk_dst3",
        RestoreFormat::Custom,
        clean,
        &file,
    )
    .await;
    assert!(
        matches!(&outcome, BackupEventKind::Done { warnings, .. } if warnings.is_empty()),
        "{outcome:?}"
    );
    assert_eq!(
        scalar(&target, r#"SELECT count(*)::text FROM "Vendas".pedidos"#).await,
        "3"
    );
    assert_eq!(
        scalar(&target, "SELECT count(*)::text FROM public.outra").await,
        "1"
    );

    drop_database("octapus_db_e2e_bk_src3").await;
    drop_database("octapus_db_e2e_bk_dst3").await;
    let _ = std::fs::remove_file(&file);
}

#[tokio::test]
#[ignore = "requires local postgres on localhost:5432 (postgres/1234) and pg_dump"]
async fn e2e_wrong_password_is_a_clear_error() {
    let install = tools().await;
    let file = scratch_file("senha.dump");
    let partial = PathBuf::from(super::partial_path(&file.display().to_string()));
    let connection = connection_args(&server(), "postgres");
    let args = dump_args(
        &connection,
        &BackupTarget::Database,
        &backup_options(DumpFormat::Custom),
        &partial.display().to_string(),
    )
    .unwrap();
    let (outcome, _) = run(JobSpec {
        tool: "pg_dump",
        program: PathBuf::from(&install.pg_dump),
        args,
        env: env("senha-errada"),
        output: Some((partial.clone(), file.clone())),
        tolerate_ignored_errors: false,
        on_success: None,
        input: None,
    })
    .await;
    match outcome {
        BackupEventKind::Error { message } => {
            assert!(
                message.contains("password authentication failed"),
                "{message}"
            )
        }
        other => panic!("esperava Error, veio {other:?}"),
    }
    assert!(!file.exists() && !partial.exists());
}

#[tokio::test]
#[ignore = "requires local postgres on localhost:5432 (postgres/1234) and pg_dump"]
async fn e2e_truncated_dump_commits_nothing() {
    let install = tools().await;
    let source = fresh_database("octapus_db_e2e_bk_src4").await;
    seed(&source).await;
    // Dados o bastante para o arquivo cortado perder parte deles
    source
        .execute_statement(
            r#"INSERT INTO "Vendas".pedidos (cliente, total) SELECT md5(g::text), g FROM generate_series(1, 20000) g"#,
        )
        .await
        .unwrap();

    let file = scratch_file("cortado.dump");
    let outcome = dump(
        &install,
        "octapus_db_e2e_bk_src4",
        BackupTarget::Database,
        backup_options(DumpFormat::Custom),
        &file,
    )
    .await;
    assert!(
        matches!(outcome, BackupEventKind::Done { .. }),
        "{outcome:?}"
    );
    let bytes = std::fs::read(&file).unwrap();
    std::fs::write(&file, &bytes[..bytes.len() * 2 / 3]).unwrap();

    let target = fresh_database("octapus_db_e2e_bk_dst4").await;
    let outcome = restore(
        &install,
        "octapus_db_e2e_bk_dst4",
        RestoreFormat::Custom,
        ALL_OR_NOTHING,
        &file,
    )
    .await;
    assert!(
        matches!(outcome, BackupEventKind::Error { .. }),
        "{outcome:?}"
    );
    // "Tudo ou nada": nem o schema ficou
    assert_eq!(
        scalar(
            &target,
            "SELECT count(*)::text FROM pg_namespace WHERE nspname = 'Vendas'"
        )
        .await,
        "0"
    );

    drop_database("octapus_db_e2e_bk_src4").await;
    drop_database("octapus_db_e2e_bk_dst4").await;
    let _ = std::fs::remove_file(&file);
}
