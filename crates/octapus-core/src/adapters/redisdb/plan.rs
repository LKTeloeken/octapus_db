//! O plano do Redis do assistente (`redis-plan.schema.json` do contrato):
//! passos de comandos de leitura, com `foreach` para repetir um passo sobre
//! cada item do resultado de outro. O modelo não vê chaves nem valores
//! (RN-09), então descreve o plano e o app executa (`read_only.rs`).
//!
//! O parse é estrito e falha fechado: campo a mais, id repetido, `foreach`
//! para um passo que não veio antes ou `$item` fora de um `foreach` recusam o
//! plano inteiro. A política dos comandos (a allowlist de leitura) mora aqui
//! também, para o guard (`assistant/guard/redis.rs`) e o executor usarem a
//! mesma.

use std::fmt;

use serde::Deserialize;

/// Passos por plano (`maxItems` do contrato).
pub const MAX_STEPS: usize = 8;
/// Argumentos por passo.
pub const MAX_ARGS: usize = 64;
/// Tamanho de um argumento.
pub const MAX_ARG_CHARS: usize = 1024;
/// O marcador do item do `foreach` nos argumentos.
pub const ITEM: &str = "$item";

/// Comandos de leitura que o plano pode usar, de um key só ou de leitura
/// do keyspace. Nada que escreva, bloqueie, rode script ou administre o
/// servidor; `SORT` (que tem `STORE`), `PFCOUNT` (que grava o cache do
/// HyperLogLog) e os de módulos ficam de fora. O que não está aqui é
/// recusado — inclusive comando novo de uma versão futura.
pub const READ_COMMANDS: &[&str] = &[
    // Keyspace
    "SCAN", "EXISTS", "TYPE", "TTL", "PTTL", "EXPIRETIME", "PEXPIRETIME", "DBSIZE",
    // Strings
    "GET", "MGET", "STRLEN", "GETRANGE", "GETBIT", "BITCOUNT", "BITPOS",
    // Hashes
    "HGET", "HMGET", "HGETALL", "HKEYS", "HVALS", "HLEN", "HEXISTS", "HSTRLEN", "HRANDFIELD",
    // Listas
    "LRANGE", "LLEN", "LINDEX", "LPOS",
    // Sets
    "SMEMBERS", "SCARD", "SISMEMBER", "SMISMEMBER", "SRANDMEMBER", "SINTER", "SUNION", "SDIFF",
    "SINTERCARD",
    // Sorted sets
    "ZRANGE", "ZRANGEBYSCORE", "ZREVRANGE", "ZREVRANGEBYSCORE", "ZRANGEBYLEX", "ZREVRANGEBYLEX",
    "ZSCORE", "ZMSCORE", "ZCARD", "ZCOUNT", "ZLEXCOUNT", "ZRANK", "ZREVRANK", "ZINTER", "ZUNION",
    "ZDIFF", "ZINTERCARD", "ZRANDMEMBER",
    // Streams
    "XRANGE", "XREVRANGE", "XLEN",
    // Geo
    "GEOPOS", "GEODIST", "GEOHASH", "GEOSEARCH",
];

/// Scripts e funções: rodam código no servidor, que pode escrever.
const SCRIPT_COMMANDS: &[&str] = &["EVAL", "EVALSHA", "FCALL", "FUNCTION", "SCRIPT"];

