//! Resultado de query em lotes, para quem consome em streaming (a UI nativa).
//!
//! O [`DatabaseAdapter::execute_query`](super::DatabaseAdapter::execute_query)
//! devolve tudo materializado em `Vec<Vec<Option<String>>>`: 24 bytes de
//! cabeçalho e uma alocação por célula. Aqui as linhas saem em [`RowBatch`]es
//! compactos — o texto de todas as células de um lote num `String` só, um
//! offset `u32` por célula e um bit de nulo — por um canal limitado: se quem
//! consome não acompanha, o produtor espera e a leitura do banco pausa.
//!
//! Mantém o contrato "célula = string ou null". Não cruza o IPC do Tauri,
//! então fica fora de `models/` (sem ts-rs).

use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use crate::models::{EditableInfo, QueryColumnInfo, QueryResult};

/// Um lote fecha com esta quantidade de linhas…
pub const BATCH_MAX_ROWS: usize = 2000;
/// …ou quando a primeira linha dele espera isto (um frame a 60 Hz): o que já
/// chegou aparece logo, mesmo com o banco devolvendo devagar.
pub const BATCH_MAX_WAIT: Duration = Duration::from_millis(16);
/// …ou quando o texto passa disto (linhas com JSON grande).
pub const BATCH_MAX_BYTES: usize = 4 * 1024 * 1024;
/// Lotes que cabem no canal antes de o produtor esperar.
pub const STREAM_CHANNEL_BATCHES: usize = 8;

pub type RowBatchSender = mpsc::Sender<RowBatch>;
pub type RowBatchReceiver = mpsc::Receiver<RowBatch>;

/// O canal limitado de lotes ([`STREAM_CHANNEL_BATCHES`]).
pub fn row_batch_channel() -> (RowBatchSender, RowBatchReceiver) {
    mpsc::channel(STREAM_CHANNEL_BATCHES)
}

/// Um lote de linhas, imutável depois de pronto.
///
/// `columns` vem no primeiro lote e de novo sempre que as colunas mudam (no
/// Mongo, um documento com um campo novo acrescenta uma coluna no fim, ou o
/// tipo de uma coluna que só tinha `null` fica conhecido). Um lote mais
/// estreito que as colunas atuais lê `NULL` nas que faltam.
#[derive(Debug, Default, Clone)]
pub struct RowBatch {
    columns: Option<Vec<QueryColumnInfo>>,
    width: usize,
    rows: usize,
    text: String,
    /// `rows * width + 1` posições: a célula `i` é `text[offsets[i]..offsets[i + 1]]`
    offsets: Vec<u32>,
    /// Um bit por célula: 1 = `NULL`
    nulls: Vec<u64>,
}

/// As partes cruas de um [`RowBatch`], no mesmo layout (para a grade montar o
/// lote dela sem copiar célula a célula).
#[derive(Debug)]
pub struct RowBatchParts {
    pub columns: Option<Vec<QueryColumnInfo>>,
    pub width: usize,
    pub rows: usize,
    pub text: String,
    pub offsets: Vec<u32>,
    pub nulls: Vec<u64>,
}

impl RowBatch {
    /// As colunas, quando este lote as traz (o primeiro, ou quando mudaram).
    pub fn columns(&self) -> Option<&[QueryColumnInfo]> {
        self.columns.as_deref()
    }

