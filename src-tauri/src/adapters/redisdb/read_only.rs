//! Execução só-leitura do plano do Redis (`ReadOnlyOp::RedisPlan`), para o
//! assistente.
//!
//! O Redis não tem transação de leitura nem modo só-leitura por conexão; a
//! barreira daqui vale sozinha, sem depender do guard:
//! - **só comandos da allowlist** (`plan.rs`): cada passo é conferido de novo
//!   antes de ir ao servidor, e `SCAN` só fora de `foreach`;
//! - **o app percorre o `SCAN`** (nunca `KEYS`) e para no teto de chaves;
//! - **tetos de itens e de comandos** por execução e **tempo** no plano
//!   inteiro (`ReadOnlyLimits`). Estourar um teto é falha explícita
//!   (`Error::PlanLimit`, RN-21), nunca um resultado cortado calado.
//!
//! Só roda o que leva ao `output`, e só o que a página precisa: um `foreach`
//! que é o próprio `output` roda para os itens da página. O `SCAN` que vai
//! para a tela (ou alimenta o `output`) lê até o teto sem falhar — a grade
//! mostra a página, e o total quando o cursor chegou ao fim; o que alimenta
//! outro `foreach` precisa do keyspace inteiro e falha no teto. As páginas
//! seguintes rodam o plano de novo (C-09).
//!
//! O preflight confere o plano sem executar: parse, allowlist, opções do
//! `SCAN` e, pelo `COMMAND INFO` do servidor, que cada comando existe nesta
//! versão, que o número de argumentos bate e que o servidor também o marca
//! como leitura.

use std::collections::HashSet;
use std::future::Future;
use std::time::Instant;

use redis::aio::ConnectionManager;
use redis::Value;

use crate::adapters::{ReadOnlyLimits, STATEMENT_TIMEOUT_MESSAGE};
use crate::error::{Error, Result};
use crate::models::{QueryColumnInfo, QueryOptions, QueryResult, ReadOnlyPlan};

use super::command::{value_to_result, value_to_string};
use super::metadata::SCAN_CAP;
use super::plan::{self, command_kind, CommandKind, Plan, ScanOptions, Step, ITEM};

/// Chaves que um `SCAN` do plano junta, no máximo.
pub const MAX_SCAN_KEYS: usize = 10_000;
const _: () = assert!(MAX_SCAN_KEYS <= SCAN_CAP);
/// Itens de um `foreach` que não é o `output` (esse roda só a página).
pub const MAX_FOREACH_ITEMS: usize = 1_000;
/// Comandos por execução do plano, somando os de todos os `foreach`.
pub const MAX_COMMANDS: usize = 5_000;
/// Comandos por ida ao servidor (pipeline) num `foreach`.
const PIPELINE_BATCH: usize = 100;
/// `COUNT` do `SCAN` quando o plano não diz, e o teto do que ele diz.
const SCAN_BATCH: u32 = 1_000;
/// Colunas que um `HGETALL` por item abre, no máximo (o resto vira texto).
const MAX_PIVOT_COLUMNS: usize = 50;

/// Os nomes das colunas e as linhas de uma tabela.
type Table = (Vec<String>, Vec<Vec<Option<String>>>);

/// Erro do servidor com o código (`WRONGTYPE`, `ERR`…) para o assistente
/// classificar; o resto (conexão, tempo) segue a conversão de sempre.
pub fn redis_error(error: redis::RedisError) -> Error {
    match error.code() {
        Some(code) => Error::RedisQuery {
            code: code.to_string(),
            message: error.detail().unwrap_or_default().to_string(),
        },
        None => error.into(),
    }
}

/// O parse e a barreira do executor: sem o guard, o que não é leitura para
/// aqui.
fn parse(text: &str) -> Result<Plan> {
    let plan = plan::parse(text).map_err(|error| Error::InvalidQuery(error.to_string()))?;
    for step in &plan.steps {
        if command_kind(&step.command) != CommandKind::Read {
            return Err(Error::GuardRejected(format!(
                "{} is not allowed in a read-only plan",
                step.command
            )));
        }
        if step.command == "SCAN" && step.foreach.is_some() {
            return Err(Error::GuardRejected("SCAN cannot run inside a foreach".into()));
        }
    }
    Ok(plan)
}

