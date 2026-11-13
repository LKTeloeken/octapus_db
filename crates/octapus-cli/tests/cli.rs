//! O binário `octapus` de verdade, contra uma pasta de dados descartável
//! (`OCTAPUS_DATA_DIR`) e um SQLite temporário. Os testes rodam sem terminal:
//! é a situação de um script ou de outro programa chamando o `octapus`.

use std::path::{Path, PathBuf};
use std::process::Output;

use assert_cmd::Command;
use serde_json::Value;

/// Pasta de dados e banco de um teste, apagados no fim.
struct Sandbox {
    dir: PathBuf,
}

impl Sandbox {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("octapus-cli-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }

    fn data_dir(&self) -> PathBuf {
        self.dir.join("data")
    }

    fn command(&self) -> Command {
        let mut command = Command::cargo_bin("octapus").unwrap();
        command
            .env("OCTAPUS_DATA_DIR", self.data_dir())
            .env("OCTAPUS_CONFIG_DIR", self.dir.join("config"))
            .env_remove("NO_COLOR");
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }

    /// Um SQLite com três clientes, já cadastrado como o servidor `loja`.
    fn with_shop(label: &str) -> (Self, PathBuf) {
        let sandbox = Self::new(label);
        let path = sandbox.dir.join("loja.db");
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE customers (id INTEGER PRIMARY KEY, name TEXT NOT NULL, city TEXT);
                 INSERT INTO customers (name, city) VALUES ('Ana', 'Porto Alegre'), ('Bia', NULL), ('Caio, o \"3\"', 'Curitiba');",
            )
            .unwrap();
        let added = sandbox.run(&["server", "add", path.to_str().unwrap(), "--name", "loja"]);
        assert!(added.status.success(), "{}", stderr(&added));
        (sandbox, path)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|error| panic!("não é JSON ({error}): {text}"))
}

fn customers(path: &Path) -> i64 {
    rusqlite::Connection::open(path)
        .unwrap()
        .query_row("SELECT count(*) FROM customers", [], |row| row.get(0))
        .unwrap()
}

#[test]
fn query_reads_in_every_format() {
    let (sandbox, _path) = Sandbox::with_shop("formats");
    let query = "SELECT id, name, city FROM customers ORDER BY id";

    // Fora de um terminal, o padrão é JSON
    let output = sandbox.run(&["query", "loja", query]);
    assert!(output.status.success(), "{}", stderr(&output));
    let document = json(&stdout(&output));
    assert_eq!(document["rowCount"], 3);
    assert_eq!(document["hasMore"], false);
    assert_eq!(document["columns"][1]["name"], "name");
    // Toda célula é string ou null
    assert_eq!(document["rows"][0], serde_json::json!(["1", "Ana", "Porto Alegre"]));
    assert_eq!(document["rows"][1][2], Value::Null);

    let output = sandbox.run(&["--format", "csv", "query", "loja", query]);
    assert_eq!(
        stdout(&output),
        "id,name,city\n1,Ana,Porto Alegre\n2,Bia,\n3,\"Caio, o \"\"3\"\"\",Curitiba\n"
    );

    let output = sandbox.run(&["--format", "ndjson", "query", "loja", query, "--limit", "2"]);
    let lines: Vec<Value> = stdout(&output).lines().map(json).collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1], serde_json::json!({ "id": "2", "name": "Bia", "city": null }));

    let output = sandbox.run(&["--format", "table", "query", "loja", query]);
    assert_eq!(
        stdout(&output),
        "id  name         city\n1   Ana          Porto Alegre\n2   Bia          NULL\n3   Caio, o \"3\"  Curitiba\n"
    );
    // O rodapé não suja o stdout
    assert!(stderr(&output).contains("3 linha(s)"));
}

#[test]
fn query_comes_from_stdin_or_a_file() {
    let (sandbox, _path) = Sandbox::with_shop("stdin");

    let output = sandbox
        .command()
        .args(["--format", "csv", "query", "loja"])
        .write_stdin("SELECT name FROM customers WHERE city IS NULL")
        .output()
        .unwrap();
    assert_eq!(stdout(&output), "name\nBia\n");

    let file = sandbox.dir.join("q.sql");
    std::fs::write(&file, "SELECT count(*) AS n FROM customers;\n").unwrap();
    let output = sandbox.run(&["--format", "csv", "query", "loja", "--file", file.to_str().unwrap()]);
    assert_eq!(stdout(&output), "n\n3\n");
}