    /// Células por linha neste lote.
    pub fn width(&self) -> usize {
        self.width
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    /// A célula, ou `None` se for `NULL` (ou estiver fora do lote).
    pub fn cell(&self, row: usize, column: usize) -> Option<&str> {
        if row >= self.rows || column >= self.width {
            return None;
        }
        let index = row * self.width + column;
        if self.nulls[index / 64] & (1 << (index % 64)) != 0 {
            return None;
        }
        Some(&self.text[self.offsets[index] as usize..self.offsets[index + 1] as usize])
    }

    /// A linha inteira, como o `QueryResult` a teria.
    pub fn row(&self, row: usize) -> Vec<Option<String>> {
        (0..self.width).map(|column| self.cell(row, column).map(str::to_string)).collect()
    }

    /// Bytes ocupados no heap pelas células.
    pub fn heap_bytes(&self) -> usize {
        self.text.capacity() + self.offsets.capacity() * 4 + self.nulls.capacity() * 8
    }

    pub fn into_parts(self) -> RowBatchParts {
        RowBatchParts {
            columns: self.columns,
            width: self.width,
            rows: self.rows,
            text: self.text,
            offsets: self.offsets,
            nulls: self.nulls,
        }
    }
}

/// O que sobra de uma execução em streaming, além das linhas.
#[derive(Debug, Clone, Default)]
pub struct StreamSummary {
    /// As colunas finais (as mesmas do último lote que as trouxe).
    pub columns: Vec<QueryColumnInfo>,
    /// Linhas enviadas, somando todos os lotes.
    pub row_count: usize,
    /// O resultado passava do teto (`QueryOptions::limit`) e foi cortado.
    pub has_more: bool,
    /// Linhas afetadas por um comando que não devolve linhas, quando o banco
    /// informa.
    pub affected_rows: Option<u64>,
    pub execution_time_ms: u64,
    pub editable_info: Option<EditableInfo>,
}

/// Junta linhas num lote e diz quando ele deve sair.
pub(crate) struct BatchAccumulator {
    /// As colunas que o próximo lote leva (as primeiras, ou as que mudaram)
    pending_columns: Option<Vec<QueryColumnInfo>>,
    width: usize,
    current: RowBatch,
    cells: usize,
    /// Quando a primeira linha do lote atual chegou
    started: Option<Instant>,
}

impl BatchAccumulator {
    pub(crate) fn new(columns: Vec<QueryColumnInfo>) -> Self {
        let width = columns.len();
        Self {
            pending_columns: Some(columns),
            width,
            current: Self::empty(width),
            cells: 0,
            started: None,
        }
    }

    fn empty(width: usize) -> RowBatch {
        RowBatch {
            width,
            offsets: vec![0],
            ..RowBatch::default()
        }
    }

    /// Acrescenta uma linha. Células a menos viram `NULL`; a mais, são
    /// ignoradas.
    pub(crate) fn push_row<'a>(&mut self, cells: impl IntoIterator<Item = Option<&'a str>>) {
        let mut cells = cells.into_iter();
        for _ in 0..self.width {
            self.push_cell(cells.next().flatten());
        }
        self.current.rows += 1;
        self.started.get_or_insert_with(Instant::now);
    }

    fn push_cell(&mut self, value: Option<&str>) {
        let index = self.cells;
        if index.is_multiple_of(64) {
            self.current.nulls.push(0);
        }
        match value {
            Some(text) => self.current.text.push_str(text),
            None => {
                if let Some(word) = self.current.nulls.last_mut() {
                    *word |= 1 << (index % 64);
                }
            }
        }
        // Um lote fecha muito antes de 4 GB (BATCH_MAX_BYTES); o teto do u32
        // só seria atingido por uma célula sozinha desse tamanho.
        let end = u32::try_from(self.current.text.len()).unwrap_or(u32::MAX);
        self.current.offsets.push(end);
        self.cells += 1;
    }

    pub(crate) fn has_rows(&self) -> bool {
        self.current.rows > 0
    }

    /// O lote atual já deve sair (linhas, bytes ou tempo)?
    pub(crate) fn is_full(&self) -> bool {
        self.current.rows >= BATCH_MAX_ROWS
            || self.current.text.len() >= BATCH_MAX_BYTES
            || self.started.is_some_and(|started| started.elapsed() >= BATCH_MAX_WAIT)
    }

    /// Quanto falta para o lote atual vencer pelo tempo (`None` = vazio).
    pub(crate) fn time_left(&self) -> Option<Duration> {
        self.started.map(|started| BATCH_MAX_WAIT.saturating_sub(started.elapsed()))
    }

    /// Fecha o lote atual — também vazio, se ainda há colunas para mandar
    /// (o primeiro lote de um resultado sem linhas). `None` = nada a enviar.
    pub(crate) fn take(&mut self) -> Option<RowBatch> {
        if !self.has_rows() && self.pending_columns.is_none() {
            return None;
        }
        let mut batch = std::mem::replace(&mut self.current, Self::empty(self.width));
        batch.columns = self.pending_columns.take();
        batch.text.shrink_to_fit();
        batch.offsets.shrink_to_fit();
        batch.nulls.shrink_to_fit();
        self.cells = 0;
        self.started = None;
        Some(batch)
    }

