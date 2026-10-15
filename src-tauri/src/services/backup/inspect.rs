//! O arquivo escolhido para restaurar: que formato é (pelo conteúdo, não pela
//! extensão) e, num `.dump`, o que tem dentro (`pg_restore --list`).

use std::path::Path;
use std::time::Duration;

use tokio::io::AsyncReadExt;

use crate::models::{ArchiveSummary, RestoreFormat};

use super::tool_command;

/// Bytes lidos do começo do arquivo para reconhecer o formato.
const HEAD_BYTES: usize = 8 * 1024;
/// O `--list` lê só o índice do arquivo — rápido mesmo em dumps grandes.
const LIST_TIMEOUT: Duration = Duration::from_secs(30);
/// Schemas listados no resumo (o resto só conta).
const SUMMARY_SCHEMAS: usize = 20;

/// Formato e tamanho do arquivo.
pub async fn detect_format(path: &Path) -> Result<(RestoreFormat, u64), String> {
    let meta = tokio::fs::metadata(path)
        .await
        .map_err(|e| format!("Não foi possível abrir {}: {e}", path.display()))?;
    if meta.is_dir() {
        return Err(
            "Escolha um arquivo .dump ou .sql (backups em pasta não são suportados).".into(),
        );
    }

    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| format!("Não foi possível abrir {}: {e}", path.display()))?;
    let mut head = vec![0; HEAD_BYTES];
    let mut read = 0;
    // `read` pode devolver menos que o pedido antes do fim do arquivo
    while read < head.len() {
        let n = file
            .read(&mut head[read..])
            .await
            .map_err(|e| format!("Não foi possível ler {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        read += n;
    }
    head.truncate(read);

    Ok((format_from_head(&head)?, meta.len()))
}

/// Reconhece o formato pelos primeiros bytes.
fn format_from_head(head: &[u8]) -> Result<RestoreFormat, String> {
    if head.is_empty() {
        return Err("O arquivo está vazio.".into());
    }
    if head.starts_with(b"PGDMP") {
        return Ok(RestoreFormat::Custom);
    }
    if head.starts_with(&[0x1f, 0x8b]) {
        return Err("O arquivo está compactado (.gz). Descompacte antes de restaurar.".into());
    }
    if head.get(257..262) == Some(b"ustar") {
        return Err(
            "Backups no formato tar não são suportados. Gere um .dump (formato custom) ou um .sql."
                .into(),
        );
    }
    if head.contains(&0) {
        return Err("Não parece um backup do PostgreSQL (.dump) nem um script SQL.".into());
    }
    Ok(RestoreFormat::Plain)
}

/// O resumo do `.dump` pelo `pg_restore --list` (não toca em banco nenhum).
pub async fn summarize(pg_restore: &Path, path: &Path) -> Result<ArchiveSummary, String> {
    let output = tool_command(pg_restore).arg("--list").arg(path).output();
    let output = tokio::time::timeout(LIST_TIMEOUT, output)
        .await
        .map_err(|_| "O pg_restore demorou demais para ler o arquivo.".to_string())?
        .map_err(|e| format!("Não foi possível executar o pg_restore: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = stderr.trim();
        return Err(if message.is_empty() {
            "O pg_restore não conseguiu ler o arquivo.".into()
        } else {
            message.to_string()
        });
    }
    Ok(parse_list(&String::from_utf8_lossy(&output.stdout)))
}

/// Lê a saída do `--list`: o cabeçalho (`;     dbname: loja`) e uma linha por
/// objeto (`215; 1259 16385 TABLE public pedidos app`).
fn parse_list(list: &str) -> ArchiveSummary {
    let mut summary = ArchiveSummary::default();
    for line in list.lines() {
        if let Some(comment) = line.strip_prefix(';') {
            let comment = comment.trim();
            let value = |key: &str| comment.strip_prefix(key).map(|v| v.trim().to_string());
            if let Some(v) = value("dbname:") {
                summary.database = Some(v);
            } else if let Some(v) = value("Dumped from database version:") {
                summary.server_version = Some(v);
            } else if let Some(v) = value("Dumped by pg_dump version:") {
                summary.pg_dump_version = Some(v);
            }
            continue;
        }
        // `id; tableoid oid DESCRIÇÃO...` — a descrição começa no 3º pedaço
        let Some((_, rest)) = line.split_once("; ") else {
            continue;
        };
        let mut parts = rest.splitn(3, ' ');
        let desc = match (parts.next(), parts.next(), parts.next()) {
            (Some(_), Some(_), Some(desc)) => desc,
            _ => continue,
        };

        if let Some(schema) = desc.strip_prefix("SCHEMA - ") {
            // O último pedaço é o dono; o nome pode ter espaço
            let name = schema.rsplit_once(' ').map_or(schema, |(name, _)| name);
            summary.schema_count += 1;
            if summary.schemas.len() < SUMMARY_SCHEMAS {
                summary.schemas.push(name.to_string());
            }
        } else if desc.starts_with("TABLE DATA ") {
            summary.has_data = true;
        } else if desc.starts_with("TABLE ") {
            summary.table_count += 1;
        } else if desc.starts_with("VIEW ") || desc.starts_with("MATERIALIZED VIEW ") {
            summary.view_count += 1;
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = "\
;
; Archive created at 2026-10-15 14:32:00 -03
;     dbname: loja
;     TOC Entries: 12
;     Compression: gzip
;     Dump Version: 1.15-0
;     Format: CUSTOM
;     Dumped from database version: 16.4 (Debian 16.4-1.pgdg120+1)
;     Dumped by pg_dump version: 17.2
;
;
; Selected TOC Entries:
;
5; 2615 2200 SCHEMA - public pg_database_owner
6; 2615 16390 SCHEMA - minhas vendas app
215; 1259 16385 TABLE public pedidos app
216; 1259 16391 TABLE minhas vendas itens app
217; 1259 16395 VIEW public vw_pedidos app
218; 1259 16399 MATERIALIZED VIEW public mv_total app
219; 1259 16384 SEQUENCE public pedidos_id_seq app
3350; 0 16385 TABLE DATA public pedidos app
3351; 0 0 SEQUENCE SET public pedidos_id_seq app
3200; 2606 16400 CONSTRAINT public pedidos pedidos_pkey app
";

    #[test]
    fn summarizes_the_list() {
        let summary = parse_list(LIST);
        assert_eq!(summary.database.as_deref(), Some("loja"));
        assert_eq!(
            summary.server_version.as_deref(),
            Some("16.4 (Debian 16.4-1.pgdg120+1)")
        );
        assert_eq!(summary.pg_dump_version.as_deref(), Some("17.2"));
        assert_eq!(summary.schemas, vec!["public", "minhas vendas"]);
        assert_eq!(summary.schema_count, 2);
        assert_eq!(summary.table_count, 2);
        assert_eq!(summary.view_count, 2);
        assert!(summary.has_data);
    }

    #[test]
    fn structure_only_has_no_data() {
        let list = "215; 1259 16385 TABLE public pedidos app\n";
        let summary = parse_list(list);
        assert_eq!(summary.table_count, 1);
        assert!(!summary.has_data);
        assert_eq!(summary.database, None);
    }

    #[test]
    fn recognizes_formats_by_content() {
        assert_eq!(
            format_from_head(b"PGDMP\x01\x0f\x00"),
            Ok(RestoreFormat::Custom)
        );
        assert_eq!(
            format_from_head(b"--\n-- PostgreSQL database dump\n--\n"),
            Ok(RestoreFormat::Plain)
        );
        assert!(format_from_head(b"").unwrap_err().contains("vazio"));
        assert!(format_from_head(&[0x1f, 0x8b, 8, 0])
            .unwrap_err()
            .contains(".gz"));
        assert!(format_from_head(b"\x89PNG\r\n\x1a\n\0\0").is_err());

        let mut tar = vec![b'a'; 300];
        tar[257..262].copy_from_slice(b"ustar");
        assert!(format_from_head(&tar).unwrap_err().contains("tar"));
    }

    #[tokio::test]
    async fn detects_from_a_file_on_disk() {
        let dir = std::env::temp_dir().join(format!("octapus-inspect-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("x.sql");
        std::fs::write(&file, "CREATE TABLE t (id int);\n").unwrap();

        assert_eq!(detect_format(&file).await, Ok((RestoreFormat::Plain, 25)));
        assert!(detect_format(&dir).await.unwrap_err().contains("pasta"));
        assert!(detect_format(&dir.join("nada.dump")).await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
