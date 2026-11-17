//! A tela das rodadas: um servidor HTTP mínimo, só em `127.0.0.1`, que só lê
//! as pastas de rodada. Não há rota que escreva, inicie ou pare nada — quem
//! controla a rodada é a linha de comando, e a rodada não depende da tela.
//!
//! | rota                              | resposta                               |
//! |-----------------------------------|----------------------------------------|
//! | `/`                               | a página (`viewer.html`, embutida)     |
//! | `/api/runs`                       | as rodadas da pasta, da mais nova      |
//! | `/api/runs/<id>/manifest`         | `manifest.json`                        |
//! | `/api/runs/<id>/live`             | `live.json`                            |
//! | `/api/runs/<id>/gold`             | `gold.json`                            |
//! | `/api/runs/<id>/units?from=N`     | os testes gravados a partir da linha N |
//! | `/api/runs/<id>/summary?by=a,b`   | o resumo, pelo mesmo cálculo do CSV    |
//!
//! Sem dependência de servidor web: `GET` sobre um `TcpListener`, uma resposta
//! por conexão. O cabeçalho `Host` precisa ser a própria máquina, para uma
//! página de fora não alcançar a tela por um nome que resolva para 127.0.0.1.

use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::report::{self, Dimension};
use super::store::{self, RunDir};

const PAGE: &str = include_str!("viewer.html");
/// Teto do cabeçalho de um pedido.
const MAX_REQUEST: usize = 16 * 1024;

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
}

/// Serve a tela até o processo ser encerrado.
pub async fn serve(root: PathBuf, port: u16) -> io::Result<()> {
    // Só a interface local: a tela não é exposta na rede
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).await?;
    println!(
        "Tela das rodadas em http://127.0.0.1:{port}  (pasta: {}; Ctrl+C encerra)",
        root.display()
    );
    loop {
        let (stream, _) = listener.accept().await?;
        let root = root.clone();
        tokio::spawn(async move {
            let _ = connection(stream, &root, port).await;
        });
    }
}

async fn connection(mut stream: TcpStream, root: &Path, port: u16) -> io::Result<()> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 2048];
    while !buffer.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = stream.read(&mut chunk).await?;
        if read == 0 || buffer.len() + read > MAX_REQUEST {
            return Ok(());
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    let request = String::from_utf8_lossy(&buffer);
    let response = respond(&request, root, port);

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

/// A resposta de um pedido (o cabeçalho inteiro, como chegou).
fn respond(request: &str, root: &Path, port: u16) -> Response {
    let mut lines = request.lines();
    let mut first = lines.next().unwrap_or_default().split_whitespace();
    let (Some(method), Some(target)) = (first.next(), first.next()) else {
        return Response::error("400 Bad Request");
    };
    if method != "GET" {
        return Response::error("405 Method Not Allowed");
    }
    let host = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("host"))
        .map(|(_, value)| value.trim().to_string());
    let local = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    if !host.is_some_and(|host| local.contains(&host)) {
        return Response::error("403 Forbidden");
    }

    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let segments: Vec<&str> = path.split('/').filter(|segment| !segment.is_empty()).collect();
    match segments.as_slice() {
        [] => Response {
            status: "200 OK",
            content_type: "text/html; charset=utf-8",
            body: PAGE.as_bytes().to_vec(),
        },
        ["api", "runs"] => Response::json(&runs(root)),
        ["api", "runs", id, what] => match run_dir(root, id) {
            Some(dir) => run_route(&dir, what, query),
            None => Response::error("404 Not Found"),
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

    fn get(target: &str, host: &str) -> String {
        format!("GET {target} HTTP/1.1\r\nHost: {host}\r\nAccept: */*\r\n\r\n")
    }

    fn root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("octapus-experiment-serve-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let run = root.join("rodada-1");
        std::fs::create_dir_all(&run).unwrap();
        std::fs::write(run.join("manifest.json"), r#"{"runId":"rodada-1","total":2,"simulated":true}"#).unwrap();
        std::fs::write(run.join("live.json"), r#"{"status":"running","done":1}"#).unwrap();
        std::fs::write(run.join("units.jsonl"), "{\"key\":\"a\"}\n{\"key\":\"b\"}\n{\"key\":\"c").unwrap();
        // Fora da pasta das rodadas: não pode ser alcançado
        std::fs::write(root.join("segredo.json"), "{}").unwrap();
        root
    }

    #[test]
    fn serves_the_page_and_the_recorded_files() {
        let root = root("files");
        let local = "127.0.0.1:7878";

        let page = respond(&get("/", local), &root, 7878);
        assert_eq!((page.status, page.content_type), ("200 OK", "text/html; charset=utf-8"));

        let runs: Value = serde_json::from_slice(&respond(&get("/api/runs", local), &root, 7878).body).unwrap();
        assert_eq!(runs["runs"][0]["id"], "rodada-1");
        assert_eq!((&runs["runs"][0]["status"], &runs["runs"][0]["done"]), (&json!("running"), &json!(1)));

        // O arquivo como foi gravado
        let live = respond(&get("/api/runs/rodada-1/live", "localhost:7878"), &root, 7878);
        assert_eq!(live.body, br#"{"status":"running","done":1}"#);

        // Só as linhas completas, a partir da pedida
        let units: Value =
            serde_json::from_slice(&respond(&get("/api/runs/rodada-1/units?from=1", local), &root, 7878).body).unwrap();
        assert_eq!(units, json!({ "next": 2, "units": [{ "key": "b" }] }));

        assert_eq!(respond(&get("/api/runs/rodada-1/gold", local), &root, 7878).status, "404 Not Found");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn only_reads_from_this_machine_and_inside_the_runs_folder() {
        let root = root("guard");
        let local = "127.0.0.1:7878";

        // Outro nome no Host (DNS apontando para 127.0.0.1) e outra porta
        for host in ["evil.example.com", "evil.example.com:7878", "127.0.0.1:9999", ""] {
            assert_eq!(respond(&get("/api/runs", host), &root, 7878).status, "403 Forbidden", "{host}");
        }
        // Nada além de leitura
        for method in ["POST", "PUT", "DELETE", "PATCH"] {
            let request = format!("{method} /api/runs/rodada-1/live HTTP/1.1\r\nHost: {local}\r\n\r\n");
            assert_eq!(respond(&request, &root, 7878).status, "405 Method Not Allowed");
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
            assert_eq!(respond(&get(target, local), &root, 7878).status, "404 Not Found", "{target}");
        }
        assert_eq!(respond("lixo", &root, 7878).status, "400 Bad Request");
        let _ = std::fs::remove_dir_all(root);
    }
}