/// Roda `work` com o teto de tempo do plano inteiro.
async fn within<T>(limits: &ReadOnlyLimits, work: impl Future<Output = Result<T>>) -> Result<T> {
    match tokio::time::timeout(limits.statement_timeout, work).await {
        Ok(result) => result,
        Err(_) => Err(Error::Query(STATEMENT_TIMEOUT_MESSAGE.into())),
    }
}

pub async fn execute(
    conn: &mut ConnectionManager,
    text: &str,
    page: &QueryOptions,
    limits: &ReadOnlyLimits,
) -> Result<QueryResult> {
    let plan = parse(text)?;
    let started = Instant::now();
    let (limit, offset) = limits.page(page.limit, page.offset);
    let window = Window {
        offset: offset as usize,
        limit: limit as usize,
    };
    let mut result = within(limits, run(conn, &plan, window)).await?;
    result.execution_time_ms = started.elapsed().as_millis() as u64;
    Ok(result)
}

/// A página pedida, sobre as linhas do `output`.
#[derive(Debug, Clone, Copy)]
struct Window {
    offset: usize,
    limit: usize,
}

impl Window {
    /// Linhas lidas até o fim da página, com uma a mais para o `has_more`.
    fn end(&self) -> usize {
        self.offset + self.limit + 1
    }
}

/// Quanto do resultado de um passo o plano precisa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Demand {
    /// Não leva ao `output`: não roda
    None,
    /// Vai para a tela (o `output` ou a fonte dele): a página sai do que
    /// couber no teto
    Page,
    /// Alimenta outro `foreach`: tudo, ou falha no teto
    All,
}

/// Do `output` para trás: a fonte do `foreach` que é o `output` serve à
/// página; a de qualquer outro `foreach`, ao passo inteiro.
fn demands(plan: &Plan) -> Vec<Demand> {
    let mut demands = vec![Demand::None; plan.steps.len()];
    demands[plan.output] = Demand::Page;
    for index in (0..plan.steps.len()).rev() {
        let (Some(source), demand) = (plan.steps[index].foreach, demands[index]) else {
            continue;
        };
        let needed = match demand {
            Demand::None => continue,
            _ if index == plan.output => Demand::Page,
            _ => Demand::All,
        };
        demands[source] = demands[source].max(needed);
    }
    demands
}

/// O resultado de um passo.
#[derive(Debug, Clone, PartialEq)]
enum StepOutput {
    /// Não rodou (não leva ao `output`)
    Skipped,
    Reply(Value),
    /// `SCAN`: as chaves, sem repetir; `complete` = o cursor chegou ao fim
    Keys { keys: Vec<String>, complete: bool },
    /// `foreach`: cada item e a resposta dele. No `output`, só os da página
    /// (a partir de `first`), e `total` = itens da fonte, se ela está inteira
    PerItem {
        first: usize,
        total: Option<usize>,
        rows: Vec<(String, Value)>,
    },
}

async fn run(conn: &mut ConnectionManager, plan: &Plan, window: Window) -> Result<QueryResult> {
    let demands = demands(plan);
    let mut outputs: Vec<StepOutput> = Vec::with_capacity(plan.steps.len());
    let mut commands = 0usize;

    for (index, step) in plan.steps.iter().enumerate() {
        let output = match (demands[index], step.foreach) {
            (Demand::None, _) => StepOutput::Skipped,
            (demand, None) if step.command == "SCAN" => scan(conn, step, demand).await?,
            (_, None) => {
                commands += 1;
                let value = command(step, None).query_async::<Value>(conn).await.map_err(redis_error)?;
                StepOutput::Reply(value.extract_error().map_err(redis_error)?)
            }
            (_, Some(source)) => {
                let items = items_of(&outputs[source], &plan.steps[source]);
                let total = complete(&outputs[source]).then_some(items.len());
                // O `output` roda só a página; o resto, todos os itens
                let (first, items) = match index == plan.output {
                    true => {
                        if total.is_none() {
                            beyond_the_cap(window, items.len())?;
                        }
                        let from = window.offset.min(items.len());
                        let to = window.end().min(items.len());
                        (from, items[from..to].to_vec())
                    }
                    _ if items.len() > MAX_FOREACH_ITEMS => {
                        return Err(Error::PlanLimit(format!(
                            "step \"{}\" iterates over more than {MAX_FOREACH_ITEMS} items; \
                             a Redis plan cannot aggregate that many on the server",
                            step.id
                        )))
                    }
                    _ => (0, items),
                };
                commands += items.len();
                if commands > MAX_COMMANDS {
                    return Err(Error::PlanLimit(format!(
                        "the plan needs more than {MAX_COMMANDS} commands"
                    )));
                }
                let rows = foreach(conn, step, items).await?;
                StepOutput::PerItem { first, total, rows }
            }
        };
        if let (true, StepOutput::Keys { keys, complete: false }) = (index == plan.output, &output) {
            beyond_the_cap(window, keys.len())?;
        }
        outputs.push(output);
    }

    let output = std::mem::replace(&mut outputs[plan.output], StepOutput::Skipped);
    let source = plan.steps[plan.output].foreach.map(|source| &plan.steps[source]);
    Ok(render(&plan.steps[plan.output], source, output, window))
}