/// Escritas conhecidas, só para a recusa dizer "escreve" em vez de "não
/// permitido" (a decisão é da allowlist).
const WRITE_COMMANDS: &[&str] = &[
    "SET", "SETNX", "SETEX", "PSETEX", "MSET", "MSETNX", "GETSET", "GETDEL", "GETEX", "APPEND",
    "SETRANGE", "SETBIT", "BITOP", "BITFIELD", "INCR", "INCRBY", "INCRBYFLOAT", "DECR", "DECRBY",
    "DEL", "UNLINK", "EXPIRE", "PEXPIRE", "EXPIREAT", "PEXPIREAT", "PERSIST", "RENAME", "RENAMENX",
    "COPY", "MOVE", "RESTORE", "MIGRATE", "SORT", "HSET", "HSETNX", "HMSET", "HDEL", "HINCRBY",
    "HINCRBYFLOAT", "LPUSH", "RPUSH", "LPUSHX", "RPUSHX", "LPOP", "RPOP", "LSET", "LREM", "LTRIM",
    "LINSERT", "LMOVE", "RPOPLPUSH", "SADD", "SREM", "SPOP", "SMOVE", "SINTERSTORE",
    "SUNIONSTORE", "SDIFFSTORE", "ZADD", "ZREM", "ZINCRBY", "ZPOPMIN", "ZPOPMAX",
    "ZREMRANGEBYSCORE", "ZREMRANGEBYRANK", "ZREMRANGEBYLEX", "ZRANGESTORE", "ZINTERSTORE",
    "ZUNIONSTORE", "ZDIFFSTORE", "XADD", "XDEL", "XTRIM", "XGROUP", "XACK", "XCLAIM",
    "XAUTOCLAIM", "XSETID", "PFADD", "PFMERGE", "PFCOUNT", "GEOADD", "GEORADIUS",
    "GEORADIUSBYMEMBER", "GEOSEARCHSTORE", "FLUSHDB", "FLUSHALL", "SWAPDB", "PUBLISH",
];

/// Que tipo de comando é, para o guard e para o executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    Read,
    /// `KEYS`: varre o keyspace inteiro num comando só e trava o servidor
    Keys,
    Script,
    Write,
    /// Administração, bloqueante, transação, pub/sub, módulo…
    Other,
}

pub fn command_kind(command: &str) -> CommandKind {
    if READ_COMMANDS.contains(&command) {
        CommandKind::Read
    } else if command == "KEYS" {
        CommandKind::Keys
    } else if SCRIPT_COMMANDS.contains(&command) {
        CommandKind::Script
    } else if WRITE_COMMANDS.contains(&command) {
        CommandKind::Write
    } else {
        CommandKind::Other
    }
}

/// Um passo do plano, já conferido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub id: String,
    /// Em maiúsculas (`HGETALL`)
    pub command: String,
    pub args: Vec<String>,
    /// Índice do passo cujo resultado é iterado
    pub foreach: Option<usize>,
}

impl Step {
    /// A resposta vem em pares (`campo, valor` ou `membro, score`) numa
    /// lista só (RESP2).
    pub fn pairwise(&self) -> bool {
        self.command == "HGETALL"
            || self
                .args
                .iter()
                .any(|arg| arg.eq_ignore_ascii_case("WITHSCORES") || arg.eq_ignore_ascii_case("WITHVALUES"))
    }
}

/// O plano conferido: os passos em ordem e qual deles vai para a tela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub steps: Vec<Step>,
    pub output: usize,
}

