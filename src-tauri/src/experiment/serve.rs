//! A tela das rodadas e do cadastro de perguntas: um servidor HTTP mínimo, só
//! em `127.0.0.1`.
//!
//! **Rodadas: só leitura.** Não há rota que inicie, pare ou altere uma rodada
//! — quem controla é a linha de comando, e a rodada não depende da tela.
//!
//! **Perguntas: a única escrita.** As três rotas `POST` regravam o arquivo do
//! conjunto de perguntas e conferem um gabarito nos bancos de teste (pelo
//! caminho só-leitura). `--read-only` desliga as três.
//!
//! | rota                              | resposta                               |
//! |-----------------------------------|----------------------------------------|
//! | `GET /`                           | a página (`viewer.html`, embutida)     |
//! | `GET /api/runs`                   | as rodadas da pasta, da mais nova      |
//! | `GET /api/runs/<id>/manifest`     | `manifest.json`                        |
//! | `GET /api/runs/<id>/live`         | `live.json`                            |
//! | `GET /api/runs/<id>/gold`         | `gold.json`                            |
//! | `GET /api/runs/<id>/units?from=N` | os testes gravados a partir da linha N |
//! | `GET /api/runs/<id>/summary?by=…` | o resumo, pelo mesmo cálculo do CSV    |
//! | `GET /api/questions`              | o conjunto de perguntas e os problemas |
//! | `GET /api/catalog`                | o catálogo dos benchmarks baixados     |
//! | `POST /api/questions/save`        | grava uma pergunta (se ela passa)      |
//! | `POST /api/questions/delete`      | tira uma pergunta do conjunto          |
//! | `POST /api/questions/check`       | executa os gabaritos dela nos bancos   |
//!
//! Sem dependência de servidor web: um pedido por conexão sobre um
//! `TcpListener`. O `Host` precisa ser a própria máquina, para uma página de
//! fora não alcançar a tela por um nome que resolva para 127.0.0.1; e um
//! `POST` precisa vir da própria tela (`Origin`) como JSON.

use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::editor;
use super::report::{self, Dimension};
use super::store::{self, RunDir};

const PAGE: &str = include_str!("viewer.html");
/// Teto do cabeçalho de um pedido.
const MAX_HEAD: usize = 16 * 1024;
/// Teto do corpo de um `POST` (uma pergunta com os gabaritos).
const MAX_BODY: usize = 1024 * 1024;

/// O que a tela serve.
pub struct Desk {
    pub port: u16,
    /// A pasta das rodadas
    pub runs: PathBuf,
    /// O arquivo do conjunto de perguntas (o único que a tela escreve)
    pub questions: PathBuf,
    /// O catálogo dos benchmarks baixados (`catalog`)
    pub catalog: PathBuf,
    /// O ambiente, para conferir um gabarito nos bancos
    pub environment: PathBuf,
    /// Sem cadastro de perguntas: nenhum `POST` é aceito
    pub read_only: bool,
}

struct Response {
    status: &'static str,
    content_type: &'static str,
    body: Vec<u8>,
}

impl Response {
    fn json(value: &Value) -> Self {
        Self::raw_json(value.to_string().into_bytes())
    }

    fn raw_json(body: Vec<u8>) -> Self {
        Self {
            status: "200 OK",
            content_type: "application/json; charset=utf-8",
            body,
        }
    }

    fn error(status: &'static str) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8",
            body: status.as_bytes().to_vec(),
        }
    }

    /// Um pedido recusado, com o motivo para a tela mostrar.
    fn refused(problems: &[String]) -> Self {
        Self {
            status: "422 Unprocessable Entity",
            ..Self::json(&json!({ "problems": problems }))
        }
    }
}