/// O comando do passo, com o item do `foreach` no lugar de `$item`.
fn command(step: &Step, item: Option<&str>) -> redis::Cmd {
    let mut cmd = redis::cmd(&step.command);
    for arg in &step.args {
        match item {
            Some(item) if arg.contains(ITEM) => cmd.arg(arg.replace(ITEM, item)),
            _ => cmd.arg(arg),
        };
    }
    cmd
}

/// Percorre o cursor até o fim ou até o teto. Para a tela, o teto só corta
/// (a grade mostra que há mais); para outro `foreach`, passar do teto é
/// falha: o resultado seria parcial sem ninguém saber.
async fn scan(conn: &mut ConnectionManager, step: &Step, demand: Demand) -> Result<StepOutput> {
    let options = ScanOptions::parse(&step.args).map_err(Error::InvalidQuery)?;
    let wanted = MAX_SCAN_KEYS + 1;

    let mut seen: HashSet<String> = HashSet::new();
    let mut keys: Vec<String> = Vec::new();
    let mut cursor: u64 = 0;
    loop {
        let mut cmd = redis::cmd("SCAN");
        cmd.arg(cursor);
        if let Some(pattern) = &options.pattern {
            cmd.arg("MATCH").arg(pattern);
        }
        cmd.arg("COUNT").arg(options.count.unwrap_or(SCAN_BATCH).clamp(10, SCAN_BATCH));
        if let Some(type_name) = &options.type_name {
            cmd.arg("TYPE").arg(type_name);
        }
        let (next, batch): (u64, Vec<String>) = cmd.query_async(conn).await.map_err(redis_error)?;
        // O SCAN pode devolver a mesma chave mais de uma vez
        for key in batch {
            if seen.insert(key.clone()) {
                keys.push(key);
            }
        }
        cursor = next;
        if cursor == 0 || keys.len() >= wanted {
            break;
        }
    }

    let complete = cursor == 0 && keys.len() <= MAX_SCAN_KEYS;
    if !complete && demand == Demand::All {
        return Err(Error::PlanLimit(format!(
            "step \"{}\" matches more than {MAX_SCAN_KEYS} keys; a Redis plan cannot read the whole keyspace",
            step.id
        )));
    }
    keys.truncate(MAX_SCAN_KEYS);
    Ok(StepOutput::Keys { complete, keys })
}

/// A página de uma lista que o teto cortou: dá enquanto há o que mostrar;
/// depois do que foi lido é falha explícita, não uma lista que parece inteira
/// (RN-21).
fn beyond_the_cap(window: Window, read: usize) -> Result<()> {
    if window.offset >= read {
        return Err(Error::PlanLimit(format!(
            "the plan reads only the first {MAX_SCAN_KEYS} keys that SCAN matches; more exist"
        )));
    }
    Ok(())
}

/// A fonte inteira: o `SCAN` chegou ao fim do cursor (uma resposta ou um
/// `foreach` que rodou estão sempre inteiros).
fn complete(output: &StepOutput) -> bool {
    !matches!(output, StepOutput::Keys { complete: false, .. })
}