/// Por que o texto não é um plano do contrato. A mensagem é texto nosso, com
/// no máximo um id de passo ou nome de comando tirado do plano.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Json(String),
    NoSteps,
    TooManySteps,
    BadId(String),
    DuplicateId(String),
    BadCommand(String),
    TooManyArgs(String),
    ArgTooLong(String),
    /// `foreach` para um passo que não existe ou que não veio antes
    UnknownForeach(String),
    /// `$item` num passo sem `foreach`
    ItemWithoutForeach(String),
    UnknownOutput(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(reason) => write!(f, "the plan is not a valid redis-plan JSON ({reason})"),
            Self::NoSteps => f.write_str("the plan has no steps"),
            Self::TooManySteps => write!(f, "the plan has more than {MAX_STEPS} steps"),
            Self::BadId(id) => write!(f, "step id \"{id}\" must match ^[a-z][a-z0-9_]{{0,31}}$"),
            Self::DuplicateId(id) => write!(f, "step id \"{id}\" is repeated"),
            Self::BadCommand(id) => write!(f, "step \"{id}\": command must be an uppercase Redis command name"),
            Self::TooManyArgs(id) => write!(f, "step \"{id}\" has more than {MAX_ARGS} arguments"),
            Self::ArgTooLong(id) => write!(f, "step \"{id}\" has an argument longer than {MAX_ARG_CHARS} characters"),
            Self::UnknownForeach(id) => write!(f, "step \"{id}\": foreach must name an earlier step"),
            Self::ItemWithoutForeach(id) => write!(f, "step \"{id}\" uses $item without foreach"),
            Self::UnknownOutput(id) => write!(f, "output \"{id}\" is not a step of the plan"),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPlan {
    steps: Vec<RawStep>,
    output: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStep {
    id: String,
    command: String,
    args: Vec<String>,
    foreach: Option<String>,
}

pub fn parse(text: &str) -> Result<Plan, ParseError> {
    let raw: RawPlan = serde_json::from_str(text).map_err(|error| {
        // Só a categoria do serde: a mensagem dele pode citar um trecho
        let reason = match error.classify() {
            serde_json::error::Category::Data => "fields out of contract",
            _ => "malformed JSON",
        };
        ParseError::Json(reason.into())
    })?;
    if raw.steps.is_empty() {
        return Err(ParseError::NoSteps);
    }
    if raw.steps.len() > MAX_STEPS {
        return Err(ParseError::TooManySteps);
    }

    let mut steps: Vec<Step> = Vec::with_capacity(raw.steps.len());
    for step in raw.steps {
        if !valid_id(&step.id) {
            return Err(ParseError::BadId(short(&step.id)));
        }
        if steps.iter().any(|seen| seen.id == step.id) {
            return Err(ParseError::DuplicateId(step.id));
        }
        if !valid_command(&step.command) {
            return Err(ParseError::BadCommand(step.id));
        }
        if step.args.len() > MAX_ARGS {
            return Err(ParseError::TooManyArgs(step.id));
        }
        if step.args.iter().any(|arg| arg.chars().count() > MAX_ARG_CHARS) {
            return Err(ParseError::ArgTooLong(step.id));
        }
        let foreach = match &step.foreach {
            None => None,
            Some(source) => match steps.iter().position(|seen| &seen.id == source) {
                Some(index) => Some(index),
                None => return Err(ParseError::UnknownForeach(step.id)),
            },
        };
        if foreach.is_none() && step.args.iter().any(|arg| arg.contains(ITEM)) {
            return Err(ParseError::ItemWithoutForeach(step.id));
        }
        steps.push(Step {
            id: step.id,
            command: step.command,
            args: step.args,
            foreach,
        });
    }

    let output = steps
        .iter()
        .position(|step| step.id == raw.output)
        .ok_or_else(|| ParseError::UnknownOutput(short(&raw.output)))?;
    Ok(Plan { steps, output })
}

/// `^[a-z][a-z0-9_]{0,31}$`
fn valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && id.len() <= 32
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// `^[A-Z][A-Z.]*$`
fn valid_command(command: &str) -> bool {
    let mut chars = command.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase())
        && command.len() <= 32
        && chars.all(|c| c.is_ascii_uppercase() || c == '.')
}

/// Id inválido vai na mensagem cortado: pode ser qualquer texto.
fn short(text: &str) -> String {
    text.chars().take(32).filter(|c| c.is_ascii_alphanumeric() || *c == '_').collect()
}

/// As opções do `SCAN` do plano: o app percorre o cursor, então o cursor do
/// modelo, se vier, tem que ser `0`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScanOptions {
    pub pattern: Option<String>,
    pub count: Option<u32>,
    pub type_name: Option<String>,
}

