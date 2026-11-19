//! Exportação de resultados em CSV, JSON ou INSERTs SQL, gravada em stream.
//!
//! Mesmo formato do front React (`src/components/export-dialog/serialize.ts`),
//! que hoje monta o texto inteiro na memória e manda ao `write_export_file`.
//! Aqui cada linha vai direto para o arquivo: exportar um milhão de linhas
//! não precisa caber na memória duas vezes. Quem usa: a UI nativa (o app
//! Tauri segue com o serializador do front).
//!
//! O arquivo é escrito num temporário ao lado (com o pid no nome) e só
//! ganha o nome final no [`FileExport::commit`]: uma exportação que falha no
//! meio não deixa arquivo pela metade.

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::adapters::RowBatch;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Json,
    Sql,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Csv => "csv",
            ExportFormat::Json => "json",
            ExportFormat::Sql => "sql",
        }
    }
}

/// Uma coluna exportada, com o índice dela na linha crua (as ocultas na grade
/// ficam de fora).
#[derive(Debug, Clone)]
pub struct ExportColumn {
    pub name: String,
    pub type_name: String,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub format: ExportFormat,
    /// CSV: separador entre campos
    pub delimiter: String,
    /// CSV: primeira linha com os nomes das colunas
    pub include_header: bool,
    /// SQL: tabela alvo dos INSERTs, já qualificada e escapada
    pub sql_table: String,
    /// SQL: colunas deixadas fora dos INSERTs para o banco aplicar o valor
    /// padrão (DEFAULT, serial, identity) — vale em qualquer dialeto, ao
    /// contrário da palavra `DEFAULT` no VALUES, que o SQLite não aceita
    pub sql_default_columns: HashSet<String>,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: ExportFormat::Csv,
            delimiter: ",".into(),
            include_header: true,
            sql_table: String::new(),
            sql_default_columns: HashSet::new(),
        }
    }
}

/// Como um valor vai cru no INSERT (o resto vira string entre aspas).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SqlValue {
    Boolean,
    Number,
    Text,
}

impl SqlValue {
    /// Pelo nome do tipo do banco — a mesma tabela do `resolveCellEditor` do
    /// front, só a parte que muda o literal.
    fn of(type_name: &str) -> Self {
        match type_name.trim().to_lowercase().as_str() {
            "bool" | "boolean" => SqlValue::Boolean,
            "int2" | "int4" | "int8" | "smallint" | "integer" | "bigint" | "serial" | "bigserial"
            | "smallserial" | "float4" | "float8" | "real" | "double precision" | "numeric" | "decimal"
            | "money" => SqlValue::Number,
            _ => SqlValue::Text,
        }
    }
}

/// Escreve as linhas de um resultado num `Write`, uma a uma.
pub struct ExportWriter<W: Write> {
    out: W,
    options: ExportOptions,
    /// As colunas que saem (no SQL, sem as de valor padrão; no JSON, um nome
    /// repetido sai uma vez só, com o valor da última coluna dele)
    columns: Vec<ExportColumn>,
    sql_values: Vec<SqlValue>,
    /// `INSERT INTO t (a, b) VALUES (` — montado uma vez
    sql_prefix: String,
    rows: u64,
    /// Já escreveu alguma linha de texto (para o separador `\n` entre elas)
    started: bool,
}