/// Um comando por item, em pipelines de [`PIPELINE_BATCH`].
async fn foreach(conn: &mut ConnectionManager, step: &Step, items: Vec<String>) -> Result<Vec<(String, Value)>> {
    let mut rows = Vec::with_capacity(items.len());
    for batch in items.chunks(PIPELINE_BATCH) {
        let mut pipe = redis::pipe();
        for item in batch {
            pipe.add_command(command(step, Some(item)));
        }
        let values: Vec<Value> = pipe.query_async(conn).await.map_err(redis_error)?;
        for (item, value) in batch.iter().zip(values) {
            rows.push((item.clone(), value.extract_error().map_err(redis_error)?));
        }
    }
    Ok(rows)
}

/// O resultado de um passo como lista de itens, para o `foreach` seguinte:
/// as chaves do `SCAN`; os elementos de uma lista (o primeiro de cada par numa
/// resposta em pares, o primeiro de cada elemento aninhado); as chaves de um
/// mapa; e, de um `foreach`, todos os itens das respostas, em ordem.
fn items_of(output: &StepOutput, step: &Step) -> Vec<String> {
    match output {
        StepOutput::Skipped => Vec::new(),
        StepOutput::Keys { keys, .. } => keys.clone(),
        StepOutput::Reply(value) => flatten(value, step.pairwise()),
        StepOutput::PerItem { rows, .. } => rows.iter().flat_map(|(_, value)| flatten(value, step.pairwise())).collect(),
    }
}

fn flatten(value: &Value, pairwise: bool) -> Vec<String> {
    match value {
        Value::Nil => Vec::new(),
        Value::Array(items) | Value::Set(items) => items
            .iter()
            .step_by(if pairwise { 2 } else { 1 })
            .filter_map(|item| match item {
                Value::Array(inner) => inner.first().and_then(value_to_string),
                other => value_to_string(other),
            })
            .collect(),
        Value::Map(pairs) => pairs.iter().filter_map(|(key, _)| value_to_string(key)).collect(),
        scalar => value_to_string(scalar).into_iter().collect(),
    }
}

fn columns(names: &[String]) -> Vec<QueryColumnInfo> {
    names
        .iter()
        .map(|name| QueryColumnInfo {
            name: name.clone(),
            type_name: "string".into(),
            type_oid: None,
        })
        .collect()
}

/// A tabela da página. O resultado é só leitura (RN-16): sem `editable_info`.
/// O total vai quando se sabe (a resposta inteira, o `SCAN` até o fim): é o
/// "quantos" das perguntas de contagem.
fn render(step: &Step, source: Option<&Step>, output: StepOutput, window: Window) -> QueryResult {
    let (names, rows, skipped, total) = match output {
        StepOutput::Skipped => (vec!["value".to_string()], Vec::new(), 0, None),
        StepOutput::Keys { keys, complete } => {
            let total = complete.then_some(keys.len());
            let rows = keys.into_iter().map(|key| vec![Some(key)]).collect();
            (vec!["key".to_string()], rows, 0, total)
        }
        StepOutput::Reply(value) => {
            let (names, rows) = reply_table(step, value);
            let total = Some(rows.len());
            (names, rows, 0, total)
        }
        StepOutput::PerItem { first, total, rows } => {
            let item = if source.is_some_and(|source| source.command == "SCAN") { "key" } else { "item" };
            let (names, rows) = per_item_table(step, item, rows);
            (names, rows, first, total)
        }
    };

    // As linhas do `foreach` do `output` já começam na página
    let from = window.offset.saturating_sub(skipped).min(rows.len());
    // Sem total, a lista foi cortada no teto: sempre há mais (a página
    // depois do que foi lido falha em `beyond_the_cap`)
    let has_more = match total {
        Some(total) => total > window.offset + window.limit,
        None => true,
    };
    let page: Vec<Vec<Option<String>>> = rows.into_iter().skip(from).take(window.limit).collect();
    QueryResult {
        columns: columns(&names),
        row_count: page.len(),
        rows: page,
        total_count: total.map(|total| total as i64),
        has_more,
        execution_time_ms: 0,
        editable_info: None,
    }
}

