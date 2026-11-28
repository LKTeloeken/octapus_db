//! A carga dos bancos do BIRD no Redis de teste (`./experiment/x redis-load`),
//! a partir do PostgreSQL do experimento, para que os três paradigmas tenham
//! os mesmos dados e a mesma pergunta tenha a mesma resposta nos três.
//!
//! É preparação de dados, como o `mongorestore` do `db.sh`: roda à parte, só
//! nos bancos de teste do `ambiente.toml`, e nunca durante uma rodada (a
//! rodada continua só lendo). Cada conjunto vai para o banco numerado que o
//! `[database.redis] datasets` dá a ele, que é esvaziado antes da carga.
//!
//! O modelo das chaves, para cada tabela `t` do conjunto:
//!
//! | chave                  | tipo | o que guarda                                        |
//! |------------------------|------|-----------------------------------------------------|
//! | `t:<id>`               | hash | a linha: um campo por coluna não nula               |
//! | `t:ids`                | set  | os `<id>` de todas as linhas                        |
//! | `idx:t:<coluna>:<valor>` | set | os `<id>` com aquele valor (chave estrangeira ou coluna categórica) |
//! | `idx:t:<coluna>`       | zset | os `<id>` pontuados pela coluna numérica ou de data |
//!
//! - `<id>` é a chave primária (as partes unidas por `:` numa composta) ou,
//!   sem chave primária, o número da linha.
//! - Os valores são o texto do próprio PostgreSQL (protocolo simples), o
//!   mesmo que o adapter mostra, para o resultado comparar célula a célula.
//!   Nulo não vira campo: `HGET` devolve nulo, como o SQL.
//! - Índices só em tabelas de até [`INDEX_MAX_ROWS`] linhas (as maiores
//!   passariam da memória do Docker), em toda chave estrangeira e nas colunas
//!   de texto ou booleanas com até [`CATEGORY_MAX_DISTINCT`] valores. A data
//!   pontua como `AAAAMMDD`.
//!
//! A carga confere no fim que cada tabela tem no Redis tantas linhas quanto no
//! PostgreSQL.

use redis::aio::MultiplexedConnection;
use tokio_postgres::{Client, NoTls, SimpleQueryMessage};

use super::config::{DatabaseConfig, Environment};

/// Tabelas maiores que isto ficam sem índice (só o hash e o `t:ids`).
pub const INDEX_MAX_ROWS: i64 = 100_000;

/// Coluna de texto com mais valores distintos que isto não vira índice.
pub const CATEGORY_MAX_DISTINCT: i64 = 100;

/// Linhas lidas do PostgreSQL por vez.
const FETCH_ROWS: usize = 5_000;

/// Comandos por pipeline mandado ao Redis.
const PIPELINE: usize = 2_000;

/// O que uma coluna vira no Redis, além do campo no hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Index {
    None,
    /// `idx:t:<coluna>:<valor>`
    Set,
    /// `idx:t:<coluna>`, pontuado pelo valor
    Sorted,
    /// `idx:t:<coluna>`, pontuado pela data como `AAAAMMDD`
    SortedDate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub data_type: String,
    pub index: Index,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    /// As colunas da chave primária, em ordem (vazio = número da linha)
    pub primary_key: Vec<String>,
    pub rows: i64,
}

/// O tipo é numérico o bastante para pontuar um zset?
fn numeric(data_type: &str) -> bool {
    matches!(
        data_type,
        "smallint" | "integer" | "bigint" | "real" | "double precision" | "numeric"
    )
}

fn categorical(data_type: &str) -> bool {
    matches!(data_type, "text" | "character varying" | "character" | "boolean")
}

/// O índice de uma coluna, pelas regras do módulo. `distinct` só importa para
/// as categóricas.
pub fn index_for(column: &str, data_type: &str, table: &Table, foreign: bool, distinct: Option<i64>) -> Index {
    if table.rows > INDEX_MAX_ROWS || table.primary_key.iter().any(|key| key == column) {
        return Index::None;
    }
    if foreign {
        return Index::Set;
    }
    if numeric(data_type) {
        return Index::Sorted;
    }
    if data_type == "date" {
        return Index::SortedDate;
    }
    match (categorical(data_type), distinct) {
        (true, Some(count)) if (2..=CATEGORY_MAX_DISTINCT).contains(&count) => Index::Set,
        _ => Index::None,
    }
}

