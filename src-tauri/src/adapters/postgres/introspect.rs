//! Introspecção do catálogo no Postgres (refactor do catálogo, Fase 2).
//!
//! Lê a estrutura de um database para um [`Catalog`] por uma conexão
//! dedicada, sem nada que pese no servidor do cliente (ver
//! `perf/catalog/BASELINE.md`):
//!
//! - **camada 0** — os schemas, já sem os de sistema e os temporários;
//! - **shape-first** — numa varredura do `pg_class`, o formato (conjunto de
//!   relações) e o fingerprint de cada schema; só **um representante por
//!   formato** traz as relações pela rede (5.000 tenants viram poucos formatos);
//! - **em massa** — quando o servidor não calcula formatos, ou há formatos demais.
//!
//! Tudo numa transação `REPEATABLE READ READ ONLY` (camadas coerentes entre si)
//! com `statement_timeout`/`lock_timeout` por `SET LOCAL` (funciona atrás de
//! pgbouncer), e só lendo o catálogo: nada de `pg_total_relation_size` em
//! massa, nenhum lock de tabela. As consultas independentes saem em pipeline —
//! a camada 0 e a varredura de formatos custam uma ida e volta.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures_util::{pin_mut, TryStreamExt};
use parking_lot::RwLock;
use tokio_postgres::error::SqlState;
use tokio_postgres::types::{ToSql, Type};
use tokio_postgres::{Client, Row};

use crate::catalog::{
    Catalog, CatalogSource, ColumnDetail, ColumnMatch, ColumnSpec, ForeignKey, FunctionKind, FunctionObject,
    IndexObject, LoadState, NameScope, ObjectKind, ObjectRef, RelKind, RelationColumns, RelationDetail, SchemaDiff, SchemaHeader,
    SourceCanceller, Strategy, SyncEvent, SyncReport, TableRecord,
};
use crate::error::{Error, Result};
use crate::models::Server;

use super::pool::{connect_dedicated, CancelHandle};

// ─────────────────────────────────────────────────────────────────────────────
// Servidor e capacidades
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavor {
    Postgres,
    Cockroach,
    Redshift,
    /// Fala o protocolo, mas o `version()` não diz quem é
    Other,
}

/// O que a introspecção pode usar neste servidor. Fora do Postgres de verdade
/// tudo cai no caminho mais simples (em massa, sem fingerprint): é mais lento,
/// mas não depende de nada que o fork possa não ter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// `relkind 'p'` e `relispartition` (PG 10+)
    pub partitions: bool,
    /// Formato por schema no servidor (`md5(string_agg(... ORDER BY))`)
    pub shapes: bool,
    /// `xmin` + `hashtext` para detectar mudança por schema
    pub fingerprints: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerInfo {
    pub version: String,
    /// Como o `server_version_num` (160004 = 16.4); 0 se não deu para ler
    pub version_num: i32,
    pub flavor: Flavor,
    pub caps: Capabilities,
}

impl ServerInfo {
    /// A partir do `SELECT version()`.
    pub fn from_version(version: &str) -> Self {
        let flavor = if version.contains("CockroachDB") {
            Flavor::Cockroach
        } else if version.contains("Redshift") {
            Flavor::Redshift
        } else if version.starts_with("PostgreSQL") {
            Flavor::Postgres
        } else {
            Flavor::Other
        };
        let version_num = parse_version_num(version);
        let postgres = flavor == Flavor::Postgres;

        Self {
            version: version.to_string(),
            version_num,
            flavor,
            caps: Capabilities {
                partitions: postgres && version_num >= 100_000,
                shapes: postgres && version_num >= 90_000,
                fingerprints: postgres,
            },
        }
    }

    /// Volta ao caminho mais simples (o servidor recusou formato ou fingerprint).
    fn downgrade(&mut self) {
        self.caps.shapes = false;
        self.caps.fingerprints = false;
    }
}

/// "PostgreSQL 16.4 on …" → 160004; "PostgreSQL 9.6.24" → 90624.
fn parse_version_num(version: &str) -> i32 {
    let Some(number) = version
        .strip_prefix("PostgreSQL ")
        .and_then(|rest| rest.split(|c: char| !(c.is_ascii_digit() || c == '.')).next())
    else {
        return 0;
    };
    let parts: Vec<i32> = number.split('.').filter_map(|part| part.parse().ok()).collect();
    match parts.as_slice() {
        [major, minor, ..] if *major >= 10 => major * 10_000 + minor,
        [major] if *major >= 10 => major * 10_000,
        [major, minor, patch, ..] => major * 10_000 + minor * 100 + patch,
        [major, minor] => major * 10_000 + minor * 100,
        _ => 0,
    }
}

