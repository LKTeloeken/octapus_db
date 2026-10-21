//! Guard dos planos do Redis gerados pelo assistente.
//!
//! Falha fechado: na dúvida, recusa. O plano precisa parsear pelo contrato
//! (`adapters/redisdb/plan.rs`: ids, `foreach` para um passo anterior, `$item`
//! só dentro de `foreach`) e cada passo usar um comando da allowlist de
//! leitura. Recusa com motivo próprio:
//!
//! - **`KEYS`**: varre o keyspace inteiro num comando só (o app percorre o
//!   `SCAN`);
//! - **scripts e funções** (`EVAL`, `EVALSHA`, `FCALL`, `FUNCTION`, `SCRIPT`):
//!   rodam código no servidor, que pode escrever;
//! - **escritas** (`SET`, `DEL`, `EXPIRE`, `FLUSHDB`…) e o que mais não está
//!   na allowlist (administração, bloqueantes, transação, módulos);
//! - **`SCAN` dentro de `foreach`**: uma varredura do keyspace por item.
//!
//! Não é a única barreira: o executor (`adapters/redisdb/read_only.rs`)
//! confere a allowlist de novo e roda com tetos de chaves, itens, comandos e
//! tempo.

use std::fmt;

use crate::adapters::redisdb::plan::{self, command_kind, CommandKind, Plan};

/// Por que o guard recusou o plano.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    Empty,
    /// Não é JSON do contrato: na dúvida, não executa
    Unparseable(String),
    Keys,
    Script(String),
    Write(String),
    /// Fora da allowlist de leitura
    NotAllowed(String),
    ScanInForeach,
}

impl Rejection {
    /// Código estável, para log e classificação (nunca o texto do plano).
    pub fn code(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Unparseable(_) => "unparseable",
            Self::Keys => "keys",
            Self::Script(_) => "script",
            Self::Write(_) => "write",
            Self::NotAllowed(_) => "not_allowed",
            Self::ScanInForeach => "scan_in_foreach",
        }
    }
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("empty plan"),
            Self::Unparseable(reason) => f.write_str(reason),
            Self::Keys => f.write_str("KEYS is not allowed: use SCAN with MATCH, the app walks the cursor"),
            Self::Script(command) => write!(f, "{command} runs a script on the server and is not allowed"),
            Self::Write(command) => write!(f, "{command} writes to the database and is not allowed"),
            Self::NotAllowed(command) => write!(f, "{command} is not one of the read commands a plan can use"),
            Self::ScanInForeach => f.write_str("SCAN cannot run inside a foreach: scan once and iterate over its keys"),
        }
    }
}

/// Confere o plano (o texto do `query` da resposta do modelo) e devolve o
/// plano lido.
pub fn check(text: &str) -> Result<Plan, Rejection> {
    if text.trim().is_empty() {
        return Err(Rejection::Empty);
    }
    let plan = plan::parse(text).map_err(|error| Rejection::Unparseable(error.to_string()))?;
    for step in &plan.steps {
        let command = step.command.clone();
        match command_kind(&step.command) {
            CommandKind::Read => {}
            CommandKind::Keys => return Err(Rejection::Keys),
            CommandKind::Script => return Err(Rejection::Script(command)),
            CommandKind::Write => return Err(Rejection::Write(command)),
            CommandKind::Other => return Err(Rejection::NotAllowed(command)),
        }
        if step.command == "SCAN" && step.foreach.is_some() {
            return Err(Rejection::ScanInForeach);
        }
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(command: &str, args: &str) -> String {
        format!(r#"{{"steps":[{{"id":"a","command":"{command}","args":{args},"foreach":null}}],"output":"a"}}"#)
    }

    #[test]
    fn reads_pass() {
        for text in [
            one("SCAN", r#"["0","MATCH","user:*","TYPE","hash"]"#),
            one("HGETALL", r#"["user:1"]"#),
            one("ZRANGE", r#"["leaderboard","0","9","REV","WITHSCORES"]"#),
            one("DBSIZE", "[]"),
            r#"{"steps":[
                {"id":"users","command":"SCAN","args":["MATCH","user:*"],"foreach":null},
                {"id":"carts","command":"LLEN","args":["$item:cart"],"foreach":"users"}
            ],"output":"carts"}"#
                .to_string(),
        ] {
            assert!(check(&text).is_ok(), "{text}");
        }
    }

    #[test]
    fn rejects_keys_scripts_writes_and_the_rest() {
        let scan_in_foreach = r#"{"steps":[
            {"id":"a","command":"SMEMBERS","args":["prefixes"],"foreach":null},
            {"id":"b","command":"SCAN","args":["MATCH","$item:*"],"foreach":"a"}
        ],"output":"b"}"#;
        let write_after_read = r#"{"steps":[
            {"id":"a","command":"SCAN","args":["MATCH","session:*"],"foreach":null},
            {"id":"b","command":"DEL","args":["$item"],"foreach":"a"}
        ],"output":"b"}"#;
        let cases = [
            (one("KEYS", r#"["*"]"#), "keys"),
            (one("EVAL", r#"["return redis.call('FLUSHDB')","0"]"#), "script"),
            (one("EVALSHA", r#"["abc","0"]"#), "script"),
            (one("FCALL", r#"["f","0"]"#), "script"),
            (one("FUNCTION", r#"["LIST"]"#), "script"),
            (one("SCRIPT", r#"["FLUSH"]"#), "script"),
            (one("SET", r#"["k","v"]"#), "write"),
            (one("FLUSHDB", "[]"), "write"),
            (one("EXPIRE", r#"["k","1"]"#), "write"),
            (one("SORT", r#"["k","STORE","dst"]"#), "write"),
            (one("PFCOUNT", r#"["hll"]"#), "write"),
            (one("CONFIG", r#"["GET","*"]"#), "not_allowed"),
            (one("SELECT", r#"["1"]"#), "not_allowed"),
            (one("BLPOP", r#"["q","0"]"#), "not_allowed"),
            (one("MULTI", "[]"), "not_allowed"),
            (one("JSON.GET", r#"["doc"]"#), "not_allowed"),
            (one("EVAL_RO", r#"["return 1","0"]"#), "unparseable"),
            (scan_in_foreach.to_string(), "scan_in_foreach"),
            (write_after_read.to_string(), "write"),
            ("   ".to_string(), "empty"),
            ("SCAN 0 MATCH *".to_string(), "unparseable"),
        ];
        for (text, code) in cases {
            assert_eq!(check(&text).unwrap_err().code(), code, "{text}");
        }
    }

    #[test]
    fn messages_never_quote_the_arguments() {
        let text = one("SET", r#"["user:CANARY_7f3a","s3cr3t"]"#);
        let message = check(&text).unwrap_err().to_string();
        assert_eq!(message, "SET writes to the database and is not allowed");
    }
}