/// Sem terminal não há escrita, por nenhum caminho: o guard recusa a consulta
/// que escreve, o `--write` recusa rodar e a condição do `table` não carrega
/// um segundo comando. O banco fica como estava.
#[test]
fn nothing_writes_without_a_terminal() {
    let (sandbox, path) = Sandbox::with_shop("readonly");

    for statement in [
        "DELETE FROM customers",
        "UPDATE customers SET name = 'x'",
        "DROP TABLE customers",
        "SELECT 1; DELETE FROM customers",
        "INSERT INTO customers (name) VALUES ('x') RETURNING id",
    ] {
        let output = sandbox.run(&["query", "loja", statement]);
        assert_eq!(output.status.code(), Some(5), "{statement}: {}", stderr(&output));
        assert_eq!(json(&stderr(&output))["code"], "GUARD_REJECTED");
        assert!(stdout(&output).is_empty());
    }

    let output = sandbox.run(&["query", "loja", "--write", "DELETE FROM customers"]);
    assert_eq!(output.status.code(), Some(8), "{}", stderr(&output));
    assert_eq!(json(&stderr(&output))["code"], "WRITE_REFUSED");

    // Nem com a resposta da confirmação já na entrada padrão
    let output = sandbox
        .command()
        .args(["query", "loja", "--write", "DELETE FROM customers"])
        .write_stdin("loja\n")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(8));

    let output = sandbox.run(&["table", "loja", "customers", "--where", "1=1); DELETE FROM customers; --"]);
    assert_eq!(output.status.code(), Some(5), "{}", stderr(&output));

    assert_eq!(customers(&path), 3);
}