/// A pontuação de uma célula num zset (`None` = não entra).
pub fn score(index: Index, value: &str) -> Option<f64> {
    match index {
        Index::Sorted => value.trim().parse::<f64>().ok().filter(|number| number.is_finite()),
        Index::SortedDate => {
            // `2012-03-04` → 20120304 (o PostgreSQL escreve a data em ISO)
            let digits: String = value.get(..10)?.chars().filter(char::is_ascii_digit).collect();
            (digits.len() == 8).then(|| digits.parse().ok()).flatten()
        }
        Index::None | Index::Set => None,
    }
}

/// O `<id>` de uma linha: a chave primária, ou o número da linha sem ela.
pub fn row_id(primary_key: &[Option<&str>], row_number: u64) -> String {
    match primary_key.is_empty() {
        true => row_number.to_string(),
        false => primary_key.iter().map(|part| part.unwrap_or("")).collect::<Vec<_>>().join(":"),
    }
}

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn quote_literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// As linhas de uma consulta pelo protocolo simples (texto do PostgreSQL).
async fn rows(client: &Client, sql: &str) -> Result<Vec<Vec<Option<String>>>, String> {
    let messages = client.simple_query(sql).await.map_err(|error| error.to_string())?;
    Ok(messages
        .into_iter()
        .filter_map(|message| match message {
            SimpleQueryMessage::Row(row) => {
                Some((0..row.len()).map(|index| row.get(index).map(str::to_string)).collect())
            }
            _ => None,
        })
        .collect())
}

/// A estrutura do conjunto, já com o índice de cada coluna.
async fn tables(client: &Client) -> Result<Vec<Table>, String> {
    let names = rows(
        client,
        "SELECT table_name FROM information_schema.tables \
         WHERE table_schema = 'public' AND table_type = 'BASE TABLE' ORDER BY table_name",
    )
    .await?;
    let mut tables = Vec::new();
    for name in names.into_iter().filter_map(|row| row.into_iter().next().flatten()) {
        let literal = quote_literal(&name);
        let columns = rows(
            client,
            &format!(
                "SELECT column_name, data_type FROM information_schema.columns \
                 WHERE table_schema = 'public' AND table_name = {literal} ORDER BY ordinal_position"
            ),
        )
        .await?;
        let primary_key: Vec<String> = rows(
            client,
            &format!(
                "SELECT a.attname FROM pg_index i \
                 JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey) \
                 WHERE i.indrelid = ('public.' || quote_ident({literal}))::regclass AND i.indisprimary \
                 ORDER BY array_position(i.indkey, a.attnum)"
            ),
        )
        .await?
        .into_iter()
        .filter_map(|row| row.into_iter().next().flatten())
        .collect();
        let foreign: Vec<String> = rows(
            client,
            &format!(
                "SELECT DISTINCT a.attname FROM pg_constraint c \
                 JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = ANY(c.conkey) \
                 WHERE c.contype = 'f' AND c.conrelid = ('public.' || quote_ident({literal}))::regclass"
            ),
        )
        .await?
        .into_iter()
        .filter_map(|row| row.into_iter().next().flatten())
        .collect();
        let count = rows(client, &format!("SELECT count(*) FROM public.{}", quote_ident(&name))).await?;
        let rows_in_table = count
            .first()
            .and_then(|row| row.first().cloned().flatten())
            .and_then(|text| text.parse().ok())
            .unwrap_or(0);

        let mut table = Table {
            name: name.clone(),
            columns: Vec::new(),
            primary_key,
            rows: rows_in_table,
        };
        for column in columns {
            let (Some(column_name), Some(data_type)) = (column[0].clone(), column[1].clone()) else {
                continue;
            };
            let is_foreign = foreign.contains(&column_name);
            // Distintos só onde decidem algo: categórica, em tabela indexável
            let distinct = match categorical(&data_type) && !is_foreign && table.rows <= INDEX_MAX_ROWS {
                true => rows(
                    client,
                    &format!("SELECT count(DISTINCT {}) FROM public.{}", quote_ident(&column_name), quote_ident(&name)),
                )
                .await?
                .first()
                .and_then(|row| row.first().cloned().flatten())
                .and_then(|text| text.parse().ok()),
                false => None,
            };
            let index = index_for(&column_name, &data_type, &table, is_foreign, distinct);
            table.columns.push(Column {
                name: column_name,
                data_type,
                index,
            });
        }
        tables.push(table);
    }
    Ok(tables)
}