impl<W: Write> ExportWriter<W> {
    /// Começa a exportação: o cabeçalho do CSV ou o `[` do JSON já saem aqui.
    pub fn new(out: W, columns: Vec<ExportColumn>, options: ExportOptions) -> Result<Self> {
        let columns = match options.format {
            ExportFormat::Csv => columns,
            ExportFormat::Json => dedupe_by_name(columns),
            ExportFormat::Sql => columns
                .into_iter()
                .filter(|column| !options.sql_default_columns.contains(&column.name))
                .collect(),
        };
        let sql_values = columns.iter().map(|column| SqlValue::of(&column.type_name)).collect();
        let sql_prefix = match options.format {
            ExportFormat::Sql if columns.is_empty() => format!("INSERT INTO {} DEFAULT VALUES;", options.sql_table),
            ExportFormat::Sql => {
                let list: Vec<String> = columns.iter().map(|column| quote_ident(&column.name)).collect();
                format!("INSERT INTO {} ({}) VALUES (", options.sql_table, list.join(", "))
            }
            _ => String::new(),
        };

        let mut writer = Self {
            out,
            options,
            columns,
            sql_values,
            sql_prefix,
            rows: 0,
            started: false,
        };
        match writer.options.format {
            ExportFormat::Csv if writer.options.include_header => {
                let header: Vec<String> = writer
                    .columns
                    .iter()
                    .map(|column| csv_field(Some(&column.name), &writer.options.delimiter))
                    .collect();
                let line = header.join(&writer.options.delimiter);
                writer.line(&line)?;
            }
            ExportFormat::Json => writer.write("[")?,
            _ => {}
        }
        Ok(writer)
    }

    /// Uma linha crua (todas as colunas do resultado, na ordem do banco).
    /// Índice que não existe na linha sai como `NULL`.
    pub fn write_row(&mut self, row: &[Option<&str>]) -> Result<()> {
        let cell = |index: usize| row.get(index).copied().flatten();
        match self.options.format {
            ExportFormat::Csv => {
                let fields: Vec<String> = self
                    .columns
                    .iter()
                    .map(|column| csv_field(cell(column.index), &self.options.delimiter))
                    .collect();
                let line = fields.join(&self.options.delimiter);
                self.line(&line)?;
            }
            ExportFormat::Json => {
                let mut object = String::from(if self.rows == 0 { "\n  " } else { ",\n  " });
                if self.columns.is_empty() {
                    object.push_str("{}");
                } else {
                    object.push('{');
                    for (position, column) in self.columns.iter().enumerate() {
                        object.push_str(if position == 0 { "\n    " } else { ",\n    " });
                        object.push_str(&json_string(&column.name));
                        object.push_str(": ");
                        match cell(column.index) {
                            Some(value) => object.push_str(&json_string(value)),
                            None => object.push_str("null"),
                        }
                    }
                    object.push_str("\n  }");
                }
                self.write(&object)?;
            }
            ExportFormat::Sql => {
                let statement = if self.columns.is_empty() {
                    self.sql_prefix.clone()
                } else {
                    let values: Vec<String> = self
                        .columns
                        .iter()
                        .zip(&self.sql_values)
                        .map(|(column, kind)| sql_literal(cell(column.index), *kind))
                        .collect();
                    format!("{}{});", self.sql_prefix, values.join(", "))
                };
                self.line(&statement)?;
            }
        }
        self.rows += 1;
        Ok(())
    }

    /// As linhas de um lote do streaming (`execute_query_streamed`).
    pub fn write_batch(&mut self, batch: &RowBatch) -> Result<()> {
        let mut cells: Vec<Option<&str>> = Vec::with_capacity(batch.width());
        for row in 0..batch.rows() {
            cells.clear();
            cells.extend((0..batch.width()).map(|column| batch.cell(row, column)));
            self.write_row(&cells)?;
        }
        Ok(())
    }

    pub fn rows_written(&self) -> u64 {
        self.rows
    }

    /// Fecha o formato (o `]` do JSON), esvazia o buffer e devolve o `Write`.
    pub fn finish(mut self) -> Result<W> {
        if self.options.format == ExportFormat::Json {
            self.write(if self.rows == 0 { "]" } else { "\n]" })?;
        }
        self.out.flush().map_err(io_error)?;
        Ok(self.out)
    }

    /// Uma linha de CSV/SQL: o `\n` vai entre as linhas, não no fim.
    fn line(&mut self, text: &str) -> Result<()> {
        if self.started {
            self.write("\n")?;
        }
        self.started = true;
        self.write(text)
    }

    fn write(&mut self, text: &str) -> Result<()> {
        self.out.write_all(text.as_bytes()).map_err(io_error)
    }
}