/// Serve a tela até o processo ser encerrado.
pub async fn serve(desk: Desk) -> io::Result<()> {
    // Só a interface local: a tela não é exposta na rede
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, desk.port))).await?;
    println!(
        "Tela em http://127.0.0.1:{}  (Ctrl+C encerra)\n  rodadas:   {}\n  perguntas: {}{}",
        desk.port,
        desk.runs.display(),
        desk.questions.display(),
        if desk.read_only { "  (só leitura)" } else { "" }
    );
    let desk = std::sync::Arc::new(desk);
    loop {
        let (stream, _) = listener.accept().await?;
        let desk = std::sync::Arc::clone(&desk);
        tokio::spawn(async move {
            let _ = connection(stream, &desk).await;
        });
    }
}

async fn connection(mut stream: TcpStream, desk: &Desk) -> io::Result<()> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
        let read = stream.read(&mut chunk).await?;
        if read == 0 || buffer.len() + read > MAX_HEAD + MAX_BODY {
            return Ok(());
        }
        buffer.extend_from_slice(&chunk[..read]);
    };
    if head_end > MAX_HEAD {
        return Ok(());
    }
    let head = String::from_utf8_lossy(&buffer[..head_end]).into_owned();
    let length = header(&head, "content-length").and_then(|value| value.parse::<usize>().ok()).unwrap_or(0);
    if length > MAX_BODY {
        return Ok(());
    }
    while buffer.len() < head_end + length {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Ok(());
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    let response = respond(&head, &buffer[head_end..head_end + length], desk).await;

    let head = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        response.status,
        response.content_type,
        response.body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(&response.body).await?;
    stream.shutdown().await
}

/// O valor de um cabeçalho do pedido.
fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines()
        .skip(1)
        .filter_map(|line| line.split_once(':'))
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.trim())
}