impl ScanOptions {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut options = Self::default();
        let mut rest = args;
        if rest.first().is_some_and(|first| first.chars().all(|c| c.is_ascii_digit())) {
            if rest[0] != "0" {
                return Err("SCAN must start at cursor 0: the app walks the cursor".into());
            }
            rest = &rest[1..];
        }
        while let Some((option, tail)) = rest.split_first() {
            let Some((value, tail)) = tail.split_first() else {
                return Err(format!("SCAN option {} needs a value", option.to_uppercase()));
            };
            match option.to_uppercase().as_str() {
                "MATCH" if options.pattern.is_none() => options.pattern = Some(value.clone()),
                "COUNT" if options.count.is_none() => {
                    options.count = Some(value.parse().map_err(|_| "SCAN COUNT must be a positive integer".to_string())?)
                }
                "TYPE" if options.type_name.is_none() => options.type_name = Some(value.to_lowercase()),
                _ => return Err("SCAN accepts only MATCH, COUNT and TYPE, once each".into()),
            }
            rest = tail;
        }
        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAN: &str = r#"{"steps":[
        {"id":"users","command":"SCAN","args":["0","MATCH","user:*","TYPE","hash"],"foreach":null},
        {"id":"names","command":"HGET","args":["$item","name"],"foreach":"users"}
    ],"output":"names"}"#;

    #[test]
    fn parses_the_contract_plan() {
        let plan = parse(PLAN).unwrap();
        assert_eq!(plan.output, 1);
        assert_eq!(plan.steps[1].foreach, Some(0));
        assert_eq!(plan.steps[0].command, "SCAN");
        assert_eq!(command_kind("HGETALL"), CommandKind::Read);
        assert_eq!(command_kind("KEYS"), CommandKind::Keys);
        assert_eq!(command_kind("EVALSHA"), CommandKind::Script);
        assert_eq!(command_kind("FLUSHDB"), CommandKind::Write);
        assert_eq!(command_kind("CONFIG"), CommandKind::Other);
        assert_eq!(command_kind("BLPOP"), CommandKind::Other);
    }

    #[test]
    fn rejects_what_is_out_of_contract() {
        let step = |id: &str, command: &str, args: &str, foreach: &str| {
            format!(r#"{{"id":"{id}","command":"{command}","args":{args},"foreach":{foreach}}}"#)
        };
        let plan = |steps: &[String], output: &str| format!(r#"{{"steps":[{}],"output":"{output}"}}"#, steps.join(","));
        let get = step("a", "GET", r#"["k"]"#, "null");
        let cases = [
            ("{", ParseError::Json("malformed JSON".into())),
            (r#"{"steps":[],"output":"a","note":1}"#, ParseError::Json("fields out of contract".into())),
            (&plan(&[], "a"), ParseError::NoSteps),
            (&plan(&vec![get.clone(); 9], "a"), ParseError::TooManySteps),
            (&plan(&[step("Bad", "GET", "[]", "null")], "Bad"), ParseError::BadId("Bad".into())),
            (&plan(&[get.clone(), get.clone()], "a"), ParseError::DuplicateId("a".into())),
            (&plan(&[step("a", "get", "[]", "null")], "a"), ParseError::BadCommand("a".into())),
            (&plan(&[step("a", "EVAL_RO", "[]", "null")], "a"), ParseError::BadCommand("a".into())),
            (&plan(&[step("a", "GET", r#"["$item"]"#, "null")], "a"), ParseError::ItemWithoutForeach("a".into())),
            (&plan(&[step("a", "GET", r#"["$item"]"#, r#""a""#)], "a"), ParseError::UnknownForeach("a".into())),
            (&plan(&[get.clone(), step("b", "GET", "[]", r#""c""#)], "b"), ParseError::UnknownForeach("b".into())),
            (&plan(&[get], "z"), ParseError::UnknownOutput("z".into())),
        ];
        for (text, expected) in cases {
            assert_eq!(parse(text).unwrap_err(), expected, "{text}");
        }
    }

    #[test]
    fn scan_options() {
        let args = |list: &[&str]| list.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
        assert_eq!(
            ScanOptions::parse(&args(&["0", "MATCH", "user:*", "COUNT", "500", "TYPE", "HASH"])).unwrap(),
            ScanOptions {
                pattern: Some("user:*".into()),
                count: Some(500),
                type_name: Some("hash".into()),
            }
        );
        assert_eq!(ScanOptions::parse(&[]).unwrap(), ScanOptions::default());
        for bad in [&["17"][..], &["MATCH"], &["0", "MATCH", "a", "MATCH", "b"], &["0", "NOVALUES", "x"], &["COUNT", "-1"]] {
            assert!(ScanOptions::parse(&args(bad)).is_err(), "{bad:?}");
        }
    }
}