fn rel_kind(code: &str) -> Option<RelKind> {
    match code {
        "r" => Some(RelKind::Table),
        "v" => Some(RelKind::View),
        "m" => Some(RelKind::MaterializedView),
        "f" => Some(RelKind::Foreign),
        "p" => Some(RelKind::Partitioned),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SQL
// ─────────────────────────────────────────────────────────────────────────────

/// Pai de uma partição, quando está no mesmo schema (uma partição em outro
/// schema aparece solta, como no catálogo). Subconsulta só roda para partições.
const PARENT_EXPR: &str = "CASE WHEN c.relispartition THEN (
        SELECT pc.relname FROM pg_inherits i JOIN pg_class pc ON pc.oid = i.inhparent
        WHERE i.inhrelid = c.oid AND pc.relnamespace = c.relnamespace) END";

/// Hash por relação; a soma por schema é o fingerprint (muda com CREATE, DROP,
/// ALTER e RENAME — linha nova no `pg_class` —, não com VACUUM/ANALYZE, que
/// gravam no lugar)
const RELHASH_EXPR: &str = "hashtext(c.oid::text || ':' || c.xmin::text)";

struct Sql {
    schemas: String,
    shapes: String,
    tables_by_oid: String,
    tables_by_name: String,
    tables_all: String,
}

impl Sql {
    fn new(caps: Capabilities, only_usable_schemas: bool) -> Self {
        let kinds = if caps.partitions {
            "('r', 'v', 'm', 'f', 'p')"
        } else {
            "('r', 'v', 'm', 'f')"
        };
        let parent = if caps.partitions { PARENT_EXPR } else { "NULL::name" };
        let relhash = if caps.fingerprints { RELHASH_EXPR } else { "0" };
        let usable = if only_usable_schemas {
            " AND has_schema_privilege(n.oid, 'USAGE')"
        } else {
            ""
        };

        let tables = |filter: &str| {
            format!(
                "SELECT c.relnamespace, c.relname, c.relkind::text, {parent}, {relhash}::int4
                 FROM pg_class c
                 WHERE c.relkind IN {kinds}{filter}"
            )
        };

        Self {
            // Schemas com prefixo pg_ são todos de sistema (o nome é reservado)
            schemas: format!(
                "SELECT n.oid, n.nspname FROM pg_namespace n
                 WHERE n.nspname <> 'information_schema' AND n.nspname !~ '^pg_'{usable}"
            ),
            shapes: format!(
                "SELECT c.relnamespace,
                        md5(string_agg(c.relname || ':' || c.relkind::text || ':' || coalesce({parent}, ''),
                                       ',' ORDER BY c.relname)),
                        sum({relhash})::int8
                 FROM pg_class c
                 WHERE c.relkind IN {kinds}
                 GROUP BY c.relnamespace"
            ),
            tables_by_oid: tables(" AND c.relnamespace = ANY($1)"),
            tables_by_name: tables(
                " AND c.relnamespace IN (SELECT oid FROM pg_namespace WHERE nspname = ANY($1))",
            ),
            tables_all: tables(""),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Comparação entre schemas
// ─────────────────────────────────────────────────────────────────────────────

/// Tira o próprio schema de um tipo ou default: cada tenant aponta para a sua
/// sequência (`nextval('tenant_1.orders_id_seq'::regclass)`) e os seus tipos
/// (`tenant_1.status`), e isso não é diferença.
fn unqualified(expr: &str, schema: &str) -> String {
    format!("replace(replace({expr}, quote_ident({schema}) || '.', ''), {schema} || '.', '')")
}

/// Relações de pares (schema, tabela) num CTE: `$a` e `$b` são os arrays.
fn pairs_cte(name: &str, schemas: &str, tables: &str) -> String {
    format!(
        "{name} AS (
           SELECT c.oid FROM unnest({schemas}::text[], {tables}::text[]) AS p(s, t)
           JOIN pg_namespace n ON n.nspname = p.s
           JOIN pg_class c ON c.relnamespace = n.oid AND c.relname = p.t)"
    )
}

/// Chaves estrangeiras entre as relações do escopo (`$1`, `$2`) com uma ponta
/// nas escolhidas (`$3`, `$4`). O `conrelid IN scope` usa o índice do
/// `pg_constraint` — nunca uma varredura pelas chaves do banco inteiro.
fn foreign_keys_sql() -> String {
    let column_names = |key: &str, rel: &str| {
        format!(
            "ARRAY(SELECT a.attname::text FROM unnest(con.{key}) WITH ORDINALITY AS k(attnum, ord)
                   JOIN pg_attribute a ON a.attrelid = con.{rel} AND a.attnum = k.attnum
                   ORDER BY k.ord)"
        )
    };
    format!(
        "WITH {scope}, {chosen}
         SELECT n.nspname::text, c.relname::text, {from},
                rn.nspname::text, rc.relname::text, {to}
         FROM pg_constraint con
         JOIN pg_class c ON c.oid = con.conrelid
         JOIN pg_namespace n ON n.oid = c.relnamespace
         JOIN pg_class rc ON rc.oid = con.confrelid
         JOIN pg_namespace rn ON rn.oid = rc.relnamespace
         WHERE con.contype = 'f'
           AND con.conrelid IN (SELECT oid FROM scope)
           AND con.confrelid IN (SELECT oid FROM scope)
           AND (con.conrelid IN (SELECT oid FROM chosen) OR con.confrelid IN (SELECT oid FROM chosen))
         ORDER BY n.nspname, c.relname, con.conname
         LIMIT {FOREIGN_KEYS_CAP}",
        scope = pairs_cte("scope", "$1", "$2"),
        chosen = pairs_cte("chosen", "$3", "$4"),
        from = column_names("conkey", "conrelid"),
        to = column_names("confkey", "confrelid"),
    )
}

/// Teto de chaves estrangeiras por pedido (uma tabela central num banco grande
/// pode ser apontada por centenas)
const FOREIGN_KEYS_CAP: usize = 1000;

/// Colunas, chave primária e comentários de pares (schema, tabela).
fn relation_details_sql() -> String {
    let data_type = unqualified("format_type(a.atttypid, a.atttypmod)", "n.nspname");
    format!(
        "SELECT p.s, p.t, obj_description(c.oid, 'pg_class'), a.attname::text, {data_type},
                NOT a.attnotnull, coalesce(a.attnum = ANY(i.indkey::int2[]), false),
                col_description(c.oid, a.attnum)
         FROM unnest($1::text[], $2::text[]) AS p(s, t)
         JOIN pg_namespace n ON n.nspname = p.s
         JOIN pg_class c ON c.relnamespace = n.oid AND c.relname = p.t
         JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped
         LEFT JOIN pg_index i ON i.indrelid = c.oid AND i.indisprimary
         ORDER BY p.s, p.t, a.attnum"
    )
}

/// Colunas de pares (schema, tabela) — `$1` e `$2`, na mesma posição.
fn relation_columns_sql() -> String {
    let data_type = unqualified("format_type(a.atttypid, a.atttypmod)", "n.nspname");
    let default = unqualified("pg_get_expr(d.adbin, d.adrelid)", "n.nspname");
    format!(
        "SELECT p.s, p.t, a.attname::text, {data_type}, NOT a.attnotnull, {default}
         FROM unnest($1::text[], $2::text[]) AS p(s, t)
         JOIN pg_namespace n ON n.nspname = p.s
         JOIN pg_class c ON c.relnamespace = n.oid AND c.relname = p.t
         JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped
         LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
         ORDER BY p.s, p.t, a.attnum"
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Pastas da árvore: índices, funções e definições
// ─────────────────────────────────────────────────────────────────────────────

/// Schemas de sistema nunca entram (o mesmo critério da camada 0)
const USER_SCHEMA: &str = "n.nspname <> 'information_schema' AND n.nspname !~ '^pg_'";

/// Índices de um schema (`$1`), ou só os da relação `$2`. As colunas saem do
/// `pg_get_indexdef(oid, k, true)`, que também dá a expressão de um índice
/// sobre expressão. Só o catálogo; nenhum tamanho.
fn indexes_sql() -> String {
    format!(
        "SELECT i.relname::text, t.relname::text, am.amname::text, ix.indisunique, ix.indisprimary,
                ARRAY(SELECT pg_get_indexdef(ix.indexrelid, k, true)
                      FROM generate_series(1, ix.indnatts::int) AS k ORDER BY k),
                pg_get_expr(ix.indpred, ix.indrelid, true)
         FROM pg_index ix
         JOIN pg_class i ON i.oid = ix.indexrelid
         JOIN pg_class t ON t.oid = ix.indrelid
         JOIN pg_namespace n ON n.oid = i.relnamespace
         JOIN pg_am am ON am.oid = i.relam
         WHERE n.nspname = $1 AND {USER_SCHEMA}
           AND ($2::text IS NULL OR t.relname = $2::text)
         ORDER BY i.relname COLLATE \"C\""
    )
}

/// `prokind` (PG 11+) ou, antes, as flags que ele substituiu.
fn prokind_expr(info: &ServerInfo) -> &'static str {
    if info.flavor == Flavor::Postgres && info.version_num >= 110_000 {
        "p.prokind::text"
    } else {
        "CASE WHEN p.proisagg THEN 'a' WHEN p.proiswindow THEN 'w' ELSE 'f' END"
    }
}

/// Funções e procedures de um schema (`$1`) sem as que pertencem a uma
/// extensão (`pg_depend` com `deptype = 'e'`): o `pgcrypto` num schema de
/// tenant não vira 40 itens na pasta.
fn functions_sql(info: &ServerInfo) -> String {
    format!(
        "SELECT p.proname::text, pg_get_function_identity_arguments(p.oid),
                pg_get_function_arguments(p.oid), pg_get_function_result(p.oid),
                {kind}, l.lanname::text, obj_description(p.oid, 'pg_proc')
         FROM pg_proc p
         JOIN pg_namespace n ON n.oid = p.pronamespace
         JOIN pg_language l ON l.oid = p.prolang
         WHERE n.nspname = $1 AND {USER_SCHEMA}
           AND NOT EXISTS (SELECT 1 FROM pg_depend d
                           WHERE d.classid = 'pg_proc'::regclass AND d.objid = p.oid AND d.deptype = 'e')
         ORDER BY p.proname COLLATE \"C\", 2",
        kind = prokind_expr(info),
    )
}

/// Colunas de um schema (`$1`) cujo nome casa com o `ILIKE` `$2` (escape
/// `\`), até `$3`: tabelas, views, views materializadas e estrangeiras, sem as
/// partições (as colunas são as da mãe).
fn search_columns_sql(caps: &Capabilities) -> String {
    let data_type = unqualified("format_type(a.atttypid, a.atttypmod)", "n.nspname");
    let partitions = if caps.partitions {
        "AND NOT c.relispartition"
    } else {
        ""
    };
    format!(
        "SELECT c.relname::text, a.attname::text, {data_type}
         FROM pg_namespace n
         JOIN pg_class c ON c.relnamespace = n.oid
         JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped
         WHERE n.nspname = $1::text AND {USER_SCHEMA}
           AND c.relkind IN ('r', 'p', 'v', 'm', 'f') {partitions}
           AND a.attname ILIKE $2::text ESCAPE '\\'
         ORDER BY c.relname COLLATE \"C\", a.attnum
         LIMIT $3::int8"
    )
}

/// Comentários dos schemas `$1`. O do `public` que o Postgres cria
/// ("standard public schema") não diz nada e fica de fora.
const SCHEMA_COMMENTS_SQL: &str = "SELECT n.nspname::text, d.description
         FROM pg_namespace n
         JOIN pg_description d ON d.objoid = n.oid AND d.classoid = 'pg_namespace'::regclass
         WHERE n.nspname = ANY($1::text[]) AND d.description <> 'standard public schema'
         ORDER BY n.nspname COLLATE \"C\"";

/// Definição de uma view (`$1` schema, `$2` nome), já como `CREATE`.
const VIEW_DEFINITION_SQL: &str = "SELECT format(CASE c.relkind WHEN 'm' THEN 'CREATE MATERIALIZED VIEW %I.%I AS' \
                                       ELSE 'CREATE OR REPLACE VIEW %I.%I AS' END, n.nspname, c.relname)
            || E'\\n' || rtrim(pg_get_viewdef(c.oid, true), E'; \\n')
     FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
     WHERE n.nspname = $1 AND c.relname = $2 AND c.relkind IN ('v', 'm')";

const INDEX_DEFINITION_SQL: &str = "SELECT pg_get_indexdef(i.oid)
     FROM pg_class i JOIN pg_namespace n ON n.oid = i.relnamespace
     WHERE n.nspname = $1 AND i.relname = $2 AND i.relkind IN ('i', 'I')";

/// Definição de uma função pela assinatura (`$3`, a de identidade). Agregada
/// não tem `pg_get_functiondef` (dá erro): monta-se o `CREATE AGGREGATE` com
/// o essencial do `pg_aggregate`.
fn function_definition_sql(info: &ServerInfo) -> String {
    format!(
        "SELECT CASE WHEN {kind} = 'a' THEN
                  format(E'CREATE AGGREGATE %I.%I(%s) (\\n    SFUNC = %s,\\n    STYPE = %s%s%s\\n)',
                         n.nspname, p.proname, pg_get_function_identity_arguments(p.oid),
                         a.aggtransfn::regproc, format_type(a.aggtranstype, NULL),
                         CASE WHEN a.aggfinalfn <> 0 THEN E',\\n    FINALFUNC = ' || a.aggfinalfn::regproc END,
                         CASE WHEN a.agginitval IS NOT NULL THEN E',\\n    INITCOND = ' || quote_literal(a.agginitval) END)
                ELSE rtrim(pg_get_functiondef(p.oid), E'\\n') END
         FROM pg_proc p
         JOIN pg_namespace n ON n.oid = p.pronamespace
         LEFT JOIN pg_aggregate a ON a.aggfnoid = p.oid
         WHERE n.nspname = $1 AND p.proname = $2 AND pg_get_function_identity_arguments(p.oid) = $3",
        kind = prokind_expr(info),
    )
}

fn function_kind(code: &str) -> FunctionKind {
    match code {
        "p" => FunctionKind::Procedure,
        "a" => FunctionKind::Aggregate,
        "w" => FunctionKind::Window,
        _ => FunctionKind::Function,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Planejamento (puro)
// ─────────────────────────────────────────────────────────────────────────────

/// O que buscar e o que aproveitar numa sincronização shape-first.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ShapePlan {
    /// Representantes (oid, nome) cujas relações precisam vir do banco
    pub fetch: Vec<(u32, String)>,
    /// (schema, igual a): aproveita o formato de um schema já carregado
    pub share: Vec<(String, String)>,
    /// Schemas pendentes sem nenhuma relação
    pub empty: Vec<String>,
}

/// Para cada formato com schemas pendentes (não carregados ou velhos): se um
/// schema do mesmo formato já está carregado e em dia, todos os pendentes o
/// copiam; senão o primeiro pendente (em ordem de nome) é buscado e os demais o
/// copiam. `schemas` vem em qualquer ordem; `hashes` só tem schemas com relações.
pub(crate) fn plan_shape_first(
    catalog: &Catalog,
    schemas: &[(u32, String)],
    hashes: &HashMap<u32, String>,
) -> ShapePlan {
    let mut by_name: Vec<&(u32, String)> = schemas.iter().collect();
    by_name.sort_by(|a, b| a.1.cmp(&b.1));

    /// Schemas de um mesmo formato: um já carregado e em dia (se houver) e os pendentes
    #[derive(Default)]
    struct Group<'a> {
        fresh: Option<&'a str>,
        pending: Vec<&'a (u32, String)>,
    }

    let mut groups: BTreeMap<&str, Group> = BTreeMap::new();
    let mut plan = ShapePlan::default();

    for entry in by_name {
        let pending = catalog.schema_state(&entry.1) != Some(LoadState::Loaded);
        match hashes.get(&entry.0) {
            None => {
                if pending {
                    plan.empty.push(entry.1.clone());
                }
            }
            Some(hash) => {
                let group = groups.entry(hash.as_str()).or_default();
                if pending {
                    group.pending.push(entry);
                } else if group.fresh.is_none() {
                    group.fresh = Some(entry.1.as_str());
                }
            }
        }
    }

    for Group { fresh, pending } in groups.into_values() {
        let Some((first, rest)) = pending.split_first() else {
            continue;
        };
        let like = match fresh {
            Some(fresh) => {
                plan.share.push((first.1.clone(), fresh.to_string()));
                fresh.to_string()
            }
            None => {
                plan.fetch.push((first.0, first.1.clone()));
                first.1.clone()
            }
        };
        plan.share
            .extend(rest.iter().map(|entry| (entry.1.clone(), like.clone())));
    }
    plan
}

/// Relações de um schema como vieram do banco, com o fingerprint somado.
#[derive(Debug, Default)]
struct SchemaTables {
    records: Vec<TableRecord>,
    fingerprint: i64,
}

fn table_record(row: &Row) -> Result<(u32, TableRecord, i32)> {
    let namespace: u32 = row.try_get(0)?;
    let name: String = row.try_get(1)?;
    let code: String = row.try_get(2)?;
    let parent: Option<String> = row.try_get(3)?;
    let relhash: i32 = row.try_get(4)?;
    let kind = rel_kind(&code).ok_or_else(|| Error::Query(format!("unexpected relkind {code}")))?;

    Ok((
        namespace,
        TableRecord {
            name,
            kind,
            partition_of: parent,
        },
        relhash,
    ))
}

// ─────────────────────────────────────────────────────────────────────────────
// Introspector
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct IntrospectOptions {
    pub statement_timeout: Duration,
    pub lock_timeout: Duration,
    /// Esconde schemas sem `USAGE` (multi-tenant com um role por cliente)
    pub only_usable_schemas: bool,
    /// Acima de tantos representantes, uma varredura em massa sai mais barata
    /// que um `ANY` enorme
    pub max_representatives: usize,
    /// Ignora os formatos (testes e diagnóstico)
    pub force_bulk: bool,
    /// Escopo salvo na conexão: schemas fora dele não entram no catálogo
    pub scope: NameScope,
}

impl Default for IntrospectOptions {
    fn default() -> Self {
        Self {
            statement_timeout: Duration::from_secs(15),
            lock_timeout: Duration::from_secs(2),
            only_usable_schemas: false,
            max_representatives: 512,
            force_bulk: false,
            scope: NameScope::default(),
        }
    }
}

/// Interrompe a operação em andamento de um [`Introspector`] (de outra task).
#[derive(Clone)]
pub struct IntrospectCanceller {
    handle: CancelHandle,
    flag: Arc<AtomicBool>,
}

impl IntrospectCanceller {
    pub async fn cancel(&self) -> Result<()> {
        self.flag.store(true, Ordering::SeqCst);
        self.handle.cancel().await
    }
}

/// Introspecção de um database por uma conexão própria. Os métodos pegam
/// `&mut self`: a conexão guarda estado de transação, então uma operação por
/// vez (quem quiser concorrência abre outro).
pub struct Introspector {
    client: Client,
    cancel: CancelHandle,
    cancelled: Arc<AtomicBool>,
    info: ServerInfo,
    options: IntrospectOptions,
}

/// Abre a transação da introspecção. O JIT sai: nestas varreduras a
/// compilação custa mais que a consulta (~330 ms de JIT sobre ~570 ms no
/// tier L — ver FASE2.md); `jit` só existe a partir do PG 11.
fn begin_sql(info: &ServerInfo, options: &IntrospectOptions) -> String {
    let jit = if info.flavor == Flavor::Postgres && info.version_num >= 110_000 {
        "\n             SET LOCAL jit = off;"
    } else {
        ""
    };
    format!(
        "BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY;
             SET LOCAL statement_timeout = {};
             SET LOCAL lock_timeout = {};{jit}",
        options.statement_timeout.as_millis(),
        options.lock_timeout.as_millis()
    )
}

/// Erro de uma consulta de introspecção. `Unsupported` = o servidor não tem
/// algo que a consulta usa (fork, versão antiga): a sincronização cai para o
/// caminho simples em vez de falhar.
enum Failure {
    Unsupported,
    Other(Error),
}

fn is_unsupported(code: &SqlState) -> bool {
    *code == SqlState::UNDEFINED_FUNCTION
        || *code == SqlState::UNDEFINED_COLUMN
        || *code == SqlState::UNDEFINED_TABLE
        || *code == SqlState::FEATURE_NOT_SUPPORTED
}

impl From<tokio_postgres::Error> for Failure {
    fn from(source: tokio_postgres::Error) -> Self {
        if source.code().is_some_and(is_unsupported) {
            Failure::Unsupported
        } else {
            Failure::Other(Error::from(source))
        }
    }
}

impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Failure::Other(error)
    }
}

impl Introspector {
    pub async fn connect(server: &Server, database: &str, options: IntrospectOptions) -> Result<Self> {
        let (client, cancel) = connect_dedicated(server, database).await?;
        // query_typed: uma ida e volta (sem o prepare)
        let rows = client.query_typed("SELECT version()", &[]).await?;
        let version: &str = rows
            .first()
            .ok_or_else(|| Error::Query("version() returned no rows".into()))?
            .try_get(0)?;
        let info = ServerInfo::from_version(version);

        Ok(Self {
            client,
            cancel,
            cancelled: Arc::new(AtomicBool::new(false)),
            info,
            options,
        })
    }

    #[cfg(test)]
    pub fn info(&self) -> &ServerInfo {
        &self.info
    }

    pub fn is_closed(&self) -> bool {
        self.client.is_closed()
    }

    pub fn canceller(&self) -> IntrospectCanceller {
        IntrospectCanceller {
            handle: self.cancel.clone(),
            flag: Arc::clone(&self.cancelled),
        }
    }

    fn begin_sql(&self) -> String {
        begin_sql(&self.info, &self.options)
    }

    fn sql(&self) -> Sql {
        Sql::new(self.info.caps, self.options.only_usable_schemas)
    }

    /// Fecha a transação — sempre, inclusive depois de erro ou cancelamento,
    /// a não ser que o `ROLLBACK` já tenha ido no pipeline — e traduz o erro
    /// de uma query cancelada a pedido.
    async fn finish<T>(
        &mut self,
        result: std::result::Result<T, Failure>,
        rolled_back: bool,
    ) -> std::result::Result<T, Failure> {
        if !rolled_back || result.is_err() {
            let _ = self.client.batch_execute("ROLLBACK").await;
        }
        match result {
            Err(_) if self.cancelled.swap(false, Ordering::SeqCst) => Err(Failure::Other(Error::Cancelled)),
            other => other,
        }
    }

    /// Sincroniza o catálogo com o banco: na primeira vez carrega tudo; depois
    /// só recarrega os schemas novos ou cujo fingerprint mudou. A camada 0 é
    /// aplicada (e avisada em `on_event`) antes das relações chegarem.
    pub async fn sync(
        &mut self,
        catalog: &RwLock<Catalog>,
        mut on_event: impl FnMut(SyncEvent),
    ) -> Result<SyncReport> {
        self.cancelled.store(false, Ordering::SeqCst);
        let result = self.sync_once(catalog, &mut on_event).await;
        let result = self.finish(result, false).await;

        match result {
            Ok(report) => Ok(report),
            Err(Failure::Other(error)) => Err(error),
            // O servidor recusou formato/fingerprint: refaz pelo caminho simples
            Err(Failure::Unsupported) => {
                self.info.downgrade();
                let result = self.sync_once(catalog, &mut on_event).await;
                match self.finish(result, false).await {
                    Ok(report) => Ok(report),
                    Err(Failure::Other(error)) => Err(error),
                    Err(Failure::Unsupported) => Err(Error::UnsupportedDatabase(format!(
                        "catalog introspection is not supported by: {}",
                        self.info.version
                    ))),
                }
            }
        }
    }

    async fn sync_once(
        &mut self,
        catalog: &RwLock<Catalog>,
        on_event: &mut impl FnMut(SyncEvent),
    ) -> std::result::Result<SyncReport, Failure> {
        let start = Instant::now();
        let sql = self.sql();
        let begin = self.begin_sql();
        let use_shapes = self.info.caps.shapes && !self.options.force_bulk;
        let scope = &self.options.scope;
        let client = &self.client;

        // Pipeline: BEGIN, camada 0 e varredura de formatos saem juntos
        let mut layer0_at = Duration::ZERO;
        let mut layer0_diff = SchemaDiff::default();
        let (begun, schemas, shapes) = tokio::join!(
            client.batch_execute(&begin),
            async {
                let rows = client.query_typed(&sql.schemas, &[]).await?;
                let mut schemas: Vec<(u32, String)> = rows
                    .iter()
                    .map(|row| Ok((row.try_get(0)?, row.try_get(1)?)))
                    .collect::<std::result::Result<_, tokio_postgres::Error>>()?;
                // Fora do escopo salvo: nem lista, nem relações
                schemas.retain(|(_, name)| scope.allows(name));

                layer0_diff = catalog.write().set_schemas(schemas.iter().map(|(_, name)| SchemaHeader {
                    name: name.clone(),
                    fingerprint: None,
                }));
                layer0_at = start.elapsed();
                on_event(SyncEvent::Schemas(layer0_diff.clone()));
                Ok::<_, tokio_postgres::Error>(schemas)
            },
            async {
                if !use_shapes {
                    return Ok(None);
                }
                client.query_typed(&sql.shapes, &[]).await.map(Some)
            },
        );
        begun?;
        let schemas = schemas?;
        let shapes = shapes?;
        let known: HashSet<u32> = schemas.iter().map(|(oid, _)| *oid).collect();

        let mut report = SyncReport {
            strategy: if shapes.is_some() { Strategy::ShapeFirst } else { Strategy::Bulk },
            schemas: schemas.len(),
            shapes: None,
            fetched: 0,
            shared: 0,
            diff: layer0_diff,
            layer0: layer0_at,
            total: Duration::ZERO,
        };

        match shapes {
            Some(rows) => {
                let mut hashes: HashMap<u32, String> = HashMap::new();
                let mut fingerprints: HashMap<u32, i64> = HashMap::new();
                for row in &rows {
                    let namespace: u32 = row.try_get(0)?;
                    if !known.contains(&namespace) {
                        continue;
                    }
                    hashes.insert(namespace, row.try_get(1)?);
                    fingerprints.insert(namespace, row.try_get(2)?);
                }
                let fingerprint = |oid: u32| Some(fingerprints.get(&oid).copied().unwrap_or(0));
                report.shapes = Some(hashes.values().collect::<HashSet<_>>().len());

                let plan = {
                    let mut guard = catalog.write();
                    let diff = guard.set_schemas(schemas.iter().map(|(oid, name)| SchemaHeader {
                        name: name.clone(),
                        fingerprint: fingerprint(*oid),
                    }));
                    report.diff.changed = diff.changed;
                    plan_shape_first(&guard, &schemas, &hashes)
                };

                let oid_of: HashMap<&str, u32> = schemas.iter().map(|(oid, name)| (name.as_str(), *oid)).collect();
                let fetched = if plan.fetch.is_empty() {
                    HashMap::new()
                } else if plan.fetch.len() > self.options.max_representatives {
                    self.read_tables(&sql.tables_all, None).await?
                } else {
                    let oids: Vec<u32> = plan.fetch.iter().map(|(oid, _)| *oid).collect();
                    let param: &(dyn ToSql + Sync) = &oids;
                    self.read_tables(&sql.tables_by_oid, Some((param, Type::OID_ARRAY))).await?
                };

                {
                    let mut guard = catalog.write();
                    for (oid, name) in &plan.fetch {
                        let tables = fetched.get(oid).map(|t| t.records.clone()).unwrap_or_default();
                        guard.set_tables(name, tables, fingerprint(*oid));
                    }
                    for (schema, like) in &plan.share {
                        guard.share_shape(schema, like, fingerprint(oid_of[schema.as_str()]));
                    }
                    for schema in &plan.empty {
                        guard.set_tables(schema, Vec::new(), fingerprint(oid_of[schema.as_str()]));
                    }
                }
                report.fetched = plan.fetch.len();
                report.shared = plan.share.len();
                on_event(SyncEvent::Relations {
                    schemas: plan.fetch.len() + plan.share.len() + plan.empty.len(),
                });
            }
            None => {
                let fetched = self.read_tables(&sql.tables_all, None).await?;
                let with_fingerprints = self.info.caps.fingerprints;
                let fingerprint_of = |oid: &u32| {
                    with_fingerprints.then(|| fetched.get(oid).map_or(0, |t| t.fingerprint))
                };

                let mut loaded = 0;
                {
                    let mut guard = catalog.write();
                    let diff = guard.set_schemas(schemas.iter().map(|(oid, name)| SchemaHeader {
                        name: name.clone(),
                        fingerprint: fingerprint_of(oid),
                    }));
                    report.diff.changed = diff.changed;

                    // Sem fingerprint não dá para saber o que mudou: recarrega tudo
                    for (oid, name) in &schemas {
                        if with_fingerprints && guard.schema_state(name) == Some(LoadState::Loaded) {
                            continue;
                        }
                        let tables = fetched.get(oid).map(|t| t.records.clone()).unwrap_or_default();
                        guard.set_tables(name, tables, fingerprint_of(oid));
                        loaded += 1;
                    }
                }
                report.fetched = loaded;
                on_event(SyncEvent::Relations { schemas: loaded });
            }
        }

        catalog.write().set_fetched_at(chrono::Utc::now().timestamp_millis());
        report.total = start.elapsed();
        Ok(report)
    }

    /// Lê relações em streaming, agrupadas por schema (oid), somando o
    /// fingerprint de cada um.
    async fn read_tables(
        &self,
        sql: &str,
        param: Option<(&(dyn ToSql + Sync), Type)>,
    ) -> std::result::Result<HashMap<u32, SchemaTables>, Failure> {
        let stream = self.client.query_typed_raw(sql, param).await?;
        pin_mut!(stream);

        let mut by_schema: HashMap<u32, SchemaTables> = HashMap::new();
        while let Some(row) = stream.try_next().await? {
            let (namespace, record, relhash) = table_record(&row)?;
            let entry = by_schema.entry(namespace).or_default();
            entry.records.push(record);
            entry.fingerprint += i64::from(relhash);
        }
        Ok(by_schema)
    }

    /// Carrega já as relações de alguns schemas (o usuário abriu um schema que
    /// a sincronização ainda não alcançou). Devolve quantos foram carregados.
    pub async fn load_schemas(&mut self, catalog: &RwLock<Catalog>, schemas: &[String]) -> Result<usize> {
        let schemas: Vec<&String> = schemas
            .iter()
            .filter(|schema| self.options.scope.allows(schema))
            .collect();
        if schemas.is_empty() {
            return Ok(0);
        }
        self.cancelled.store(false, Ordering::SeqCst);

        let sql = self.sql();
        let begin = self.begin_sql();
        let names: Vec<&str> = schemas.iter().map(|schema| schema.as_str()).collect();
        let lookup = "SELECT oid, nspname FROM pg_namespace WHERE nspname = ANY($1)";

        let result: std::result::Result<usize, Failure> = async {
            let client = &self.client;
            let param: &(dyn ToSql + Sync) = &names;
            let lookup_params = [(param, Type::TEXT_ARRAY)];
            // Tudo numa ida e volta, ROLLBACK incluído
            let (begun, oids, tables, _) = tokio::join!(
                client.batch_execute(&begin),
                client.query_typed(lookup, &lookup_params),
                self.read_tables(&sql.tables_by_name, Some((param, Type::TEXT_ARRAY))),
                client.batch_execute("ROLLBACK"),
            );
            begun?;
            let oids = oids?;
            let tables = tables?;
            let with_fingerprints = self.info.caps.fingerprints;

            let mut guard = catalog.write();
            for row in &oids {
                let oid: u32 = row.try_get(0)?;
                let name: String = row.try_get(1)?;
                let found = tables.get(&oid);
                let records = found.map(|t| t.records.clone()).unwrap_or_default();
                let fingerprint = with_fingerprints.then(|| found.map_or(0, |t| t.fingerprint));
                guard.set_tables(&name, records, fingerprint);
            }
            Ok(oids.len())
        }
        .await;

        match self.finish(result, true).await {
            Ok(count) => Ok(count),
            Err(Failure::Other(error)) => Err(error),
            Err(Failure::Unsupported) => Err(Error::UnsupportedDatabase(self.info.version.clone())),
        }
    }

    /// Tamanho exato de uma relação (dados + índices + TOAST) — só da que o
    /// usuário abriu; em massa isto lê o disco e incha a memória do servidor.
    pub async fn relation_size(&mut self, schema: &str, table: &str) -> Result<Option<i64>> {
        self.single_value(
            "SELECT pg_total_relation_size(c.oid) FROM pg_class c
             JOIN pg_namespace n ON n.oid = c.relnamespace
             WHERE n.nspname = $1 AND c.relname = $2",
            &[
                (&schema as &(dyn ToSql + Sync), Type::NAME),
                (&table as &(dyn ToSql + Sync), Type::NAME),
            ],
        )
        .await
    }

    /// Colunas de pares (schema, tabela) — os dois lados da comparação.
    pub async fn relation_columns(&mut self, relations: &[(String, String)]) -> Result<RelationColumns> {
        let mut columns = RelationColumns::new();
        if relations.is_empty() {
            return Ok(columns);
        }
        let schemas: Vec<&str> = relations.iter().map(|(schema, _)| schema.as_str()).collect();
        let tables: Vec<&str> = relations.iter().map(|(_, table)| table.as_str()).collect();
        let rows = self
            .read_rows(
                &relation_columns_sql(),
                &[
                    (&schemas as &(dyn ToSql + Sync), Type::TEXT_ARRAY),
                    (&tables as &(dyn ToSql + Sync), Type::TEXT_ARRAY),
                ],
            )
            .await?;

        for row in &rows {
            let key: (String, String) = (row.try_get(0)?, row.try_get(1)?);
            let column = (
                row.try_get::<_, String>(2)?,
                ColumnSpec {
                    data_type: row.try_get(3)?,
                    nullable: row.try_get(4)?,
                    default_value: row.try_get(5)?,
                },
            );
            columns.entry(key).or_default().push(column);
        }
        Ok(columns)
    }

    /// Chaves estrangeiras entre as relações do escopo, com uma ponta em `of`.
    pub async fn foreign_keys(
        &mut self,
        scope: &[(String, String)],
        of: &[(String, String)],
    ) -> Result<Vec<ForeignKey>> {
        if scope.is_empty() || of.is_empty() {
            return Ok(Vec::new());
        }
        let split = |pairs: &[(String, String)]| -> (Vec<String>, Vec<String>) {
            pairs.iter().cloned().unzip()
        };
        let (scope_schemas, scope_tables) = split(scope);
        let (of_schemas, of_tables) = split(of);
        let rows = self
            .read_rows(
                &foreign_keys_sql(),
                &[
                    (&scope_schemas as &(dyn ToSql + Sync), Type::TEXT_ARRAY),
                    (&scope_tables as &(dyn ToSql + Sync), Type::TEXT_ARRAY),
                    (&of_schemas as &(dyn ToSql + Sync), Type::TEXT_ARRAY),
                    (&of_tables as &(dyn ToSql + Sync), Type::TEXT_ARRAY),
                ],
            )
            .await?;

        rows.iter()
            .map(|row| {
                Ok(ForeignKey {
                    schema: row.try_get(0)?,
                    table: row.try_get(1)?,
                    columns: row.try_get(2)?,
                    ref_schema: row.try_get(3)?,
                    ref_table: row.try_get(4)?,
                    ref_columns: row.try_get(5)?,
                })
            })
            .collect()
    }

    /// Colunas, chave primária e comentários das relações, na ordem pedida.
    pub async fn relation_details(&mut self, relations: &[(String, String)]) -> Result<Vec<RelationDetail>> {
        if relations.is_empty() {
            return Ok(Vec::new());
        }
        let (schemas, tables): (Vec<String>, Vec<String>) = relations.iter().cloned().unzip();
        let rows = self
            .read_rows(
                &relation_details_sql(),
                &[
                    (&schemas as &(dyn ToSql + Sync), Type::TEXT_ARRAY),
                    (&tables as &(dyn ToSql + Sync), Type::TEXT_ARRAY),
                ],
            )
            .await?;

        let mut by_key: HashMap<(String, String), RelationDetail> = HashMap::new();
        for row in &rows {
            let key: (String, String) = (row.try_get(0)?, row.try_get(1)?);
            let comment: Option<String> = row.try_get(2)?;
            let detail = by_key.entry(key.clone()).or_insert_with(|| RelationDetail {
                schema: key.0.clone(),
                table: key.1.clone(),
                comment,
                columns: Vec::new(),
            });
            detail.columns.push(ColumnDetail {
                name: row.try_get(3)?,
                data_type: row.try_get(4)?,
                nullable: row.try_get(5)?,
                primary_key: row.try_get(6)?,
                comment: row.try_get(7)?,
            });
        }
        Ok(relations.iter().filter_map(|key| by_key.remove(key)).collect())
    }

    /// Índices de um schema, ou de uma relação dele.
    pub async fn list_indexes(&mut self, schema: &str, table: Option<&str>) -> Result<Vec<IndexObject>> {
        let rows = self
            .read_rows(
                &indexes_sql(),
                &[
                    (&schema as &(dyn ToSql + Sync), Type::TEXT),
                    (&table as &(dyn ToSql + Sync), Type::TEXT),
                ],
            )
            .await?;
        rows.iter()
            .map(|row| {
                Ok(IndexObject {
                    name: row.try_get(0)?,
                    table: row.try_get(1)?,
                    method: row.try_get(2)?,
                    unique: row.try_get(3)?,
                    primary: row.try_get(4)?,
                    columns: row.try_get(5)?,
                    predicate: row.try_get(6)?,
                })
            })
            .collect()
    }

    /// Funções e procedures de um schema, fora as de extensões.
    pub async fn list_functions(&mut self, schema: &str) -> Result<Vec<FunctionObject>> {
        let sql = functions_sql(&self.info);
        let rows = self
            .read_rows(&sql, &[(&schema as &(dyn ToSql + Sync), Type::TEXT)])
            .await?;
        rows.iter()
            .map(|row| {
                let kind: String = row.try_get(4)?;
                Ok(FunctionObject {
                    name: row.try_get(0)?,
                    signature: row.try_get(1)?,
                    arguments: row.try_get(2)?,
                    returns: row.try_get(3)?,
                    function_kind: function_kind(&kind),
                    language: row.try_get(5)?,
                    comment: row.try_get(6)?,
                })
            })
            .collect()
    }

    /// Colunas de um schema pelo nome (ver [`CatalogSource::search_columns`]).
    pub async fn search_columns(&mut self, schema: &str, pattern: &str, limit: usize) -> Result<Vec<ColumnMatch>> {
        let sql = search_columns_sql(&self.info.caps);
        let limit = limit as i64;
        let rows = self
            .read_rows(
                &sql,
                &[
                    (&schema as &(dyn ToSql + Sync), Type::TEXT),
                    (&pattern as &(dyn ToSql + Sync), Type::TEXT),
                    (&limit as &(dyn ToSql + Sync), Type::INT8),
                ],
            )
            .await?;
        rows.iter()
            .map(|row| {
                Ok(ColumnMatch {
                    table: row.try_get(0)?,
                    column: row.try_get(1)?,
                    data_type: row.try_get(2)?,
                })
            })
            .collect()
    }

    /// Os comentários dos schemas pedidos que têm um.
    pub async fn schema_comments(&mut self, schemas: &[String]) -> Result<Vec<(String, String)>> {
        if schemas.is_empty() {
            return Ok(Vec::new());
        }
        let rows = self
            .read_rows(SCHEMA_COMMENTS_SQL, &[(&schemas as &(dyn ToSql + Sync), Type::TEXT_ARRAY)])
            .await?;
        rows.iter()
            .map(|row| Ok((row.try_get(0)?, row.try_get(1)?)))
            .collect()
    }

    /// A definição completa de uma view, índice ou função, terminada em `;`.
    pub async fn definition(&mut self, target: &ObjectRef) -> Result<Option<String>> {
        let signature = target.signature.clone().unwrap_or_default();
        let (sql, with_signature) = match target.kind {
            ObjectKind::View | ObjectKind::MaterializedView => (VIEW_DEFINITION_SQL.to_string(), false),
            ObjectKind::Index => (INDEX_DEFINITION_SQL.to_string(), false),
            ObjectKind::Function => (function_definition_sql(&self.info), true),
        };
        let mut params: Vec<(&(dyn ToSql + Sync), Type)> = vec![
            (&target.schema as &(dyn ToSql + Sync), Type::TEXT),
            (&target.name as &(dyn ToSql + Sync), Type::TEXT),
        ];
        if with_signature {
            params.push((&signature as &(dyn ToSql + Sync), Type::TEXT));
        }
        let rows = self.read_rows(&sql, &params).await?;
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        let text: Option<String> = row.try_get(0)?;
        Ok(text.map(|text| format!("{};\n", text.trim_end().trim_end_matches(';'))))
    }

    /// Linhas de uma consulta ao catálogo, na transação só-leitura de sempre.
    async fn read_rows(&mut self, sql: &str, params: &[(&(dyn ToSql + Sync), Type)]) -> Result<Vec<Row>> {
        self.cancelled.store(false, Ordering::SeqCst);
        let begin = self.begin_sql();
        let result: std::result::Result<Vec<Row>, Failure> = async {
            let (begun, rows, _) = tokio::join!(
                self.client.batch_execute(&begin),
                self.client.query_typed(sql, params),
                self.client.batch_execute("ROLLBACK"),
            );
            begun?;
            Ok(rows?)
        }
        .await;

        match self.finish(result, true).await {
            Ok(rows) => Ok(rows),
            Err(Failure::Other(error)) => Err(error),
            Err(Failure::Unsupported) => Err(Error::UnsupportedDatabase(self.info.version.clone())),
        }
    }

    async fn single_value(&mut self, sql: &str, params: &[(&(dyn ToSql + Sync), Type)]) -> Result<Option<i64>> {
        self.cancelled.store(false, Ordering::SeqCst);
        let begin = self.begin_sql();
        let result: std::result::Result<Option<i64>, Failure> = async {
            let (begun, rows, _) = tokio::join!(
                self.client.batch_execute(&begin),
                self.client.query_typed(sql, params),
                self.client.batch_execute("ROLLBACK"),
            );
            begun?;
            Ok(match rows?.first() {
                Some(row) => row.try_get(0)?,
                None => None,
            })
        }
        .await;

        match self.finish(result, true).await {
            Ok(value) => Ok(value),
            Err(Failure::Other(error)) => Err(error),
            Err(Failure::Unsupported) => Err(Error::UnsupportedDatabase(self.info.version.clone())),
        }
    }
}

#[async_trait]
impl SourceCanceller for IntrospectCanceller {
    async fn cancel(&self) -> Result<()> {
        IntrospectCanceller::cancel(self).await
    }
}

#[async_trait]
impl CatalogSource for Introspector {
    async fn sync(
        &mut self,
        catalog: &RwLock<Catalog>,
        on_event: &mut (dyn FnMut(SyncEvent) + Send),
    ) -> Result<SyncReport> {
        Introspector::sync(self, catalog, on_event).await
    }

    async fn load_schemas(&mut self, catalog: &RwLock<Catalog>, schemas: &[String]) -> Result<usize> {
        Introspector::load_schemas(self, catalog, schemas).await
    }

    async fn relation_size(&mut self, schema: &str, table: &str) -> Result<Option<i64>> {
        Introspector::relation_size(self, schema, table).await
    }

    async fn relation_columns(&mut self, relations: &[(String, String)]) -> Result<RelationColumns> {
        Introspector::relation_columns(self, relations).await
    }

    async fn foreign_keys(
        &mut self,
        scope: &[(String, String)],
        of: &[(String, String)],
    ) -> Result<Vec<ForeignKey>> {
        Introspector::foreign_keys(self, scope, of).await
    }

    async fn relation_details(&mut self, relations: &[(String, String)]) -> Result<Vec<RelationDetail>> {
        Introspector::relation_details(self, relations).await
    }

    async fn list_indexes(&mut self, schema: &str, table: Option<&str>) -> Result<Vec<IndexObject>> {
        Introspector::list_indexes(self, schema, table).await
    }

    async fn list_functions(&mut self, schema: &str) -> Result<Vec<FunctionObject>> {
        Introspector::list_functions(self, schema).await
    }

    async fn definition(&mut self, target: &ObjectRef) -> Result<Option<String>> {
        Introspector::definition(self, target).await
    }

    async fn search_columns(&mut self, schema: &str, pattern: &str, limit: usize) -> Result<Vec<ColumnMatch>> {
        Introspector::search_columns(self, schema, pattern, limit).await
    }

    async fn schema_comments(&mut self, schemas: &[String]) -> Result<Vec<(String, String)>> {
        Introspector::schema_comments(self, schemas).await
    }

    fn canceller(&self) -> Box<dyn SourceCanceller> {
        Box::new(Introspector::canceller(self))
    }

    fn is_closed(&self) -> bool {
        Introspector::is_closed(self)
    }

    fn server_version(&self) -> String {
        self.info.version.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_version_and_capabilities() {
        let pg16 = ServerInfo::from_version("PostgreSQL 16.4 on aarch64-unknown-linux-musl, compiled by gcc");
        assert_eq!(pg16.version_num, 160_004);
        assert_eq!(pg16.flavor, Flavor::Postgres);
        assert_eq!(
            pg16.caps,
            Capabilities {
                partitions: true,
                shapes: true,
                fingerprints: true,
            }
        );

        let pg96 = ServerInfo::from_version("PostgreSQL 9.6.24 on x86_64-pc-linux-gnu");
        assert_eq!(pg96.version_num, 90_624);
        assert!(!pg96.caps.partitions);
        assert!(pg96.caps.shapes);

        let yugabyte = ServerInfo::from_version("PostgreSQL 11.2-YB-2.20.1.0-b0 on x86_64");
        assert_eq!(yugabyte.version_num, 110_002);
        assert_eq!(yugabyte.flavor, Flavor::Postgres);

        let redshift = ServerInfo::from_version(
            "PostgreSQL 8.0.2 on i686-pc-linux-gnu, compiled by GCC gcc (GCC) 3.4.2, Redshift 1.0.77467",
        );
        assert_eq!(redshift.flavor, Flavor::Redshift);
        assert_eq!(
            redshift.caps,
            Capabilities {
                partitions: false,
                shapes: false,
                fingerprints: false,
            }
        );

        let cockroach = ServerInfo::from_version("CockroachDB CCL v23.1.11 (aarch64-apple-darwin21.2)");
        assert_eq!(cockroach.flavor, Flavor::Cockroach);
        assert!(!cockroach.caps.shapes && !cockroach.caps.fingerprints);
        assert_eq!(ServerInfo::from_version("whatever").flavor, Flavor::Other);
    }

    #[test]
    fn downgrade_falls_back_to_the_simple_path() {
        let mut info = ServerInfo::from_version("PostgreSQL 16.4");
        info.downgrade();
        assert!(!info.caps.shapes && !info.caps.fingerprints);
        assert!(info.caps.partitions);
    }

    #[test]
    fn begin_disables_jit_only_where_it_exists() {
        let options = IntrospectOptions::default();
        let pg16 = begin_sql(&ServerInfo::from_version("PostgreSQL 16.4"), &options);
        assert!(pg16.contains("SET LOCAL jit = off"));
        assert!(pg16.contains("statement_timeout = 15000"));
        assert!(pg16.contains("lock_timeout = 2000"));
        assert!(pg16.contains("REPEATABLE READ READ ONLY"));

        let pg10 = begin_sql(&ServerInfo::from_version("PostgreSQL 10.23"), &options);
        assert!(!pg10.contains("jit"));
        let redshift = begin_sql(&ServerInfo::from_version("PostgreSQL 8.0.2, Redshift 1.0"), &options);
        assert!(!redshift.contains("jit"));
    }

    #[test]
    fn schema_comparison_only_reads_the_catalog() {
        let sql = relation_columns_sql();
        // O schema de cada lado sai do tipo e do default
        assert!(sql.contains("replace(replace(pg_get_expr(d.adbin, d.adrelid), quote_ident(n.nspname)"));
        assert!(!sql.contains("relation_size") && !sql.contains("pg_stat"));
    }

    #[test]
    fn maps_relkinds() {
        assert_eq!(rel_kind("r"), Some(RelKind::Table));
        assert_eq!(rel_kind("p"), Some(RelKind::Partitioned));
        assert_eq!(rel_kind("m"), Some(RelKind::MaterializedView));
        assert_eq!(rel_kind("i"), None);
    }

    #[test]
    fn sql_follows_capabilities() {
        let full = Sql::new(ServerInfo::from_version("PostgreSQL 16.4").caps, false);
        assert!(full.shapes.contains("relispartition"));
        assert!(full.tables_all.contains("'p'"));
        assert!(full.tables_all.contains("hashtext"));
        assert!(!full.schemas.contains("has_schema_privilege"));
        // Nada que leia disco ou pegue lock de relação
        for sql in [&full.schemas, &full.shapes, &full.tables_all, &full.tables_by_oid] {
            assert!(!sql.contains("relation_size") && !sql.contains("database_size"));
        }

        let old = Sql::new(ServerInfo::from_version("PostgreSQL 9.6.24").caps, true);
        assert!(!old.tables_all.contains("relispartition"));
        assert!(!old.tables_all.contains("'p'"));
        assert!(old.schemas.contains("has_schema_privilege"));

        let redshift = Sql::new(
            ServerInfo::from_version("PostgreSQL 8.0.2 on i686, Redshift 1.0").caps,
            false,
        );
        assert!(!redshift.tables_all.contains("hashtext"));
    }

    fn schemas(names: &[&str]) -> Vec<(u32, String)> {
        names
            .iter()
            .enumerate()
            .map(|(i, name)| (i as u32 + 1, name.to_string()))
            .collect()
    }

    fn hashes(entries: &[(u32, &str)]) -> HashMap<u32, String> {
        entries.iter().map(|(oid, hash)| (*oid, hash.to_string())).collect()
    }

    #[test]
    fn first_sync_fetches_one_representative_per_shape() {
        let catalog = Catalog::new();
        // t_c e t_a têm o formato A, t_b o B, empty não tem relações
        let schemas = schemas(&["t_c", "t_a", "t_b", "empty"]);
        let plan = plan_shape_first(&catalog, &schemas, &hashes(&[(1, "A"), (2, "A"), (3, "B")]));

        assert_eq!(plan.fetch, [(2, "t_a".to_string()), (3, "t_b".to_string())]);
        assert_eq!(plan.share, [("t_c".to_string(), "t_a".to_string())]);
        assert_eq!(plan.empty, ["empty"]);
    }

    #[test]
    fn revalidation_reuses_fresh_schemas_and_fetches_nothing_new() {
        let mut catalog = Catalog::new();
        let tables = || vec![TableRecord::new("orders", RelKind::Table)];
        catalog.set_tables("t_a", tables(), Some(1));
        catalog.set_tables("t_b", tables(), Some(1));
        // Novo tenant com o mesmo formato, e um velho que mudou para o formato B
        catalog.set_schemas(
            [("t_a", Some(1)), ("t_b", Some(9)), ("t_new", None)].map(|(name, fingerprint)| SchemaHeader {
                name: name.into(),
                fingerprint,
            }),
        );

        let schemas = schemas(&["t_a", "t_b", "t_new"]);
        let plan = plan_shape_first(&catalog, &schemas, &hashes(&[(1, "A"), (2, "B"), (3, "A")]));

        assert_eq!(plan.fetch, [(2, "t_b".to_string())]);
        assert_eq!(plan.share, [("t_new".to_string(), "t_a".to_string())]);
        assert!(plan.empty.is_empty());

        // Tudo em dia: nada a fazer
        let mut fresh = Catalog::new();
        fresh.set_tables("t_a", tables(), Some(1));
        let plan = plan_shape_first(&fresh, &schemas[..1], &hashes(&[(1, "A")]));
        assert_eq!(plan, ShapePlan::default());
    }

    /// Checagem em tempo de compilação: a Fase 3 roda a sincronização numa task.
    #[allow(dead_code)]
    fn sync_future_is_send(introspector: &mut Introspector, catalog: &RwLock<Catalog>) {
        fn assert_send<T: Send>(_: &T) {}
        let future = introspector.sync(catalog, |_| {});
        assert_send(&future);
    }
}

/// Pastas da árvore contra o Postgres dos e2e (localhost:5432, postgres/1234),
/// num banco scratch próprio. `cargo test --lib postgres::introspect::e2e -- --ignored`
#[cfg(test)]
mod e2e {
    use super::*;
    use crate::adapters::postgres::PostgresAdapter;
    use crate::adapters::DatabaseAdapter;
    use crate::catalog::{CatalogObject, CatalogPath};
    use crate::models::{DatabaseType, SslMode};

    const DATABASE: &str = "octapus_db_e2e_folders";

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
            ssl_mode: SslMode::Disable,
            ssl_root_cert: None,
            connection_uri: None,
            scope_databases: None,
            scope_schemas: None,
            group_id: None,
            assistant_sample_values: false,
            created_at: 0,
        }
    }

    fn target(kind: ObjectKind, name: &str, signature: Option<&str>) -> ObjectRef {
        ObjectRef {
            kind,
            schema: "app".into(),
            name: name.into(),
            table: None,
            signature: signature.map(str::to_string),
        }
    }

    #[tokio::test]
    #[ignore = "requires local postgres on localhost:5432 (postgres/1234)"]
    async fn e2e_folders_indexes_functions_and_definitions() {
        let admin = PostgresAdapter::new(&server(), "postgres").unwrap();
        admin
            .execute_statement(&format!("DROP DATABASE IF EXISTS {DATABASE} WITH (FORCE)"))
            .await
            .unwrap();
        admin
            .execute_statement(&format!("CREATE DATABASE {DATABASE}"))
            .await
            .unwrap();
        let setup = PostgresAdapter::new(&server(), DATABASE).unwrap();
        setup
            .execute_transaction(
                [
                    "CREATE SCHEMA app",
                    "CREATE TABLE app.orders (id serial PRIMARY KEY, customer text NOT NULL, total numeric(10,2))",
                    "CREATE INDEX orders_customer_lower ON app.orders (lower(customer), total)",
                    "CREATE VIEW app.big_orders AS SELECT * FROM app.orders WHERE total > 1000",
                    "CREATE MATERIALIZED VIEW app.totals AS SELECT customer, sum(total) AS total FROM app.orders GROUP BY customer",
                    "CREATE FUNCTION app.add(a integer, b integer DEFAULT 1) RETURNS integer LANGUAGE sql AS 'SELECT a + b'",
                    "CREATE FUNCTION app.add(a text, b text) RETURNS text LANGUAGE sql AS 'SELECT a || b'",
                    "CREATE PROCEDURE app.archive(days integer) LANGUAGE sql AS 'DELETE FROM app.orders WHERE id < days'",
                    "CREATE AGGREGATE app.concat_all(text) (SFUNC = textcat, STYPE = text, INITCOND = '')",
                    // Funções de extensão não entram na pasta
                    "CREATE EXTENSION IF NOT EXISTS pgcrypto SCHEMA app",
                ]
                .map(String::from)
                .to_vec(),
            )
            .await
            .unwrap();
        drop(setup);

        let mut source = Introspector::connect(&server(), DATABASE, IntrospectOptions::default())
            .await
            .unwrap();

        // Views saem das relações soltas
        let catalog = RwLock::new(Catalog::new());
        source.sync(&catalog, |_| {}).await.unwrap();
        let loose: Vec<String> = catalog
            .read()
            .children(&CatalogPath::Tables("app".into()), None, 0, 100)
            .unwrap()
            .items
            .into_iter()
            .map(|node| node.name)
            .collect();
        assert_eq!(loose, ["orders"]);
        let views = catalog
            .read()
            .objects(ObjectKind::View, "app", None, None, 0, 10)
            .unwrap();
        assert_eq!(
            views.items,
            [CatalogObject::View {
                name: "big_orders".into()
            }]
        );

        let indexes = source.list_indexes("app", None).await.unwrap();
        let names: Vec<&str> = indexes.iter().map(|index| index.name.as_str()).collect();
        assert_eq!(names, ["orders_customer_lower", "orders_pkey"]);
        let expression = &indexes[0];
        assert_eq!(expression.table, "orders");
        assert_eq!(expression.method, "btree");
        assert_eq!(expression.columns, ["lower(customer)", "total"]);
        assert!(indexes[1].primary && indexes[1].unique);
        assert_eq!(indexes[0].predicate, None);
        assert_eq!(source.list_indexes("app", Some("totals")).await.unwrap().len(), 0);
        assert_eq!(source.list_indexes("app", Some("orders")).await.unwrap().len(), 2);

        let functions = source.list_functions("app").await.unwrap();
        let listed: Vec<(&str, &str, FunctionKind)> = functions
            .iter()
            .map(|f| (f.name.as_str(), f.signature.as_str(), f.function_kind))
            .collect();
        assert_eq!(
            listed,
            [
                ("add", "a integer, b integer", FunctionKind::Function),
                ("add", "a text, b text", FunctionKind::Function),
                ("archive", "IN days integer", FunctionKind::Procedure),
                ("concat_all", "text", FunctionKind::Aggregate),
            ],
            "sem as do pgcrypto"
        );
        assert_eq!(functions[0].arguments, "a integer, b integer DEFAULT 1");
        assert_eq!(functions[0].returns.as_deref(), Some("integer"));
        assert_eq!(functions[2].returns, None);
        assert_eq!(functions[0].language, "sql");

        let view = source
            .definition(&target(ObjectKind::View, "big_orders", None))
            .await
            .unwrap()
            .unwrap();
        assert!(view.starts_with("CREATE OR REPLACE VIEW app.big_orders AS\n"), "{view}");
        assert!(view.ends_with("1000::numeric;\n"), "{view}");
        let materialized = source
            .definition(&target(ObjectKind::MaterializedView, "totals", None))
            .await
            .unwrap()
            .unwrap();
        assert!(
            materialized.starts_with("CREATE MATERIALIZED VIEW app.totals AS\n"),
            "{materialized}"
        );
        let index = source
            .definition(&target(ObjectKind::Index, "orders_customer_lower", None))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            index,
            "CREATE INDEX orders_customer_lower ON app.orders USING btree (lower(customer), total);\n"
        );
        let text_add = source
            .definition(&target(ObjectKind::Function, "add", Some("a text, b text")))
            .await
            .unwrap()
            .unwrap();
        assert!(
            text_add.contains("app.add(a text, b text)") && text_add.contains("a || b"),
            "{text_add}"
        );
        let aggregate = source
            .definition(&target(ObjectKind::Function, "concat_all", Some("text")))
            .await
            .unwrap()
            .unwrap();
        assert!(
            aggregate.starts_with("CREATE AGGREGATE app.concat_all(text) (\n    SFUNC = textcat,"),
            "{aggregate}"
        );
        assert!(aggregate.contains("INITCOND = ''"), "{aggregate}");
        assert!(source
            .definition(&target(ObjectKind::Function, "add", Some("numeric")))
            .await
            .unwrap()
            .is_none());

        drop(source);
        admin
            .execute_statement(&format!("DROP DATABASE IF EXISTS {DATABASE} WITH (FORCE)"))
            .await
            .unwrap();
    }
}