/// A resposta de um pedido (o cabeçalho como chegou e o corpo).
async fn respond(head: &str, body: &[u8], desk: &Desk) -> Response {
    let mut first = head.lines().next().unwrap_or_default().split_whitespace();
    let (Some(method), Some(target)) = (first.next(), first.next()) else {
        return Response::error("400 Bad Request");
    };
    let port = desk.port;
    let local = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    if !header(head, "host").is_some_and(|host| local.iter().any(|name| name == host)) {
        return Response::error("403 Forbidden");
    }

    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let segments: Vec<&str> = path.split('/').filter(|segment| !segment.is_empty()).collect();
    match (method, segments.as_slice()) {
        ("GET", []) => Response {
            status: "200 OK",
            content_type: "text/html; charset=utf-8",
            body: PAGE.as_bytes().to_vec(),
        },
        ("GET", ["api", "runs"]) => Response::json(&runs(&desk.runs)),
        ("GET", ["api", "runs", id, what]) => match run_dir(&desk.runs, id) {
            Some(dir) => run_route(&dir, what, query),
            None => Response::error("404 Not Found"),
        },
        ("GET", ["api", "questions"]) => match editor::list(&desk.questions) {
            Ok(mut listed) => {
                listed["readOnly"] = json!(desk.read_only);
                Response::json(&listed)
            }
            Err(message) => Response::refused(&[message]),
        },
        ("GET", ["api", "catalog"]) => match std::fs::read(&desk.catalog) {
            Ok(body) => Response::raw_json(body),
            Err(_) => Response::error("404 Not Found"),
        },
        ("POST", ["api", "questions", action]) => {
            // Só a própria tela escreve: o navegador manda o `Origin` dela,
            // e um formulário de outra página não consegue mandar JSON
            let origins = local.map(|host| format!("http://{host}"));
            let from_page = header(head, "origin").is_none_or(|origin| origins.iter().any(|name| name == origin));
            let is_json = header(head, "content-type").is_some_and(|kind| kind.starts_with("application/json"));
            if desk.read_only {
                return Response::error("405 Method Not Allowed");
            }
            if !from_page || !is_json {
                return Response::error("403 Forbidden");
            }
            questions_route(action, body, desk).await
        }
        ("GET", _) => Response::error("404 Not Found"),
        // As rodadas só aceitam leitura
        _ => Response::error("405 Method Not Allowed"),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ById {
    id: String,
}

/// As três escritas da tela, todas sobre o conjunto de perguntas.
async fn questions_route(action: &str, body: &[u8], desk: &Desk) -> Response {
    fn read<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, Response> {
        serde_json::from_slice(body).map_err(|error| Response::refused(&[format!("pedido inválido: {error}")]))
    }
    match action {
        "save" => match read::<editor::Save>(body) {
            Ok(request) => match editor::save(&desk.questions, request) {
                Ok(()) => {
                    log::info!("experimento: pergunta gravada no conjunto");
                    Response::json(&json!({ "ok": true }))
                }
                Err(problems) => Response::refused(&problems),
            },
            Err(response) => response,
        },
        "delete" => match read::<ById>(body) {
            Ok(request) => match editor::delete(&desk.questions, &request.id) {
                Ok(()) => Response::json(&json!({ "ok": true })),
                Err(message) => Response::refused(&[message]),
            },
            Err(response) => response,
        },
        "check" => match read::<ById>(body) {
            Ok(request) => match editor::check(&desk.questions, &desk.environment, &request.id).await {
                Ok(checked) => Response::json(&checked),
                Err(message) => Response::refused(&[message]),
            },
            Err(response) => response,
        },
        _ => Response::error("404 Not Found"),
    }
}

/// A pasta da rodada `id`, se o nome for só um nome (nada de `..` nem `/`).
fn run_dir(root: &Path, id: &str) -> Option<RunDir> {
    let plain = !id.starts_with('.') && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if !plain {
        return None;
    }
    RunDir::open(&root.join(id)).ok()
}

fn run_route(dir: &RunDir, what: &str, query: &str) -> Response {
    let file = |name: &str| match std::fs::read(dir.path().join(name)) {
        Ok(body) => Response::raw_json(body),
        Err(_) => Response::error("404 Not Found"),
    };
    let parameter = |name: &str| {
        query
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value)
    };
    match what {
        "manifest" => file(store::MANIFEST),
        "live" => file(store::LIVE),
        "gold" => file(store::GOLD),
        "units" => {
            let from = parameter("from").and_then(|from| from.parse().ok()).unwrap_or(0);
            Response::json(&units_from(dir.path(), from))
        }
        "summary" => {
            let dimensions: Vec<Dimension> = match parameter("by") {
                Some(by) => by.split(',').filter_map(Dimension::parse).collect(),
                None => report::DEFAULT_DIMENSIONS.to_vec(),
            };
            match dir.units() {
                Ok(records) => Response::json(&json!({
                    "by": dimensions.iter().map(|dimension| dimension.name()).collect::<Vec<_>>(),
                    "rows": report::summarize(&records, &dimensions),
                })),
                Err(_) => Response::error("500 Internal Server Error"),
            }
        }
        _ => Response::error("404 Not Found"),
    }
}

/// As rodadas da pasta, com o que a lista mostra de cada uma.
fn runs(root: &Path) -> Value {
    let runs: Vec<Value> = store::list(root)
        .iter()
        .filter_map(|path| {
            let id = path.file_name()?.to_str()?;
            let read = |name: &str| -> Option<Value> { serde_json::from_slice(&std::fs::read(path.join(name)).ok()?).ok() };
            let manifest = read(store::MANIFEST)?;
            let live = read(store::LIVE);
            let pick = |value: &Value, key: &str| value.get(key).cloned().unwrap_or(Value::Null);
            Some(json!({
                "id": id,
                "createdAt": pick(&manifest, "createdAt"),
                "total": pick(&manifest, "total"),
                "simulated": pick(&manifest, "simulated"),
                "status": live.as_ref().map(|live| pick(live, "status")),
                "done": live.as_ref().map(|live| pick(live, "done")),
                "updatedAt": live.as_ref().map(|live| pick(live, "updatedAt")),
            }))
        })
        .collect();
    json!({ "runs": runs })
}

/// As linhas do `units.jsonl` a partir de `from`, como foram gravadas, e
/// quantas linhas o arquivo tem (o próximo `from`). A página junta por chave:
/// de um teste que rodou de novo vale a última linha.
fn units_from(dir: &Path, from: usize) -> Value {
    let text = std::fs::read_to_string(dir.join(store::UNITS)).unwrap_or_default();
    // Uma linha ainda sem a quebra do fim está sendo escrita: fica para depois
    let complete = text.rfind('\n').map_or("", |end| &text[..end]);
    let lines: Vec<&str> = complete.lines().collect();
    let units: Vec<Value> = lines
        .iter()
        .skip(from)
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    json!({ "next": lines.len(), "units": units })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCAL: &str = "127.0.0.1:7878";

    fn get(target: &str, host: &str) -> String {
        format!("GET {target} HTTP/1.1\r\nHost: {host}\r\nAccept: */*\r\n\r\n")
    }

    fn post(target: &str, extra: &str) -> String {
        format!("POST {target} HTTP/1.1\r\nHost: {LOCAL}\r\n{extra}\r\n")
    }

    const FROM_PAGE: &str = "Origin: http://127.0.0.1:7878\r\nContent-Type: application/json\r\n";

    fn desk(name: &str) -> Desk {
        let root = std::env::temp_dir().join(format!("octapus-experiment-serve-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let run = root.join("runs").join("rodada-1");
        std::fs::create_dir_all(&run).unwrap();
        std::fs::write(run.join("manifest.json"), r#"{"runId":"rodada-1","total":2,"simulated":true}"#).unwrap();
        std::fs::write(run.join("live.json"), r#"{"status":"running","done":1}"#).unwrap();
        std::fs::write(run.join("units.jsonl"), "{\"key\":\"a\"}\n{\"key\":\"b\"}\n{\"key\":\"c").unwrap();
        // Fora da pasta das rodadas: não pode ser alcançado
        std::fs::write(root.join("segredo.json"), "{}").unwrap();
        Desk {
            port: 7878,
            runs: root.join("runs"),
            questions: root.join("perguntas.toml"),
            catalog: root.join("catalog.json"),
            environment: root.join("ambiente.toml"),
            read_only: false,
        }
    }

    fn cleanup(desk: &Desk) {
        let _ = std::fs::remove_dir_all(desk.runs.parent().unwrap());
    }

    async fn status(request: &str, desk: &Desk) -> &'static str {
        respond(request, b"", desk).await.status
    }

    #[tokio::test]
    async fn serves_the_page_and_the_recorded_files() {
        let desk = desk("files");

        let page = respond(&get("/", LOCAL), b"", &desk).await;
        assert_eq!((page.status, page.content_type), ("200 OK", "text/html; charset=utf-8"));

        let runs: Value = serde_json::from_slice(&respond(&get("/api/runs", LOCAL), b"", &desk).await.body).unwrap();
        assert_eq!(runs["runs"][0]["id"], "rodada-1");
        assert_eq!((&runs["runs"][0]["status"], &runs["runs"][0]["done"]), (&json!("running"), &json!(1)));

        // O arquivo como foi gravado
        let live = respond(&get("/api/runs/rodada-1/live", "localhost:7878"), b"", &desk).await;
        assert_eq!(live.body, br#"{"status":"running","done":1}"#);

        // Só as linhas completas, a partir da pedida
        let units = respond(&get("/api/runs/rodada-1/units?from=1", LOCAL), b"", &desk).await;
        let units: Value = serde_json::from_slice(&units.body).unwrap();
        assert_eq!(units, json!({ "next": 2, "units": [{ "key": "b" }] }));

        assert_eq!(status(&get("/api/runs/rodada-1/gold", LOCAL), &desk).await, "404 Not Found");
        assert_eq!(status(&get("/api/catalog", LOCAL), &desk).await, "404 Not Found");
        cleanup(&desk);
    }

    #[tokio::test]
    async fn only_reads_from_this_machine_and_inside_the_runs_folder() {
        let desk = desk("guard");

        // Outro nome no Host (DNS apontando para 127.0.0.1) e outra porta
        for host in ["evil.example.com", "evil.example.com:7878", "127.0.0.1:9999", ""] {
            assert_eq!(status(&get("/api/runs", host), &desk).await, "403 Forbidden", "{host}");
        }
        // As rodadas não aceitam nada além de leitura
        for method in ["POST", "PUT", "DELETE", "PATCH"] {
            let request = format!("{method} /api/runs/rodada-1/live HTTP/1.1\r\nHost: {LOCAL}\r\n{FROM_PAGE}\r\n");
            assert_eq!(status(&request, &desk).await, "405 Method Not Allowed", "{method}");
        }
        // Nenhum caminho sai da pasta das rodadas
        for target in [
            "/api/runs/../manifest",
            "/api/runs/..%2f..%2fetc/manifest",
            "/api/runs/.hidden/manifest",
            "/api/runs/rodada-1/../../segredo.json",
            "/api/runs/rodada-1/segredo.json",
            "/segredo.json",
        ] {
            assert_eq!(status(&get(target, LOCAL), &desk).await, "404 Not Found", "{target}");
        }
        assert_eq!(status("lixo", &desk).await, "400 Bad Request");
        cleanup(&desk);
    }

    #[tokio::test]
    async fn the_page_registers_questions_and_nothing_else_writes() {
        let desk = desk("questions");
        let body = json!({
            "question": {
                "id": "q1", "difficulty": "simple",
                "pt": "Quantos clientes existem no cadastro da loja?",
                "en": "How many customers are there in the store records?",
                "gold": { "sqlite": { "query": "SELECT count(*) FROM customers" } },
            },
            "replaces": null,
        })
        .to_string();
        let save = post("/api/questions/save", FROM_PAGE);

        let saved = respond(&save, body.as_bytes(), &desk).await;
        assert_eq!(saved.status, "200 OK", "{}", String::from_utf8_lossy(&saved.body));
        let listed: Value = serde_json::from_slice(&respond(&get("/api/questions", LOCAL), b"", &desk).await.body).unwrap();
        assert_eq!(listed["questions"][0]["id"], "q1");
        assert_eq!((&listed["problems"], &listed["readOnly"]), (&json!([]), &json!(false)));

        // O que não passa volta com o motivo, e não é gravado
        let refused = respond(&save, body.replace("SELECT count(*)", "DELETE").as_bytes(), &desk).await;
        assert_eq!(refused.status, "422 Unprocessable Entity");
        assert!(String::from_utf8_lossy(&refused.body).contains("problems"));
        assert_eq!(respond(&save, b"{", &desk).await.status, "422 Unprocessable Entity");

        // De outra página, ou sem ser JSON: recusado
        let foreign = post("/api/questions/save", "Origin: http://evil.example.com\r\nContent-Type: application/json\r\n");
        assert_eq!(respond(&foreign, body.as_bytes(), &desk).await.status, "403 Forbidden");
        let form = post("/api/questions/save", "Origin: http://127.0.0.1:7878\r\nContent-Type: text/plain\r\n");
        assert_eq!(respond(&form, body.as_bytes(), &desk).await.status, "403 Forbidden");
        assert_eq!(respond(&post("/api/questions/outra", FROM_PAGE), b"{}", &desk).await.status, "404 Not Found");

        let delete = post("/api/questions/delete", FROM_PAGE);
        assert_eq!(respond(&delete, br#"{"id":"q1"}"#, &desk).await.status, "200 OK");
        assert_eq!(respond(&delete, br#"{"id":"q1"}"#, &desk).await.status, "422 Unprocessable Entity");

        // Com `--read-only` a tela não escreve nada
        let locked = Desk {
            read_only: true,
            ..desk
        };
        assert_eq!(respond(&save, body.as_bytes(), &locked).await.status, "405 Method Not Allowed");
        assert!(!locked.questions.exists() || !std::fs::read_to_string(&locked.questions).unwrap().contains("q1"));
        cleanup(&locked);
    }
}