/// Um comando só: resposta em pares vira duas colunas (`member`/`score` ou
/// `field`/`value`); o resto, como no editor.
fn reply_table(step: &Step, value: Value) -> Table {
    if let (true, Value::Array(items)) = (step.pairwise(), &value) {
        if items.len() % 2 == 0 {
            let scores = step.args.iter().any(|arg| arg.eq_ignore_ascii_case("WITHSCORES"));
            let names = if scores { ["member", "score"] } else { ["field", "value"] };
            let rows = items
                .chunks(2)
                .map(|pair| vec![value_to_string(&pair[0]), value_to_string(&pair[1])])
                .collect();
            return (names.map(String::from).to_vec(), rows);
        }
    }
    let result = value_to_result(&step.command, value);
    (result.columns.into_iter().map(|column| column.name).collect(), result.rows)
}

/// Um `foreach`: uma linha por item. `HGET` e `HMGET` dão nome às colunas
/// pelos campos pedidos; `HGETALL` abre uma coluna por campo; o resto vai numa
/// coluna com o nome do comando.
fn per_item_table(step: &Step, item: &str, rows: Vec<(String, Value)>) -> Table {
    let fields: Vec<String> = step.args.iter().skip(1).cloned().collect();
    let named_fields = !fields.is_empty() && fields.iter().all(|field| !field.contains(ITEM));

    match step.command.as_str() {
        "HGET" if named_fields && fields.len() == 1 => {
            let names = vec![item.to_string(), fields[0].clone()];
            let rows = rows.into_iter().map(|(item, value)| vec![Some(item), value_to_string(&value)]).collect();
            return (names, rows);
        }
        "HMGET" if named_fields => {
            let mut names = vec![item.to_string()];
            names.extend(fields.iter().cloned());
            let rows = rows
                .into_iter()
                .map(|(item, value)| {
                    let mut row = vec![Some(item)];
                    match value {
                        Value::Array(values) => row.extend(values.iter().map(value_to_string)),
                        other => row.push(value_to_string(&other)),
                    }
                    row.resize(names.len(), None);
                    row
                })
                .collect();
            return (names, rows);
        }
        "HGETALL" => {
            if let Some(table) = pivot(item, &rows) {
                return table;
            }
        }
        _ => {}
    }

    let names = vec![item.to_string(), step.command.to_lowercase()];
    let rows = rows.into_iter().map(|(item, value)| vec![Some(item), value_to_string(&value)]).collect();
    (names, rows)
}

/// `HGETALL` por item: uma coluna por campo, na ordem em que aparecem.
/// `None` se passar de [`MAX_PIVOT_COLUMNS`] campos (é mapa, não registro).
fn pivot(item: &str, rows: &[(String, Value)]) -> Option<Table> {
    let pairs = |value: &Value| -> Vec<(Option<String>, Option<String>)> {
        match value {
            Value::Array(items) => items
                .chunks(2)
                .map(|pair| (value_to_string(&pair[0]), pair.get(1).and_then(value_to_string)))
                .collect(),
            Value::Map(entries) => entries.iter().map(|(k, v)| (value_to_string(k), value_to_string(v))).collect(),
            _ => Vec::new(),
        }
    };
    let mut fields: Vec<String> = Vec::new();
    for (_, value) in rows {
        for (field, _) in pairs(value) {
            let field = field.unwrap_or_default();
            if !fields.contains(&field) {
                if fields.len() == MAX_PIVOT_COLUMNS {
                    return None;
                }
                fields.push(field);
            }
        }
    }
    let table = rows
        .iter()
        .map(|(key, value)| {
            let mut row = vec![None; fields.len() + 1];
            row[0] = Some(key.clone());
            for (field, cell) in pairs(value) {
                let position = fields.iter().position(|seen| *seen == field.clone().unwrap_or_default());
                if let Some(position) = position {
                    row[position + 1] = cell;
                }
            }
            row
        })
        .collect();
    let mut names = vec![item.to_string()];
    names.extend(fields);
    Some((names, table))
}

// ─────────────────────────────────────────────────────────────────────────────
// Preflight
// ─────────────────────────────────────────────────────────────────────────────