/// Uma exportação para arquivo: grava num temporário ao lado e troca pelo
/// nome final no [`FileExport::commit`]. Largada sem commit (erro, cancelar),
/// apaga o temporário.
pub struct FileExport {
    writer: Option<ExportWriter<BufWriter<File>>>,
    temp: PathBuf,
    path: PathBuf,
}

impl FileExport {
    pub fn create(path: impl AsRef<Path>, columns: Vec<ExportColumn>, options: ExportOptions) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| Error::External(format!("Caminho de exportação inválido: {}", path.display())))?;
        let temp = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
        let file = File::create(&temp)
            .map_err(|e| Error::External(format!("Não foi possível criar {}: {e}", path.display())))?;
        let writer = ExportWriter::new(BufWriter::new(file), columns, options);
        match writer {
            Ok(writer) => Ok(Self {
                writer: Some(writer),
                temp,
                path,
            }),
            Err(error) => {
                let _ = std::fs::remove_file(&temp);
                Err(error)
            }
        }
    }

    pub fn write_row(&mut self, row: &[Option<&str>]) -> Result<()> {
        self.writer_mut()?.write_row(row)
    }

    pub fn write_batch(&mut self, batch: &RowBatch) -> Result<()> {
        self.writer_mut()?.write_batch(batch)
    }

    /// Termina o arquivo e dá o nome final (substitui um que já exista).
    /// Devolve quantas linhas foram gravadas.
    pub fn commit(mut self) -> Result<u64> {
        let writer = self.writer.take().ok_or_else(|| Error::InvalidState("exportação já encerrada".into()))?;
        let rows = writer.rows_written();
        let file = writer
            .finish()?
            .into_inner()
            .map_err(|e| io_error(e.into_error()))?;
        file.sync_all().map_err(io_error)?;
        drop(file);
        std::fs::rename(&self.temp, &self.path)
            .map_err(|e| Error::External(format!("Não foi possível gravar {}: {e}", self.path.display())))?;
        Ok(rows)
    }

    fn writer_mut(&mut self) -> Result<&mut ExportWriter<BufWriter<File>>> {
        self.writer.as_mut().ok_or_else(|| Error::InvalidState("exportação já encerrada".into()))
    }
}

impl Drop for FileExport {
    fn drop(&mut self) {
        if self.writer.take().is_some() {
            let _ = std::fs::remove_file(&self.temp);
        }
    }
}

fn io_error(error: std::io::Error) -> Error {
    Error::External(format!("Falha ao gravar a exportação: {error}"))
}

/// Nome repetido sai uma vez só, na posição do primeiro e com o valor do
/// último — como o `Object.fromEntries` do front.
fn dedupe_by_name(columns: Vec<ExportColumn>) -> Vec<ExportColumn> {
    let mut out: Vec<ExportColumn> = Vec::with_capacity(columns.len());
    for column in columns {
        match out.iter_mut().find(|seen| seen.name == column.name) {
            Some(seen) => seen.index = column.index,
            None => out.push(column),
        }
    }
    out
}