#[test]
fn restore_needs_a_terminal_and_backup_is_postgres_only() {
    let (sandbox, path) = Sandbox::with_shop("backup");
    let dump = sandbox.dir.join("loja.sql");
    std::fs::write(&dump, "DELETE FROM customers;\n").unwrap();
    let dump = dump.to_str().unwrap();

    let output = sandbox.run(&["restore", "loja", dump]);
    assert_eq!(output.status.code(), Some(8), "{}", stderr(&output));
    assert_eq!(json(&stderr(&output))["code"], "WRITE_REFUSED");

    // Nem com a resposta da confirmação já na entrada padrão
    let output = sandbox.command().args(["restore", "loja", dump]).write_stdin("loja\n").output().unwrap();
    assert_eq!(output.status.code(), Some(8));

    // Fora do Postgres não há backup, e nenhum arquivo aparece
    let target = sandbox.dir.join("saida.dump");
    let output = sandbox.run(&["backup", "loja", "--output", target.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(7), "{}", stderr(&output));
    assert_eq!(json(&stderr(&output))["code"], "UNSUPPORTED_DATABASE");
    assert!(!target.exists());

    assert_eq!(customers(&path), 3);
}

#[test]
fn structure_and_table_rows() {
    let (sandbox, _path) = Sandbox::with_shop("structure");

    let tables = json(&stdout(&sandbox.run(&["ls", "loja"])));
    assert_eq!(tables[0]["name"], "customers");

    let described = json(&stdout(&sandbox.run(&["describe", "loja", "customers"])));
    assert_eq!(described["table"], "customers");
    let columns: Vec<&str> = described["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|column| column["name"].as_str().unwrap())
        .collect();
    assert_eq!(columns, ["id", "name", "city"]);

    let output = sandbox.run(&[
        "--format", "csv", "table", "loja", "customers", "--where", "city IS NOT NULL", "--order", "name:desc",
    ]);
    assert_eq!(stdout(&output), "id,name,city\n3,\"Caio, o \"\"3\"\"\",Curitiba\n1,Ana,Porto Alegre\n");

    // Tabela que não existe: "não encontrado" ou o erro do próprio banco
    let missing = sandbox.run(&["describe", "loja", "nope"]);
    assert!(matches!(missing.status.code(), Some(3 | 6)), "{}", stderr(&missing));
}

/// A senha entra pela URL, por variável de ambiente ou pela entrada padrão e
/// não volta em saída nenhuma — nem dentro da URI de conexão.
#[test]
fn passwords_never_come_back() {
    let sandbox = Sandbox::new("secrets");
    let secret = "canario-S3nh4-8842";

    let added = sandbox.run(&[
        "server",
        "add",
        &format!("postgres://ana:{secret}@db.example.com:5432/loja"),
        "--name",
        "pg",
    ]);
    assert!(added.status.success(), "{}", stderr(&added));
    // O que só a URI descreve fica como URI, com a credencial dentro
    let added = sandbox.run(&[
        "server",
        "add",
        &format!("mongodb+srv://ana:{secret}@cluster.example.net/loja?retryWrites=true"),
        "--name",
        "mongo",
    ]);
    assert!(added.status.success(), "{}", stderr(&added));
    let added = sandbox
        .command()
        .args(["server", "add", "--type", "redis", "--host", "cache.example.com", "--name", "redis"])
        .args(["--password-env", "OCTAPUS_TEST_PASSWORD"])
        .env("OCTAPUS_TEST_PASSWORD", secret)
        .output()
        .unwrap();
    assert!(added.status.success(), "{}", stderr(&added));
    let changed = sandbox
        .command()
        .args(["server", "set-password", "pg", "--password-stdin"])
        .write_stdin(format!("{secret}\n"))
        .output()
        .unwrap();
    assert!(changed.status.success(), "{}", stderr(&changed));

    let mut seen = String::new();
    for output in [added, changed] {
        seen.push_str(&stdout(&output));
        seen.push_str(&stderr(&output));
    }
    for format in ["json", "table", "csv", "ndjson"] {
        for args in [vec!["server", "list"], vec!["server", "show", "pg"], vec!["server", "show", "mongo"]] {
            let output = sandbox.command().args(["--format", format]).args(&args).output().unwrap();
            assert!(output.status.success(), "{}", stderr(&output));
            seen.push_str(&stdout(&output));
            seen.push_str(&stderr(&output));
        }
    }
    assert!(!seen.contains(secret), "a senha apareceu na saída");
    assert!(seen.contains("mongodb+srv://ana:***@cluster.example.net"));

    // No disco, só cifrada; e o log do terminal também não a tem
    let database = std::fs::read(sandbox.data_dir().join("app.db")).unwrap();
    let in_database = String::from_utf8_lossy(&database).matches(secret).count();
    // A URI do Mongo é guardada como veio (igual ao app); a senha em si, não
    assert_eq!(in_database, 1, "só a URI colada pode ter a credencial");
    let log = std::fs::read_to_string(sandbox.data_dir().join("logs/octapus-cli.log")).unwrap_or_default();
    assert!(!log.contains(secret));

    // Sem terminal, não há a quem perguntar a senha
    let asked = sandbox.run(&["server", "set-password", "pg"]);
    assert_eq!(asked.status.code(), Some(2));
}

#[test]
fn servers_and_groups_round_trip() {
    let (sandbox, _path) = Sandbox::with_shop("crud");

    assert!(sandbox.run(&["group", "add", "produção"]).status.success());
    let edited = sandbox.run(&["server", "edit", "loja", "--name", "Loja", "--group", "produção"]);
    assert!(edited.status.success(), "{}", stderr(&edited));
    // O nome não diferencia maiúsculas; o id também serve
    let shown = json(&stdout(&sandbox.run(&["server", "show", "LOJA"])));
    assert_eq!(shown["name"], "Loja");
    assert_eq!(shown["groupId"], 1);
    assert_eq!(json(&stdout(&sandbox.run(&["server", "show", "1"])))["id"], 1);

    assert!(sandbox.run(&["server", "test", "loja"]).status.success());

    // Excluir fora de um terminal pede --yes
    assert_eq!(sandbox.run(&["server", "rm", "loja"]).status.code(), Some(2));
    assert!(sandbox.run(&["server", "rm", "loja", "--yes"]).status.success());
    assert_eq!(json(&stdout(&sandbox.run(&["server", "list"]))), serde_json::json!([]));

    let missing = sandbox.run(&["server", "show", "loja"]);
    assert_eq!(missing.status.code(), Some(3));
    let error = json(&stderr(&missing));
    assert_eq!(error["code"], "NOT_FOUND");
    assert!(error["message"].as_str().unwrap().contains("loja"));
}

/// Vários processos partindo juntos numa máquina nova: todos criam a pasta,
/// a chave do vault e o `app.db` ao mesmo tempo. Nenhum falha, e as senhas que
/// cada um gravou abrem com a chave que ficou no disco.
#[test]
fn processes_starting_together_share_one_vault_and_database() {
    let sandbox = Sandbox::new("race");
    const PROCESSES: usize = 8;

    let children: Vec<_> = (0..PROCESSES)
        .map(|index| {
            let mut command = std::process::Command::new(assert_cmd::cargo::cargo_bin("octapus"));
            command
                .env("OCTAPUS_DATA_DIR", sandbox.data_dir())
                .args(["server", "add", &format!("postgres://u:senha{index}@host{index}.example.com/db")])
                .args(["--name", &format!("s{index}")])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            command.spawn().unwrap()
        })
        .collect();
    for child in children {
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{}", stderr(&output));
    }

    let servers = json(&stdout(&sandbox.run(&["server", "list"])));
    assert_eq!(servers.as_array().unwrap().len(), PROCESSES);
    assert_eq!(std::fs::read(sandbox.data_dir().join("vault.key")).unwrap().len(), 32);
    // Nenhum temporário da chave sobrou
    let leftovers: Vec<_> = std::fs::read_dir(sandbox.data_dir())
        .unwrap()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");

    // Toda senha abre com a chave do disco: o teste de conexão chega a tentar
    // conectar (erro de conexão), em vez de falhar ao decifrar
    for index in 0..PROCESSES {
        let output = sandbox.command().args(["server", "edit", &format!("s{index}"), "--port", "1"]).output().unwrap();
        assert!(output.status.success(), "{}", stderr(&output));
    }
}

#[test]
fn usage_errors_and_paths() {
    let sandbox = Sandbox::new("usage");

    let paths = json(&stdout(&sandbox.run(&["paths"])));
    assert_eq!(paths["data"], sandbox.data_dir().to_str().unwrap());
    // `paths` e `completions` não criam nada
    assert!(!sandbox.data_dir().exists());
    assert!(stdout(&sandbox.run(&["completions", "fish"])).contains("octapus"));

    // Sem subcomando e sem terminal: a ajuda, com código de uso
    let output = sandbox.run(&[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).contains("Usage: octapus"));

    assert_eq!(sandbox.run(&["server", "add"]).status.code(), Some(2));
    assert_eq!(sandbox.run(&["server", "add", "isto não é uma url"]).status.code(), Some(2));
}