pub async fn explain(conn: &mut ConnectionManager, text: &str, limits: &ReadOnlyLimits) -> Result<ReadOnlyPlan> {
    let plan = parse(text)?;
    for step in plan.steps.iter().filter(|step| step.command == "SCAN") {
        ScanOptions::parse(&step.args).map_err(Error::InvalidQuery)?;
    }
    within(limits, check_with_server(conn, &plan)).await?;
    Ok(ReadOnlyPlan {
        estimated_rows: None,
        estimated_cost: None,
        plan: summary(&plan),
    })
}

/// `COMMAND INFO` de cada comando: existe nesta versão do servidor, o número
/// de argumentos bate (a aridade do Redis: exata, ou mínima quando negativa)
/// e o servidor não o marca como escrita. Servidor que esconde o `COMMAND`
/// (alguns gerenciados) pula a conferência: o guard e o executor bastam.
async fn check_with_server(conn: &mut ConnectionManager, plan: &Plan) -> Result<()> {
    let mut names: Vec<&str> = plan.steps.iter().map(|step| step.command.as_str()).collect();
    names.sort_unstable();
    names.dedup();

    let mut cmd = redis::cmd("COMMAND");
    cmd.arg("INFO");
    for name in &names {
        cmd.arg(*name);
    }
    let info: Value = match cmd.query_async(conn).await {
        Ok(info) => info,
        Err(error) if error.code().is_some() => {
            log::info!("assistente: COMMAND INFO indisponível no Redis, preflight só local");
            return Ok(());
        }
        Err(error) => return Err(redis_error(error)),
    };
    let Value::Array(entries) = info else {
        return Ok(());
    };

    for (name, entry) in names.iter().zip(entries) {
        let Value::Array(fields) = entry else {
            return Err(Error::RedisQuery {
                code: "ERR".into(),
                message: format!("unknown command '{name}' in this Redis version"),
            });
        };
        let arity = match fields.get(1) {
            Some(Value::Int(arity)) => *arity,
            _ => continue,
        };
        let flags = match fields.get(2) {
            Some(Value::Array(flags) | Value::Set(flags)) => flags.iter().filter_map(value_to_string).collect(),
            _ => Vec::new(),
        };
        if flags.iter().any(|flag| flag == "write" || flag == "admin" || flag == "blocking") {
            return Err(Error::GuardRejected(format!("the server marks {name} as a write or admin command")));
        }
        for step in plan.steps.iter().filter(|step| step.command == *name) {
            let sent = if step.command == "SCAN" { sent_scan_args(step) } else { step.args.len() } + 1;
            let fits = if arity >= 0 { sent as i64 == arity } else { sent as i64 >= -arity };
            if !fits {
                return Err(Error::RedisQuery {
                    code: "ERR".into(),
                    message: format!("wrong number of arguments for '{}' command", name.to_lowercase()),
                });
            }
        }
    }
    Ok(())
}

/// O `SCAN` sai com o cursor do app na frente.
fn sent_scan_args(step: &Step) -> usize {
    let has_cursor = step.args.first().is_some_and(|first| first.chars().all(|c| c.is_ascii_digit()));
    step.args.len() + usize::from(!has_cursor)
}