/// Carrega uma tabela, lendo do PostgreSQL por cursor e gravando em pipelines.
async fn load_table(client: &Client, redis: &mut MultiplexedConnection, table: &Table) -> Result<(), String> {
    let select: Vec<String> = table.columns.iter().map(|column| quote_ident(&column.name)).collect();
    let key_positions: Vec<usize> = table
        .primary_key
        .iter()
        .filter_map(|key| table.columns.iter().position(|column| &column.name == key))
        .collect();
    client
        .batch_execute(&format!(
            "BEGIN READ ONLY; DECLARE carga NO SCROLL CURSOR FOR SELECT {} FROM public.{}",
            select.join(", "),
            quote_ident(&table.name)
        ))
        .await
        .map_err(|error| error.to_string())?;

    let ids_key = format!("{}:ids", table.name);
    let mut row_number: u64 = 0;
    let mut pipe = redis::pipe();
    let mut queued = 0;
    loop {
        let batch = rows(client, &format!("FETCH {FETCH_ROWS} FROM carga")).await?;
        if batch.is_empty() {
            break;
        }
        for row in &batch {
            row_number += 1;
            let key_parts: Vec<Option<&str>> = key_positions.iter().map(|&index| row[index].as_deref()).collect();
            let id = row_id(&key_parts, row_number);
            let mut fields: Vec<(&str, &str)> = Vec::new();
            for (column, value) in table.columns.iter().zip(row) {
                let Some(value) = value.as_deref() else { continue };
                fields.push((&column.name, value));
                match column.index {
                    Index::None => {}
                    Index::Set => {
                        pipe.sadd(format!("idx:{}:{}:{value}", table.name, column.name), &id).ignore();
                        queued += 1;
                    }
                    Index::Sorted | Index::SortedDate => {
                        if let Some(score) = score(column.index, value) {
                            pipe.zadd(format!("idx:{}:{}", table.name, column.name), &id, score).ignore();
                            queued += 1;
                        }
                    }
                }
            }
            // Toda linha tem ao menos a chave primária ou alguma coluna; uma
            // linha toda nula (sem chave primária) não tem o que guardar
            if !fields.is_empty() {
                pipe.hset_multiple(format!("{}:{id}", table.name), &fields).ignore();
            }
            pipe.sadd(&ids_key, &id).ignore();
            queued += 2;
            if queued >= PIPELINE {
                pipe.query_async::<()>(redis).await.map_err(|error| error.to_string())?;
                pipe = redis::pipe();
                queued = 0;
            }
        }
    }
    if queued > 0 {
        pipe.query_async::<()>(redis).await.map_err(|error| error.to_string())?;
    }
    client.batch_execute("CLOSE carga; COMMIT").await.map_err(|error| error.to_string())?;

    let loaded: i64 = redis::cmd("SCARD")
        .arg(&ids_key)
        .query_async(redis)
        .await
        .map_err(|error| error.to_string())?;
    if loaded != table.rows {
        return Err(format!(
            "{}: {} linha(s) no PostgreSQL e {loaded} no Redis (chave primária repetida?)",
            table.name, table.rows
        ));
    }
    Ok(())
}

/// O que a carga de um conjunto fez.
#[derive(Debug)]
pub struct Loaded {
    pub dataset: String,
    pub redis_db: String,
    pub tables: usize,
    pub rows: i64,
    pub indexed_tables: usize,
    pub keys: i64,
}

fn connection_string(config: &DatabaseConfig) -> String {
    let mut parts = vec![
        format!("host={}", config.host.as_deref().unwrap_or("localhost")),
        format!("port={}", config.port.unwrap_or(5432)),
        format!("dbname={}", config.database),
    ];
    if let Some(user) = &config.username {
        parts.push(format!("user={user}"));
    }
    parts.join(" ")
}