/// RFC 4180: aspas dobram dentro do campo e só se aspeia o que precisa. NULL
/// sai como campo vazio e a string vazia como `""`, para os dois não se
/// confundirem.
fn csv_field(value: Option<&str>, delimiter: &str) -> String {
    match value {
        None => String::new(),
        Some(value)
            if value.is_empty()
                || (!delimiter.is_empty() && value.contains(delimiter))
                || value.contains(['"', '\n', '\r']) =>
        {
            format!("\"{}\"", value.replace('"', "\"\""))
        }
        Some(value) => value.to_string(),
    }
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

/// O Postgres devolve booleano como "t"/"f", que nenhum INSERT aceita de volta.
fn sql_literal(value: Option<&str>, kind: SqlValue) -> String {
    let Some(value) = value else {
        return "NULL".into();
    };
    match kind {
        SqlValue::Boolean => if matches!(value, "true" | "t" | "1") { "true" } else { "false" }.into(),
        SqlValue::Number if is_numeric_literal(value) => value.to_string(),
        _ => format!("'{}'", value.replace('\'', "''")),
    }
}

/// `-?\d+(\.\d+)?([eE][+-]?\d+)?` — o literal que pode ir cru no INSERT.
fn is_numeric_literal(value: &str) -> bool {
    fn digits(bytes: &[u8], mut at: usize) -> usize {
        while at < bytes.len() && bytes[at].is_ascii_digit() {
            at += 1;
        }
        at
    }
    let bytes = value.as_bytes();
    let mut at = usize::from(bytes.first() == Some(&b'-'));
    let end = digits(bytes, at);
    if end == at {
        return false;
    }
    at = end;
    if bytes.get(at) == Some(&b'.') {
        let end = digits(bytes, at + 1);
        if end == at + 1 {
            return false;
        }
        at = end;
    }
    if matches!(bytes.get(at), Some(b'e' | b'E')) {
        at += 1;
        if matches!(bytes.get(at), Some(b'+' | b'-')) {
            at += 1;
        }
        let end = digits(bytes, at);
        if end == at {
            return false;
        }
        at = end;
    }
    at == bytes.len()
}

pub fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns() -> Vec<ExportColumn> {
        [("id", "int4", 0), ("name", "text", 1), ("created_at", "timestamptz", 2)]
            .into_iter()
            .map(|(name, type_name, index)| ExportColumn {
                name: name.into(),
                type_name: type_name.into(),
                index,
            })
            .collect()
    }

    fn export(columns: Vec<ExportColumn>, rows: &[Vec<Option<&str>>], options: ExportOptions) -> String {
        let mut writer = ExportWriter::new(Vec::new(), columns, options).unwrap();
        for row in rows {
            writer.write_row(row).unwrap();
        }
        String::from_utf8(writer.finish().unwrap()).unwrap()
    }

    fn sql(defaults: &[&str]) -> ExportOptions {
        ExportOptions {
            format: ExportFormat::Sql,
            sql_table: "\"public\".\"users\"".into(),
            sql_default_columns: defaults.iter().map(|name| name.to_string()).collect(),
            ..ExportOptions::default()
        }
    }

    fn row<'a>(cells: &[Option<&'a str>]) -> Vec<Option<&'a str>> {
        cells.to_vec()
    }

    // Os três primeiros espelham o serialize.test.ts do front

    #[test]
    fn sql_includes_every_column_by_default() {
        let rows = [row(&[Some("1"), Some("O'Neil"), Some("2026-01-01 00:00:00+00")])];
        assert_eq!(
            export(columns(), &rows, sql(&[])),
            "INSERT INTO \"public\".\"users\" (\"id\", \"name\", \"created_at\") VALUES (1, 'O''Neil', '2026-01-01 00:00:00+00');"
        );
    }

    #[test]
    fn sql_leaves_default_columns_to_the_database() {
        let rows = [row(&[Some("1"), Some("O'Neil"), Some("2026-01-01 00:00:00+00")])];
        assert_eq!(
            export(columns(), &rows, sql(&["id", "created_at"])),
            "INSERT INTO \"public\".\"users\" (\"name\") VALUES ('O''Neil');"
        );
        assert_eq!(
            export(columns()[..1].to_vec(), &rows, sql(&["id"])),
            "INSERT INTO \"public\".\"users\" DEFAULT VALUES;"
        );
    }

    #[test]
    fn sql_literals_by_type() {
        let columns = vec![
            ExportColumn { name: "ok".into(), type_name: "bool".into(), index: 0 },
            ExportColumn { name: "n".into(), type_name: "numeric".into(), index: 1 },
            ExportColumn { name: "x".into(), type_name: "text".into(), index: 2 },
        ];
        let rows = [row(&[Some("t"), Some("-1.5e3"), None]), row(&[Some("f"), Some("NaN"), Some("1")])];
        assert_eq!(
            export(columns, &rows, sql(&[])),
            "INSERT INTO \"public\".\"users\" (\"ok\", \"n\", \"x\") VALUES (true, -1.5e3, NULL);\n\
             INSERT INTO \"public\".\"users\" (\"ok\", \"n\", \"x\") VALUES (false, 'NaN', '1');"
        );
        assert!(is_numeric_literal("10") && is_numeric_literal("-0.25") && is_numeric_literal("1E+9"));
        assert!(!is_numeric_literal("1.") && !is_numeric_literal(".5") && !is_numeric_literal("1e") && !is_numeric_literal("-"));
    }

    #[test]
    fn csv_quotes_only_what_needs_it_and_keeps_null_apart_from_empty() {
        let rows = [
            row(&[Some("1"), Some("a,b"), None]),
            row(&[Some("2"), Some(""), Some("diz \"oi\"\nlinha")]),
        ];
        assert_eq!(
            export(columns(), &rows, ExportOptions::default()),
            "id,name,created_at\n1,\"a,b\",\n2,\"\",\"diz \"\"oi\"\"\nlinha\""
        );

        let options = ExportOptions { delimiter: ";".into(), include_header: false, ..ExportOptions::default() };
        assert_eq!(export(columns(), &[row(&[Some("1"), Some("a,b"), Some("x;y")])], options), "1;a,b;\"x;y\"");
        // Sem cabeçalho e sem linhas: vazio
        let options = ExportOptions { include_header: false, ..ExportOptions::default() };
        assert_eq!(export(columns(), &[], options), "");
    }

    #[test]
    fn json_matches_the_pretty_print_of_the_front() {
        let options = ExportOptions { format: ExportFormat::Json, ..ExportOptions::default() };
        let rows = [row(&[Some("1"), Some("ção \"x\""), None])];
        assert_eq!(
            export(columns()[..2].to_vec(), &rows, options.clone()),
            "[\n  {\n    \"id\": \"1\",\n    \"name\": \"ção \\\"x\\\"\"\n  }\n]"
        );
        assert_eq!(export(columns(), &[], options.clone()), "[]");
        assert_eq!(export(vec![], &[row(&[])], options.clone()), "[\n  {}\n]");

        // Nome repetido: uma vez, com o valor da última coluna
        let mut repeated = columns()[..2].to_vec();
        repeated.push(ExportColumn { name: "id".into(), type_name: "int4".into(), index: 2 });
        assert_eq!(
            export(repeated, &[row(&[Some("1"), Some("a"), Some("9")])], options),
            "[\n  {\n    \"id\": \"9\",\n    \"name\": \"a\"\n  }\n]"
        );
    }

    #[test]
    fn hidden_columns_and_short_rows() {
        // Só a coluna 2, e uma linha sem ela: NULL
        let picked = vec![columns().remove(2)];
        let options = ExportOptions { include_header: false, ..ExportOptions::default() };
        assert_eq!(export(picked, &[row(&[Some("1"), Some("x"), Some("d")]), row(&[Some("2")])], options), "d\n");
    }

    #[test]
    fn a_file_only_appears_on_commit() {
        let dir = std::env::temp_dir().join(format!("octapus_export_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("saida.csv");
        let _ = std::fs::remove_file(&path);

        let mut export = FileExport::create(&path, columns(), ExportOptions::default()).unwrap();
        let mut acc = crate::adapters::BatchAccumulator::new(Vec::new());
        let _ = acc.set_columns(vec![crate::adapters::column("id", "int4"), crate::adapters::column("name", "text")]);
        acc.push_row([Some("1"), Some("Ana")]);
        acc.push_row([Some("2"), None]);
        export.write_batch(&acc.take().unwrap()).unwrap();
        assert!(!path.exists(), "o arquivo final só nasce no commit");
        assert_eq!(export.commit().unwrap(), 2);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "id,name,created_at\n1,Ana,\n2,,");

        // Largada sem commit: nada fica para trás
        let other = dir.join("abortada.json");
        let options = ExportOptions { format: ExportFormat::Json, ..ExportOptions::default() };
        let mut export = FileExport::create(&other, columns(), options).unwrap();
        export.write_row(&[Some("1")]).unwrap();
        drop(export);
        assert!(!other.exists());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "sobrou temporário");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