    /// Troca as colunas (Mongo). Se a largura muda com linhas pendentes, o
    /// lote atual fecha antes, na largura antiga, e é devolvido para enviar.
    pub(crate) fn set_columns(&mut self, columns: Vec<QueryColumnInfo>) -> Option<RowBatch> {
        let flushed = if columns.len() != self.width && self.has_rows() {
            self.take()
        } else {
            None
        };
        if !self.has_rows() {
            self.width = columns.len();
            self.current = Self::empty(self.width);
        }
        self.pending_columns = Some(columns);
        flushed
    }
}

/// Manda um lote; `false` = quem recebia já foi embora (aba fechada).
pub(crate) async fn send(rows: &RowBatchSender, batch: Option<RowBatch>) -> bool {
    match batch {
        Some(batch) => rows.send(batch).await.is_ok(),
        None => true,
    }
}

/// Um resultado já materializado, em lotes de [`BATCH_MAX_ROWS`] (o default
/// da trait e os comandos que não têm cursor). Sempre sai ao menos um lote,
/// com as colunas.
pub(crate) async fn send_whole_result(result: QueryResult, rows: &RowBatchSender) -> crate::Result<StreamSummary> {
    let summary = StreamSummary {
        columns: result.columns.clone(),
        row_count: result.rows.len(),
        has_more: result.has_more,
        affected_rows: None,
        execution_time_ms: result.execution_time_ms,
        editable_info: result.editable_info,
    };
    let mut accumulator = BatchAccumulator::new(result.columns);
    for chunk in result.rows.chunks(BATCH_MAX_ROWS) {
        for row in chunk {
            accumulator.push_row(row.iter().map(Option::as_deref));
        }
        if !send(rows, accumulator.take()).await {
            return Err(crate::Error::Cancelled);
        }
    }
    if !send(rows, accumulator.take()).await {
        return Err(crate::Error::Cancelled);
    }
    Ok(summary)
}

#[cfg(test)]
pub(crate) fn column(name: &str, type_name: &str) -> QueryColumnInfo {
    QueryColumnInfo {
        name: name.into(),
        type_name: type_name.into(),
        type_oid: None,
    }
}

/// Roda uma execução em streaming e junta o que saiu (testes).
#[cfg(test)]
pub(crate) async fn collect<F, Fut>(run: F) -> (crate::Result<StreamSummary>, Vec<RowBatch>)
where
    F: FnOnce(RowBatchSender) -> Fut,
    Fut: std::future::Future<Output = crate::Result<StreamSummary>> + Send + 'static,
{
    let (tx, mut rx) = row_batch_channel();
    let running = tokio::spawn(run(tx));
    let mut batches = Vec::new();
    while let Some(batch) = rx.recv().await {
        batches.push(batch);
    }
    (running.await.expect("a execução entrou em pânico"), batches)
}