/// Carrega cada conjunto no seu banco do Redis. O PostgreSQL e o Redis são os
/// do `ambiente.toml`; os dois precisam usar `{dataset}`.
pub async fn load(
    environment: &Environment,
    datasets: &[String],
    env: &dyn Fn(&str) -> Option<String>,
    progress: &dyn Fn(&str),
) -> Result<Vec<Loaded>, String> {
    let postgres = environment.database.get("postgres").ok_or("o ambiente não tem [database.postgres]")?;
    let redis = environment.database.get("redis").ok_or("o ambiente não tem [database.redis]")?;
    if !postgres.per_dataset() || !redis.per_dataset() {
        return Err("[database.postgres] e [database.redis] precisam de `database = \"{dataset}\"`".into());
    }
    if postgres.password_env.is_some() || postgres.uri_env.is_some() || redis.uri_env.is_some() {
        return Err("a carga só conhece os bancos de teste sem senha do db.sh".into());
    }

    let mut done = Vec::new();
    for dataset in datasets {
        let pg_config = postgres.for_dataset(Some(dataset))?;
        let redis_config = redis.for_dataset(Some(dataset))?;
        let redis_db: u8 = redis_config
            .database
            .parse()
            .map_err(|_| format!("{dataset}: o banco do Redis precisa ser um número (`datasets` no ambiente)"))?;

        let (client, connection) = tokio_postgres::connect(&connection_string(&pg_config), NoTls)
            .await
            .map_err(|error| format!("{dataset}: PostgreSQL: {error}"))?;
        tokio::spawn(connection);

        let password = redis_config.password_env.as_deref().and_then(env).unwrap_or_default();
        let auth = match password.is_empty() {
            true => String::new(),
            false => format!(":{password}@"),
        };
        let url = format!(
            "redis://{auth}{}:{}/{redis_db}",
            redis_config.host.as_deref().unwrap_or("localhost"),
            redis_config.port.unwrap_or(6379)
        );
        let mut redis_conn = redis::Client::open(url)
            .map_err(|error| error.to_string())?
            .get_multiplexed_async_connection()
            .await
            .map_err(|error| format!("{dataset}: Redis: {error}"))?;

        let structure = tables(&client).await.map_err(|error| format!("{dataset}: {error}"))?;
        // Do zero: uma carga interrompida deixa chaves pela metade
        redis::cmd("FLUSHDB")
            .query_async::<()>(&mut redis_conn)
            .await
            .map_err(|error| error.to_string())?;
        for table in &structure {
            progress(&format!("{dataset}.{} ({} linhas)", table.name, table.rows));
            load_table(&client, &mut redis_conn, table)
                .await
                .map_err(|error| format!("{dataset}: {error}"))?;
        }
        let keys: i64 = redis::cmd("DBSIZE")
            .query_async(&mut redis_conn)
            .await
            .map_err(|error| error.to_string())?;
        log::info!("experimento: conjunto carregado no Redis ({} tabelas, {keys} chaves)", structure.len());
        done.push(Loaded {
            dataset: dataset.clone(),
            redis_db: redis_db.to_string(),
            tables: structure.len(),
            rows: structure.iter().map(|table| table.rows).sum(),
            indexed_tables: structure.iter().filter(|table| table.rows <= INDEX_MAX_ROWS).count(),
            keys,
        });
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(rows: i64) -> Table {
        Table {
            name: "schools".into(),
            columns: Vec::new(),
            primary_key: vec!["CDSCode".into()],
            rows,
        }
    }

    #[test]
    fn picks_the_index_of_each_column() {
        let small = table(10);
        assert_eq!(index_for("CDSCode", "text", &small, false, Some(10)), Index::None, "a chave primária já é o id");
        assert_eq!(index_for("district_id", "integer", &small, true, None), Index::Set, "estrangeira vira conjunto");
        assert_eq!(index_for("Enrollment", "integer", &small, false, None), Index::Sorted);
        assert_eq!(index_for("OpenDate", "date", &small, false, None), Index::SortedDate);
        assert_eq!(index_for("County", "text", &small, false, Some(58)), Index::Set);
        assert_eq!(index_for("School", "text", &small, false, Some(9_000)), Index::None, "texto livre não");
        assert_eq!(index_for("Charter", "boolean", &small, false, Some(1)), Index::None, "um valor só não separa nada");
        // Tabela grande fica sem índice nenhum
        assert_eq!(index_for("district_id", "integer", &table(INDEX_MAX_ROWS + 1), true, None), Index::None);
    }

    #[test]
    fn scores_numbers_and_dates() {
        assert_eq!(score(Index::Sorted, "12.5"), Some(12.5));
        assert_eq!(score(Index::Sorted, "-3"), Some(-3.0));
        assert_eq!(score(Index::Sorted, "NaN"), None);
        assert_eq!(score(Index::SortedDate, "2012-03-04"), Some(20120304.0));
        assert_eq!(score(Index::SortedDate, "2012-03"), None);
        assert_eq!(score(Index::Set, "1"), None);
    }

    #[test]
    fn the_row_id_is_the_primary_key_or_the_row_number() {
        assert_eq!(row_id(&[Some("42")], 7), "42");
        assert_eq!(row_id(&[Some("1"), Some("15")], 7), "1:15");
        assert_eq!(row_id(&[], 7), "7");
    }
}