/// O plano como texto, só com os comandos (nada de argumento): `SCAN →
/// foreach HGET`.
fn summary(plan: &Plan) -> String {
    plan.steps
        .iter()
        .enumerate()
        .filter(|(index, _)| *index <= plan.output)
        .map(|(_, step)| match step.foreach {
            Some(_) => format!("foreach {}", step.command),
            None => step.command.clone(),
        })
        .collect::<Vec<_>>()
        .join(" → ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bulk(text: &str) -> Value {
        Value::BulkString(text.as_bytes().to_vec())
    }

    fn plan(text: &str) -> Plan {
        super::parse(text).unwrap()
    }

    const SCAN_THEN_HGET: &str = r#"{"steps":[
        {"id":"users","command":"SCAN","args":["0","MATCH","user:*"],"foreach":null},
        {"id":"names","command":"HGET","args":["$item","name"],"foreach":"users"}
    ],"output":"names"}"#;

    #[test]
    fn only_what_leads_to_the_output_runs() {
        assert_eq!(demands(&plan(SCAN_THEN_HGET)), [Demand::Page, Demand::Page]);

        // foreach de foreach: o do meio roda inteiro (os itens dele são as
        // respostas achatadas), então a fonte dele também; o passo morto não roda
        let chained = plan(
            r#"{"steps":[
                {"id":"users","command":"SCAN","args":["MATCH","user:*"],"foreach":null},
                {"id":"carts","command":"SMEMBERS","args":["$item:cart"],"foreach":"users"},
                {"id":"prices","command":"HGET","args":["$item","price"],"foreach":"carts"},
                {"id":"unused","command":"DBSIZE","args":[],"foreach":null}
            ],"output":"prices"}"#,
        );
        assert_eq!(demands(&chained), [Demand::All, Demand::Page, Demand::Page, Demand::None]);
    }

    #[test]
    fn the_executor_refuses_without_the_guard() {
        for text in [
            r#"{"steps":[{"id":"a","command":"KEYS","args":["*"],"foreach":null}],"output":"a"}"#,
            r#"{"steps":[{"id":"a","command":"DEL","args":["k"],"foreach":null}],"output":"a"}"#,
            r#"{"steps":[{"id":"a","command":"EVAL","args":["return 1","0"],"foreach":null}],"output":"a"}"#,
            r#"{"steps":[{"id":"a","command":"GET","args":["k"],"foreach":null},
                {"id":"b","command":"SCAN","args":["MATCH","$item:*"],"foreach":"a"}],"output":"b"}"#,
        ] {
            assert_eq!(super::parse(text).unwrap_err().code(), "GUARD_REJECTED", "{text}");
        }
        assert_eq!(super::parse("{").unwrap_err().code(), "INVALID_QUERY");
    }

    #[test]
    fn items_of_each_reply_shape() {
        let step = |command: &str, args: &[&str]| Step {
            id: "a".into(),
            command: command.into(),
            args: args.iter().map(|arg| arg.to_string()).collect(),
            foreach: None,
        };
        let list = Value::Array(vec![bulk("a"), bulk("b")]);
        assert_eq!(items_of(&StepOutput::Reply(list), &step("SMEMBERS", &["k"])), ["a", "b"]);
        let scored = Value::Array(vec![bulk("ana"), bulk("10"), bulk("bia"), bulk("7")]);
        assert_eq!(items_of(&StepOutput::Reply(scored), &step("ZRANGE", &["k", "0", "-1", "WITHSCORES"])), ["ana", "bia"]);
        let entries = Value::Array(vec![Value::Array(vec![bulk("1-0"), Value::Array(vec![bulk("f"), bulk("v")])])]);
        assert_eq!(items_of(&StepOutput::Reply(entries), &step("XRANGE", &["s", "-", "+"])), ["1-0"]);
        assert_eq!(items_of(&StepOutput::Reply(bulk("one")), &step("GET", &["k"])), ["one"]);
        assert!(items_of(&StepOutput::Reply(Value::Nil), &step("GET", &["k"])).is_empty());
        let per_item = StepOutput::PerItem {
            first: 0,
            total: None,
            rows: vec![("u1".into(), Value::Array(vec![bulk("p1")])), ("u2".into(), Value::Array(vec![bulk("p2"), bulk("p3")]))],
        };
        assert_eq!(items_of(&per_item, &step("SMEMBERS", &["$item"])), ["p1", "p2", "p3"]);
    }

    #[test]
    fn foreach_rows_take_their_names_from_the_fields() {
        let hget = &plan(SCAN_THEN_HGET).steps[1];
        let (names, rows) = per_item_table(hget, "key", vec![("user:1".into(), bulk("Ana")), ("user:2".into(), Value::Nil)]);
        assert_eq!(names, ["key", "name"]);
        assert_eq!(rows, [vec![Some("user:1".into()), Some("Ana".into())], vec![Some("user:2".into()), None]]);

        let hgetall = Step {
            id: "all".into(),
            command: "HGETALL".into(),
            args: vec!["$item".into()],
            foreach: Some(0),
        };
        let (names, rows) = per_item_table(
            &hgetall,
            "key",
            vec![
                ("user:1".into(), Value::Array(vec![bulk("name"), bulk("Ana"), bulk("age"), bulk("30")])),
                ("user:2".into(), Value::Array(vec![bulk("name"), bulk("Bia"), bulk("city"), bulk("Recife")])),
            ],
        );
        assert_eq!(names, ["key", "name", "age", "city"]);
        assert_eq!(rows[1], [Some("user:2".into()), Some("Bia".into()), None, Some("Recife".into())]);

        let llen = Step {
            id: "len".into(),
            command: "LLEN".into(),
            args: vec!["$item".into()],
            foreach: Some(0),
        };
        let (names, _) = per_item_table(&llen, "item", vec![("q".into(), Value::Int(3))]);
        assert_eq!(names, ["item", "llen"]);
    }

    #[test]
    fn pages_over_the_rows() {
        let step = plan(r#"{"steps":[{"id":"a","command":"SMEMBERS","args":["k"],"foreach":null}],"output":"a"}"#).steps[0].clone();
        let members = Value::Array((0..5).map(|i| bulk(&format!("m{i}"))).collect());
        let page = render(&step, None, StepOutput::Reply(members.clone()), Window { offset: 2, limit: 2 });
        assert_eq!((page.row_count, page.has_more, page.total_count), (2, true, Some(5)));
        assert_eq!(page.rows[0][0].as_deref(), Some("m2"));
        assert!(page.editable_info.is_none());
        let last = render(&step, None, StepOutput::Reply(members), Window { offset: 4, limit: 2 });
        assert_eq!((last.row_count, last.has_more), (1, false));

        // O foreach do output já vem a partir da página
        let hget = plan(SCAN_THEN_HGET).steps[1].clone();
        let rows: Vec<(String, Value)> = (2..5).map(|i| (format!("user:{i}"), bulk("x"))).collect();
        let output = StepOutput::PerItem { first: 2, total: Some(5), rows: rows.clone() };
        let page = render(&hget, None, output, Window { offset: 2, limit: 2 });
        assert_eq!((page.row_count, page.has_more, page.total_count), (2, true, Some(5)));
        assert_eq!(page.rows[0][0].as_deref(), Some("user:2"));
        // Fonte cortada no teto: sem total, e sempre há mais
        let output = StepOutput::PerItem { first: 2, total: None, rows: rows[..2].to_vec() };
        let page = render(&hget, None, output, Window { offset: 2, limit: 2 });
        assert_eq!((page.row_count, page.has_more, page.total_count), (2, true, None));
        assert!(beyond_the_cap(Window { offset: 2, limit: 2 }, 3).is_ok());
        assert_eq!(classify_limit(beyond_the_cap(Window { offset: 3, limit: 2 }, 3)), "QUERY_ERROR");

        let keys = StepOutput::Keys { keys: (0..3).map(|i| format!("k{i}")).collect(), complete: true };
        let page = render(&plan(SCAN_THEN_HGET).steps[0], None, keys, Window { offset: 0, limit: 200 });
        assert_eq!((page.row_count, page.has_more, page.total_count), (3, false, Some(3)));
        assert_eq!(page.columns[0].name, "key");
    }

    fn classify_limit(result: Result<()>) -> &'static str {
        let error = result.unwrap_err();
        assert!(matches!(error, Error::PlanLimit(_)), "{error:?}");
        error.code()
    }

    #[test]
    fn scored_reply_becomes_member_and_score() {
        let step = plan(r#"{"steps":[{"id":"top","command":"ZRANGE","args":["board","0","2","REV","WITHSCORES"],"foreach":null}],"output":"top"}"#).steps[0].clone();
        let (names, rows) = reply_table(&step, Value::Array(vec![bulk("ana"), bulk("10"), bulk("bia"), bulk("7")]));
        assert_eq!(names, ["member", "score"]);
        assert_eq!(rows[1], [Some("bia".into()), Some("7".into())]);
    }

    #[test]
    fn summary_has_only_the_commands() {
        assert_eq!(summary(&plan(SCAN_THEN_HGET)), "SCAN → foreach HGET");
        assert_eq!(sent_scan_args(&plan(SCAN_THEN_HGET).steps[0]), 3);
    }
}