/// Todas as linhas dos lotes, em ordem (testes).
#[cfg(test)]
pub(crate) fn all_rows(batches: &[RowBatch]) -> Vec<Vec<Option<String>>> {
    batches.iter().flat_map(|batch| (0..batch.rows()).map(|row| batch.row(row))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_keep_null_empty_and_accents_apart() {
        let mut acc = BatchAccumulator::new(vec![column("a", "text"), column("b", "text")]);
        acc.push_row([Some("1"), None]);
        acc.push_row([Some(""), Some("ção")]);
        // Linha curta completa com NULL; longa é cortada
        acc.push_row([Some("x")]);
        acc.push_row([Some("y"), Some("z"), Some("extra")]);
        let batch = acc.take().unwrap();

        assert_eq!(batch.columns().unwrap().len(), 2);
        assert_eq!((batch.rows(), batch.width()), (4, 2));
        assert_eq!(batch.cell(0, 0), Some("1"));
        assert_eq!(batch.cell(0, 1), None);
        assert_eq!(batch.cell(1, 0), Some(""));
        assert_eq!(batch.cell(1, 1), Some("ção"));
        assert_eq!(batch.row(2), [Some("x".to_string()), None]);
        assert_eq!(batch.row(3), [Some("y".to_string()), Some("z".to_string())]);
        assert_eq!(batch.cell(9, 0), None);
        assert_eq!(batch.cell(0, 9), None);

        // As colunas só vão no primeiro lote; sem linhas, nada a enviar
        assert!(acc.take().is_none());
        acc.push_row([Some("2"), Some("w")]);
        assert!(acc.take().unwrap().columns().is_none());
    }

    #[test]
    fn the_null_bitmap_crosses_64_cells() {
        let mut acc = BatchAccumulator::new(vec![column("a", "int4")]);
        for i in 0..130 {
            acc.push_row([(i % 2 == 0).then_some("v")]);
        }
        let batch = acc.take().unwrap();
        assert_eq!(batch.cell(128, 0), Some("v"));
        assert_eq!(batch.cell(129, 0), None);
        assert_eq!(batch.into_parts().offsets.len(), 131);
    }

    #[test]
    fn an_empty_result_still_sends_its_columns() {
        let mut acc = BatchAccumulator::new(vec![column("a", "int4")]);
        let batch = acc.take().unwrap();
        assert_eq!(batch.rows(), 0);
        assert_eq!(batch.columns().unwrap()[0].name, "a");
        assert!(acc.take().is_none());
    }

    #[test]
    fn a_batch_is_full_by_rows_or_time() {
        let mut acc = BatchAccumulator::new(vec![column("a", "int4")]);
        assert!(!acc.is_full() && acc.time_left().is_none());
        for _ in 0..BATCH_MAX_ROWS - 1 {
            acc.push_row([Some("1")]);
        }
        assert!(acc.time_left().is_some());
        // Pelo tempo pode já ter vencido numa máquina lenta; pelas linhas, ainda não
        assert!(acc.current.rows < BATCH_MAX_ROWS);
        acc.push_row([Some("1")]);
        assert!(acc.is_full());

        let mut slow = BatchAccumulator::new(vec![column("a", "int4")]);
        slow.push_row([Some("1")]);
        std::thread::sleep(BATCH_MAX_WAIT + Duration::from_millis(5));
        assert!(slow.is_full());
    }

    #[test]
    fn widening_flushes_the_pending_rows_at_the_old_width() {
        let mut acc = BatchAccumulator::new(vec![column("_id", "objectId")]);
        acc.push_row([Some("1")]);
        let flushed = acc.set_columns(vec![column("_id", "objectId"), column("nome", "string")]).unwrap();
        assert_eq!((flushed.rows(), flushed.width()), (1, 1));
        assert_eq!(flushed.columns().unwrap().len(), 1);

        acc.push_row([Some("2"), Some("Ana")]);
        let next = acc.take().unwrap();
        assert_eq!(next.columns().unwrap().len(), 2);
        assert_eq!(next.cell(0, 1), Some("Ana"));

        // Mesma largura (só o tipo mudou): nada fecha, as colunas vão no próximo
        acc.push_row([Some("3"), None]);
        assert!(acc.set_columns(vec![column("_id", "objectId"), column("nome", "string")]).is_none());
        let typed = acc.take().unwrap();
        assert_eq!((typed.rows(), typed.columns().map(<[_]>::len)), (1, Some(2)));
    }

    #[tokio::test]
    async fn a_whole_result_goes_in_chunks_with_the_columns_first() {
        let result = QueryResult {
            columns: vec![column("n", "int4")],
            rows: (0..BATCH_MAX_ROWS + 1).map(|i| vec![Some(i.to_string())]).collect(),
            row_count: BATCH_MAX_ROWS + 1,
            total_count: None,
            has_more: true,
            execution_time_ms: 3,
            editable_info: None,
        };
        let (tx, mut rx) = row_batch_channel();
        let sending = tokio::spawn(async move { send_whole_result(result, &tx).await });
        let mut batches = Vec::new();
        while let Some(batch) = rx.recv().await {
            batches.push(batch);
        }
        let summary = sending.await.unwrap().unwrap();

        assert_eq!(batches.len(), 2);
        assert!(batches[0].columns().is_some() && batches[1].columns().is_none());
        assert_eq!(batches[0].rows() + batches[1].rows(), BATCH_MAX_ROWS + 1);
        assert_eq!(batches[1].cell(0, 0), Some(BATCH_MAX_ROWS.to_string().as_str()));
        assert_eq!((summary.row_count, summary.has_more), (BATCH_MAX_ROWS + 1, true));

        // Sem ninguém ouvindo, vira cancelamento
        let (tx, rx) = row_batch_channel();
        drop(rx);
        let empty = QueryResult {
            rows: vec![],
            ..sending_result()
        };
        assert!(matches!(send_whole_result(empty, &tx).await, Err(crate::Error::Cancelled)));
    }

    fn sending_result() -> QueryResult {
        QueryResult {
            columns: vec![column("n", "int4")],
            rows: vec![],
            row_count: 0,
            total_count: None,
            has_more: false,
            execution_time_ms: 0,
            editable_info: None,
        }
    }
}
